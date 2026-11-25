//! Port of Symfony\Component\ExpressionLanguage\SyntaxError.php, and the
//! error type every fallible call of this crate returns.
//!
//! PHP throws exceptions of several classes here: the component's own
//! `SyntaxError` (a `\LogicException`), the SPL exceptions the nodes throw
//! (`\RuntimeException`, `\LogicException`, ...), and the engine's errors
//! from PHP's operators and functions (`\TypeError`,
//! `\DivisionByZeroError`, ...). One [`Error`] carries its [`Class`];
//! [`Error::is_a`] answers `instanceof` tests.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/expression-language SyntaxError.php

use std::fmt;

use php_value::{PhpString, Thrown, string::levenshtein};

/// The class of an [`Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// `Symfony\Component\ExpressionLanguage\SyntaxError` (extends
    /// `\LogicException`).
    SyntaxError,
    /// `\LogicException`.
    LogicException,
    /// `\RuntimeException`.
    RuntimeException,
    /// `\InvalidArgumentException` (extends `\LogicException`).
    InvalidArgumentException,
    /// `\BadMethodCallException` (extends `\BadFunctionCallException`, a
    /// `\LogicException`).
    BadMethodCallException,
    /// `\Error`.
    Error,
    /// `\TypeError` (extends `\Error`).
    TypeError,
    /// `\ArgumentCountError` (extends `\TypeError`).
    ArgumentCountError,
    /// `\ValueError` (extends `\Error`).
    ValueError,
    /// `\ArithmeticError` (extends `\Error`).
    ArithmeticError,
    /// `\DivisionByZeroError` (extends `\ArithmeticError`).
    DivisionByZeroError,
    /// Not a PHP class: PHP accepts the input, but this port does not
    /// reproduce what PHP does with it (README.md, "Differences").
    Unsupported,
    /// Any other class, as a user function throws it.
    Other(String),
}

impl Class {
    /// The class name, as `$e::class` gives it.
    pub fn name(&self) -> &str {
        match self {
            Self::SyntaxError => "Symfony\\Component\\ExpressionLanguage\\SyntaxError",
            Self::LogicException => "LogicException",
            Self::RuntimeException => "RuntimeException",
            Self::InvalidArgumentException => "InvalidArgumentException",
            Self::BadMethodCallException => "BadMethodCallException",
            Self::Error => "Error",
            Self::TypeError => "TypeError",
            Self::ArgumentCountError => "ArgumentCountError",
            Self::ValueError => "ValueError",
            Self::ArithmeticError => "ArithmeticError",
            Self::DivisionByZeroError => "DivisionByZeroError",
            Self::Unsupported => "(unsupported by this port)",
            Self::Other(class) => class,
        }
    }

    /// The class for a class name (the inverse of [`Class::name`]).
    pub fn from_name(name: &str) -> Self {
        [
            Self::SyntaxError,
            Self::LogicException,
            Self::RuntimeException,
            Self::InvalidArgumentException,
            Self::BadMethodCallException,
            Self::Error,
            Self::TypeError,
            Self::ArgumentCountError,
            Self::ValueError,
            Self::ArithmeticError,
            Self::DivisionByZeroError,
        ]
        .into_iter()
        .find(|class| class.name() == name)
        .unwrap_or_else(|| Self::Other(name.to_owned()))
    }

    /// The parent class, `None` for a root (`\Exception`'s children and
    /// `\Error`).
    fn parent(&self) -> Option<Self> {
        match self {
            Self::SyntaxError | Self::InvalidArgumentException | Self::BadMethodCallException => {
                Some(Self::LogicException)
            }
            Self::TypeError | Self::ValueError | Self::ArithmeticError => Some(Self::Error),
            Self::ArgumentCountError => Some(Self::TypeError),
            Self::DivisionByZeroError => Some(Self::ArithmeticError),
            _ => None,
        }
    }

    /// `instanceof $class`, for the classes above.
    pub fn is_a(&self, class: &Self) -> bool {
        let mut current = Some(self.clone());
        while let Some(c) = current {
            if &c == class {
                return true;
            }
            current = c.parent();
        }
        false
    }
}

/// An exception or engine error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    class: Class,
    message: PhpString,
}

impl Error {
    pub fn new(class: Class, message: impl Into<PhpString>) -> Self {
        Self {
            class,
            message: message.into(),
        }
    }

    /// `new SyntaxError($message, $cursor, $expression)`: the message gets
    /// the position and the expression.
    pub fn syntax(message: impl AsRef<[u8]>, cursor: usize, expression: &[u8]) -> Self {
        Self::syntax_with_proposals(message, cursor, expression, None)
    }

    /// `new SyntaxError($message, $cursor, $expression, $subject,
    /// $proposals)`: as [`Error::syntax`], then " Did you mean "x"?" for
    /// the first proposal at the smallest `levenshtein()` distance, when
    /// it is below 3.
    pub fn syntax_with_proposals(
        message: impl AsRef<[u8]>,
        cursor: usize,
        expression: &[u8],
        proposals: Option<(&[u8], &[PhpString])>,
    ) -> Self {
        let message = message.as_ref();
        let trimmed = rtrim_dots(message);
        let mut out = trimmed.to_vec();
        out.extend_from_slice(format!(" around position {cursor}").as_bytes());
        if !expression.is_empty() && expression != b"0" {
            out.extend_from_slice(b" for expression `");
            out.extend_from_slice(expression);
            out.push(b'`');
        }
        out.push(b'.');
        if let Some((subject, proposals)) = proposals {
            let mut best: Option<(&PhpString, usize)> = None;
            for proposal in proposals {
                let distance = levenshtein(subject, proposal.as_bytes());
                if best.is_none_or(|(_, min)| distance < min) {
                    best = Some((proposal, distance));
                }
            }
            if let Some((guess, distance)) = best
                && distance < 3
            {
                out.extend_from_slice(b" Did you mean \"");
                out.extend_from_slice(guess.as_bytes());
                out.extend_from_slice(b"\"?");
            }
        }
        Self::new(Class::SyntaxError, out)
    }

    pub fn class(&self) -> &Class {
        &self.class
    }

    /// `$e->getMessage()`.
    pub fn message(&self) -> &PhpString {
        &self.message
    }

    /// `$e instanceof $class`.
    pub fn is_a(&self, class: &Class) -> bool {
        self.class.is_a(class)
    }

    pub(crate) fn type_error(message: impl Into<PhpString>) -> Self {
        Self::new(Class::TypeError, message)
    }

    pub(crate) fn runtime(message: impl Into<PhpString>) -> Self {
        Self::new(Class::RuntimeException, message)
    }
}

/// `rtrim($message, '.')`.
fn rtrim_dots(message: &[u8]) -> &[u8] {
    let end = message
        .iter()
        .rposition(|&b| b != b'.')
        .map_or(0, |i| i + 1);
    &message[..end]
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}",
            self.class.name(),
            self.message.to_string_lossy()
        )
    }
}

impl std::error::Error for Error {}

/// An exception a function written with php-value's [`Thrown`] threw.
impl From<Thrown> for Error {
    fn from(thrown: Thrown) -> Self {
        Self::new(Class::from_name(&thrown.class), thrown.message)
    }
}
