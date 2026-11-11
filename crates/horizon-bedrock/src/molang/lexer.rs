//! Source text to tokens. Identifiers are lower-cased: Molang ignores case outside strings.

use super::parser::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Num(f32),
    Str(String),
    Ident(String),
    Dot,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Semi,
    Question,
    Colon,
    Coalesce,
    Plus,
    Minus,
    Star,
    Slash,
    Bang,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Ne,
    AndAnd,
    OrOr,
    Assign,
    Arrow,
}

pub fn lex(src: &str) -> Result<Vec<(Tok, usize)>, ParseError> {
    let b = src.as_bytes();
    let mut out: Vec<(Tok, usize)> = Vec::new();
    let mut i = 0;
    let err = |at: usize, m: String| ParseError { at, message: m };
    while i < b.len() {
        let c = b[i];
        let at = i;
        match c {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'\'' => {
                let start = i + 1;
                let mut j = start;
                while j < b.len() && b[j] != b'\'' {
                    j += 1;
                }
                if j >= b.len() {
                    return Err(err(at, "unterminated string".into()));
                }
                out.push((Tok::Str(src[start..j].to_owned()), at));
                i = j + 1;
            }
            b'0'..=b'9' | b'.' => {
                let after_name = matches!(
                    out.last().map(|t| &t.0),
                    Some(Tok::Ident(_)) | Some(Tok::RParen) | Some(Tok::RBracket)
                );
                if c == b'.' && (after_name || !b.get(i + 1).is_some_and(u8::is_ascii_digit)) {
                    out.push((Tok::Dot, at));
                    i += 1;
                    continue;
                }
                let mut j = i;
                while j < b.len() && b[j].is_ascii_digit() {
                    j += 1;
                }
                if j < b.len() && b[j] == b'.' && b.get(j + 1).is_some_and(u8::is_ascii_digit) {
                    j += 1;
                    while j < b.len() && b[j].is_ascii_digit() {
                        j += 1;
                    }
                } else if j < b.len()
                    && b[j] == b'.'
                    && !b
                        .get(j + 1)
                        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
                {
                    // `3.` is the number 3.
                    j += 1;
                }
                if j < b.len() && (b[j] == b'e' || b[j] == b'E') {
                    let mut k = j + 1;
                    if k < b.len() && (b[k] == b'+' || b[k] == b'-') {
                        k += 1;
                    }
                    if k < b.len() && b[k].is_ascii_digit() {
                        while k < b.len() && b[k].is_ascii_digit() {
                            k += 1;
                        }
                        j = k;
                    }
                }
                let text = &src[i..j];
                let n: f32 = text
                    .trim_end_matches('.')
                    .parse()
                    .map_err(|_| err(at, format!("bad number {text}")))?;
                out.push((Tok::Num(n), at));
                i = j;
                // `1.0f`
                if i < b.len()
                    && (b[i] == b'f' || b[i] == b'F')
                    && !b
                        .get(i + 1)
                        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
                {
                    i += 1;
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let mut j = i;
                while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                    j += 1;
                }
                out.push((Tok::Ident(src[i..j].to_ascii_lowercase()), at));
                i = j;
            }
            _ => {
                let two = |n: u8| b.get(i + 1) == Some(&n);
                let (tok, len) = match c {
                    b'(' => (Tok::LParen, 1),
                    b')' => (Tok::RParen, 1),
                    b'{' => (Tok::LBrace, 1),
                    b'}' => (Tok::RBrace, 1),
                    b'[' => (Tok::LBracket, 1),
                    b']' => (Tok::RBracket, 1),
                    b',' => (Tok::Comma, 1),
                    b';' => (Tok::Semi, 1),
                    b'?' if two(b'?') => (Tok::Coalesce, 2),
                    b'?' => (Tok::Question, 1),
                    b':' => (Tok::Colon, 1),
                    b'+' => (Tok::Plus, 1),
                    b'-' if two(b'>') => (Tok::Arrow, 2),
                    b'-' => (Tok::Minus, 1),
                    b'*' => (Tok::Star, 1),
                    b'/' => (Tok::Slash, 1),
                    b'!' if two(b'=') => (Tok::Ne, 2),
                    b'!' => (Tok::Bang, 1),
                    b'<' if two(b'=') => (Tok::Le, 2),
                    b'<' => (Tok::Lt, 1),
                    b'>' if two(b'=') => (Tok::Ge, 2),
                    b'>' => (Tok::Gt, 1),
                    b'=' if two(b'=') => (Tok::EqEq, 2),
                    b'=' => (Tok::Assign, 1),
                    b'&' if two(b'&') => (Tok::AndAnd, 2),
                    b'|' if two(b'|') => (Tok::OrOr, 2),
                    _ => {
                        let ch = src[i..].chars().next().unwrap_or('?');
                        return Err(err(at, format!("unexpected {ch:?}")));
                    }
                };
                out.push((tok, at));
                i += len;
            }
        }
    }
    Ok(out)
}
