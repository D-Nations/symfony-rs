//! Port of Doctrine\DBAL\Schema\Name\Identifier.php and
//! UnquotedIdentifierFolding.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Name/Identifier.php src/Schema/Name/UnquotedIdentifierFolding.php

use std::fmt;

use crate::exception::Exception;
use crate::platforms::AbstractPlatform;
use crate::schema::exception;

/// Represents the database platform's behavior of folding the case of
/// unquoted identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnquotedIdentifierFolding {
    Upper,
    Lower,
    None,
}

impl UnquotedIdentifierFolding {
    /// The case name (`UPPER`, `LOWER`, `NONE`).
    pub fn name(self) -> &'static str {
        match self {
            Self::Upper => "UPPER",
            Self::Lower => "LOWER",
            Self::None => "NONE",
        }
    }

    /// Folds the case of the unquoted identifier (`strtoupper()`,
    /// `strtolower()`: ASCII only, as PHP 8.2 and later).
    pub fn fold_unquoted_identifier(self, value: &str) -> String {
        match self {
            Self::Upper => value.to_ascii_uppercase(),
            Self::Lower => value.to_ascii_lowercase(),
            Self::None => value.to_owned(),
        }
    }
}

/// Represents an SQL identifier (an object name or a qualifier).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    value: String,
    is_quoted: bool,
}

impl Identifier {
    fn new(value: &str, is_quoted: bool) -> Result<Self, Exception> {
        if value.is_empty() {
            return Err(exception::identifier_from_empty());
        }
        Ok(Self {
            value: value.to_owned(),
            is_quoted,
        })
    }

    /// Creates a quoted identifier.
    pub fn quoted(value: &str) -> Result<Self, Exception> {
        Self::new(value, true)
    }

    /// Creates an unquoted identifier.
    pub fn unquoted(value: &str) -> Result<Self, Exception> {
        Self::new(value, false)
    }

    pub fn get_value(&self) -> &str {
        &self.value
    }

    pub fn is_quoted(&self) -> bool {
        self.is_quoted
    }

    /// Returns whether this identifier is equal to the other.
    pub fn equals(&self, other: &Self, folding: UnquotedIdentifierFolding) -> bool {
        self.to_normalized_value(folding) == other.to_normalized_value(folding)
    }

    pub fn to_sql(&self, platform: &(impl AbstractPlatform + ?Sized)) -> String {
        platform.quote_single_identifier(
            &self.to_normalized_value(platform.get_unquoted_identifier_folding()),
        )
    }

    /// Returns the identifier normalized according to the rules of the
    /// given folding.
    pub fn to_normalized_value(&self, folding: UnquotedIdentifierFolding) -> String {
        if !self.is_quoted {
            return folding.fold_unquoted_identifier(&self.value);
        }
        self.value.clone()
    }
}

/// `toString()`: the value, or `"value"` with `"` doubled when quoted.
impl fmt::Display for Identifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.is_quoted {
            return f.write_str(&self.value);
        }
        write!(f, "\"{}\"", self.value.replace('"', "\"\""))
    }
}
