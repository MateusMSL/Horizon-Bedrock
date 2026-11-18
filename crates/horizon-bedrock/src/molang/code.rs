//! A [`Program`] compiled to a flat list of operations, run by a small stack machine.
//!
//! The tree an expression parses to is spread over the heap, one allocation a node: walking it
//! spent most of its time waiting for the next node's memory. Compiled once, when the pack is
//! loaded, the same expression is one array read in order, with jumps for `?:`, `&&`, `||`,
//! `??` and loops. Numbers (most of what conditions and keyframes are) stay on a stack of
//! `f32` apart from the [`Value`]s.
//!
//! It gives what [`super::eval`]'s tree walker gives, which stays as the reference: the
//! `HORIZON_MOLANG_INTERP=1` environment variable runs every program on it instead, and the tests
//! run each expression on both. A program with a construct this compiler leaves out (a `return`,
//! `break` or `continue` on the left of `??`, whose meaning there is the walker's alone) is not
//! compiled and runs on the walker.

use super::ast::{BinOp, Expr, Program, Scope, Sym};
use super::eval::{EvalCtx, LOOP_LIMIT, STEP_LIMIT, Value, Vars, builtin_query, store};
use super::math::MathFn;
use std::cell::Cell;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Op {
    // On the number stack.
    Num(f32),
    /// A variable as a number.
    VarNum(Scope, Sym),
    Neg,
    Not,
    /// `+ - * / < <= > >=` on the top two numbers.
    Bin(BinOp),
    /// `&&`: a zero on top stays as the answer and jumps; anything else is dropped.
    AndSkip(u32),
    /// `||`: a non-zero on top becomes 1 and jumps; a zero is dropped.
    OrSkip(u32),
    /// The top number as a boolean (1 or 0).
    Truthy,
    /// `+ - * / < <= > >=` of the top number and this one (`x * 2`).
    BinK(BinOp, f32),
    /// `query.names[i]` of the top `n` values, as a number.
    QueryNum(u32, u16),
    /// Drops the top number and jumps when it is zero.
    JumpIfZero(u32),
    Jump(u32),
    /// The function of the top `n` numbers.
    Math(MathFn, u16),
    /// The top number made a value, and the other way.
    ToVal,
    ToNum,
    PopNum,

    // On the value stack.
    Const(u32),
    Var(Scope, Sym),
    /// `query.names[i]` of the top `n` values.
    Query(u32, u16),
    Context(u32),
    /// `array.names[i]` at the index on top of the number stack.
    Array(u32),
    This,
    /// `==` (true) or `!=` of the top two values, onto the number stack.
    Equal(bool),
    /// Stores the top value, keeping it (`Assign`) or not (`AssignPop`).
    Assign(Scope, Sym),
    AssignPop(Scope, Sym),
    /// Stores the top number.
    AssignNum(Scope, Sym),
    /// Stores this number.
    SetNum(Scope, Sym, f32),
    Pop,
    /// `??`: a value on top that is not null is the answer and jumps; null is dropped.
    CoalesceSkip(u32),

    // Loops and leaving.
    /// Starts a `loop` of the count on top of the number stack.
    LoopInit,
    /// Starts a `for_each` over `array.names[i]`.
    ForInit(u32),
    /// Next round of a `loop`, or jumps past it.
    LoopNext(u32),
    /// The next element into the variable, or jumps past it.
    ForNext(Scope, Sym, u32),
    LoopEnd,
    /// `break` and `continue`: both stacks back to what they held when the loop's body started,
    /// then the jump.
    Leave(u16, u16, u32),
    /// `return` with the top value.
    Ret,
    /// `return` with the top number.
    RetNum,
    /// Leaves with 0 (a `break` or `continue` outside a loop).
    RetZero,

    // `entity -> ...`.
    /// The top value not an entity: null instead, and the jump past the right side.
    ArrowCheck(u32),
    ArrowVar(Sym),
    /// `entity -> query.names[i]` of the top `n` values, the entity under them.
    ArrowQuery(u32, u16),
    /// A right side the arrow does not read: null.
    ArrowNull,
}

