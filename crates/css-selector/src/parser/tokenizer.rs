//! Port of Symfony\Component\CssSelector\Parser\Tokenizer\*.php: the
//! patterns (PCRE without `/u`, so byte-wise and ASCII-only case folding;
//! `regex::bytes` with `(?-u)` here), the escape decoding and the tokenizer.
//!
//! Upstream: symfony/css-selector Parser/Tokenizer/Tokenizer.php Parser/Tokenizer/TokenizerEscaping.php Parser/Tokenizer/TokenizerPatterns.php

use std::sync::OnceLock;

use php_value::numeric::{Number, hexdec};
use regex::bytes::{Captures, Regex};

use super::handler::{
    CommentHandler, HandlerInterface, HashHandler, IdentifierHandler, NumberHandler, StringHandler,
    WhitespaceHandler,
};
use super::{Reader, Token, TokenStream, TokenType};
use crate::exception::Exception;

const UNICODE_ESCAPE: &str = r"\\([0-9a-f]{1,6})(?:\r\n|[ \n\r\t\f])?";
const SIMPLE_ESCAPE: &str = r"\\(.)";
const NEW_LINE_ESCAPE: &str = r"\\(?:\n|\r\n|\r|\f)";
const NON_ASCII: &str = r"[^\x00-\x7F]";
const NUMBER: &str = r"[+-]?(?:[0-9]*\.[0-9]+|[0-9]+)";

fn escape() -> String {
    format!(r"{UNICODE_ESCAPE}|\\[^\n\r\f0-9a-f]")
}

fn nm_char() -> String {
    format!("[_a-z0-9-]|{}|{NON_ASCII}", escape())
}

fn nm_start() -> String {
    format!("[_a-z]|{}|{NON_ASCII}", escape())
}

/// Compiles one of the constant patterns above.
fn compile(pattern: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(e) => panic!("invalid built-in pattern {pattern}: {e}"),
    }
}

struct Patterns {
    unicode_escape: Regex,
    simple_escape: Regex,
    new_line_escape: Regex,
    identifier: Regex,
    hash: Regex,
    number: Regex,
    quoted_single: Regex,
    quoted_double: Regex,
    whitespace: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        let quoted = |quote: char| {
            compile(&format!(
                r"(?i-u)^([^\n\r\f\\{quote}]|{NEW_LINE_ESCAPE}|{})*",
                escape()
            ))
        };
        Patterns {
            unicode_escape: compile(&format!("(?i-u){UNICODE_ESCAPE}")),
            simple_escape: compile(&format!("(?-u){SIMPLE_ESCAPE}")),
            new_line_escape: compile(&format!("(?-u){NEW_LINE_ESCAPE}")),
            identifier: compile(&format!("(?i-u)^-?(?:{})(?:{})*", nm_start(), nm_char())),
            hash: compile(&format!("(?i-u)^#((?:{})+)", nm_char())),
            number: compile(&format!("(?-u)^{NUMBER}")),
            quoted_single: quoted('\''),
            quoted_double: quoted('"'),
            whitespace: compile(r"(?-u)^[ \t\r\n\f]+"),
        }
    })
}

/// `TokenizerPatterns`: the compiled patterns, anchored at the reader's
/// position where upstream's start with `^`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenizerPatterns;

impl TokenizerPatterns {
    pub fn new() -> Self {
        Self
    }

    pub fn get_new_line_escape_pattern(&self) -> &'static Regex {
        &patterns().new_line_escape
    }

    pub fn get_simple_escape_pattern(&self) -> &'static Regex {
        &patterns().simple_escape
    }

    pub fn get_unicode_escape_pattern(&self) -> &'static Regex {
        &patterns().unicode_escape
    }

    pub fn get_identifier_pattern(&self) -> &'static Regex {
        &patterns().identifier
    }

    pub fn get_hash_pattern(&self) -> &'static Regex {
        &patterns().hash
    }

    pub fn get_number_pattern(&self) -> &'static Regex {
        &patterns().number
    }

    /// For `'` or `"`; `None` for anything else (upstream only asks for
    /// those two).
    pub fn get_quoted_string_pattern(&self, quote: &[u8]) -> Option<&'static Regex> {
        match quote {
            b"'" => Some(&patterns().quoted_single),
            b"\"" => Some(&patterns().quoted_double),
            _ => None,
        }
    }

    pub(crate) fn get_whitespace_pattern(&self) -> &'static Regex {
        &patterns().whitespace
    }
}

