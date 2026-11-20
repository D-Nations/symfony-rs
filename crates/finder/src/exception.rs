//! Port of Symfony\Component\Finder\Exception\AccessDeniedException.php and
//! DirectoryNotFoundException.php, with the PHP exceptions the component
//! throws, as one error type.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Exception/AccessDeniedException.php Exception/DirectoryNotFoundException.php

use std::fmt;

use crate::pcre::PcreError;

/// What Finder throws, by PHP exception class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// `\InvalidArgumentException`.
    InvalidArgument(String),
    /// `DirectoryNotFoundException` (an `\InvalidArgumentException`).
    DirectoryNotFound(String),
    /// `AccessDeniedException` (an `\UnexpectedValueException`).
    AccessDenied(String),
    /// `\UnexpectedValueException`.
    UnexpectedValue(String),
    /// `\LogicException`.
    Logic(String),
    /// `\OutOfRangeException`.
    OutOfRange(String),
    /// `\RuntimeException`.
    Runtime(String),
    /// A pattern PHP accepts that this port does not translate (pcre.rs).
    UnsupportedPattern(String),
}

impl Error {
    /// The PHP exception class.
    pub fn class(&self) -> &'static str {
        match self {
            Self::InvalidArgument(_) => "InvalidArgumentException",
            Self::DirectoryNotFound(_) => {
                r"Symfony\Component\Finder\Exception\DirectoryNotFoundException"
            }
            Self::AccessDenied(_) => r"Symfony\Component\Finder\Exception\AccessDeniedException",
            Self::UnexpectedValue(_) => "UnexpectedValueException",
            Self::Logic(_) => "LogicException",
            Self::OutOfRange(_) => "OutOfRangeException",
            Self::Runtime(_) => "RuntimeException",
            Self::UnsupportedPattern(_) => "symfony_finder::UnsupportedPattern",
        }
    }

    /// `instanceof $class`, for the classes Finder throws.
    pub fn is_instance_of(&self, class: &str) -> bool {
        let class = class.trim_start_matches('\\');
        class == self.class()
            || match self {
                Self::DirectoryNotFound(_) => class == "InvalidArgumentException",
                Self::AccessDenied(_) => class == "UnexpectedValueException",
                _ => false,
            }
            || class == "Exception" && !matches!(self, Self::UnsupportedPattern(_))
    }

    /// `getMessage()`.
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidArgument(m)
            | Self::DirectoryNotFound(m)
            | Self::AccessDenied(m)
            | Self::UnexpectedValue(m)
            | Self::Logic(m)
            | Self::OutOfRange(m)
            | Self::Runtime(m)
            | Self::UnsupportedPattern(m) => m,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class(), self.message())
    }
}

impl std::error::Error for Error {}

impl From<PcreError> for Error {
    fn from(e: PcreError) -> Self {
        Self::UnsupportedPattern(e.to_string())
    }
}
