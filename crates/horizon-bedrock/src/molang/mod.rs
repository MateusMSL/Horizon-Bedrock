//! Molang, the expression language Bedrock packs drive their visuals with: bone angles in
//! animations, state changes in animation controllers, which texture or geometry a render
//! controller picks, which sound variant plays.
//!
//! [`parse`] turns source text into a [`Program`], once. [`Program::eval`] runs it against a
//! [`Host`] (the game's answers to `query.*` and `context.*`) and the entity's [`Vars`]
//! (`variable.*`, which persist between runs). Everything is read only toward the game: an
//! expression can write its own variables and temporaries and nothing else.
//!
//! The language follows Microsoft's Molang documentation: C-like precedence, `??`, ternaries
//! with or without an else, `{ ... }` blocks with `return`, `loop`, `for_each`, `break`,
//! `continue`, strings in single quotes, `array.name[i]`, the `->` operator, degrees in the
//! trigonometric functions, and errors that evaluate to 0 instead of failing.

mod ast;
mod eval;
mod lexer;
mod math;
mod parser;

pub use ast::{BinOp, Expr, Program, ResKind, Scope, Sym, intern, sym_name};
pub use eval::{EvalCtx, Host, NoHost, Value, Vars, shared_temps};
pub use math::{MathFn, Rng};
pub use parser::{ParseError, parse};

#[cfg(test)]
mod tests;
