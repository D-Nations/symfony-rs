//! Port of Symfony\Component\PasswordHasher\Exception\*.php: the
//! component's exception classes and their marker interface, and the PHP
//! exceptions the hashers let through.
//!
//! PHP's exceptions form a class hierarchy that callers test with
//! `instanceof`; here one [`Error`] carries its [`Class`], and the
//! `is_*` methods answer those tests.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/password-hasher Exception/ExceptionInterface.php Exception/InvalidPasswordException.php Exception/LogicException.php

use std::fmt;

/// The class of an [`Error`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Class {
    /// `Exception\InvalidPasswordException` (extends `\RuntimeException`).
    InvalidPassword,
    /// `Exception\LogicException` (extends `\LogicException`).
    Logic,
    /// `\InvalidArgumentException` (extends `\LogicException`).
    InvalidArgument,
    /// `\RuntimeException`.
    Runtime,
    /// `\ValueError`, from a PHP function (an `\Error`).
    ValueError,
    /// `\TypeError`, from a PHP argument or operand (an `\Error`).
    TypeError,
    /// `\ArgumentCountError` (extends `\TypeError`).
    ArgumentCountError,
    /// `\Error`, e.g. `Class "X" not found`.
    Error,
    /// `\SodiumException` (extends `\Exception`).
    SodiumException,
}

impl Class {
    /// The class name, as `$e::class` gives it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::InvalidPassword => {
                "Symfony\\Component\\PasswordHasher\\Exception\\InvalidPasswordException"
            }
            Self::Logic => "Symfony\\Component\\PasswordHasher\\Exception\\LogicException",
            Self::InvalidArgument => "InvalidArgumentException",
            Self::Runtime => "RuntimeException",
            Self::ValueError => "ValueError",
            Self::TypeError => "TypeError",
            Self::ArgumentCountError => "ArgumentCountError",
            Self::Error => "Error",
            Self::SodiumException => "SodiumException",
        }
    }

    /// `instanceof Exception\ExceptionInterface`.
    pub fn is_exception_interface(&self) -> bool {
        matches!(self, Self::InvalidPassword | Self::Logic)
    }

    /// `instanceof \LogicException`.
    pub fn is_logic(&self) -> bool {
        matches!(self, Self::Logic | Self::InvalidArgument)
    }

    /// `instanceof \RuntimeException`.
    pub fn is_runtime(&self) -> bool {
        matches!(self, Self::InvalidPassword | Self::Runtime)
    }

    /// `instanceof \Exception`, as opposed to an `\Error`.
    pub fn is_exception(&self) -> bool {
        self.is_logic() || self.is_runtime() || matches!(self, Self::SodiumException)
    }
}

/// An exception: its class and message.
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

    /// `new InvalidPasswordException()`: its default message.
    pub fn invalid_password() -> Self {
        Self::new(Class::InvalidPassword, "Invalid password.")
    }

    /// `new Exception\LogicException($message)`.
    pub fn logic(message: impl Into<String>) -> Self {
        Self::new(Class::Logic, message)
    }

    /// `new \InvalidArgumentException($message)`.
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(Class::InvalidArgument, message)
    }

    /// `new \RuntimeException($message)`.
    pub fn runtime(message: impl Into<String>) -> Self {
        Self::new(Class::Runtime, message)
    }

    pub fn class(&self) -> &Class {
        &self.class
    }

    /// `getMessage()`.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// `instanceof Exception\ExceptionInterface`.
    pub fn is_exception_interface(&self) -> bool {
        self.class.is_exception_interface()
    }

    /// `instanceof \LogicException`.
    pub fn is_logic(&self) -> bool {
        self.class.is_logic()
    }

    /// `instanceof \RuntimeException`.
    pub fn is_runtime(&self) -> bool {
        self.class.is_runtime()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class.name(), self.message)
    }
}

impl std::error::Error for Error {}
