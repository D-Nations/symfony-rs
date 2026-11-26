//! Port of Doctrine\DBAL\Schema\ForeignKeyConstraintEditor.php.
//!
//! A by-value builder; an invalid name given as a string (PHP throws at
//! once) is kept and returned by `create()`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/ForeignKeyConstraintEditor.php

use php_value::{PhpArray, Value};

use crate::exception::Exception;

use super::exception;
use super::foreign_key_constraint::{
    Deferrability, ForeignKeyConstraint, MatchType, ReferentialAction,
};
use super::name::{OptionallyQualifiedName, UnqualifiedName};

#[derive(Debug, Clone)]
pub struct ForeignKeyConstraintEditor {
    error: Option<Exception>,
    name: Option<UnqualifiedName>,
    referencing_column_names: Vec<UnqualifiedName>,
    referenced_table_name: Option<OptionallyQualifiedName>,
    referenced_column_names: Vec<UnqualifiedName>,
    match_type: MatchType,
    on_update_action: ReferentialAction,
    on_delete_action: ReferentialAction,
    deferrability: Deferrability,
}

impl Default for ForeignKeyConstraintEditor {
    fn default() -> Self {
        Self::new()
    }
}

fn names(
    names: &[&str],
    make: fn(&str) -> Result<UnqualifiedName, Exception>,
) -> Result<Vec<UnqualifiedName>, Exception> {
    names.iter().map(|name| make(name)).collect()
}

