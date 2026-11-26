//! Port of Symfony\Component\ExpressionLanguage\Lexer.php.
//!
//! Upstream's `preg_match()` patterns are written here as byte scanners
//! that accept the same text: the number pattern
//! (`\.LNUM | LNUM(\.(?!\.)LNUM?)?` then an optional exponent, where
//! `LNUM` is `[0-9]+(_[0-9]+)*`), the quoted strings (a backslash takes
//! the next byte; an unterminated string matches nothing), the comments
//! (`/*` to the first `*/`), the operator alternation that
//! `Resources/bin/generate_operator_regex.php` generates (longest first;
//! a word operator needs the start or `\s`/`(` before it and `\s`/`(`
//! after it) and the names (`[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Lexer.php Resources/bin/generate_operator_regex.php

#![expect(
    clippy::doc_markdown,
    reason = "the Upstream line names files as scripts/sources.sh reads them"
)]

use php_value::{PhpString, Value, numeric::numeric_value};

use crate::{
    php,
    syntax_error::Error,
    token::{Token, TokenType},
    token_stream::TokenStream,
};

/// The operators, as `generate_operator_regex.php` lists them.
pub const OPERATORS: [&str; 34] = [
    "not",
    "!",
    "or",
    "||",
    "xor",
    "&&",
    "and",
    "|",
    "^",
    "&",
    "==",
    "===",
    "!=",
    "!==",
    "<",
    ">",
    ">=",
    "<=",
    "not in",
    "in",
    "..",
    "+",
    "-",
    "~",
    "*",
    "/",
    "%",
    "contains",
    "starts with",
    "ends with",
    "matches",
    "**",
    "<<",
    ">>",
];

/// The operators in the order of the generated alternation: by length,
/// longest first (`arsort()`; operators of one length keep the order
/// PHP's sort gives them, recorded in the generated regex).
const ALTERNATION: [&str; 34] = [
    "starts with",
    "ends with",
    "contains",
    "matches",
    "not in",
    "not",
    "xor",
    "and",
    "===",
    "!==",
    "or",
    "||",
    "&&",
    "==",
    "!=",
    ">=",
    "<=",
    "in",
    "..",
    "**",
    "<<",
    ">>",
    "!",
    "|",
    "^",
    "&",
    "<",
    ">",
    "+",
    "-",
    "~",
    "*",
    "/",
    "%",
];

/// The regex upstream's Lexer.php holds, as the script generates it.
pub fn operator_regex() -> String {
    let mut parts = Vec::new();
    for operator in ALTERNATION {
        let bytes = operator.as_bytes();
        let mut part = String::new();
        if bytes[0].is_ascii_alphabetic() {
            part.push_str(r"(?<=^|[\s(])");
        }
        let quoted = php_pcre::preg_quote(bytes, Some(b'/'));
        part.push_str(&String::from_utf8_lossy(&quoted));
        if bytes[bytes.len() - 1].is_ascii_alphabetic() {
            part.push_str(r"(?=[\s(])");
        }
        parts.push(part);
    }
    format!("/{}/A", parts.join("|"))
}

/// Tokenizes expressions.
#[derive(Debug, Clone, Copy, Default)]
pub struct Lexer;

impl Lexer {
    pub fn new() -> Self {
        Self
    }

    /// `tokenize($expression)`.
    #[expect(clippy::too_many_lines, reason = "one function, as upstream's")]
    pub fn tokenize(&self, expression: impl AsRef<[u8]>) -> Result<TokenStream, Error> {
        let expression: Vec<u8> = expression
            .as_ref()
            .iter()
            .map(|&b| {
                if matches!(b, b'\r' | b'\n' | b'\t' | 0x0b | 0x0c) {
                    b' '
                } else {
                    b
                }
            })
            .collect();
        let e = expression.as_slice();
        let mut cursor = 0;
        let mut tokens = Vec::new();
        let mut brackets: Vec<(u8, usize)> = Vec::new();
        let end = e.len();

        while cursor < end {
            let c = e[cursor];
            if c == b' ' {
                cursor += 1;
                continue;
            }

            if let Some(len) = match_number(e, cursor) {
                // numbers
                let text: Vec<u8> = e[cursor..cursor + len]
                    .iter()
                    .copied()
                    .filter(|&b| b != b'_')
                    .collect();
                let value = match numeric_value(&text) {
                    Some(php_value::Number::Int(i)) => Value::Int(i),
                    Some(php_value::Number::Float(f)) => Value::Float(f),
                    None => Value::Int(0),
                };
                tokens.push(Token::new(TokenType::Number, value, cursor + 1));
                cursor += len;
            } else if b"([{".contains(&c) {
                // opening bracket
                brackets.push((c, cursor));
                tokens.push(Token::new(TokenType::Punctuation, byte_str(c), cursor + 1));
                cursor += 1;
            } else if b")]}".contains(&c) {
                // closing bracket
                let Some((expect, cur)) = brackets.pop() else {
                    return Err(Error::syntax(
                        [b"Unexpected \"".as_slice(), &[c], b"\"."].concat(),
                        cursor,
                        e,
                    ));
                };
                let closing = match expect {
                    b'(' => b')',
                    b'[' => b']',
                    _ => b'}',
                };
                if c != closing {
                    return Err(Error::syntax(
                        [b"Unclosed \"".as_slice(), &[expect], b"\"."].concat(),
                        cur,
                        e,
                    ));
                }
                tokens.push(Token::new(TokenType::Punctuation, byte_str(c), cursor + 1));
                cursor += 1;
            } else if let Some(len) = match_string(e, cursor) {
                // strings
                let value = php::stripcslashes(&e[cursor + 1..cursor + len - 1]);
                tokens.push(Token::new(
                    TokenType::String,
                    Value::String(PhpString::from(value)),
                    cursor + 1,
                ));
                cursor += len;
            } else if let Some(len) = match_comment(e, cursor) {
                // comments
                cursor += len;
            } else if let Some(operator) = match_operator(e, cursor) {
                // operators
                tokens.push(Token::new(TokenType::Operator, operator, cursor + 1));
                cursor += operator.len();
            } else if c == b'?' && e.get(cursor + 1) == Some(&b'.') {
                // null-safe
                cursor += 1;
                tokens.push(Token::new(TokenType::Punctuation, "?.", cursor));
                cursor += 1;
            } else if c == b'?' && e.get(cursor + 1) == Some(&b'?') {
                // null-coalescing
                cursor += 1;
                tokens.push(Token::new(TokenType::Punctuation, "??", cursor));
                cursor += 1;
            } else if b".,?:".contains(&c) {
                // punctuation
                tokens.push(Token::new(TokenType::Punctuation, byte_str(c), cursor + 1));
                cursor += 1;
            } else if let Some(len) = match_name(e, cursor) {
                // names
                tokens.push(Token::new(
                    TokenType::Name,
                    Value::String(PhpString::from(&e[cursor..cursor + len])),
                    cursor + 1,
                ));
                cursor += len;
            } else {
                // unlexable
                return Err(Error::syntax(
                    [b"Unexpected character \"".as_slice(), &[c], b"\"."].concat(),
                    cursor,
                    e,
                ));
            }
        }

        tokens.push(Token {
            kind: TokenType::Eof,
            value: Value::Null,
            cursor: Some(cursor + 1),
        });

        if let Some((expect, cur)) = brackets.pop() {
            return Err(Error::syntax(
                [b"Unclosed \"".as_slice(), &[expect], b"\"."].concat(),
                cur,
                e,
            ));
        }

        Ok(TokenStream::new(tokens, expression))
    }
}

