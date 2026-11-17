//! Running a [`Program`].

use super::ast::{BinOp, Expr, Program, ResKind, Scope, Sym};
use super::math::Rng;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;

/// Rounds of `loop` and elements of `for_each` that run: Molang's own limit.
pub const LOOP_LIMIT: usize = 1024;
/// Nodes one run may evaluate before it is abandoned (and gives 0): a guard against a pack's
/// runaway expression, not a limit real packs come near.
pub const STEP_LIMIT: u32 = 4_000_000;

/// What an expression gives.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// Nothing: a variable never set, a query the host does not know. Counts as 0 in
    /// arithmetic and is what `??` replaces.
    #[default]
    Null,
    Num(f32),
    Str(Arc<str>),
    /// `geometry.x`, `texture.x`, `material.x`: a name the render controller resolves.
    Res(ResKind, Arc<str>),
    /// An entity, by the host's id for it (`c.owning_entity`).
    Entity(u64),
    /// Three numbers: what `query.bone_origin` returns. Assigned to `v.a`, its members are
    /// read as `v.a.x`, `v.a.y` and `v.a.z`.
    Vec3([f32; 3]),
    /// Translation, rotation and scale: what `query.bone_orientation_trs` returns. Assigned to
    /// `v.a`, its members are read as `v.a.t.x`, `v.a.r.y`, `v.a.s.z` and so on.
    Trs([[f32; 3]; 3]),
}

impl Value {
    pub fn num(&self) -> f32 {
        match self {
            Value::Num(n) => *n,
            _ => 0.0,
        }
    }

    pub fn truthy(&self) -> bool {
        self.num() != 0.0
    }

    pub fn bool(b: bool) -> Value {
        Value::Num(if b { 1.0 } else { 0.0 })
    }

    fn equals(&self, other: &Value) -> bool {
        use Value::*;
        match (self, other) {
            (Num(a), Num(b)) => a == b,
            (Null, Null) => true,
            (Null, Num(n)) | (Num(n), Null) => *n == 0.0,
            (Str(a), Str(b)) => a == b,
            (Res(k, a), Res(l, b)) => k == l && a == b,
            (Entity(a), Entity(b)) => a == b,
            _ => false,
        }
    }
}

impl From<f32> for Value {
    fn from(n: f32) -> Self {
        Value::Num(n)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::bool(b)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(Arc::from(s))
    }
}

/// The members a value assigned to `sym` is read by: a vector's `sym.x`, `sym.y`, `sym.z`, and
/// a transform's `sym.t.x` to `sym.s.z` (row by row). Kept per thread: each assignment made the
/// names and took the shared table's lock for each.
fn member_syms(sym: Sym) -> ([Sym; 3], [Sym; 9]) {
    type Members = HashMap<Sym, ([Sym; 3], [Sym; 9]), BuildHasherDefault<IdHasher>>;
    thread_local! {
        static MEMBERS: std::cell::RefCell<Members> = std::cell::RefCell::new(HashMap::default());
    }
    MEMBERS.with(|members| {
        *members.borrow_mut().entry(sym).or_insert_with(|| {
            let base = super::ast::sym_name(sym);
            let axes = ["x", "y", "z"];
            let vector = axes.map(|axis| super::ast::intern(&format!("{base}.{axis}")));
            let mut trs = [0; 9];
            for (i, part) in ["t", "r", "s"].into_iter().enumerate() {
                for (j, axis) in axes.into_iter().enumerate() {
                    trs[i * 3 + j] = super::ast::intern(&format!("{base}.{part}.{axis}"));
                }
            }
            (vector, trs)
        })
    })
}

/// What the game answers. Every method has a default that answers "unknown", so a host
/// implements only what it has; unknown answers are [`Value::Null`], which reads as 0.
pub trait Host {
    /// `query.name(args)`; `name` is lower case.
    fn query(&mut self, name: &str, args: &[Value]) -> Value;

