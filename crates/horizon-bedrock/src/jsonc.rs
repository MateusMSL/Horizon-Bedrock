//! Bedrock's JSON: `//` and `/* */` comments, trailing commas and a byte-order mark are all
//! accepted by the game. [`strip`] turns such text into plain JSON; [`parse`] reads it.

use crate::{Error, Result};
use serde_json::Value;

/// `text` as plain JSON: comments and trailing commas removed, string contents untouched.
pub fn strip(text: &str) -> String {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'"' => {
                // A string: copy through its closing quote, honouring escapes.
                out.push(b);
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    out.push(c);
                    i += 1;
                    if c == b'\\' && i < bytes.len() {
                        out.push(bytes[i]);
                        i += 1;
                    } else if c == b'"' {
                        break;
                    }
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i = (i + 2).min(bytes.len());
                out.push(b' ');
            }
            b',' => {
                // A comma before `}` or `]` (past whitespace and comments) is dropped.
                let mut j = i + 1;
                loop {
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if bytes.get(j) == Some(&b'/') && bytes.get(j + 1) == Some(&b'/') {
                        while j < bytes.len() && bytes[j] != b'\n' {
                            j += 1;
                        }
                    } else if bytes.get(j) == Some(&b'/') && bytes.get(j + 1) == Some(&b'*') {
                        j += 2;
                        while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                            j += 1;
                        }
                        j = (j + 2).min(bytes.len());
                    } else {
                        break;
                    }
                }
                if !matches!(bytes.get(j), Some(b'}') | Some(b']')) {
                    out.push(b);
                }
                i += 1;
            }
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }
    // Only ASCII bytes were removed, so the rest is still valid UTF-8.
    String::from_utf8(out).unwrap_or_default()
}

/// Parses Bedrock JSON.
pub fn parse(text: &str) -> Result<Value> {
    serde_json::from_str(&strip(text)).map_err(|e| Error::Json(e.to_string()))
}

/// Parses Bedrock JSON from bytes (UTF-8, with or without a byte-order mark).
pub fn parse_bytes(bytes: &[u8]) -> Result<Value> {
    let text = std::str::from_utf8(bytes).map_err(|e| Error::Json(e.to_string()))?;
    parse(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_trailing_commas() {
        let v = parse("{\n // one\n \"a\": 1, /* two */\n \"b\": [1, 2, ], }").unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"][1], 2);
    }

    #[test]
    fn strings_keep_comment_markers_and_commas() {
        let v = parse(r#"{"u": "http://x//y, ]", "q": "a\"//b"}"#).unwrap();
        assert_eq!(v["u"], "http://x//y, ]");
        assert_eq!(v["q"], "a\"//b");
    }

    #[test]
    fn byte_order_mark() {
        assert_eq!(parse("\u{feff}{\"a\":2}").unwrap()["a"], 2);
    }

    #[test]
    fn comment_between_comma_and_brace() {
        assert_eq!(parse("{\"a\":1, // x\n}").unwrap()["a"], 1);
    }
}
