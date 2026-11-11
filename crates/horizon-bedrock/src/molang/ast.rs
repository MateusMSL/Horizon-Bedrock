//! The parsed form of an expression.

use super::math::MathFn;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// An interned variable name (lower case, the whole path: `v.a.b` is `a.b`).
pub type Sym = u32;

#[derive(Default)]
struct Interner {
    ids: HashMap<Arc<str>, Sym>,
    names: Vec<Arc<str>>,
}

fn interner() -> &'static Mutex<Interner> {
    static INTERNER: OnceLock<Mutex<Interner>> = OnceLock::new();
    INTERNER.get_or_init(Mutex::default)
}

/// The symbol of `name`, which must already be lower case.
pub fn intern(name: &str) -> Sym {
    let mut i = interner().lock().expect("interner lock");
    if let Some(&id) = i.ids.get(name) {
        return id;
    }
    let id = i.names.len() as Sym;
    let name: Arc<str> = Arc::from(name);
    i.names.push(name.clone());
    i.ids.insert(name, id);
    id
}

/// The name a symbol stands for.
pub fn sym_name(sym: Sym) -> Arc<str> {
    interner().lock().expect("interner lock").names[sym as usize].clone()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// `variable.` / `v.`: kept on the entity between runs.
    Variable,
    /// `temp.` / `t.`: lives for one run.
    Temp,
}

/// What `geometry.x`, `texture.x` and `material.x` name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResKind {
    Geometry,
    Texture,
    Material,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f32),
    Str(Arc<str>),
    Var(Scope, Sym),
    /// `query.name(args)`, the name lower case.
    Query(Arc<str>, Box<[Expr]>),
    /// `context.name`.
    Context(Arc<str>),
    Math(MathFn, Box<[Expr]>),
    /// `array.name[index]`.
    Array(Arc<str>, Box<Expr>),
    Resource(ResKind, Arc<str>),
    /// The value an animation channel holds before this expression changes it.
    This,
    Neg(Box<Expr>),
    Not(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    /// `cond ? then : else`; without an else, a false condition gives 0.
    Ternary(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    /// `a ?? b`: `b` when `a` cannot be resolved.
    Coalesce(Box<Expr>, Box<Expr>),
    Assign(Scope, Sym, Box<Expr>),
    Block(Box<[Expr]>),
    /// `loop(count, { ... })`, at most 1024 rounds.
    Loop(Box<Expr>, Box<Expr>),
    /// `for_each(var, array.name, { ... })`.
    ForEach(Scope, Sym, Arc<str>, Box<Expr>),
    Return(Box<Expr>),
    Break,
    Continue,
    /// `entity -> v.x` or `entity -> q.x(...)`: the right side evaluated on that entity.
    Arrow(Box<Expr>, Box<Expr>),
}

/// A parsed expression: a list of statements.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub body: Vec<Expr>,
    /// One expression with no `;`: its value is the program's. Otherwise the value is what
    /// `return` gives, or 0.
    pub single: bool,
}

impl Program {
    /// The program is a bare number, like most animation keyframes: `Some(n)`.
    pub fn constant(&self) -> Option<f32> {
        match (self.single, self.body.as_slice()) {
            (true, [Expr::Num(n)]) => Some(*n),
            (true, [Expr::Neg(inner)]) => match **inner {
                Expr::Num(n) => Some(-n),
                _ => None,
            },
            _ => None,
        }
    }
}