/// `TokenizerEscaping`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokenizerEscaping {
    patterns: TokenizerPatterns,
}

impl TokenizerEscaping {
    pub fn new(patterns: TokenizerPatterns) -> Self {
        Self { patterns }
    }

    pub fn escape_unicode(&self, value: &[u8]) -> Vec<u8> {
        let value = self.replace_unicode_sequences(value);
        self.patterns
            .get_simple_escape_pattern()
            .replace_all(&value, &b"$1"[..])
            .into_owned()
    }

    pub fn escape_unicode_and_new_line(&self, value: &[u8]) -> Vec<u8> {
        let value = self
            .patterns
            .get_new_line_escape_pattern()
            .replace_all(value, &b""[..]);
        self.escape_unicode(&value)
    }

    /// `\XXXXXX` escapes to UTF-8, code point taken modulo 0x200000;
    /// from U+10000 on they are dropped, and surrogates are encoded as
    /// they come (upstream's own encoder).
    fn replace_unicode_sequences(&self, value: &[u8]) -> Vec<u8> {
        self.patterns
            .get_unicode_escape_pattern()
            .replace_all(value, |caps: &Captures<'_>| {
                let digits = caps.get(1).map_or(&b""[..], |m| m.as_bytes());
                // At most six hex digits: always an int.
                let c = match hexdec(digits) {
                    Number::Int(i) => i,
                    Number::Float(f) => f as i64,
                } % 0x0020_0000;
                // `c` is below 0x200000, so the casts below keep its bits.
                if c < 0x80 {
                    vec![c as u8]
                } else if c < 0x800 {
                    vec![0xC0 | (c >> 6) as u8, 0x80 | (c & 0x3F) as u8]
                } else if c < 0x10000 {
                    vec![
                        0xE0 | (c >> 12) as u8,
                        0x80 | ((c >> 6) & 0x3F) as u8,
                        0x80 | (c & 0x3F) as u8,
                    ]
                } else {
                    Vec::new()
                }
            })
            .into_owned()
    }
}

/// `Tokenizer`.
pub struct Tokenizer {
    handlers: Vec<Box<dyn HandlerInterface>>,
}

impl Default for Tokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Tokenizer {
    pub fn new() -> Self {
        let patterns = TokenizerPatterns::new();
        let escaping = TokenizerEscaping::new(patterns);
        Self {
            handlers: vec![
                Box::new(WhitespaceHandler),
                Box::new(IdentifierHandler::new(patterns, escaping)),
                Box::new(HashHandler::new(patterns, escaping)),
                Box::new(StringHandler::new(patterns, escaping)),
                Box::new(NumberHandler::new(patterns)),
                Box::new(CommentHandler),
            ],
        }
    }

    pub fn tokenize(&self, reader: &mut Reader) -> Result<TokenStream, Exception> {
        let mut stream = TokenStream::new();
        'read: while !reader.is_eof() {
            for handler in &self.handlers {
                if handler.handle(reader, &mut stream)? {
                    continue 'read;
                }
            }
            let delimiter = reader.get_substring(1, 0).to_vec();
            stream.push(Token::of(
                TokenType::Delimiter,
                &delimiter,
                reader.get_position(),
            ));
            reader.move_forward(1);
        }
        stream
            .push(Token::new(
                Some(TokenType::FileEnd),
                None,
                Some(reader.get_position()),
            ))
            .freeze();
        Ok(stream)
    }
}