/// A compiled program.
#[derive(Debug, PartialEq)]
pub struct Code {
    ops: Box<[Op]>,
    consts: Box<[Value]>,
    names: Box<[Arc<str>]>,
}

/// Where a loop's body starts, and the jumps out of it still to be placed.
struct LoopAt {
    top: u32,
    ns: u16,
    vs: u16,
    breaks: Vec<usize>,
}

struct Compiler {
    ops: Vec<Op>,
    consts: Vec<Value>,
    names: Vec<Arc<str>>,
    /// How many numbers and values the stacks hold at this point of the program.
    ns: u16,
    vs: u16,
    loops: Vec<LoopAt>,
}

/// Leaves the compiler with `None`: the program runs on the tree walker.
struct Unsupported;

type Done = Result<(), Unsupported>;

impl Code {
    /// The program compiled, or `None` when it uses what only the walker runs.
    pub(super) fn compile(program: &Program) -> Option<Code> {
        let mut c = Compiler {
            ops: Vec::new(),
            consts: Vec::new(),
            names: Vec::new(),
            ns: 0,
            vs: 0,
            loops: Vec::new(),
        };
        let body = &program.body;
        if program.single && body.len() == 1 {
            c.ret(&body[0]).ok()?;
        } else {
            for stmt in body {
                c.effect(stmt).ok()?;
            }
            c.emit(Op::RetZero);
        }
        Some(Code {
            ops: c.ops.into(),
            consts: c.consts.into(),
            names: c.names.into(),
        })
    }
}

/// Whether `e` holds a `return`, `break` or `continue` anywhere.
fn leaves(e: &Expr) -> bool {
    match e {
        Expr::Return(_) | Expr::Break | Expr::Continue => true,
        Expr::Num(_)
        | Expr::Str(_)
        | Expr::Var(..)
        | Expr::Context(_)
        | Expr::Resource(..)
        | Expr::This => false,
        Expr::Query(_, args) | Expr::Math(_, args) | Expr::Block(args) => args.iter().any(leaves),
        Expr::Array(_, a) | Expr::Neg(a) | Expr::Not(a) | Expr::Assign(_, _, a) => leaves(a),
        Expr::ForEach(_, _, _, a) => leaves(a),
        Expr::Bin(_, a, b) | Expr::Coalesce(a, b) | Expr::Loop(a, b) | Expr::Arrow(a, b) => {
            leaves(a) || leaves(b)
        }
        Expr::Ternary(a, b, c) => leaves(a) || leaves(b) || c.as_deref().is_some_and(leaves),
    }
}

/// Whether `e` always gives a number: what [`Compiler::num`] compiles without going through the
/// value stack. Not a variable, which may hold a string, an entity or a vector.
fn numeric(e: &Expr) -> bool {
    match e {
        Expr::Num(_) | Expr::Neg(_) | Expr::Not(_) | Expr::Math(..) => true,
        Expr::Bin(..) => true,
        Expr::Ternary(_, a, b) => numeric(a) && b.as_deref().is_none_or(numeric),
        _ => false,
    }
}

impl Compiler {
    fn emit(&mut self, op: Op) -> usize {
        self.ops.push(op);
        self.ops.len() - 1
    }

    fn here(&self) -> u32 {
        self.ops.len() as u32
    }

    /// Points the jump at `at` to the next operation.
    fn land(&mut self, at: usize) {
        let to = self.here();
        match &mut self.ops[at] {
            Op::AndSkip(t)
            | Op::OrSkip(t)
            | Op::JumpIfZero(t)
            | Op::Jump(t)
            | Op::CoalesceSkip(t)
            | Op::LoopNext(t)
            | Op::ForNext(_, _, t)
            | Op::Leave(_, _, t)
            | Op::ArrowCheck(t) => *t = to,
            other => unreachable!("not a jump: {other:?}"),
        }
    }

    fn name(&mut self, name: &Arc<str>) -> u32 {
        match self.names.iter().position(|n| n == name) {
            Some(i) => i as u32,
            None => {
                self.names.push(name.clone());
                self.names.len() as u32 - 1
            }
        }
    }