    /// An animation has been added to the pose being built: what `bone_origin` and the like
    /// answer follows the pose as it is now.
    fn pose_changed(&mut self, _pose: &crate::animation::Pose) {}

    /// The `variable.name` values the game itself keeps for an entity (`variable.attack_time`),
    /// set before the entity's own scripts run each update.
    fn variables(&mut self) -> Vec<(&'static str, Value)> {
        Vec::new()
    }

    /// `context.name`.
    fn context(&mut self, _name: &str) -> Value {
        Value::Null
    }

    /// The elements of `array.name`.
    fn array(&self, _name: &str) -> Option<Vec<Value>> {
        None
    }

    /// `variable.x` of another entity, for `entity -> v.x`.
    fn entity_var(&mut self, _entity: u64, _var: Sym) -> Value {
        Value::Null
    }

    /// `query.name(args)` on another entity, for `entity -> q.name`.
    fn entity_query(&mut self, _entity: u64, _name: &str, _args: &[Value]) -> Value {
        Value::Null
    }
}

/// A host that knows nothing.
pub struct NoHost;

impl Host for NoHost {
    fn query(&mut self, _name: &str, _args: &[Value]) -> Value {
        Value::Null
    }
}

#[derive(Default)]
pub struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 << 8) | u64::from(b);
        }
    }

    fn write_u32(&mut self, n: u32) {
        self.0 = u64::from(n).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

/// An entity's `variable.*`, by symbol.
pub type Vars = HashMap<Sym, Value, BuildHasherDefault<IdHasher>>;

/// Everything one run touches.
pub struct EvalCtx<'a> {
    pub host: &'a mut dyn Host,
    pub vars: &'a mut Vars,
    pub rng: &'a mut Rng,
    /// The value of `this`.
    pub this: Value,
}

/// How a run leaves the expression it is in: `return`, `break`, `continue`, or abandoned.
enum Flow {
    Return(Value),
    Break,
    Continue,
    Abort,
}

struct Machine<'a, 'b> {
    ctx: &'a mut EvalCtx<'b>,
    temps: Vars,
    steps: u32,
    /// Set when the run leaves the expressions it is in (`return`, `break`, `continue`, too
    /// many steps): every enclosing expression stops where it is, as soon as it sees it. A
    /// field rather than a `Result` returned by each node: the results were most of a run.
    flow: Option<Flow>,
}

thread_local! {
    /// The `temp.*` variables the programs run inside [`shared_temps`] have in common.
    static SHARED_TEMPS: std::cell::RefCell<Option<Vars>> = const { std::cell::RefCell::new(None) };
}

/// Runs `f` with every program it evaluates sharing its `temp.*` variables, which is how an
/// animation's vector components (`["...; t.y = 1; return t.x;", "t.y", "t.z"]`) hand values to
/// one another. Outside it each program starts with none.
pub fn shared_temps<R>(f: impl FnOnce() -> R) -> R {
    let outer = SHARED_TEMPS.with(|s| s.borrow_mut().replace(Vars::default()));
    let result = f();
    SHARED_TEMPS.with(|s| *s.borrow_mut() = outer);
    result
}

impl Program {
    /// Runs the program. Errors, a missing variable and an abandoned run all give 0.
    pub fn eval(&self, ctx: &mut EvalCtx) -> Value {
        let shared = SHARED_TEMPS.with(|s| s.borrow_mut().as_mut().map(std::mem::take));
        let is_shared = shared.is_some();
        let mut m = Machine {
            ctx,
            temps: shared.unwrap_or_default(),
            steps: 0,
            flow: None,
        };
        let result = self.run_body(&mut m);
        if is_shared {
            let temps = std::mem::take(&mut m.temps);
            SHARED_TEMPS.with(|s| *s.borrow_mut() = Some(temps));
        }
        result
    }

