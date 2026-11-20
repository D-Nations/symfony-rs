//! The parts of symfony/string that symfony/console needs: `UnicodeString`
//! (NFC, graphemes, width, wordwrap) and PHP's native `wordwrap()`.
//!
//! Upstream: symfony/string Exception/InvalidArgumentException.php

#![forbid(unsafe_code)]

pub mod php;
mod tables;
pub mod unicode_string;

pub use unicode_string::UnicodeString;

/// `Symfony\Component\String\Exception\InvalidArgumentException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidArgumentException(pub String);

impl std::fmt::Display for InvalidArgumentException {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InvalidArgumentException {}

/// `u($string)`.
pub fn u(string: &str) -> UnicodeString {
    UnicodeString::new(string)
}
