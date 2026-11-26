//! Port of Doctrine\DBAL\Schema\UniqueConstraintEditor.php (a by-value
//! builder; an invalid name given as a string is returned by `create()`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/UniqueConstraintEditor.php

use php_value::PhpArray;

use crate::exception::Exception;

use super::exception;
use super::name::UnqualifiedName;
use super::unique_constraint::UniqueConstraint;

#[derive(Debug, Clone, Default)]
pub struct UniqueConstraintEditor {
    error: Option<Exception>,
    name: Option<UnqualifiedName>,
    column_names: Vec<UnqualifiedName>,
    is_clustered: bool,
}

impl UniqueConstraintEditor {
    /// `@internal` Use `UniqueConstraint::editor()` or
    /// `UniqueConstraint::edit()`.
    pub fn new() -> Self {
        Self::default()
    }

    fn keep<T>(&mut self, result: Result<T, Exception>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(e) => {
                self.error.get_or_insert(e);
                None
            }
        }
    }

    #[must_use]
    pub fn set_name(mut self, name: Option<UnqualifiedName>) -> Self {
        self.name = name;
        self
    }

    #[must_use]
    pub fn set_unquoted_name(mut self, name: &str) -> Self {
        if let Some(name) = self.keep(UnqualifiedName::unquoted(name)) {
            self.name = Some(name);
        }
        self
    }

    #[must_use]
    pub fn set_quoted_name(mut self, name: &str) -> Self {
        if let Some(name) = self.keep(UnqualifiedName::quoted(name)) {
            self.name = Some(name);
        }
        self
    }

    #[must_use]
    pub fn set_column_names(mut self, names: Vec<UnqualifiedName>) -> Self {
        self.column_names = names;
        self
    }

    #[must_use]
    pub fn set_unquoted_column_names(mut self, names: &[&str]) -> Self {
        let parsed: Result<Vec<_>, _> = names.iter().map(|n| UnqualifiedName::unquoted(n)).collect();
        if let Some(names) = self.keep(parsed) {
            self.column_names = names;
        }
        self
    }

    #[must_use]
    pub fn set_quoted_column_names(mut self, names: &[&str]) -> Self {
        let parsed: Result<Vec<_>, _> = names.iter().map(|n| UnqualifiedName::quoted(n)).collect();
        if let Some(names) = self.keep(parsed) {
            self.column_names = names;
        }
        self
    }

    #[must_use]
    pub fn add_column_name(mut self, name: UnqualifiedName) -> Self {
        self.column_names.push(name);
        self
    }

    #[must_use]
    pub fn add_unquoted_column_name(mut self, name: &str) -> Self {
        if let Some(name) = self.keep(UnqualifiedName::unquoted(name)) {
            self.column_names.push(name);
        }
        self
    }

    #[must_use]
    pub fn add_quoted_column_name(mut self, name: &str) -> Self {
        if let Some(name) = self.keep(UnqualifiedName::quoted(name)) {
            self.column_names.push(name);
        }
        self
    }

    #[must_use]
    pub fn set_is_clustered(mut self, is_clustered: bool) -> Self {
        self.is_clustered = is_clustered;
        self
    }

    /// `create()`.
    pub fn create(self) -> Result<UniqueConstraint, Exception> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.column_names.is_empty() {
            return Err(exception::unique_constraint_column_names_not_set(
                self.name.as_ref(),
            ));
        }
        Ok(UniqueConstraint::new(
            &self.name.as_ref().map(ToString::to_string).unwrap_or_default(),
            self.column_names.iter().map(ToString::to_string),
            if self.is_clustered { vec!["clustered"] } else { vec![] },
            PhpArray::new(),
        ))
    }
}
