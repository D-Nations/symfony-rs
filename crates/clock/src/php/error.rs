//! The exceptions of PHP's date extension that Clock throws or lets
//! through: `DateMalformedStringException`, `DateInvalidTimeZoneException`
//! and `DateRangeError`, with PHP's messages. [`DateError::Unsupported`]
//! is this port's own: a date text or format outside what it reads, which
//! is refused rather than approximated.
//!
//! Upstream: none

use std::fmt;

/// An exception of PHP's date extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateError {
    /// `\DateMalformedStringException`.
    MalformedString(String),
    /// `\DateInvalidTimeZoneException`.
    InvalidTimeZone(String),
    /// `\DateRangeError`.
    Range(String),
    /// Input this port does not read (no PHP class): PHP may accept it, but
    /// its answer was not recorded, so it is refused.
    Unsupported(String),
}

impl DateError {
    /// The PHP class name, as `$e::class` gives it.
    pub fn class(&self) -> &'static str {
        match self {
            Self::MalformedString(_) => "DateMalformedStringException",
            Self::InvalidTimeZone(_) => "DateInvalidTimeZoneException",
            Self::Range(_) => "DateRangeError",
            Self::Unsupported(_) => "Unsupported",
        }
    }

    /// `$e->getMessage()`.
    pub fn message(&self) -> &str {
        match self {
            Self::MalformedString(m)
            | Self::InvalidTimeZone(m)
            | Self::Range(m)
            | Self::Unsupported(m) => m,
        }
    }

    pub(crate) fn unsupported(what: &str, input: &str) -> Self {
        Self::Unsupported(format!("{what} not supported by this port: {input}"))
    }
}

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.class(), self.message())
    }
}

impl std::error::Error for DateError {}