    fn constant(&mut self, v: Value) {
        // The same constant once; `-0` and `0` are two (equal as numbers, not as answers).
        let same = |c: &Value| match (c, &v) {
            (Value::Num(a), Value::Num(b)) => a.to_bits() == b.to_bits(),
            _ => *c == v,
        };
        let i = match self.consts.iter().position(same) {
            Some(i) => i,
            None => {
                self.consts.push(v);
                self.consts.len() - 1
            }
        };
        self.emit(Op::Const(i as u32));
        self.vs += 1;
    }

    /// Leaves the program with `e`'s value.
    fn ret(&mut self, e: &Expr) -> Done {
        if numeric(e) {
            self.num(e)?;
            self.emit(Op::RetNum);
            self.ns -= 1;
        } else {
            self.val(e)?;
            self.emit(Op::Ret);
            self.vs -= 1;
        }
        Ok(())
    }

    /// `e`, its number on the number stack.
    fn num(&mut self, e: &Expr) -> Done {
        match e {
            Expr::Num(n) => {
                self.emit(Op::Num(*n));
                self.ns += 1;
            }
            Expr::Var(scope, sym) => {
                self.emit(Op::VarNum(*scope, *sym));
                self.ns += 1;
            }
            Expr::Neg(inner) => {
                self.num(inner)?;
                self.emit(Op::Neg);
            }
            Expr::Not(inner) => {
                self.num(inner)?;
                self.emit(Op::Not);
            }
            Expr::Bin(op @ (BinOp::And | BinOp::Or), l, r) => {
                self.num(l)?;
                let skip = self.emit(if *op == BinOp::And {
                    Op::AndSkip(0)
                } else {
                    Op::OrSkip(0)
                });
                self.ns -= 1;
                self.num(r)?;
                self.emit(Op::Truthy);
                self.land(skip);
            }
            Expr::Bin(op @ (BinOp::Eq | BinOp::Ne), l, r) => {
                self.val(l)?;
                self.val(r)?;
                self.emit(Op::Equal(*op == BinOp::Eq));
                self.vs -= 2;
                self.ns += 1;
            }
            Expr::Bin(op, l, r) => {
                self.num(l)?;
                match **r {
                    Expr::Num(k) => {
                        self.emit(Op::BinK(*op, k));
                    }
                    _ => {
                        self.num(r)?;
                        self.emit(Op::Bin(*op));
                        self.ns -= 1;
                    }
                }
            }
            Expr::Query(name, args) => {
                for a in args.iter() {
                    self.val(a)?;
                }
                let i = self.name(name);
                self.emit(Op::QueryNum(i, args.len() as u16));
                self.vs -= args.len() as u16;
                self.ns += 1;
            }
            Expr::Math(f, args) => {
                for a in args.iter() {
                    self.num(a)?;
                }
                self.emit(Op::Math(*f, args.len() as u16));
                self.ns -= args.len() as u16;
                self.ns += 1;
            }
            Expr::Ternary(cond, then, otherwise) => {
                self.num(cond)?;
                let to_else = self.emit(Op::JumpIfZero(0));
                self.ns -= 1;
                self.num(then)?;
                let to_end = self.emit(Op::Jump(0));
                self.ns -= 1;
                self.land(to_else);
                match otherwise {
                    Some(o) => self.num(o)?,
                    None => {
                        self.emit(Op::Num(0.0));
                        self.ns += 1;
                    }
                }
                self.land(to_end);
            }
            _ => {
                self.val(e)?;
                self.emit(Op::ToNum);
                self.vs -= 1;
                self.ns += 1;
            }
        }
        Ok(())
    }

