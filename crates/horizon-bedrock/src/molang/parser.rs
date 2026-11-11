//! Tokens to a [`Program`]. Precedence, loosest first: `=`, `??`, `?:`, `||`, `&&`, `== !=`,
//! `< <= > >=`, `+ -`, `* /`, unary `- !`, then primaries (with `->` binding tightest).

use super::ast::{BinOp, Expr, Program, ResKind, Scope, intern};
use super::lexer::{Tok, lex};
use super::math::MathFn;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// Byte offset in the source.
    pub at: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (at {})", self.message, self.at)
    }
}

impl std::error::Error for ParseError {}

/// Parses `src`.
pub fn parse(src: &str) -> Result<Program, ParseError> {
    let toks = lex(src)?;
    let mut p = Parser {
        toks,
        pos: 0,
        end: src.len(),
    };
    let (body, trailing_semi) = p.statements(false)?;
    if p.pos < p.toks.len() {
        return Err(p.error("unexpected token"));
    }
    let single = body.len() == 1 && !trailing_semi;
    Ok(Program { body, single })
}

struct Parser {
    toks: Vec<(Tok, usize)>,
    pos: usize,
    end: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.0)
    }

    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.pos + n).map(|t| &t.0)
    }

    fn here(&self) -> usize {
        self.toks.get(self.pos).map_or(self.end, |t| t.1)
    }

    fn error(&self, m: &str) -> ParseError {
        ParseError {
            at: self.here(),
            message: m.to_owned(),
        }
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).map(|t| t.0.clone());
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, tok: Tok, what: &str) -> Result<(), ParseError> {
        if self.eat(&tok) {
            Ok(())
        } else {
            Err(self.error(&format!("expected {what}")))
        }
    }

    /// Statements up to the end, or up to `}` when `in_block`. Returns them and whether the
    /// last one ended with `;`.
    fn statements(&mut self, in_block: bool) -> Result<(Vec<Expr>, bool), ParseError> {
        let mut out = Vec::new();
        let mut trailing = false;
        loop {
            while self.eat(&Tok::Semi) {
                trailing = true;
            }
            match self.peek() {
                None => break,
                Some(Tok::RBrace) if in_block => break,
                _ => {}
            }
            out.push(self.assign()?);
            trailing = false;
            if self.eat(&Tok::Semi) {
                trailing = true;
            } else if !matches!(self.peek(), None | Some(Tok::RBrace)) {
                return Err(self.error("expected ; or end"));
            }
        }
        Ok((out, trailing))
    }

    fn assign(&mut self) -> Result<Expr, ParseError> {
        let lhs = self.coalesce()?;
        if self.peek() == Some(&Tok::Assign) {
            let Expr::Var(scope, sym) = lhs else {
                return Err(self.error("only variable. and temp. can be assigned"));
            };
            self.pos += 1;
            let rhs = self.assign()?;
            return Ok(Expr::Assign(scope, sym, Box::new(rhs)));
        }
        Ok(lhs)
    }

    fn coalesce(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.ternary()?;
        while self.eat(&Tok::Coalesce) {
            let right = self.ternary()?;
            left = Expr::Coalesce(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// A ternary branch: nested ternaries and assignments allowed.
    fn branch(&mut self) -> Result<Expr, ParseError> {
        let e = self.ternary()?;
        if self.peek() == Some(&Tok::Assign)
            && let Expr::Var(scope, sym) = e
        {
            self.pos += 1;
            let rhs = self.branch()?;
            return Ok(Expr::Assign(scope, sym, Box::new(rhs)));
        }
        Ok(e)
    }

    fn ternary(&mut self) -> Result<Expr, ParseError> {
        let cond = self.binary(0)?;
        if !self.eat(&Tok::Question) {
            return Ok(cond);
        }
        let then = self.branch()?;
        let otherwise = if self.eat(&Tok::Colon) {
            Some(Box::new(self.branch()?))
        } else {
            None
        };
        Ok(Expr::Ternary(Box::new(cond), Box::new(then), otherwise))
    }

    fn binary(&mut self, level: usize) -> Result<Expr, ParseError> {
        const LEVELS: [&[(Tok, BinOp)]; 6] = [
            &[(Tok::OrOr, BinOp::Or)],
            &[(Tok::AndAnd, BinOp::And)],
            &[(Tok::EqEq, BinOp::Eq), (Tok::Ne, BinOp::Ne)],
            &[
                (Tok::Lt, BinOp::Lt),
                (Tok::Le, BinOp::Le),
                (Tok::Gt, BinOp::Gt),
                (Tok::Ge, BinOp::Ge),
            ],
            &[(Tok::Plus, BinOp::Add), (Tok::Minus, BinOp::Sub)],
            &[(Tok::Star, BinOp::Mul), (Tok::Slash, BinOp::Div)],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        'outer: loop {
            for (tok, op) in LEVELS[level] {
                if self.peek() == Some(tok) {
                    self.pos += 1;
                    let right = self.binary(level + 1)?;
                    left = Expr::Bin(*op, Box::new(left), Box::new(right));
                    continue 'outer;
                }
            }
            return Ok(left);
        }
    }

    fn unary(&mut self) -> Result<Expr, ParseError> {
        if self.eat(&Tok::Minus) {
            return Ok(match self.unary()? {
                Expr::Num(n) => Expr::Num(-n),
                e => Expr::Neg(Box::new(e)),
            });
        }
        if self.eat(&Tok::Plus) {
            return self.unary();
        }
        if self.eat(&Tok::Bang) {
            return Ok(Expr::Not(Box::new(self.unary()?)));
        }
        let e = self.primary()?;
        if self.eat(&Tok::Arrow) {
            let rhs = self.primary()?;
            if !matches!(rhs, Expr::Var(Scope::Variable, _) | Expr::Query(..)) {
                return Err(self.error("after -> only variable. and query. are allowed"));
            }
            return Ok(Expr::Arrow(Box::new(e), Box::new(rhs)));
        }
        Ok(e)
    }

    /// `.name.name` after the namespace word.
    fn path(&mut self) -> Result<String, ParseError> {
        let mut out = String::new();
        while self.peek() == Some(&Tok::Dot) {
            let Some(Tok::Ident(part)) = self.peek_at(1) else {
                return Err(self.error("expected a name after ."));
            };
            if !out.is_empty() {
                out.push('.');
            }
            out.push_str(part);
            self.pos += 2;
        }
        if out.is_empty() {
            return Err(self.error("expected .name"));
        }
        Ok(out)
    }

    fn args(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut out = Vec::new();
        self.expect(Tok::LParen, "(")?;
        if self.eat(&Tok::RParen) {
            return Ok(out);
        }
        loop {
            out.push(self.assign()?);
            if self.eat(&Tok::Comma) {
                continue;
            }
            self.expect(Tok::RParen, ")")?;
            return Ok(out);
        }
    }

    fn block(&mut self) -> Result<Expr, ParseError> {
        self.expect(Tok::LBrace, "{")?;
        let (body, _) = self.statements(true)?;
        self.expect(Tok::RBrace, "}")?;
        Ok(Expr::Block(body.into_boxed_slice()))
    }

    fn primary(&mut self) -> Result<Expr, ParseError> {
        let at = self.here();
        let Some(tok) = self.next() else {
            return Err(ParseError {
                at,
                message: "unexpected end".into(),
            });
        };
        match tok {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => Ok(Expr::Str(Arc::from(s.as_str()))),
            Tok::LParen => {
                // Packs assign inside parentheses: `cond ? (v.a = 1)`, and Blockbench writes
                // whole statement lists there: `( v.a = 4; v.b = 2; v.a * v.b; )`, which is
                // worth its last statement.
                let mut list = vec![self.assign()?];
                while self.eat(&Tok::Semi) {
                    if self.peek() == Some(&Tok::RParen) {
                        break;
                    }
                    list.push(self.assign()?);
                }
                self.expect(Tok::RParen, ")")?;
                if list.len() == 1 {
                    Ok(list.pop().unwrap())
                } else {
                    Ok(Expr::Block(list.into_boxed_slice()))
                }
            }
            Tok::LBrace => {
                self.pos -= 1;
                self.block()
            }
            Tok::Ident(word) => self.word(&word, at),
            other => Err(ParseError {
                at,
                message: format!("unexpected {other:?}"),
            }),
        }
    }

    fn word(&mut self, word: &str, at: usize) -> Result<Expr, ParseError> {
        match word {
            "math" => {
                let name = self.path()?;
                let f = MathFn::from_name(&name).ok_or_else(|| ParseError {
                    at,
                    message: format!("unknown math.{name}"),
                })?;
                let args = if self.peek() == Some(&Tok::LParen) {
                    self.args()?
                } else {
                    Vec::new()
                };
                Ok(Expr::Math(f, args.into_boxed_slice()))
            }
            "q" | "query" => {
                let name = self.path()?;
                let args = if self.peek() == Some(&Tok::LParen) {
                    self.args()?
                } else {
                    Vec::new()
                };
                Ok(Expr::Query(
                    Arc::from(name.as_str()),
                    args.into_boxed_slice(),
                ))
            }
            "v" | "variable" => Ok(Expr::Var(Scope::Variable, intern(&self.path()?))),
            "t" | "temp" => Ok(Expr::Var(Scope::Temp, intern(&self.path()?))),
            "c" | "context" => Ok(Expr::Context(Arc::from(self.path()?.as_str()))),
            "geometry" => Ok(Expr::Resource(
                ResKind::Geometry,
                Arc::from(self.path()?.as_str()),
            )),
            "texture" => Ok(Expr::Resource(
                ResKind::Texture,
                Arc::from(self.path()?.as_str()),
            )),
            "material" => Ok(Expr::Resource(
                ResKind::Material,
                Arc::from(self.path()?.as_str()),
            )),
            "array" => {
                let name = self.path()?;
                self.expect(Tok::LBracket, "[")?;
                let index = self.coalesce()?;
                self.expect(Tok::RBracket, "]")?;
                Ok(Expr::Array(Arc::from(name.as_str()), Box::new(index)))
            }
            "this" => Ok(Expr::This),
            "true" => Ok(Expr::Num(1.0)),
            "false" => Ok(Expr::Num(0.0)),
            "return" => Ok(Expr::Return(Box::new(self.assign()?))),
            "break" => Ok(Expr::Break),
            "continue" => Ok(Expr::Continue),
            "loop" => {
                self.expect(Tok::LParen, "(")?;
                let count = self.coalesce()?;
                self.expect(Tok::Comma, ",")?;
                let body = self.block()?;
                self.expect(Tok::RParen, ")")?;
                Ok(Expr::Loop(Box::new(count), Box::new(body)))
            }
            "for_each" => {
                self.expect(Tok::LParen, "(")?;
                let var = self.primary()?;
                let Expr::Var(scope, sym) = var else {
                    return Err(self.error("for_each needs a variable. or temp. first"));
                };
                self.expect(Tok::Comma, ",")?;
                match self.next() {
                    Some(Tok::Ident(a)) if a == "array" => {}
                    _ => return Err(self.error("for_each needs array.name second")),
                }
                let name = self.path()?;
                self.expect(Tok::Comma, ",")?;
                let body = self.block()?;
                self.expect(Tok::RParen, ")")?;
                Ok(Expr::ForEach(
                    scope,
                    sym,
                    Arc::from(name.as_str()),
                    Box::new(body),
                ))
            }
            other => Err(ParseError {
                at,
                message: format!("unknown word {other:?}"),
            }),
        }
    }
}
