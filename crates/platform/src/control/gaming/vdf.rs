//! Valve KeyValues (text VDF) parser: `libraryfolders.vdf`, `appmanifest_*.acf`.
//!
//! Grammar: `"key" "value"` pairs and `"key" { ... }` blocks, `//` line comments, optional
//! `[$CONDITION]` tags after a token (ignored), unquoted tokens, escapes `\\ \" \n \t` in
//! quoted strings. Keys are matched case-insensitively (Steam writes `LastPlayed` and older
//! clients `lastplayed`).

/// A parsed value: a string or a block of ordered pairs (keys may repeat).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VdfValue {
    Str(String),
    Block(Vec<(String, VdfValue)>),
}

impl VdfValue {
    /// First child named `key` (case-insensitive).
    pub fn get(&self, key: &str) -> Option<&VdfValue> {
        match self {
            Self::Block(pairs) => pairs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(key))
                .map(|(_, v)| v),
            Self::Str(_) => None,
        }
    }

    /// String child named `key`.
    pub fn str(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Self::Str(s) => Some(s),
            Self::Block(_) => None,
        }
    }

    pub fn pairs(&self) -> &[(String, VdfValue)] {
        match self {
            Self::Block(pairs) => pairs,
            Self::Str(_) => &[],
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Text(String),
    Open,
    Close,
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            c if c.is_whitespace() => {
                chars.next();
            }
            '/' => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
            }
            '{' => {
                chars.next();
                out.push(Token::Open);
            }
            '}' => {
                chars.next();
                out.push(Token::Close);
            }
            '[' => {
                // Conditional tag such as `[$WIN32]`: skipped.
                for c in chars.by_ref() {
                    if c == ']' {
                        break;
                    }
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => break,
                        },
                        other => s.push(other),
                    }
                }
                out.push(Token::Text(s));
            }
            _ => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, '{' | '}' | '"') {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                out.push(Token::Text(s));
            }
        }
    }
    out
}

fn parse_block(tokens: &[Token], pos: &mut usize) -> Vec<(String, VdfValue)> {
    let mut pairs = Vec::new();
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Close => {
                *pos += 1;
                return pairs;
            }
            Token::Open => {
                // A block without a key: tolerate by parsing and dropping it.
                *pos += 1;
                parse_block(tokens, pos);
            }
            Token::Text(key) => {
                let key = key.clone();
                *pos += 1;
                match tokens.get(*pos) {
                    Some(Token::Text(value)) => {
                        pairs.push((key, VdfValue::Str(value.clone())));
                        *pos += 1;
                    }
                    Some(Token::Open) => {
                        *pos += 1;
                        let block = parse_block(tokens, pos);
                        pairs.push((key, VdfValue::Block(block)));
                    }
                    Some(Token::Close) | None => {}
                }
            }
        }
    }
    pairs
}

/// Parses a whole document as the root block. Malformed input yields what could be read.
pub fn parse(input: &str) -> VdfValue {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let tokens = tokenize(input);
    let mut pos = 0;
    VdfValue::Block(parse_block(&tokens, &mut pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_blocks_comments_escapes_and_conditions() {
        let doc = parse(
            "// comment\n\"Root\"\n{\n\t\"a\"\t\t\"1\"\n\t\"Path\"\t\"C:\\\\Games\\\\Steam\"\n\tkey [$WIN32] \"x\\\"y\"\n\t\"inner\" { \"b\" \"2\" }\n}\n",
        );
        let root = doc.get("root").expect("root");
        assert_eq!(root.str("A"), Some("1"));
        assert_eq!(root.str("path"), Some("C:\\Games\\Steam"));
        assert_eq!(root.str("key"), Some("x\"y"));
        assert_eq!(root.get("inner").and_then(|b| b.str("b")), Some("2"));
    }

    #[test]
    fn truncated_input_does_not_panic() {
        let doc = parse("\"a\" { \"b\" \"c\" \"d\"");
        assert_eq!(doc.get("a").and_then(|a| a.str("b")), Some("c"));
    }
}