    fn run_body(&self, m: &mut Machine) -> Value {
        let mut last = Value::Null;
        for stmt in &self.body {
            last = m.eval(stmt);
            match m.flow.take() {
                None => {}
                Some(Flow::Return(v)) => return v,
                Some(_) => return Value::Num(0.0),
            }
        }
        if self.single { last } else { Value::Num(0.0) }
    }

    /// [`Program::eval`] as a number.
    pub fn eval_f32(&self, ctx: &mut EvalCtx) -> f32 {
        self.eval(ctx).num()
    }
}

/// The queries that are plain functions of their arguments, whatever the host:
/// `any(a, x, y...)` (a equals one of x, y), `all(a, x, y...)` (a equals each),
/// `approx_eq(a, b...)` (all within a millionth of a).
fn builtin_query(name: &str, args: &[Value]) -> Option<Value> {
    let (first, rest) = args.split_first()?;
    match name {
        "any" => Some(Value::bool(rest.iter().any(|v| first.equals(v)))),
        "all" => Some(Value::bool(
            !rest.is_empty() && rest.iter().all(|v| first.equals(v)),
        )),
        "approx_eq" => Some(Value::bool(
            rest.iter().all(|v| (first.num() - v.num()).abs() < 1e-6),
        )),
        _ => None,
    }
}

/// Evaluates `$e` on `$m`, leaving the enclosing node (with `Null`) when the run leaves it.
macro_rules! ev {
    ($m:ident, $e:expr) => {{
        let v = $m.eval($e);
        if $m.flow.is_some() {
            return Value::Null;
        }
        v
    }};
}

/// [`ev!`] as a number ([`Machine::num`]).
macro_rules! num {
    ($m:ident, $e:expr) => {{
        let n = $m.num($e);
        if $m.flow.is_some() {
            return Value::Null;
        }
        n
    }};
}

