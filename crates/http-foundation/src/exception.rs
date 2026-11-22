//! Port of Symfony\Component\HttpFoundation\Exception\BadRequestException.php,
//! UnexpectedValueException.php and RequestExceptionInterface.php, with
//! the PHP exceptions the bags throw (`\UnexpectedValueException`,
//! `\InvalidArgumentException`) and PHP's `\Error` for an enum class that
//! has no `from()`.
//!
//! One error type carries the class PHP would throw, so callers can test
//! it as `catch`/`instanceof` would ([`Error::is_a`]). Messages are
//! upstream's, built from its format strings; they are bytes, since an
//! enum `\ValueError` message quotes the value as given.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Exception/BadRequestException.php Exception/UnexpectedValueException.php Exception/RequestExceptionInterface.php

use php_value::PhpString;

/// The exception classes this crate throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `Symfony\Component\HttpFoundation\Exception\BadRequestException`,
    /// a RequestExceptionInterface (HttpKernel answers it with 400).
    BadRequest,
    /// `Symfony\Component\HttpFoundation\Exception\UnexpectedValueException`.
    UnexpectedValue,
    /// PHP's `\UnexpectedValueException`.
    PhpUnexpectedValue,
    /// PHP's `\InvalidArgumentException`.
    PhpInvalidArgument,
    /// PHP's `\Error`.
    PhpError,
    /// Input this port does not reproduce; not a PHP class. It is refused
    /// rather than approximated.
    NotPorted,
}

impl Class {
    /// The class name, as `$e::class` gives it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::BadRequest => {
                "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException"
            }
            Self::UnexpectedValue => {
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException"
            }
            Self::PhpUnexpectedValue => "UnexpectedValueException",
            Self::PhpInvalidArgument => "InvalidArgumentException",
            Self::PhpError => "Error",
            Self::NotPorted => "NotPorted",
        }
    }

    /// The classes and interfaces it is an instance of, itself first.
    const fn ancestry(self) -> &'static [&'static str] {
        match self {
            Self::BadRequest => &[
                "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException",
                "Symfony\\Component\\HttpFoundation\\Exception\\RequestExceptionInterface",
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException",
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::UnexpectedValue => &[
                "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException",
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::PhpUnexpectedValue => &[
                "UnexpectedValueException",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::PhpInvalidArgument => &[
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::PhpError => &["Error", "Throwable"],
            Self::NotPorted => &["NotPorted"],
        }
    }
}

/// An exception thrown by a bag.
#[derive(Debug, Clone, PartialEq)]
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

    pub fn class(&self) -> Class {
        self.class
    }

    /// `getMessage()`.
    pub fn message(&self) -> &PhpString {
        &self.message
    }

    /// `$e instanceof $class`.
    pub fn is_a(&self, class: &str) -> bool {
        self.class.ancestry().contains(&class)
    }

    /// The same message as another class, as `InputBag::getEnum()`
    /// rethrows: `new BadRequestException($e->getMessage(), ...)`.
    pub(crate) fn rethrown_as(self, class: Class) -> Self {
        Self { class, ..self }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            self.class.name(),
            self.message.to_string_lossy()
        )
    }
}

impl std::error::Error for Error {}

impl From<php_value::filter::Unsupported> for Error {
    fn from(error: php_value::filter::Unsupported) -> Self {
        Self::new(Class::NotPorted, error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
