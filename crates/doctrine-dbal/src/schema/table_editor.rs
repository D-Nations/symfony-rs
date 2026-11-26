//! Port of Doctrine\DBAL\Schema\TableEditor.php.
//!
//! A by-value builder. An exception PHP throws from a setter is kept and
//! returned by `create()` (the first one wins). `setIndexes(Index|
//! IndexEditor ...)` takes [`TableIndex`] values; `modifyColumn()` takes
//! a function from the column's editor to the changed editor.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/TableEditor.php

use php_value::{PhpArray, Value};

use crate::exception::Exception;

use super::collections::{
    CollectionError, OptionallyUnqualifiedNamedObjectSet, UnqualifiedNamedObjectSet,
};
use super::column::Column;
use super::column_editor::ColumnEditor;
use super::exception::{self, TableObject};
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::index::{Index, IndexedColumn};
use super::index_editor::IndexEditor;
use super::name::{OptionallyQualifiedName, UnqualifiedName};
use super::primary_key_constraint::PrimaryKeyConstraint;
use super::schema_config::TableConfiguration;
use super::table::{Table, TableParts};
use super::unique_constraint::UniqueConstraint;

/// An argument of `setIndexes()` / `addIndex()`: `Index|IndexEditor`.
#[derive(Debug, Clone)]
pub enum TableIndex {
    Index(Index),
    Editor(IndexEditor),
}

impl From<Index> for TableIndex {
    fn from(index: Index) -> Self {
        Self::Index(index)
    }
}