    /// `e`, its value on the value stack.
    fn val(&mut self, e: &Expr) -> Done {
        match e {
            Expr::Num(n) => self.constant(Value::Num(*n)),
            Expr::Str(s) => self.constant(Value::Str(s.clone())),
            Expr::Resource(kind, name) => self.constant(Value::Res(*kind, name.clone())),
            Expr::Var(scope, sym) => {
                self.emit(Op::Var(*scope, *sym));
                self.vs += 1;
            }
            Expr::Query(name, args) => {
                for a in args.iter() {
                    self.val(a)?;
                }
                let i = self.name(name);
                self.emit(Op::Query(i, args.len() as u16));
                self.vs -= args.len() as u16;
                self.vs += 1;
            }
            Expr::Context(name) => {
                let i = self.name(name);
                self.emit(Op::Context(i));
                self.vs += 1;
            }
            Expr::Array(name, index) => {
                self.num(index)?;
                let i = self.name(name);
                self.emit(Op::Array(i));
                self.ns -= 1;
                self.vs += 1;
            }
            Expr::This => {
                self.emit(Op::This);
                self.vs += 1;
            }
            Expr::Ternary(cond, then, otherwise) if !numeric(e) => {
                self.num(cond)?;
                let to_else = self.emit(Op::JumpIfZero(0));
                self.ns -= 1;
                self.val(then)?;
                let to_end = self.emit(Op::Jump(0));
                self.vs -= 1;
                self.land(to_else);
                match otherwise {
                    Some(o) => self.val(o)?,
                    None => self.constant(Value::Num(0.0)),
                }
                self.land(to_end);
            }
            Expr::Coalesce(a, b) => {
                // A `return`, `break` or `continue` on the left goes on to the right side
                // there: the walker's rule, left to it.
                if leaves(a) {
                    return Err(Unsupported);
                }
                self.val(a)?;
                let skip = self.emit(Op::CoalesceSkip(0));
                self.vs -= 1;
                self.val(b)?;
                self.land(skip);
            }
            Expr::Assign(scope, sym, rhs) => {
                self.val(rhs)?;
                self.emit(Op::Assign(*scope, *sym));
            }
            Expr::Block(stmts) => match stmts.split_last() {
                None => self.constant(Value::Null),
                Some((last, first)) => {
                    for s in first {
                        self.effect(s)?;
                    }
                    self.val(last)?;
                }
            },
            Expr::Loop(..) | Expr::ForEach(..) => {
                self.looped(e)?;
                self.constant(Value::Num(0.0));
            }
            Expr::Return(inner) => {
                self.ret(inner)?;
                // Never reached: counted as the value the expression stands for.
                self.vs += 1;
            }
            Expr::Break | Expr::Continue => {
                self.leave(matches!(e, Expr::Break));
                // Never reached: counted as the value the expression stands for.
                self.vs += 1;
            }
            Expr::Arrow(entity, rhs) => {
                self.val(entity)?;
                let skip = self.emit(Op::ArrowCheck(0));
                match &**rhs {
                    Expr::Var(_, sym) => {
                        self.emit(Op::ArrowVar(*sym));
                    }
                    Expr::Query(name, args) => {
                        for a in args.iter() {
                            self.val(a)?;
                        }
                        let i = self.name(name);
                        self.emit(Op::ArrowQuery(i, args.len() as u16));
                        self.vs -= args.len() as u16;
                    }
                    _ => {
                        self.emit(Op::ArrowNull);
                    }
                }
                self.land(skip);
            }
            // The numeric nodes: their number, made a value.
            _ => {
                self.num(e)?;
                self.emit(Op::ToVal);
                self.ns -= 1;
                self.vs += 1;
            }
        }
        Ok(())
    }

