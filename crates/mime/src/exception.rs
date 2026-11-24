//! Port of Symfony\Component\Mime\Exception\ExceptionInterface.php,
//! LogicException.php, InvalidArgumentException.php and
//! RuntimeException.php: the exceptions the MIME type part throws.
//!
//! One error type carries the class PHP would throw, so callers can test
//! it as `catch`/`instanceof` would ([`Error::is_a`]). Messages are
//! upstream's.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/mime Exception/ExceptionInterface.php Exception/LogicException.php Exception/InvalidArgumentException.php Exception/RuntimeException.php

/// The exception classes this crate throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `Symfony\Component\Mime\Exception\LogicException`.
    Logic,
    /// `Symfony\Component\Mime\Exception\InvalidArgumentException`.
    InvalidArgument,
    /// `Symfony\Component\Mime\Exception\RuntimeException`.
    Runtime,
    /// Input this port does not reproduce; not a PHP class. It is refused
    /// rather than approximated.
    NotPorted,
}

impl Class {
    /// The class name, as `$e::class` gives it.
    pub const fn name(self) -> &'static str {
        self.ancestry()[0]
    }

    /// The classes and interfaces it is an instance of, itself first.
    pub const fn ancestry(self) -> &'static [&'static str] {
        match self {
            Self::Logic => &[
                "Symfony\\Component\\Mime\\Exception\\LogicException",
                "Symfony\\Component\\Mime\\Exception\\ExceptionInterface",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::InvalidArgument => &[
                "Symfony\\Component\\Mime\\Exception\\InvalidArgumentException",
                "Symfony\\Component\\Mime\\Exception\\ExceptionInterface",
                "InvalidArgumentException",
                "LogicException",
                "Exception",
                "Throwable",
            ],
            Self::Runtime => &[
                "Symfony\\Component\\Mime\\Exception\\RuntimeException",
                "Symfony\\Component\\Mime\\Exception\\ExceptionInterface",
                "RuntimeException",
                "Exception",
                "Throwable",
            ],
            Self::NotPorted => &["NotPorted"],
        }
    }
}

/// An exception thrown by this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    class: Class,
    message: String,
}

impl Error {
    pub fn new(class: Class, message: impl Into<String>) -> Self {
        Self {
            class,
            message: message.into(),
        }
    }

    pub fn class(&self) -> Class {
        self.class
    }

    /// `getMessage()`.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// `$e instanceof $class`.
    pub fn is_a(&self, class: &str) -> bool {
        self.class.ancestry().contains(&class)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.class.name(), self.message)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
