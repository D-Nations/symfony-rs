//! Port of Doctrine\DBAL\ParameterType.php, and of the parameter type that
//! `Connection.php` accepts for one bound value (its phpstan type
//! `WrapperParameterType`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/ParameterType.php

use std::sync::Arc;

use php_value::PhpString;

use crate::{ArrayParameterType, types::Type};

/// Statement parameter type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParameterType {
    /// Represents the SQL NULL data type.
    Null,
    /// Represents the SQL INTEGER data type.
    Integer,
    /// Represents the SQL CHAR, VARCHAR, or other string data type.
    String,
    /// Represents the SQL large object data type.
    LargeObject,
    /// Represents a boolean data type.
    Boolean,
    /// Represents a binary string data type.
    Binary,
    /// Represents an ASCII string data type
    Ascii,
}

impl ParameterType {
    /// Every case, in declaration order (`ParameterType::cases()`).
    pub const CASES: [Self; 7] = [
        Self::Null,
        Self::Integer,
        Self::String,
        Self::LargeObject,
        Self::Boolean,
        Self::Binary,
        Self::Ascii,
    ];

    /// The case's PHP name (`ParameterType::INTEGER->name`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Null => "NULL",
            Self::Integer => "INTEGER",
            Self::String => "STRING",
            Self::LargeObject => "LARGE_OBJECT",
            Self::Boolean => "BOOLEAN",
            Self::Binary => "BINARY",
            Self::Ascii => "ASCII",
        }
    }

    /// The case with this PHP name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::CASES.into_iter().find(|case| case.name() == name)
    }
}

/// The type of one parameter passed to `Connection` methods:
/// `string|ParameterType|Type|ArrayParameterType`. A string names a
/// registered DBAL type (`'string'`, `'datetime_immutable'`, ...); a
/// `Type` object is compared by identity, as PHP's `===` does.
#[derive(Debug, Clone)]
pub enum WrapperParameterType {
    Parameter(ParameterType),
    Array(ArrayParameterType),
    Name(PhpString),
    Type(Arc<dyn Type>),
}

impl PartialEq for WrapperParameterType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Parameter(a), Self::Parameter(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Name(a), Self::Name(b)) => a == b,
            (Self::Type(a), Self::Type(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for WrapperParameterType {}

impl From<Arc<dyn Type>> for WrapperParameterType {
    fn from(value: Arc<dyn Type>) -> Self {
        Self::Type(value)
    }
}

impl From<ParameterType> for WrapperParameterType {
    fn from(value: ParameterType) -> Self {
        Self::Parameter(value)
    }
}

impl From<ArrayParameterType> for WrapperParameterType {
    fn from(value: ArrayParameterType) -> Self {
        Self::Array(value)
    }
}

impl From<&str> for WrapperParameterType {
    fn from(value: &str) -> Self {
        Self::Name(PhpString::from(value))
    }
}