    /// `e` for what it does, its value dropped.
    fn effect(&mut self, e: &Expr) -> Done {
        match e {
            Expr::Num(_) | Expr::Str(_) | Expr::Var(..) | Expr::Resource(..) | Expr::This => {}
            Expr::Assign(scope, sym, rhs) if matches!(**rhs, Expr::Num(_)) => {
                let Expr::Num(k) = **rhs else { unreachable!() };
                self.emit(Op::SetNum(*scope, *sym, k));
            }
            Expr::Assign(scope, sym, rhs) if numeric(rhs) => {
                self.num(rhs)?;
                self.emit(Op::AssignNum(*scope, *sym));
                self.ns -= 1;
            }
            Expr::Assign(scope, sym, rhs) => {
                self.val(rhs)?;
                self.emit(Op::AssignPop(*scope, *sym));
                self.vs -= 1;
            }
            Expr::Block(stmts) => {
                for s in stmts.iter() {
                    self.effect(s)?;
                }
            }
            Expr::Ternary(cond, then, otherwise) => {
                self.num(cond)?;
                let to_else = self.emit(Op::JumpIfZero(0));
                self.ns -= 1;
                self.effect(then)?;
                match otherwise {
                    Some(o) => {
                        let to_end = self.emit(Op::Jump(0));
                        self.land(to_else);
                        self.effect(o)?;
                        self.land(to_end);
                    }
                    None => self.land(to_else),
                }
            }
            Expr::Loop(..) | Expr::ForEach(..) => self.looped(e)?,
            Expr::Return(inner) => self.ret(inner)?,
            Expr::Break | Expr::Continue => self.leave(matches!(e, Expr::Break)),
            _ if numeric(e) => {
                self.num(e)?;
                self.emit(Op::PopNum);
                self.ns -= 1;
            }
            _ => {
                self.val(e)?;
                self.emit(Op::Pop);
                self.vs -= 1;
            }
        }
        Ok(())
    }

    /// A `loop` or a `for_each`, leaving nothing on the stacks.
    fn looped(&mut self, e: &Expr) -> Done {
        let body = match e {
            Expr::Loop(count, body) => {
                self.num(count)?;
                self.emit(Op::LoopInit);
                self.ns -= 1;
                body
            }
            Expr::ForEach(_, _, array, body) => {
                let i = self.name(array);
                self.emit(Op::ForInit(i));
                body
            }
            _ => unreachable!(),
        };
        let top = self.here();
        let next = match e {
            Expr::ForEach(scope, sym, ..) => self.emit(Op::ForNext(*scope, *sym, 0)),
            _ => self.emit(Op::LoopNext(0)),
        };
        self.loops.push(LoopAt {
            top,
            ns: self.ns,
            vs: self.vs,
            breaks: Vec::new(),
        });
        self.effect(body)?;
        self.emit(Op::Jump(top));
        let at = self.loops.pop().expect("the loop pushed above");
        self.land(next);
        for b in at.breaks {
            self.land(b);
        }
        self.emit(Op::LoopEnd);
        Ok(())
    }

    /// `break` (true) or `continue`: to the innermost loop, or out of the program with 0.
    fn leave(&mut self, out: bool) {
        let Some(at) = self.loops.last() else {
            self.emit(Op::RetZero);
            return;
        };
        let (ns, vs, top) = (at.ns, at.vs, at.top);
        let jump = self.emit(Op::Leave(ns, vs, top));
        if out {
            self.loops
                .last_mut()
                .expect("checked above")
                .breaks
                .push(jump);
        }
    }
}

/// A loop being run: the rounds left of a `loop`, the elements left of a `for_each`.
enum Frame {
    Count(usize),
    Items(std::iter::Take<std::vec::IntoIter<Value>>),
}

/// The stacks a run uses, kept per thread between runs so a run allocates nothing.
#[derive(Default)]
struct Stacks {
    ns: Vec<f32>,
    vs: Vec<Value>,
    frames: Vec<Frame>,
}

thread_local! {
    static STACKS: Cell<Stacks> = Cell::default();
}

fn bool(b: bool) -> f32 {
    if b { 1.0 } else { 0.0 }
}

/// `x op y` for the numeric binary operators.
#[inline(always)]
fn arith(op: BinOp, x: f32, y: f32) -> f32 {
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
        BinOp::And | BinOp::Or | BinOp::Eq | BinOp::Ne => 0.0,
    }
}

impl Code {
    /// Runs the program; `temps` are its `temp.*` variables.
    pub(super) fn run(&self, ctx: &mut EvalCtx, temps: &mut Vars) -> Value {
        let mut stacks = STACKS.with(Cell::take);
        let result = self.exec(ctx, temps, &mut stacks);
        stacks.ns.clear();
        stacks.vs.clear();
        stacks.frames.clear();
        STACKS.with(|s| s.set(stacks));
        result
    }