impl ForeignKeyConstraintEditor {
    /// `@internal` Use `ForeignKeyConstraint::editor()` or
    /// `ForeignKeyConstraint::edit()`.
    pub fn new() -> Self {
        Self {
            error: None,
            name: None,
            referencing_column_names: Vec::new(),
            referenced_table_name: None,
            referenced_column_names: Vec::new(),
            match_type: MatchType::Simple,
            on_update_action: ReferentialAction::NoAction,
            on_delete_action: ReferentialAction::NoAction,
            deferrability: Deferrability::NotDeferrable,
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
    pub fn set_referencing_column_names(mut self, names: Vec<UnqualifiedName>) -> Self {
        self.referencing_column_names = names;
        self
    }

    #[must_use]
    pub fn set_unquoted_referencing_column_names(mut self, column_names: &[&str]) -> Self {
        if let Some(n) = self.keep(names(column_names, UnqualifiedName::unquoted)) {
            self.referencing_column_names = n;
        }
        self
    }

    #[must_use]
    pub fn set_quoted_referencing_column_names(mut self, column_names: &[&str]) -> Self {
        if let Some(n) = self.keep(names(column_names, UnqualifiedName::quoted)) {
            self.referencing_column_names = n;
        }
        self
    }

    #[must_use]
    pub fn add_referencing_column_name(mut self, name: UnqualifiedName) -> Self {
        self.referencing_column_names.push(name);
        self
    }

    #[must_use]
    pub fn add_unquoted_referencing_column_name(mut self, name: &str) -> Self {
        if let Some(name) = self.keep(UnqualifiedName::unquoted(name)) {
            self.referencing_column_names.push(name);
        }
        self
    }

    #[must_use]
    pub fn add_quoted_referencing_column_name(mut self, name: &str) -> Self {
        if let Some(name) = self.keep(UnqualifiedName::quoted(name)) {
            self.referencing_column_names.push(name);
        }
        self
    }

    #[must_use]
    pub fn set_referenced_table_name(mut self, name: OptionallyQualifiedName) -> Self {
        self.referenced_table_name = Some(name);
        self
    }

    #[must_use]
    pub fn set_unquoted_referenced_table_name(
        mut self,
        name: &str,
        qualifier: Option<&str>,
    ) -> Self {
        if let Some(name) = self.keep(OptionallyQualifiedName::unquoted(name, qualifier)) {
            self.referenced_table_name = Some(name);
        }
        self
    }

    #[must_use]
    pub fn set_quoted_referenced_table_name(mut self, name: &str, qualifier: Option<&str>) -> Self {
        if let Some(name) = self.keep(OptionallyQualifiedName::quoted(name, qualifier)) {
            self.referenced_table_name = Some(name);
        }
        self
    }

    #[must_use]
    pub fn set_referenced_column_names(mut self, names: Vec<UnqualifiedName>) -> Self {
        self.referenced_column_names = names;
        self
    }

    #[must_use]
    pub fn set_unquoted_referenced_column_names(mut self, column_names: &[&str]) -> Self {
        if let Some(n) = self.keep(names(column_names, UnqualifiedName::unquoted)) {
            self.referenced_column_names = n;
        }
        self
    }

    #[must_use]
    pub fn set_quoted_referenced_column_names(mut self, column_names: &[&str]) -> Self {
        if let Some(n) = self.keep(names(column_names, UnqualifiedName::quoted)) {
            self.referenced_column_names = n;
        }
        self
    }

    #[must_use]
    pub fn add_referenced_column_name(mut self, name: UnqualifiedName) -> Self {
        self.referenced_column_names.push(name);
        self
    }

    #[must_use]
    pub fn add_unquoted_referenced_column_name(mut self, name: &str) -> Self {
        if let Some(name) = self.keep(UnqualifiedName::unquoted(name)) {
            self.referenced_column_names.push(name);
        }
        self
    }

    #[must_use]
    pub fn add_quoted_referenced_column_name(mut self, name: &str) -> Self {
        if let Some(name) = self.keep(UnqualifiedName::quoted(name)) {
            self.referenced_column_names.push(name);
        }
        self
    }

    #[must_use]
    pub fn set_match_type(mut self, match_type: MatchType) -> Self {
        self.match_type = match_type;
        self
    }

    #[must_use]
    pub fn set_on_update_action(mut self, action: ReferentialAction) -> Self {
        self.on_update_action = action;
        self
    }

    #[must_use]
    pub fn set_on_delete_action(mut self, action: ReferentialAction) -> Self {
        self.on_delete_action = action;
        self
    }

    #[must_use]
    pub fn set_deferrability(mut self, deferrability: Deferrability) -> Self {
        self.deferrability = deferrability;
        self
    }

    /// `create()`.
    pub fn create(self) -> Result<ForeignKeyConstraint, Exception> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if self.referencing_column_names.is_empty() {
            return Err(exception::foreign_key_referencing_column_names_not_set(
                self.name.as_ref(),
            ));
        }
        let Some(referenced_table_name) = &self.referenced_table_name else {
            return Err(exception::foreign_key_referenced_table_name_not_set(
                self.name.as_ref(),
            ));
        };
        if self.referenced_column_names.is_empty() {
            return Err(exception::foreign_key_referenced_column_names_not_set(
                self.name.as_ref(),
            ));
        }

        let mut options = PhpArray::new();
        if self.match_type != MatchType::Simple {
            options.insert("match", Value::string(self.match_type.value()));
        }
        if self.on_update_action != ReferentialAction::NoAction {
            options.insert("onUpdate", Value::string(self.on_update_action.value()));
        }
        if self.on_delete_action != ReferentialAction::NoAction {
            options.insert("onDelete", Value::string(self.on_delete_action.value()));
        }
        match self.deferrability {
            Deferrability::NotDeferrable => {}
            Deferrability::Deferrable => options.insert("deferrable", Value::Bool(true)),
            Deferrability::Deferred => {
                options.insert("deferrable", Value::Bool(true));
                options.insert("deferred", Value::Bool(true));
            }
        }

        ForeignKeyConstraint::new(
            self.referencing_column_names
                .iter()
                .map(ToString::to_string),
            &referenced_table_name.to_string(),
            self.referenced_column_names.iter().map(ToString::to_string),
            &self
                .name
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
            options,
        )
    }
}
