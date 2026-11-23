//! Port of Symfony\Component\Filesystem\Exception\IOException.php,
//! InvalidArgumentException.php and RuntimeException.php (with their
//! interfaces): one error type, its class a [`Class`].
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/filesystem Exception/IOException.php Exception/InvalidArgumentException.php Exception/RuntimeException.php

use std::fmt;

/// The class of a thrown [`Error`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// `Exception\IOException` (extends `\RuntimeException`), with the
    /// path it concerns.
    Io,
    /// `Exception\InvalidArgumentException` (extends
    /// `\InvalidArgumentException`).
    InvalidArgument,
    /// `Exception\RuntimeException` (extends `\RuntimeException`).
    Runtime,
    /// Not a PHP class: input this port refuses rather than approximates
    /// (a stream wrapper other than `file://`, a directory to rename).
    Unsupported,
}

impl Class {
    /// The PHP class name.
    pub fn name(self) -> &'static str {
        match self {
            Class::Io => "Symfony\\Component\\Filesystem\\Exception\\IOException",
            Class::InvalidArgument => {
                "Symfony\\Component\\Filesystem\\Exception\\InvalidArgumentException"
            }
            Class::Runtime => "Symfony\\Component\\Filesystem\\Exception\\RuntimeException",
            Class::Unsupported => "(unsupported by the port)",
        }
    }
}

/// A thrown exception: its class, message and, for an IOException, the
/// path (`getPath()`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    class: Class,
    message: Vec<u8>,
    path: Option<Vec<u8>>,
}

impl Error {
    pub fn new(class: Class, message: impl Into<Vec<u8>>) -> Self {
        Self {
            class,
            message: message.into(),
            path: None,
        }
    }

    /// `new IOException($message, 0, null, $path)`.
    pub fn io(message: impl Into<Vec<u8>>, path: Option<&[u8]>) -> Self {
        Self {
            class: Class::Io,
            message: message.into(),
            path: path.map(<[u8]>::to_vec),
        }
    }

    pub fn class(&self) -> Class {
        self.class
    }

    /// `getMessage()`.
    pub fn message(&self) -> &[u8] {
        &self.message
    }

    /// `IOException::getPath()`.
    pub fn path(&self) -> Option<&[u8]> {
        self.path.as_deref()
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
