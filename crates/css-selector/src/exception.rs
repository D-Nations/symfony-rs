//! Port of Symfony\Component\CssSelector\Exception\*.php: one error type
//! whose kind is the PHP class, with upstream's messages.
//!
//! Upstream: symfony/css-selector Exception/ExceptionInterface.php Exception/ParseException.php Exception/SyntaxErrorException.php Exception/ExpressionErrorException.php Exception/InternalErrorException.php

use std::fmt;

use crate::parser::Token;
use crate::php::cat;

/// The exception classes. All but [`Kind::TypeError`] and [`Kind::Error`]
/// extend `ParseException` and implement `ExceptionInterface`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `ParseException`.
    Parse,
    /// `SyntaxErrorException`.
    SyntaxError,
    /// `ExpressionErrorException`.
    ExpressionError,
    /// `InternalErrorException`.
    InternalError,
    /// PHP's `\TypeError`, from a `null` where upstream wants a string
    /// (reachable only through the public extension API).
    TypeError,
    /// PHP's `\Error`, from a method call on `null` (likewise).
    Error,
}

impl Kind {
    /// The PHP class name.
    pub fn class(self) -> &'static str {
        match self {
            Self::Parse => "Symfony\\Component\\CssSelector\\Exception\\ParseException",
            Self::SyntaxError => "Symfony\\Component\\CssSelector\\Exception\\SyntaxErrorException",
            Self::ExpressionError => {
                "Symfony\\Component\\CssSelector\\Exception\\ExpressionErrorException"
            }
            Self::InternalError => {
                "Symfony\\Component\\CssSelector\\Exception\\InternalErrorException"
            }
            Self::TypeError => "TypeError",
            Self::Error => "Error",
        }
    }
}

/// A thrown exception: its class and its message (bytes, as PHP's are).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exception {
    kind: Kind,
    message: Vec<u8>,
}

impl Exception {
    pub fn new(kind: Kind, message: impl Into<Vec<u8>>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn message(&self) -> &[u8] {
        &self.message
    }

    /// `$e instanceof ParseException` (and `ExceptionInterface`).
    pub fn is_parse_exception(&self) -> bool {
        !matches!(self.kind, Kind::TypeError | Kind::Error)
    }

    pub(crate) fn syntax(message: impl Into<Vec<u8>>) -> Self {
        Self::new(Kind::SyntaxError, message)
    }

    pub(crate) fn expression(message: impl Into<Vec<u8>>) -> Self {
        Self::new(Kind::ExpressionError, message)
    }

    pub(crate) fn internal(message: impl Into<Vec<u8>>) -> Self {
        Self::new(Kind::InternalError, message)
    }

    // SyntaxErrorException's named constructors.

    /// `SyntaxErrorException::unexpectedToken()`.
    pub fn unexpected_token(expected_value: &[u8], found_token: &Token) -> Self {
        Self::syntax(cat(&[
            b"Expected ",
            expected_value,
            b", but ",
            &found_token.to_php_string(),
            b" found.",
        ]))
    }

    /// `SyntaxErrorException::pseudoElementFound()`.
    pub fn pseudo_element_found(pseudo_element: &[u8], unexpected_location: &[u8]) -> Self {
        Self::syntax(cat(&[
            b"Unexpected pseudo-element \"::",
            pseudo_element,
            b"\" found ",
            unexpected_location,
            b".",
        ]))
    }

    /// `SyntaxErrorException::unclosedString()`.
    pub fn unclosed_string(position: i64) -> Self {
        Self::syntax(format!("Unclosed/invalid string at {position}."))
    }

    /// `SyntaxErrorException::nestedNot()`.
    pub fn nested_not() -> Self {
        Self::syntax("Got nested :not().")
    }

    /// `SyntaxErrorException::nestedHas()`.
    pub fn nested_has() -> Self {
        Self::syntax("Got too deeply nested :has().")
    }

    /// `SyntaxErrorException::notAtTheStartOfASelector()`.
    pub fn not_at_the_start_of_a_selector(pseudo_element: &[u8]) -> Self {
        Self::syntax(cat(&[
            b"Got immediate child pseudo-element \":",
            pseudo_element,
            b"\" not at the start of a selector",
        ]))
    }

    /// `SyntaxErrorException::stringAsFunctionArgument()`.
    pub fn string_as_function_argument() -> Self {
        Self::syntax("String not allowed as function argument.")
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.message))
    }
}

impl std::error::Error for Exception {}