fn byte_str(c: u8) -> Value {
    Value::String(PhpString::from(vec![c]))
}

/// `LNUM` at `i`: its length.
fn lnum(e: &[u8], i: usize) -> Option<usize> {
    let mut j = i;
    let digits = |j: usize| e[j..].iter().take_while(|b| b.is_ascii_digit()).count();
    let first = digits(j);
    if first == 0 {
        return None;
    }
    j += first;
    while e.get(j) == Some(&b'_') {
        let more = digits(j + 1);
        if more == 0 {
            break;
        }
        j += 1 + more;
    }
    Some(j - i)
}

/// The number pattern at `i`: its length.
fn match_number(e: &[u8], i: usize) -> Option<usize> {
    let mut j = if e.get(i) == Some(&b'.') {
        i + 1 + lnum(e, i + 1)?
    } else {
        let mut j = i + lnum(e, i)?;
        if e.get(j) == Some(&b'.') && e.get(j + 1) != Some(&b'.') {
            j += 1;
            j += lnum(e, j).unwrap_or(0);
        }
        j
    };
    if matches!(e.get(j), Some(b'e' | b'E')) {
        let mut k = j + 1;
        if matches!(e.get(k), Some(b'+' | b'-')) {
            k += 1;
        }
        if let Some(len) = lnum(e, k) {
            j = k + len;
        }
    }
    Some(j - i)
}

/// A double- or single-quoted string at `i`: its length, quotes
/// included.
fn match_string(e: &[u8], i: usize) -> Option<usize> {
    let quote = *e.get(i)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let mut j = i + 1;
    while j < e.len() {
        match e[j] {
            b'\\' => {
                if j + 1 >= e.len() {
                    return None;
                }
                j += 2;
            }
            b if b == quote => return Some(j + 1 - i),
            _ => j += 1,
        }
    }
    None
}

/// A comment at `i`: its length.
fn match_comment(e: &[u8], i: usize) -> Option<usize> {
    if !e[i..].starts_with(b"/*") {
        return None;
    }
    let rest = &e[i + 2..];
    let close = rest.windows(2).position(|w| w == b"*/")?;
    Some(2 + close + 2)
}

/// PCRE's `\s` without `/u`.
fn is_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// The operator alternation at `i`.
fn match_operator(e: &[u8], i: usize) -> Option<&'static str> {
    ALTERNATION.into_iter().find(|operator| {
        let bytes = operator.as_bytes();
        if !e[i..].starts_with(bytes) {
            return false;
        }
        if bytes[0].is_ascii_alphabetic() && i > 0 && !(is_space(e[i - 1]) || e[i - 1] == b'(') {
            return false;
        }
        if bytes[bytes.len() - 1].is_ascii_alphabetic() {
            match e.get(i + bytes.len()) {
                Some(&b) if is_space(b) || b == b'(' => {}
                _ => return false,
            }
        }
        true
    })
}

fn is_name_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f
}

/// A name at `i`: its length.
pub(crate) fn match_name(e: &[u8], i: usize) -> Option<usize> {
    if !is_name_start(*e.get(i)?) {
        return None;
    }
    let len = 1 + e[i + 1..]
        .iter()
        .take_while(|&&b| is_name_start(b) || b.is_ascii_digit())
        .count();
    Some(len)
}
