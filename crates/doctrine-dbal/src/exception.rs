//! Port of Doctrine\DBAL\Exception.php (the interface every DBAL exception
//! implements) and Exception\InvalidArgumentException.php.
//!
//! `Exception` is a closed enum of the DBAL exceptions ported so far; a
//! `catch (Exception $e)` in PHP is a match on it.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Exception.php src/Exception/InvalidArgumentException.php

use std::fmt;

use crate::platforms::NotSupported;
use crate::query::QueryException;

/// `Doctrine\DBAL\Exception\InvalidArgumentException`: invalid arguments
/// passed to a DBAL API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidArgumentException {
    message: String,
}

impl InvalidArgumentException {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Exception\\InvalidArgumentException";

    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for InvalidArgumentException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InvalidArgumentException {}

/// A `Doctrine\DBAL\Exception`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Exception {
    /// `Query\QueryException` and its subclasses.
    Query(QueryException),
    /// `Platforms\Exception\NotSupported`.
    NotSupported(NotSupported),
    /// `Exception\InvalidArgumentException`.
    InvalidArgument(InvalidArgumentException),
}

impl Exception {
    /// The PHP class of the exception.
    pub fn class(&self) -> &'static str {
        match self {
            Self::Query(e) => e.class(),
            Self::NotSupported(_) => NotSupported::CLASS,
            Self::InvalidArgument(_) => InvalidArgumentException::CLASS,
        }
    }

    /// `getMessage()`.
    pub fn message(&self) -> &str {
        match self {
            Self::Query(e) => e.message(),
            Self::NotSupported(e) => e.message(),
            Self::InvalidArgument(e) => e.message(),
        }
    }
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for Exception {}

impl From<QueryException> for Exception {
    fn from(value: QueryException) -> Self {
        Self::Query(value)
    }
}

impl From<NotSupported> for Exception {
    fn from(value: NotSupported) -> Self {
        Self::NotSupported(value)
    }
}

impl From<InvalidArgumentException> for Exception {
    fn from(value: InvalidArgumentException) -> Self {
        Self::InvalidArgument(value)
    }
}
