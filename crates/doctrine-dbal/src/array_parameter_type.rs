//! Port of Doctrine\DBAL\ArrayParameterType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/ArrayParameterType.php

use crate::ParameterType;

/// The type of a list parameter, expanded by Doctrine's SQL parsing into
/// one placeholder per element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArrayParameterType {
    /// Represents an array of ints to be expanded by Doctrine SQL parsing.
    Integer,
    /// Represents an array of strings to be expanded by Doctrine SQL parsing.
    String,
    /// Represents an array of ascii strings to be expanded by Doctrine SQL parsing.
    Ascii,
    /// Represents an array of ascii strings to be expanded by Doctrine SQL parsing.
    Binary,
}

impl ArrayParameterType {
    /// Every case, in declaration order (`ArrayParameterType::cases()`).
    pub const CASES: [Self; 4] = [Self::Integer, Self::String, Self::Ascii, Self::Binary];

    /// The case's PHP name (`ArrayParameterType::INTEGER->name`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Integer => "INTEGER",
            Self::String => "STRING",
            Self::Ascii => "ASCII",
            Self::Binary => "BINARY",
        }
    }

    /// The case with this PHP name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::CASES.into_iter().find(|case| case.name() == name)
    }

    /// The type of each element (`@internal`).
    pub fn to_element_parameter_type(self) -> ParameterType {
        match self {
            Self::Integer => ParameterType::Integer,
            Self::String => ParameterType::String,
            Self::Ascii => ParameterType::Ascii,
            Self::Binary => ParameterType::Binary,
        }
    }
}