impl From<IndexEditor> for TableIndex {
    fn from(editor: IndexEditor) -> Self {
        Self::Editor(editor)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TableEditor {
    error: Option<Exception>,
    name: Option<OptionallyQualifiedName>,
    columns: UnqualifiedNamedObjectSet<Column>,
    renamed_columns: Vec<(String, String)>,
    indexes: UnqualifiedNamedObjectSet<Index>,
    index_editors: Vec<IndexEditor>,
    primary_key_constraint: Option<PrimaryKeyConstraint>,
    unique_constraints: OptionallyUnqualifiedNamedObjectSet<UniqueConstraint>,
    foreign_key_constraints: OptionallyUnqualifiedNamedObjectSet<ForeignKeyConstraint>,
    options: PhpArray,
    comment: String,
    configuration: Option<TableConfiguration>,
}

fn names_equal(a: &UnqualifiedName, b: &UnqualifiedName) -> bool {
    a.get_identifier()
        .get_value()
        .eq_ignore_ascii_case(b.get_identifier().get_value())
}

fn column_key(name: &UnqualifiedName) -> String {
    name.get_identifier().get_value().to_ascii_lowercase()
}

impl TableEditor {
    /// `@internal` Use `Table::editor()` or `Table::edit()`.
    pub fn new() -> Self {
        Self::default()
    }

    fn fail(&mut self, e: Exception) {
        self.error.get_or_insert(e);
    }

    /// Maps a set's failure to the `InvalidTableModification` PHP throws.
    fn modification_error(&self, object: TableObject, e: CollectionError) -> Exception {
        let previous = e.exception();
        match e {
            CollectionError::AlreadyExists(name) => {
                exception::table_object_already_exists(object, self.name.as_ref(), &name, previous)
            }
            CollectionError::DoesNotExist(name) => {
                exception::table_object_does_not_exist(object, self.name.as_ref(), &name, previous)
            }
            CollectionError::Other(e) => e,
        }
    }

    fn check(&mut self, object: TableObject, result: Result<(), CollectionError>) {
        if let Err(e) = result {
            let e = self.modification_error(object, e);
            self.fail(e);
        }
    }

    #[must_use]
    pub fn set_name(mut self, name: OptionallyQualifiedName) -> Self {
        self.name = Some(name);
        self
    }

    #[must_use]
    pub fn set_unquoted_name(mut self, name: &str, qualifier: Option<&str>) -> Self {
        match OptionallyQualifiedName::unquoted(name, qualifier) {
            Ok(name) => self.name = Some(name),
            Err(e) => self.fail(e),
        }
        self
    }

    #[must_use]
    pub fn set_quoted_name(mut self, name: &str, qualifier: Option<&str>) -> Self {
        match OptionallyQualifiedName::quoted(name, qualifier) {
            Ok(name) => self.name = Some(name),
            Err(e) => self.fail(e),
        }
        self
    }

    #[must_use]
    pub fn set_columns(mut self, columns: Vec<Column>) -> Self {
        self.columns.clear();
        for column in columns {
            self = self.add_column(column);
        }
        self
    }

    #[must_use]
    pub fn add_column(mut self, column: Column) -> Self {
        let result = self.columns.add(column);
        self.check(TableObject::Column, result);
        self
    }

    #[must_use]
    pub fn modify_column(
        mut self,
        column_name: &UnqualifiedName,
        modification: impl FnOnce(ColumnEditor) -> ColumnEditor,
    ) -> Self {
        let result = self
            .columns
            .modify(column_name, |column| modification(column.edit()?).create());
        self.check(TableObject::Column, result);
        self
    }

    #[must_use]
    pub fn modify_column_by_unquoted_name(
        mut self,
        column_name: &str,
        modification: impl FnOnce(ColumnEditor) -> ColumnEditor,
    ) -> Self {
        match UnqualifiedName::unquoted(column_name) {
            Ok(name) => self.modify_column(&name, modification),
            Err(e) => {
                self.fail(e);
                self
            }
        }
    }

    #[must_use]
    pub fn rename_column(
        mut self,
        old_column_name: &UnqualifiedName,
        new_column_name: &UnqualifiedName,
    ) -> Self {
        let new_name = new_column_name.clone();
        self = self.modify_column(old_column_name, move |editor| editor.set_name(new_name));
        if self.error.is_some() {
            return self;
        }

        if let Err(e) = self.rename_column_in_indexes(old_column_name, new_column_name) {
            self.fail(e);
        }
        if let Err(e) =
            self.rename_column_in_primary_key_constraint(old_column_name, new_column_name)
        {
            self.fail(e);
        }
        if let Err(e) =
            self.rename_column_in_foreign_key_constraints(old_column_name, new_column_name)
        {
            self.fail(e);
        }
        if let Err(e) = self.rename_column_in_unique_constraints(old_column_name, new_column_name) {
            self.fail(e);
        }

        let mut old_key = column_key(old_column_name);
        let new_key = column_key(new_column_name);

        // If a column is renamed multiple times, only the original and
        // the last new name are kept.
        if let Some(position) = self.renamed_columns.iter().position(|(k, _)| *k == old_key) {
            old_key = self.renamed_columns.remove(position).1;
        }
        if new_key != old_key {
            match self.renamed_columns.iter_mut().find(|(k, _)| *k == new_key) {
                Some(entry) => entry.1 = old_key,
                None => self.renamed_columns.push((new_key, old_key)),
            }
        }
        self
    }

    fn rename_column_in_indexes(
        &mut self,
        old_column_name: &UnqualifiedName,
        new_column_name: &UnqualifiedName,
    ) -> Result<(), Exception> {
        let mut changes = Vec::new();
        for index in self.indexes.iter() {
            let mut modified = false;
            let mut columns = Vec::new();
            for column in index.get_indexed_columns()? {
                if names_equal(column.get_column_name(), old_column_name) {
                    columns.push(IndexedColumn::new(
                        new_column_name.clone(),
                        column.get_length(),
                    )?);
                    modified = true;
                } else {
                    columns.push(column.clone());
                }
            }
            if modified {
                changes.push((index.get_object_name()?, columns));
            }
        }
        for (name, columns) in changes {
            self.indexes
                .modify(&name, |index| index.edit()?.set_columns(columns).create())
                .map_err(Exception::from)?;
        }
        Ok(())
    }

    fn rename_column_in_primary_key_constraint(
        &mut self,
        old_column_name: &UnqualifiedName,
        new_column_name: &UnqualifiedName,
    ) -> Result<(), Exception> {
        let Some(constraint) = &self.primary_key_constraint else {
            return Ok(());
        };
        let mut modified = false;
        let column_names: Vec<UnqualifiedName> = constraint
            .get_column_names()
            .iter()
            .map(|name| {
                if names_equal(name, old_column_name) {
                    modified = true;
                    new_column_name.clone()
                } else {
                    name.clone()
                }
            })
            .collect();
        if modified {
            self.primary_key_constraint =
                Some(constraint.edit().set_column_names(column_names).create()?);
        }
        Ok(())
    }

    fn rename_column_in_unique_constraints(
        &mut self,
        old_column_name: &UnqualifiedName,
        new_column_name: &UnqualifiedName,
    ) -> Result<(), Exception> {
        let mut constraints = Vec::new();
        let mut any_modified = false;
        for constraint in self.unique_constraints.iter() {
            let (names, modified) = replace_name(
                constraint.get_column_names()?,
                old_column_name,
                new_column_name,
            );
            if modified {
                constraints.push(constraint.edit()?.set_column_names(names).create()?);
                any_modified = true;
            } else {
                constraints.push(constraint.clone());
            }
        }
        if any_modified {
            self.unique_constraints.clear();
            for constraint in constraints {
                self.unique_constraints.add(constraint)?;
            }
        }
        Ok(())
    }

    fn rename_column_in_foreign_key_constraints(
        &mut self,
        old_column_name: &UnqualifiedName,
        new_column_name: &UnqualifiedName,
    ) -> Result<(), Exception> {
        let mut constraints = Vec::new();
        let mut any_modified = false;
        for constraint in self.foreign_key_constraints.iter() {
            let (names, modified) = replace_name(
                constraint.get_referencing_column_names()?,
                old_column_name,
                new_column_name,
            );
            if modified {
                constraints.push(
                    constraint
                        .edit()?
                        .set_referencing_column_names(names)
                        .create()?,
                );
                any_modified = true;
            } else {
                constraints.push(constraint.clone());
            }
        }
        if any_modified {
            self.foreign_key_constraints.clear();
            for constraint in constraints {
                self.foreign_key_constraints.add(constraint)?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn rename_column_by_unquoted_name(
        mut self,
        old_column_name: &str,
        new_column_name: &str,
    ) -> Self {
        match (
            UnqualifiedName::unquoted(old_column_name),
            UnqualifiedName::unquoted(new_column_name),
        ) {
            (Ok(old), Ok(new)) => self.rename_column(&old, &new),
            (Err(e), _) | (_, Err(e)) => {
                self.fail(e);
                self
            }
        }
    }

    #[must_use]
    pub fn drop_column(mut self, column_name: &UnqualifiedName) -> Self {
        let result = self.columns.remove(column_name);
        self.check(TableObject::Column, result);
        self
    }

    #[must_use]
    pub fn drop_column_by_unquoted_name(mut self, column_name: &str) -> Self {
        match UnqualifiedName::unquoted(column_name) {
            Ok(name) => self.drop_column(&name),
            Err(e) => {
                self.fail(e);
                self
            }
        }
    }

    #[must_use]
    pub fn set_indexes<I: Into<TableIndex>>(mut self, indexes: Vec<I>) -> Self {
        self.indexes.clear();
        self.index_editors.clear();
        for index in indexes {
            self = self.add_index(index);
        }
        self
    }

    #[must_use]
    pub fn add_index(mut self, index: impl Into<TableIndex>) -> Self {
        match index.into() {
            TableIndex::Editor(editor) => self.index_editors.push(editor),
            TableIndex::Index(index) => {
                let result = self.indexes.add(index);
                self.check(TableObject::Index, result);
            }
        }
        self
    }

    #[must_use]
    pub fn rename_index(
        mut self,
        old_index_name: &UnqualifiedName,
        new_index_name: &UnqualifiedName,
    ) -> Self {
        let result = self.indexes.modify(old_index_name, |index| {
            index
                .edit()?
                .set_name(Some(new_index_name.clone()))
                .create()
        });
        self.check(TableObject::Index, result);
        self
    }

    #[must_use]
    pub fn rename_index_by_unquoted_name(
        mut self,
        old_index_name: &str,
        new_index_name: &str,
    ) -> Self {
        match (
            UnqualifiedName::unquoted(old_index_name),
            UnqualifiedName::unquoted(new_index_name),
        ) {
            (Ok(old), Ok(new)) => self.rename_index(&old, &new),
            (Err(e), _) | (_, Err(e)) => {
                self.fail(e);
                self
            }
        }
    }

    #[must_use]
    pub fn drop_index(mut self, index_name: &UnqualifiedName) -> Self {
        let result = self.indexes.remove(index_name);
        self.check(TableObject::Index, result);
        self
    }

    #[must_use]
    pub fn drop_index_by_unquoted_name(mut self, index_name: &str) -> Self {
        match UnqualifiedName::unquoted(index_name) {
            Ok(name) => self.drop_index(&name),
            Err(e) => {
                self.fail(e);
                self
            }
        }
    }

    #[must_use]
    pub fn set_primary_key_constraint(mut self, constraint: Option<PrimaryKeyConstraint>) -> Self {
        self.primary_key_constraint = constraint;
        let primary: Vec<UnqualifiedName> = self
            .indexes
            .iter()
            .filter(|index| index.is_primary())
            .filter_map(|index| index.get_object_name().ok())
            .collect();
        for name in primary {
            let result = self.indexes.remove(&name);
            self.check(TableObject::Index, result);
        }
        self
    }

    #[must_use]
    pub fn add_primary_key_constraint(mut self, constraint: PrimaryKeyConstraint) -> Self {
        if self.primary_key_constraint.is_some() {
            let e = exception::primary_key_constraint_already_exists(self.name.as_ref());
            self.fail(e);
            return self;
        }
        self.set_primary_key_constraint(Some(constraint))
    }

    #[must_use]
    pub fn drop_primary_key_constraint(mut self) -> Self {
        if self.primary_key_constraint.is_none() {
            let e = exception::primary_key_constraint_does_not_exist(self.name.as_ref());
            self.fail(e);
            return self;
        }
        self.set_primary_key_constraint(None)
    }

    #[must_use]
    pub fn set_unique_constraints(mut self, constraints: Vec<UniqueConstraint>) -> Self {
        self.unique_constraints.clear();
        for constraint in constraints {
            self = self.add_unique_constraint(constraint);
        }
        self
    }

    #[must_use]
    pub fn add_unique_constraint(mut self, constraint: UniqueConstraint) -> Self {
        let result = self.unique_constraints.add(constraint);
        self.check(TableObject::UniqueConstraint, result);
        self
    }

    #[must_use]
    pub fn drop_unique_constraint(mut self, constraint_name: &UnqualifiedName) -> Self {
        let result = self.unique_constraints.remove(constraint_name);
        self.check(TableObject::UniqueConstraint, result);
        self
    }

    #[must_use]
    pub fn drop_unique_constraint_by_unquoted_name(mut self, constraint_name: &str) -> Self {
        match UnqualifiedName::unquoted(constraint_name) {
            Ok(name) => self.drop_unique_constraint(&name),
            Err(e) => {
                self.fail(e);
                self
            }
        }
    }

    #[must_use]
    pub fn set_foreign_key_constraints(mut self, constraints: Vec<ForeignKeyConstraint>) -> Self {
        self.foreign_key_constraints.clear();
        for constraint in constraints {
            self = self.add_foreign_key_constraint(constraint);
        }
        self
    }

    #[must_use]
    pub fn add_foreign_key_constraint(mut self, constraint: ForeignKeyConstraint) -> Self {
        let result = self.foreign_key_constraints.add(constraint);
        self.check(TableObject::ForeignKeyConstraint, result);
        self
    }

    #[must_use]
    pub fn drop_foreign_key_constraint(mut self, constraint_name: &UnqualifiedName) -> Self {
        let result = self.foreign_key_constraints.remove(constraint_name);
        self.check(TableObject::ForeignKeyConstraint, result);
        self
    }

    #[must_use]
    pub fn drop_foreign_key_constraint_by_unquoted_name(mut self, constraint_name: &str) -> Self {
        match UnqualifiedName::unquoted(constraint_name) {
            Ok(name) => self.drop_foreign_key_constraint(&name),
            Err(e) => {
                self.fail(e);
                self
            }
        }
    }

    #[must_use]
    pub fn set_comment(mut self, comment: &str) -> Self {
        comment.clone_into(&mut self.comment);
        self
    }

    #[must_use]
    pub fn set_options(mut self, options: PhpArray) -> Self {
        self.options = options;
        self
    }

    #[must_use]
    pub fn set_configuration(mut self, configuration: TableConfiguration) -> Self {
        self.configuration = Some(configuration);
        self
    }

    /// `create()`.
    pub fn create(self) -> Result<Table, Exception> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let Some(name) = &self.name else {
            return Err(exception::table_name_not_set());
        };
        if self.columns.is_empty() {
            return Err(exception::table_columns_not_set(name));
        }

        let mut options = self.options.clone();
        if !self.comment.is_empty() {
            options.insert("comment", Value::string(self.comment.clone()));
        }

        let mut table = Table::with_parts(
            &name.to_string(),
            TableParts {
                columns: self.columns.iter().cloned().collect(),
                indexes: self.indexes.iter().cloned().collect(),
                unique_constraints: self.unique_constraints.iter().cloned().collect(),
                fk_constraints: self.foreign_key_constraints.iter().cloned().collect(),
                options,
                configuration: self.configuration,
                primary_key_constraint: self.primary_key_constraint.clone(),
                renamed_columns: self.renamed_columns.clone(),
            },
        )?;

        for editor in self.index_editors {
            editor.add_to_table(&mut table)?;
        }

        Ok(table)
    }
}

fn replace_name(
    names: &[UnqualifiedName],
    old: &UnqualifiedName,
    new: &UnqualifiedName,
) -> (Vec<UnqualifiedName>, bool) {
    let mut modified = false;
    let names = names
        .iter()
        .map(|name| {
            if names_equal(name, old) {
                modified = true;
                new.clone()
            } else {
                name.clone()
            }
        })
        .collect();
    (names, modified)
}
