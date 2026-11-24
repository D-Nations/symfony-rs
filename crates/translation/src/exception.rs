//! Port of Symfony\Component\Translation\Exception's classes that the
//! ported files throw: InvalidResourceException, NotFoundResourceException,
//! InvalidArgumentException, LogicException and RuntimeException, as
//! classes of the crate's one [`Error`]. PHP's own errors that the loader
//! lets through (`ValueError`, `TypeError`) are classes too, and so is
//! symfony/config's exception a loader wraps (the previous exception).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/translation Exception/ExceptionInterface.php Exception/InvalidResourceException.php Exception/NotFoundResourceException.php Exception/InvalidArgumentException.php Exception/LogicException.php Exception/RuntimeException.php

use std::fmt;

/// The class of an [`Error`].
#[derive(Clone, Debug, PartialEq)]
pub enum Class {
    /// `Exception\InvalidResourceException` (extends
    /// `\InvalidArgumentException`).
    InvalidResource,
    /// `Exception\NotFoundResourceException` (extends
    /// `\InvalidArgumentException`).
    NotFoundResource,
    /// `Exception\InvalidArgumentException` (extends
    /// `\InvalidArgumentException`).
    InvalidArgument,
    /// PHP native \InvalidArgumentException (invalid catalogue batch).
    NativeInvalidArgument,
    /// `Exception\LogicException` (extends `\LogicException`).
    Logic,
    /// `Exception\RuntimeException` (extends `\RuntimeException`).
    Runtime,
    /// `\ValueError`, an `\Error` (`mb_convert_encoding()` given an
    /// encoding it does not know).
    ValueError,
    /// `\TypeError`, an `\Error`.
    TypeError,
    /// `\Error` itself (a method called on `null`).
    PhpError,
    /// A resource this port refuses instead of reading it the way PHP
    /// would (a `phar://` archive): no PHP class, stated in README.md.
    Refused,
    /// An exception of symfony/config that upstream lets through
    /// unwrapped (Filesystem's IOException when a file cannot be read).
    Config(Box<symfony_config::Error>),
}

impl Class {
    /// The class name, as `$e::class` gives it.
    pub fn name(&self) -> &str {
        match self {
            Self::InvalidResource => {
                "Symfony\\Component\\Translation\\Exception\\InvalidResourceException"
            }
            Self::NotFoundResource => {
                "Symfony\\Component\\Translation\\Exception\\NotFoundResourceException"
            }
            Self::InvalidArgument => {
                "Symfony\\Component\\Translation\\Exception\\InvalidArgumentException"
            }
            Self::NativeInvalidArgument => "InvalidArgumentException",
            Self::Logic => "Symfony\\Component\\Translation\\Exception\\LogicException",
            Self::Runtime => "Symfony\\Component\\Translation\\Exception\\RuntimeException",
            Self::ValueError => "ValueError",
            Self::TypeError => "TypeError",
            Self::PhpError => "Error",
            Self::Refused => "(refused)",
            Self::Config(e) => e.class().name(),
        }
    }

    /// `instanceof \InvalidArgumentException`.
    pub fn is_invalid_argument(&self) -> bool {
        matches!(
            self,
            Self::InvalidResource
                | Self::NotFoundResource
                | Self::InvalidArgument
                | Self::NativeInvalidArgument
        )
    }

    /// `instanceof ExceptionInterface`.
    pub fn is_translation_exception(&self) -> bool {
        matches!(
            self,
            Self::InvalidResource
                | Self::NotFoundResource
                | Self::InvalidArgument
                | Self::Logic
                | Self::Runtime
        )
    }
}

/// The exception an [`Error`] wraps (`getPrevious()`).
#[derive(Clone, Debug, PartialEq)]
pub enum Previous {
    Translation(Box<Error>),
    /// symfony/config's XmlUtils exceptions, which the XLIFF loader wraps.
    Config(symfony_config::Error),
}

/// A thrown exception or error.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    class: Class,
    message: Vec<u8>,
    code: i64,
    previous: Option<Box<Previous>>,
}

impl Error {
    pub fn new(class: Class, message: impl Into<Vec<u8>>) -> Self {
        Self {
            class,
            message: message.into(),
            code: 0,
            previous: None,
        }
    }

    /// `new $class($message, $code, $previous)`.
    pub fn with_previous(
        class: Class,
        message: impl Into<Vec<u8>>,
        code: i64,
        previous: Previous,
    ) -> Self {
        Self {
            class,
            message: message.into(),
            code,
            previous: Some(Box::new(previous)),
        }
    }

    pub fn class(&self) -> &Class {
        &self.class
    }

    pub fn get_message(&self) -> &[u8] {
        &self.message
    }

    pub fn get_code(&self) -> i64 {
        self.code
    }

    pub fn get_previous(&self) -> Option<&Previous> {
        self.previous.as_deref()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}",
            self.class.name(),
            String::from_utf8_lossy(&self.message)
        )
    }
}

impl std::error::Error for Error {}
