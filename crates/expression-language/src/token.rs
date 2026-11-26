//! Port of Symfony\Component\ExpressionLanguage\Token.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language Token.php

use std::fmt;

use php_value::{PhpString, Value, compare::loose_equals};

/// A token type (`Token::*_TYPE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    Eof,
    Name,
    Number,
    String,
    Operator,
    Punctuation,
}

impl TokenType {
    /// The type's string, as upstream's constants hold it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eof => "end of expression",
            Self::Name => "name",
            Self::Number => "number",
            Self::String => "string",
            Self::Operator => "operator",
            Self::Punctuation => "punctuation",
        }
    }

    /// The type for its string.
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::Eof,
            Self::Name,
            Self::Number,
            Self::String,
            Self::Operator,
            Self::Punctuation,
        ]
        .into_iter()
        .find(|t| t.as_str() == name)
    }
}

impl fmt::Display for TokenType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A token: its type, its value (`string|int|float|null`) and its cursor
/// (the byte position in the expression, from 1).
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenType,
    pub value: Value,
    pub cursor: Option<usize>,
}

impl Token {
    pub fn new(kind: TokenType, value: impl Into<Value>, cursor: usize) -> Self {
        Self {
            kind,
            value: value.into(),
            cursor: Some(cursor),
        }
    }

    /// `test($type, $value)`: the type, and the value by `==`.
    pub fn test(&self, kind: TokenType, value: Option<&str>) -> bool {
        self.kind == kind && value.is_none_or(|v| loose_equals(&self.value, &Value::from(v)))
    }

    /// The value as `(string)` gives it (`""` for null).
    pub fn value_string(&self) -> PhpString {
        self.value.to_php_string().unwrap_or_default()
    }

    /// The value's bytes when it is a string (names, operators and
    /// punctuation).
    pub(crate) fn value_bytes(&self) -> &[u8] {
        match &self.value {
            Value::String(s) => s.as_bytes(),
            _ => b"",
        }
    }

    pub(crate) fn cursor(&self) -> usize {
        self.cursor.unwrap_or(0)
    }
}

/// `__toString()`: `sprintf('%3d %-11s %s', ...)`.
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:>3} {:<11} {}",
            self.cursor.unwrap_or(0),
            self.kind.as_str().to_ascii_uppercase(),
            self.value_string().to_string_lossy()
        )
    }
}
