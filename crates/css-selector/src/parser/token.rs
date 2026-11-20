//! Port of Symfony\Component\CssSelector\Parser\Token.php.
//!
//! Upstream: symfony/css-selector Parser/Token.php

use crate::php::{cat, truthy_opt};

/// `Token::TYPE_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    FileEnd,
    Delimiter,
    Whitespace,
    Identifier,
    Hash,
    Number,
    String,
}

impl TokenType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileEnd => "eof",
            Self::Delimiter => "delimiter",
            Self::Whitespace => "whitespace",
            Self::Identifier => "identifier",
            Self::Hash => "hash",
            Self::Number => "number",
            Self::String => "string",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        [
            Self::FileEnd,
            Self::Delimiter,
            Self::Whitespace,
            Self::Identifier,
            Self::Hash,
            Self::Number,
            Self::String,
        ]
        .into_iter()
        .find(|t| t.as_str() == s)
    }
}

/// A token: type, value and position, each nullable as upstream's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    type_: Option<TokenType>,
    value: Option<Vec<u8>>,
    position: Option<usize>,
}

impl Token {
    pub fn new(type_: Option<TokenType>, value: Option<&[u8]>, position: Option<usize>) -> Self {
        Self {
            type_,
            value: value.map(<[u8]>::to_vec),
            position,
        }
    }

    pub(crate) fn of(type_: TokenType, value: &[u8], position: usize) -> Self {
        Self::new(Some(type_), Some(value), Some(position))
    }

    pub fn get_type(&self) -> Option<TokenType> {
        self.type_
    }

    pub fn get_value(&self) -> Option<&[u8]> {
        self.value.as_deref()
    }

    pub fn get_position(&self) -> Option<usize> {
        self.position
    }

    pub fn is_file_end(&self) -> bool {
        self.type_ == Some(TokenType::FileEnd)
    }

    /// A delimiter, and one of `values` unless it is empty.
    pub fn is_delimiter(&self, values: &[&[u8]]) -> bool {
        if self.type_ != Some(TokenType::Delimiter) {
            return false;
        }
        if values.is_empty() {
            return true;
        }
        self.value.as_deref().is_some_and(|v| values.contains(&v))
    }

    pub fn is_whitespace(&self) -> bool {
        self.type_ == Some(TokenType::Whitespace)
    }

    pub fn is_identifier(&self) -> bool {
        self.type_ == Some(TokenType::Identifier)
    }

    pub fn is_hash(&self) -> bool {
        self.type_ == Some(TokenType::Hash)
    }

    pub fn is_number(&self) -> bool {
        self.type_ == Some(TokenType::Number)
    }

    pub fn is_string(&self) -> bool {
        self.type_ == Some(TokenType::String)
    }

    /// `__toString()`: `<type "value" at position>`, without the value
    /// when it is false (`""`, `"0"`).
    pub fn to_php_string(&self) -> Vec<u8> {
        let type_ = self.type_.map_or("", TokenType::as_str).as_bytes();
        let position = self.position.map(|p| p.to_string()).unwrap_or_default();
        if truthy_opt(self.value.as_deref()) {
            cat(&[
                b"<",
                type_,
                b" \"",
                self.value.as_deref().unwrap_or_default(),
                b"\" at ",
                position.as_bytes(),
                b">",
            ])
        } else {
            cat(&[b"<", type_, b" at ", position.as_bytes(), b">"])
        }
    }
}
