//! `texts/<language>.lang`: `key=value` lines, `##` comment lines, and a `\t#` comment after a
//! value.

use std::collections::BTreeMap;

/// The pairs in `text`. A later line wins over an earlier one with the same key.
pub fn parse(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim_end_matches('\r');
        if line.trim_start().starts_with("##") {
            continue;
        }
        let Some((key, rest)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let value = match rest.find("\t#") {
            Some(at) => &rest[..at],
            None => rest,
        };
        out.insert(key.to_owned(), value.trim_end().to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairs_comments_and_tabs() {
        let m = parse(
            "## header\npack.name=Actions & Stuff §gby\n\nplayer.joined=§7[§a+§7] §a%s\t#\r\nbad line\na=1\na=2\n",
        );
        assert_eq!(m["pack.name"], "Actions & Stuff §gby");
        assert_eq!(m["player.joined"], "§7[§a+§7] §a%s");
        assert_eq!(m["a"], "2");
        assert_eq!(m.len(), 3);
    }

    #[test]
    fn equals_inside_a_value() {
        assert_eq!(parse("k=a=b")["k"], "a=b");
    }
}
