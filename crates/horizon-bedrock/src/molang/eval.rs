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

enum Flow {
    Return(Value),
    Break,
    Continue,
    Abort,
}

type Res = Result<Value, Flow>;

struct Machine<'a, 'b> {
    ctx: &'a mut EvalCtx<'b>,
    temps: Vars,
    steps: u32,
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
            match m.eval(stmt) {
                Ok(v) => last = v,
                Err(Flow::Return(v)) => return v,
                Err(_) => return Value::Num(0.0),
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
        "all" => Some(Value::bool(!rest.is_empty() && rest.iter().all(|v| first.equals(v)))),
        "approx_eq" => Some(Value::bool(
            rest.iter().all(|v| (first.num() - v.num()).abs() < 1e-6),
        )),
        _ => None,
    }
}

impl Machine<'_, '_> {
    fn eval(&mut self, e: &Expr) -> Res {
        self.steps += 1;
        if self.steps > STEP_LIMIT {
            return Err(Flow::Abort);
        }
        Ok(match e {
            Expr::Num(n) => Value::Num(*n),
            Expr::Str(s) => Value::Str(s.clone()),
            Expr::Var(Scope::Variable, s) => self.ctx.vars.get(s).cloned().unwrap_or_default(),
            Expr::Var(Scope::Temp, s) => self.temps.get(s).cloned().unwrap_or_default(),
            Expr::Query(name, args) => {
                let args = self.eval_args(args)?;
                match builtin_query(name, &args) {
                    Some(v) => v,
                    None => self.ctx.host.query(name, &args),
                }
            }
            Expr::Context(name) => self.ctx.host.context(name),
            Expr::Math(f, args) => {
                let mut nums = [0f32; 4];
                let mut extra = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let n = self.eval(a)?.num();
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
                let i = self.eval(index)?.num();
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
            Expr::Neg(inner) => Value::Num(-self.eval(inner)?.num()),
            Expr::Not(inner) => Value::bool(!self.eval(inner)?.truthy()),
            Expr::Bin(op, l, r) => self.binary(*op, l, r)?,
            Expr::Ternary(cond, then, otherwise) => {
                if self.eval(cond)?.truthy() {
                    self.eval(then)?
                } else if let Some(o) = otherwise {
                    self.eval(o)?
                } else {
                    Value::Num(0.0)
                }
            }
            Expr::Coalesce(a, b) => match self.eval(a) {
                Ok(Value::Null) => self.eval(b)?,
                Ok(v) => v,
                Err(Flow::Abort) => return Err(Flow::Abort),
                Err(_) => self.eval(b)?,
            },
            Expr::Assign(scope, sym, rhs) => {
                let v = self.eval(rhs)?;
                match scope {
                    Scope::Variable => self.ctx.vars.insert(*sym, v.clone()),
                    Scope::Temp => self.temps.insert(*sym, v.clone()),
                };
                let mut members: Vec<(String, f32)> = Vec::new();
                match &v {
                    Value::Vec3(xyz) => {
                        for (axis, n) in ["x", "y", "z"].into_iter().zip(xyz) {
                            members.push((axis.to_owned(), *n));
                        }
                    }
                    Value::Trs(trs) => {
                        for (part, xyz) in ["t", "r", "s"].into_iter().zip(trs) {
                            for (axis, n) in ["x", "y", "z"].into_iter().zip(xyz) {
                                members.push((format!("{part}.{axis}"), *n));
                            }
                        }
                    }
                    _ => {}
                }
                if !members.is_empty() {
                    let base = super::ast::sym_name(*sym);
                    for (name, n) in members {
                        let member = super::ast::intern(&format!("{base}.{name}"));
                        match scope {
                            Scope::Variable => self.ctx.vars.insert(member, Value::Num(n)),
                            Scope::Temp => self.temps.insert(member, Value::Num(n)),
                        };
                    }
                }
                v
            }
            Expr::Block(stmts) => {
                let mut last = Value::Null;
                for s in stmts.iter() {
                    last = self.eval(s)?;
                }
                last
            }
            Expr::Loop(count, body) => {
                let n = self.eval(count)?.num();
                let n = if n > 0.0 {
                    (n as usize).min(LOOP_LIMIT)
                } else {
                    0
                };
                for _ in 0..n {
                    match self.eval(body) {
                        Ok(_) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(other) => return Err(other),
                    }
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
                    match self.eval(body) {
                        Ok(_) | Err(Flow::Continue) => {}
                        Err(Flow::Break) => break,
                        Err(other) => return Err(other),
                    }
                }
                Value::Num(0.0)
            }
            Expr::Return(inner) => return Err(Flow::Return(self.eval(inner)?)),
            Expr::Break => return Err(Flow::Break),
            Expr::Continue => return Err(Flow::Continue),
            Expr::Arrow(entity, rhs) => match self.eval(entity)? {
                Value::Entity(id) => match &**rhs {
                    Expr::Var(_, sym) => self.ctx.host.entity_var(id, *sym),
                    Expr::Query(name, args) => {
                        let args = self.eval_args(args)?;
                        self.ctx.host.entity_query(id, name, &args)
                    }
                    _ => Value::Null,
                },
                _ => Value::Null,
            },
        })
    }

    fn eval_args(&mut self, args: &[Expr]) -> Result<Vec<Value>, Flow> {
        let mut out = Vec::with_capacity(args.len());
        for a in args {
            out.push(self.eval(a)?);
        }
        Ok(out)
    }

    fn binary(&mut self, op: BinOp, l: &Expr, r: &Expr) -> Res {
        match op {
            BinOp::And => {
                return Ok(Value::bool(
                    self.eval(l)?.truthy() && self.eval(r)?.truthy(),
                ));
            }
            BinOp::Or => {
                return Ok(Value::bool(
                    self.eval(l)?.truthy() || self.eval(r)?.truthy(),
                ));
            }
            _ => {}
        }
        let (a, b) = (self.eval(l)?, self.eval(r)?);
        Ok(match op {
            BinOp::Eq => Value::bool(a.equals(&b)),
            BinOp::Ne => Value::bool(!a.equals(&b)),
            _ => {
                let (x, y) = (a.num(), b.num());
                match op {
                    BinOp::Add => Value::Num(x + y),
                    BinOp::Sub => Value::Num(x - y),
                    BinOp::Mul => Value::Num(x * y),
                    // Dividing by zero is an error in Molang, and errors are 0.
                    BinOp::Div => Value::Num(if y == 0.0 { 0.0 } else { x / y }),
                    BinOp::Lt => Value::bool(x < y),
                    BinOp::Le => Value::bool(x <= y),
                    BinOp::Gt => Value::bool(x > y),
                    BinOp::Ge => Value::bool(x >= y),
                    BinOp::Eq | BinOp::Ne | BinOp::And | BinOp::Or => Value::Null,
                }
            }
        })
    }
}