impl Machine<'_, '_> {
    /// Counts a node, abandoning the run past [`STEP_LIMIT`]: `false` when it is abandoned.
    fn step(&mut self) -> bool {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            self.flow = Some(Flow::Abort);
            return false;
        }
        true
    }

    /// `e`'s value as a number, without making a [`Value`] for the nodes that only make
    /// numbers (arithmetic, comparisons, logic, variables): most of what conditions and
    /// keyframes are. The same as `self.eval(e).num()`.
    fn num(&mut self, e: &Expr) -> f32 {
        let bool = |b: bool| if b { 1.0 } else { 0.0 };
        match e {
            Expr::Num(n) => {
                if !self.step() {
                    return 0.0;
                }
                *n
            }
            Expr::Var(scope, s) => {
                if !self.step() {
                    return 0.0;
                }
                let vars = match scope {
                    Scope::Variable => &*self.ctx.vars,
                    Scope::Temp => &self.temps,
                };
                vars.get(s).map_or(0.0, Value::num)
            }
            Expr::Neg(inner) => {
                if !self.step() {
                    return 0.0;
                }
                -self.num(inner)
            }
            Expr::Not(inner) => {
                if !self.step() {
                    return 0.0;
                }
                let n = self.num(inner);
                bool(n == 0.0)
            }
            Expr::Bin(op, l, r) if !matches!(op, BinOp::Eq | BinOp::Ne) => {
                if !self.step() {
                    return 0.0;
                }
                self.num_bin(*op, l, r)
            }
            _ => self.eval(e).num(),
        }
    }

    fn eval(&mut self, e: &Expr) -> Value {
        if !self.step() {
            return Value::Null;
        }
        match e {
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Var(Scope::Variable, s) => self.ctx.vars.get(s).cloned().unwrap_or_default(),
            Expr::Var(Scope::Temp, s) => self.temps.get(s).cloned().unwrap_or_default(),
            Expr::Query(name, args) => self
                .with_args(args, |m, args| match builtin_query(name, args) {
                    Some(v) => v,
                    None => m.ctx.host.query(name, args),
                })
                .unwrap_or_default(),
            Expr::Context(name) => self.ctx.host.context(name),
            Expr::Math(f, args) => {
                let mut nums = [0f32; 4];
                let mut extra = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let n = num!(self, a);
                    if i < nums.len() {
                        nums[i] = n;
                    } else {
                        extra.push(n);
                    }
                }
                let n = args.len().min(nums.len());
                if extra.is_empty() {
                    Value::Num(f.call(&nums[..n], self.ctx.rng))
                } else {
                    let mut all = nums.to_vec();
                    all.extend(extra);
                    Value::Num(f.call(&all, self.ctx.rng))
                }
            }
            Expr::Array(name, index) => {
                let i = num!(self, index);
                match self.ctx.host.array(name) {
                    Some(items) if !items.is_empty() => {
                        // Negative indexes use 0; larger ones wrap by the array's size.
                        let i = if i > 0.0 { i as usize } else { 0 };
                        items[i % items.len()].clone()
                    }
                    _ => Value::Null,
                }
            }
            Expr::Resource(kind, name) => Value::Res(*kind, name.clone()),
            Expr::This => self.ctx.this.clone(),
            Expr::Neg(inner) => Value::Num(-num!(self, inner)),
            Expr::Not(inner) => Value::bool(num!(self, inner) == 0.0),
            Expr::Bin(op, l, r) => self.binary(*op, l, r),
            Expr::Ternary(cond, then, otherwise) => {
                if num!(self, cond) != 0.0 {
                    self.eval(then)
                } else if let Some(o) = otherwise {
                    self.eval(o)
                } else {
                    Value::Num(0.0)
                }
            }
            Expr::Coalesce(a, b) => {
                let v = self.eval(a);
                match self.flow {
                    None if v != Value::Null => v,
                    None => self.eval(b),
                    Some(Flow::Abort) => Value::Null,
                    // `return`, `break` or `continue` inside the left side: the right side.
                    Some(_) => {
                        self.flow = None;
                        self.eval(b)
                    }
                }
            }
            Expr::Assign(scope, sym, rhs) => {
                let v = ev!(self, rhs);
                match scope {
                    Scope::Variable => self.ctx.vars.insert(*sym, v.clone()),
                    Scope::Temp => self.temps.insert(*sym, v.clone()),
                };
                let (numbers, transform): (&[f32], bool) = match &v {
                    Value::Vec3(xyz) => (xyz, false),
                    Value::Trs(trs) => (trs.as_flattened(), true),
                    _ => (&[], false),
                };
                if !numbers.is_empty() {
                    let (vector, trs) = member_syms(*sym);
                    let members: &[Sym] = if transform { &trs } else { &vector };
                    for (member, n) in members.iter().zip(numbers) {
                        match scope {
                            Scope::Variable => self.ctx.vars.insert(*member, Value::Num(*n)),
                            Scope::Temp => self.temps.insert(*member, Value::Num(*n)),
                        };
                    }
                }
                v
            }
            Expr::Block(stmts) => {
                let mut last = Value::Null;
                for s in stmts.iter() {
                    last = ev!(self, s);
                }
                last
            }
            Expr::Loop(count, body) => {
                let n = num!(self, count);
                let n = if n > 0.0 {
                    (n as usize).min(LOOP_LIMIT)
                } else {
                    0
                };
                for _ in 0..n {
                    self.eval(body);
                    if self.leave_loop() {
                        break;
                    }
                }
                if self.flow.is_some() {
                    return Value::Null;
                }
                Value::Num(0.0)
            }
            Expr::ForEach(scope, sym, array, body) => {
                let items = self.ctx.host.array(array).unwrap_or_default();
                for item in items.into_iter().take(LOOP_LIMIT) {
                    match scope {
                        Scope::Variable => self.ctx.vars.insert(*sym, item),
                        Scope::Temp => self.temps.insert(*sym, item),
                    };
                    self.eval(body);
                    if self.leave_loop() {
                        break;
                    }
                }
                if self.flow.is_some() {
                    return Value::Null;
                }
                Value::Num(0.0)
            }
            Expr::Return(inner) => {
                let v = ev!(self, inner);
                self.flow = Some(Flow::Return(v));
                Value::Null
            }
            Expr::Break => {
                self.flow = Some(Flow::Break);
                Value::Null
            }
            Expr::Continue => {
                self.flow = Some(Flow::Continue);
                Value::Null
            }
            Expr::Arrow(entity, rhs) => match ev!(self, entity) {
                Value::Entity(id) => match &**rhs {
                    Expr::Var(_, sym) => self.ctx.host.entity_var(id, *sym),
                    Expr::Query(name, args) => self
                        .with_args(args, |m, args| m.ctx.host.entity_query(id, name, args))
                        .unwrap_or_default(),
                    _ => Value::Null,
                },
                _ => Value::Null,
            },
        }
    }

    /// After a loop's body: `true` when the loop stops (`break`, or a `return` or an abandoned
    /// run, which stay set for the nodes around it); `continue` goes on to the next round.
    fn leave_loop(&mut self) -> bool {
        match self.flow {
            None => false,
            Some(Flow::Continue) => {
                self.flow = None;
                false
            }
            Some(Flow::Break) => {
                self.flow = None;
                true
            }
            Some(_) => true,
        }
    }

    /// Runs `f` with the arguments' values; `None` when the run leaves while evaluating them.
    /// Up to four are kept on the stack: a query's arguments were an allocation each time.
    fn with_args<R>(
        &mut self,
        args: &[Expr],
        f: impl FnOnce(&mut Self, &[Value]) -> R,
    ) -> Option<R> {
        if args.len() <= 4 {
            let mut values: [Value; 4] = Default::default();
            for (slot, a) in values.iter_mut().zip(args) {
                *slot = self.eval(a);
                if self.flow.is_some() {
                    return None;
                }
            }
            return Some(f(self, &values[..args.len()]));
        }
        let mut values = Vec::with_capacity(args.len());
        for a in args {
            let v = self.eval(a);
            if self.flow.is_some() {
                return None;
            }
            values.push(v);
        }
        Some(f(self, &values))
    }

    fn binary(&mut self, op: BinOp, l: &Expr, r: &Expr) -> Value {
        match op {
            BinOp::Eq | BinOp::Ne => {
                let a = ev!(self, l);
                let b = ev!(self, r);
                Value::bool(a.equals(&b) == (op == BinOp::Eq))
            }
            _ => {
                let n = self.num_bin(op, l, r);
                if self.flow.is_some() {
                    return Value::Null;
                }
                Value::Num(n)
            }
        }
    }

    /// The numeric binary operators on their sides' numbers; `&&` and `||` stop early.
    fn num_bin(&mut self, op: BinOp, l: &Expr, r: &Expr) -> f32 {
        let x = self.num(l);
        if self.flow.is_some() {
            return 0.0;
        }
        match op {
            BinOp::And if x == 0.0 => return 0.0,
            BinOp::Or if x != 0.0 => return 1.0,
            _ => {}
        }
        let y = self.num(r);
        if self.flow.is_some() {
            return 0.0;
        }
        let bool = |b: bool| if b { 1.0 } else { 0.0 };
        match op {
            BinOp::Add => x + y,
            BinOp::Sub => x - y,
            BinOp::Mul => x * y,
            // Dividing by zero is an error in Molang, and errors are 0.
            BinOp::Div => {
                if y == 0.0 {
                    0.0
                } else {
                    x / y
                }
            }
            BinOp::Lt => bool(x < y),
            BinOp::Le => bool(x <= y),
            BinOp::Gt => bool(x > y),
            BinOp::Ge => bool(x >= y),
            BinOp::And | BinOp::Or => bool(y != 0.0),
            BinOp::Eq | BinOp::Ne => unreachable!(),
        }
    }
}
