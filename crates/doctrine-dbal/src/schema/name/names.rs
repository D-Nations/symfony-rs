//! Port of Doctrine\DBAL\Schema\Name.php (the interface, here the
//! [`Name`] enum of its implementations), Name\UnqualifiedName.php,
//! Name\OptionallyQualifiedName.php and Name\GenericName.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Name.php src/Schema/Name/UnqualifiedName.php
//! Upstream: doctrine/dbal src/Schema/Name/OptionallyQualifiedName.php src/Schema/Name/GenericName.php

use std::fmt;

use crate::exception::Exception;
use crate::platforms::AbstractPlatform;
use crate::schema::exception;

use super::{Identifier, UnquotedIdentifierFolding};

/// A database object name (`Doctrine\DBAL\Schema\Name`): one of its
/// three implementations.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Name {
    Unqualified(UnqualifiedName),
    OptionallyQualified(OptionallyQualifiedName),
    Generic(GenericName),
}

impl Name {
    /// Returns the SQL representation of the name for the given platform.
    pub fn to_sql(&self, platform: &(impl AbstractPlatform + ?Sized)) -> String {
        match self {
            Self::Unqualified(name) => name.to_sql(platform),
            Self::OptionallyQualified(name) => name.to_sql(platform),
            Self::Generic(name) => name.to_sql(platform),
        }
    }
}

/// `toString()`: the string representation of the name, quoted parts
/// in ANSI quotes.
impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unqualified(name) => name.fmt(f),
            Self::OptionallyQualified(name) => name.fmt(f),
            Self::Generic(name) => name.fmt(f),
        }
    }
}

/// An unqualified name: one identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnqualifiedName {
    identifier: Identifier,
}

impl UnqualifiedName {
    pub fn new(identifier: Identifier) -> Self {
        Self { identifier }
    }

    pub fn get_identifier(&self) -> &Identifier {
        &self.identifier
    }

    pub fn to_sql(&self, platform: &(impl AbstractPlatform + ?Sized)) -> String {
        self.identifier.to_sql(platform)
    }

    pub fn equals(&self, other: &Self, folding: UnquotedIdentifierFolding) -> bool {
        self.identifier.equals(other.get_identifier(), folding)
    }

    /// Creates a quoted name.
    pub fn quoted(value: &str) -> Result<Self, Exception> {
        Ok(Self::new(Identifier::quoted(value)?))
    }

    /// Creates an unquoted name.
    pub fn unquoted(value: &str) -> Result<Self, Exception> {
        Ok(Self::new(Identifier::unquoted(value)?))
    }
}

impl fmt::Display for UnqualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.identifier.fmt(f)
    }
}

/// An optionally qualified name: an unqualified name and an optional
/// qualifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OptionallyQualifiedName {
    unqualified_name: Identifier,
    qualifier: Option<Identifier>,
}

impl OptionallyQualifiedName {
    pub fn new(unqualified_name: Identifier, qualifier: Option<Identifier>) -> Self {
        Self {
            unqualified_name,
            qualifier,
        }
    }

    pub fn get_unqualified_name(&self) -> &Identifier {
        &self.unqualified_name
    }

    pub fn get_qualifier(&self) -> Option<&Identifier> {
        self.qualifier.as_ref()
    }

    pub fn to_sql(&self, platform: &(impl AbstractPlatform + ?Sized)) -> String {
        let unqualified_name = self.unqualified_name.to_sql(platform);
        match &self.qualifier {
            None => unqualified_name,
            Some(qualifier) => format!("{}.{unqualified_name}", qualifier.to_sql(platform)),
        }
    }

    /// Returns whether this name is equal to the other; throws
    /// `IncomparableNames` when one is qualified and the other is not.
    pub fn equals(&self, other: &Self, folding: UnquotedIdentifierFolding) -> Result<bool, Exception> {
        if self.qualifier.is_none() != other.qualifier.is_none() {
            return Err(exception::incomparable_names(self, other));
        }
        if !self
            .unqualified_name
            .equals(other.get_unqualified_name(), folding)
        {
            return Ok(false);
        }
        Ok(match (&self.qualifier, &other.qualifier) {
            (Some(a), Some(b)) => a.equals(b, folding),
            _ => true,
        })
    }

    /// Creates an optionally qualified name with all identifiers quoted.
    pub fn quoted(unqualified_name: &str, qualifier: Option<&str>) -> Result<Self, Exception> {
        Ok(Self::new(
            Identifier::quoted(unqualified_name)?,
            qualifier.map(Identifier::quoted).transpose()?,
        ))
    }

    /// Creates an optionally qualified name with all identifiers unquoted.
    pub fn unquoted(unqualified_name: &str, qualifier: Option<&str>) -> Result<Self, Exception> {
        Ok(Self::new(
            Identifier::unquoted(unqualified_name)?,
            qualifier.map(Identifier::unquoted).transpose()?,
        ))
    }
}

impl fmt::Display for OptionallyQualifiedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(qualifier) = &self.qualifier {
            write!(f, "{qualifier}.")?;
        }
        self.unqualified_name.fmt(f)
    }
}

/// A generic name: one or more identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GenericName {
    identifiers: Vec<Identifier>,
}

impl GenericName {
    pub fn new(first_identifier: Identifier, other_identifiers: Vec<Identifier>) -> Self {
        let mut identifiers = vec![first_identifier];
        identifiers.extend(other_identifiers);
        Self { identifiers }
    }

    /// The identifiers, never empty.
    pub fn get_identifiers(&self) -> &[Identifier] {
        &self.identifiers
    }

    pub fn to_sql(&self, platform: &(impl AbstractPlatform + ?Sized)) -> String {
        self.identifiers
            .iter()
            .map(|identifier| identifier.to_sql(platform))
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl fmt::Display for GenericName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, identifier) in self.identifiers.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            identifier.fmt(f)?;
        }
        Ok(())
    }
}
