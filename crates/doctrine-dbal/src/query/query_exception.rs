//! Port of Doctrine\DBAL\Query\QueryException.php and its subclasses
//! Query\Exception\NonUniqueAlias.php and UnknownAlias.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/QueryException.php src/Query/Exception/NonUniqueAlias.php src/Query/Exception/UnknownAlias.php

use std::fmt;

/// The PHP class of a [`QueryException`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryExceptionKind {
    /// `Query\QueryException` itself.
    Query,
    /// `Query\Exception\NonUniqueAlias`.
    NonUniqueAlias,
    /// `Query\Exception\UnknownAlias`.
    UnknownAlias,
}

impl QueryExceptionKind {
    /// The PHP class name.
    pub fn class(self) -> &'static str {
        match self {
            Self::Query => "Doctrine\\DBAL\\Query\\QueryException",
            Self::NonUniqueAlias => "Doctrine\\DBAL\\Query\\Exception\\NonUniqueAlias",
            Self::UnknownAlias => "Doctrine\\DBAL\\Query\\Exception\\UnknownAlias",
        }
    }
}

/// `Query\QueryException`, or one of its subclasses (`kind`). Every kind
/// is an instance of `QueryException`, as in PHP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryException {
    kind: QueryExceptionKind,
    message: String,
}

impl QueryException {
    /// An exception of this `kind` with this message, as read back from a
    /// [`crate::Exception`].
    pub fn with_kind(kind: QueryExceptionKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// `new QueryException($message)`.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: QueryExceptionKind::Query,
            message: message.into(),
        }
    }

    /// `NonUniqueAlias::new($alias, $registeredAliases)`.
    pub fn non_unique_alias<S: AsRef<str>>(alias: &str, registered_aliases: &[S]) -> Self {
        Self {
            kind: QueryExceptionKind::NonUniqueAlias,
            message: format!(
                "The given alias \"{alias}\" is not unique in FROM and JOIN clause table. \
                 The currently registered aliases are: {}.",
                join(registered_aliases)
            ),
        }
    }

    /// `UnknownAlias::new($alias, $registeredAliases)`.
    pub fn unknown_alias<S: AsRef<str>>(alias: &str, registered_aliases: &[S]) -> Self {
        Self {
            kind: QueryExceptionKind::UnknownAlias,
            message: format!(
                "The given alias \"{alias}\" is not part of any FROM or JOIN clause table. \
                 The currently registered aliases are: {}.",
                join(registered_aliases)
            ),
        }
    }

    pub fn kind(&self) -> QueryExceptionKind {
        self.kind
    }

    /// The PHP class of the exception.
    pub fn class(&self) -> &'static str {
        self.kind.class()
    }

    /// `getMessage()`.
    pub fn message(&self) -> &str {
        &self.message
    }
}

fn join<S: AsRef<str>>(items: &[S]) -> String {
    items
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(", ")
}

impl fmt::Display for QueryException {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QueryException {}
