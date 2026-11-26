//! Port of Doctrine\DBAL\Schema\PrimaryKeyConstraint.php and
//! PrimaryKeyConstraintEditor.php (a by-value builder; an invalid name
//! given as a string is returned by `create()`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/PrimaryKeyConstraint.php
//! Upstream: doctrine/dbal src/Schema/PrimaryKeyConstraintEditor.php

use crate::exception::Exception;

use super::exception;
use super::name::UnqualifiedName;

/// `Doctrine\DBAL\Schema\PrimaryKeyConstraint`.
#[derive(Debug, Clone, PartialEq)]
pub struct PrimaryKeyConstraint {
    name: Option<UnqualifiedName>,
    column_names: Vec<UnqualifiedName>,
    is_clustered: bool,
}

impl PrimaryKeyConstraint {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\PrimaryKeyConstraint";

    /// `@internal` Use `PrimaryKeyConstraint::editor()`.
    pub fn new(
        name: Option<UnqualifiedName>,
        column_names: Vec<UnqualifiedName>,
        is_clustered: bool,
    ) -> Result<Self, Exception> {
        if column_names.is_empty() {
            return Err(exception::primary_key_column_names_not_set());
        }
        Ok(Self {
            name,
            column_names,
            is_clustered,
        })
    }

    pub fn get_object_name(&self) -> Option<&UnqualifiedName> {
        self.name.as_ref()
    }

    pub fn get_column_names(&self) -> &[UnqualifiedName] {
        &self.column_names
    }

    pub fn is_clustered(&self) -> bool {
        self.is_clustered
    }

    pub fn editor() -> PrimaryKeyConstraintEditor {
        PrimaryKeyConstraintEditor::new()
    }

    #[must_use]
    pub fn edit(&self) -> PrimaryKeyConstraintEditor {
        Self::editor()
            .set_name(self.name.clone())
            .set_column_names(self.column_names.clone())
            .set_is_clustered(self.is_clustered)
    }
}

/// `Doctrine\DBAL\Schema\PrimaryKeyConstraintEditor`.
#[derive(Debug, Clone)]
pub struct PrimaryKeyConstraintEditor {
    error: Option<Exception>,
    name: Option<UnqualifiedName>,
    column_names: Vec<UnqualifiedName>,
    is_clustered: bool,
}

impl Default for PrimaryKeyConstraintEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl PrimaryKeyConstraintEditor {
    pub fn new() -> Self {
        Self {
            error: None,
            name: None,
            column_names: Vec::new(),
            is_clustered: true,
        }
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

    pub fn create(self) -> Result<PrimaryKeyConstraint, Exception> {
        if let Some(error) = self.error {
            return Err(error);
        }
        PrimaryKeyConstraint::new(self.name, self.column_names, self.is_clustered)
    }
}