    fn exec(&self, ctx: &mut EvalCtx, temps: &mut Vars, st: &mut Stacks) -> Value {
        let ops = &*self.ops;
        let (ns, vs, frames) = (&mut st.ns, &mut st.vs, &mut st.frames);
        let mut pc = 0usize;
        // Rounds of loops run: only loops make a run long, and past the limit it is abandoned.
        let mut rounds = 0u32;
        macro_rules! pop_num {
            () => {
                ns.pop().unwrap_or_default()
            };
        }
        macro_rules! top_num {
            () => {
                match ns.last_mut() {
                    Some(n) => n,
                    None => return Value::Num(0.0),
                }
            };
        }
        loop {
            let Some(&op) = ops.get(pc) else {
                return Value::Num(0.0);
            };
            pc += 1;
            match op {
                Op::Num(n) => ns.push(n),
                Op::VarNum(scope, sym) => {
                    let vars = match scope {
                        Scope::Variable => &*ctx.vars,
                        Scope::Temp => &*temps,
                    };
                    ns.push(vars.get(&sym).map_or(0.0, Value::num));
                }
                Op::Neg => {
                    let x = top_num!();
                    *x = -*x;
                }
                Op::Not => {
                    let x = top_num!();
                    *x = bool(*x == 0.0);
                }
                Op::Bin(op) => {
                    let y = pop_num!();
                    let x = top_num!();
                    *x = arith(op, *x, y);
                }
                Op::BinK(op, y) => {
                    let x = top_num!();
                    *x = arith(op, *x, y);
                }
                Op::QueryNum(i, n) => {
                    let name = &*self.names[i as usize];
                    let at = vs.len().saturating_sub(n as usize);
                    let args = &vs[at..];
                    let v = match builtin_query(name, args) {
                        Some(v) => v.num(),
                        None => ctx.host.query(name, args).num(),
                    };
                    vs.truncate(at);
                    ns.push(v);
                }
                Op::SetNum(scope, sym, n) => {
                    match scope {
                        Scope::Variable => ctx.vars.insert(sym, Value::Num(n)),
                        Scope::Temp => temps.insert(sym, Value::Num(n)),
                    };
                }
                Op::RetNum => return Value::Num(pop_num!()),
                Op::AndSkip(to) => {
                    if *top_num!() == 0.0 {
                        *top_num!() = 0.0;
                        pc = to as usize;
                    } else {
                        ns.pop();
                    }
                }
                Op::OrSkip(to) => {
                    if *top_num!() != 0.0 {
                        *top_num!() = 1.0;
                        pc = to as usize;
                    } else {
                        ns.pop();
                    }
                }
                Op::Truthy => {
                    let x = top_num!();
                    *x = bool(*x != 0.0);
                }
                Op::JumpIfZero(to) => {
                    if pop_num!() == 0.0 {
                        pc = to as usize;
                    }
                }
                Op::Jump(to) => pc = to as usize,
                Op::Math(f, n) => {
                    let at = ns.len().saturating_sub(n as usize);
                    let r = f.call(&ns[at..], ctx.rng);
                    ns.truncate(at);
                    ns.push(r);
                }
                Op::ToVal => vs.push(Value::Num(pop_num!())),
                Op::ToNum => ns.push(vs.pop().map_or(0.0, |v| v.num())),
                Op::PopNum => {
                    ns.pop();
                }
                Op::Const(i) => vs.push(self.consts[i as usize].clone()),
                Op::Var(scope, sym) => {
                    let vars = match scope {
                        Scope::Variable => &*ctx.vars,
                        Scope::Temp => &*temps,
                    };
                    vs.push(vars.get(&sym).cloned().unwrap_or_default());
                }
                Op::Query(i, n) => {
                    let name = &*self.names[i as usize];
                    let at = vs.len().saturating_sub(n as usize);
                    let args = &vs[at..];
                    let v = match builtin_query(name, args) {
                        Some(v) => v,
                        None => ctx.host.query(name, args),
                    };
                    vs.truncate(at);
                    vs.push(v);
                }
                Op::Context(i) => vs.push(ctx.host.context(&self.names[i as usize])),
                Op::Array(i) => {
                    let at = pop_num!();
                    let v = ctx.host.array_at(&self.names[i as usize], at);
                    vs.push(v.unwrap_or_default());
                }
                Op::This => vs.push(ctx.this.clone()),
                Op::Equal(eq) => {
                    let b = vs.pop().unwrap_or_default();
                    let a = vs.pop().unwrap_or_default();
                    ns.push(bool(a.equals(&b) == eq));
                }
                Op::Assign(scope, sym) => {
                    let v = vs.last().cloned().unwrap_or_default();
                    store(ctx.vars, temps, scope, sym, v);
                }
                Op::AssignPop(scope, sym) => {
                    let v = vs.pop().unwrap_or_default();
                    store(ctx.vars, temps, scope, sym, v);
                }
                Op::AssignNum(scope, sym) => {
                    let v = Value::Num(pop_num!());
                    match scope {
                        Scope::Variable => ctx.vars.insert(sym, v),
                        Scope::Temp => temps.insert(sym, v),
                    };
                }
                Op::Pop => {
                    vs.pop();
                }
                Op::CoalesceSkip(to) => {
                    if vs.last().is_some_and(|v| *v != Value::Null) {
                        pc = to as usize;
                    } else {
                        vs.pop();
                    }
                }
                Op::LoopInit => {
                    let n = pop_num!();
                    let n = if n > 0.0 {
                        (n as usize).min(LOOP_LIMIT)
                    } else {
                        0
                    };
                    frames.push(Frame::Count(n));
                }
                Op::ForInit(i) => {
                    let items = ctx.host.array(&self.names[i as usize]).unwrap_or_default();
                    frames.push(Frame::Items(items.into_iter().take(LOOP_LIMIT)));
                }
                Op::LoopNext(to) => {
                    rounds += 1;
                    if rounds > STEP_LIMIT {
                        return Value::Num(0.0);
                    }
                    match frames.last_mut() {
                        Some(Frame::Count(left)) if *left > 0 => *left -= 1,
                        _ => pc = to as usize,
                    }
                }
                Op::ForNext(scope, sym, to) => match frames.last_mut() {
                    Some(Frame::Items(items)) => match items.next() {
                        Some(item) => {
                            match scope {
                                Scope::Variable => ctx.vars.insert(sym, item),
                                Scope::Temp => temps.insert(sym, item),
                            };
                        }
                        None => pc = to as usize,
                    },
                    _ => pc = to as usize,
                },
                Op::LoopEnd => {
                    frames.pop();
                }
                Op::Leave(n, v, to) => {
                    ns.truncate(n as usize);
                    vs.truncate(v as usize);
                    pc = to as usize;
                }
                Op::Ret => return vs.pop().unwrap_or_default(),
                Op::RetZero => return Value::Num(0.0),
                Op::ArrowCheck(to) => {
                    if !matches!(vs.last(), Some(Value::Entity(_))) {
                        vs.pop();
                        vs.push(Value::Null);
                        pc = to as usize;
                    }
                }
                Op::ArrowVar(sym) => {
                    let v = match vs.pop() {
                        Some(Value::Entity(id)) => ctx.host.entity_var(id, sym),
                        _ => Value::Null,
                    };
                    vs.push(v);
                }
                Op::ArrowQuery(i, n) => {
                    let at = vs.len().saturating_sub(n as usize);
                    let v = match at.checked_sub(1).map(|e| &vs[e]) {
                        Some(&Value::Entity(id)) => {
                            ctx.host
                                .entity_query(id, &self.names[i as usize], &vs[at..])
                        }
                        _ => Value::Null,
                    };
                    vs.truncate(at.saturating_sub(1));
                    vs.push(v);
                }
                Op::ArrowNull => {
                    vs.pop();
                    vs.push(Value::Null);
                }
            }
        }
    }
}
