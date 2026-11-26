//! Port of Doctrine\DBAL\Schema\Table.php: an object representation of a
//! database table.
//!
//! PHP hands out the table's objects by handle; here the getters borrow
//! them and `get_column_mut()` / `get_index_mut()` give the handle a
//! caller changes. The fluent methods return `Result<&mut Self, _>`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Table.php

use std::ops::Deref;

use php_value::{Key, PhpArray, Value};

use crate::deprecations;
use crate::exception::Exception;

use super::abstract_asset::{
    AbstractAsset, AssetKind, NameParser, generate_identifier_name, trim_quotes,
};
use super::array_map::ArrayMap;
use super::collections::{NamedObject, OptionallyNamedObject};
use super::column::{Column, array_merge_into};
use super::exception::{self, invalid_state};
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::index::Index;
use super::name::{Name, OptionallyQualifiedName, UnqualifiedName};
use super::primary_key_constraint::PrimaryKeyConstraint;
use super::schema_config::{SchemaConfig, TableConfiguration};
use super::table_editor::TableEditor;
use super::unique_constraint::UniqueConstraint;

const LINK_EDIT: &str = "https://github.com/doctrine/dbal/pull/7389";

fn edit_deprecated(method: &str, replacement: &str) -> impl FnOnce() -> String {
    let message = format!(
        "Doctrine\\DBAL\\Schema\\Table::{method} is deprecated. Use Table::edit() and TableEditor::{replacement}() instead."
    );
    move || message
}

/// An object representation of a database table.
#[derive(Debug, Clone)]
pub struct Table {
    asset: AbstractAsset,
    columns: ArrayMap<Column>,
    renamed_columns: ArrayMap<String>,
    indexes: ArrayMap<Index>,
    implicit_index_names: ArrayMap<bool>,
    primary_key_name: Option<String>,
    unique_constraints: ArrayMap<UniqueConstraint>,
    fk_constraints: ArrayMap<ForeignKeyConstraint>,
    options: PhpArray,
    schema_config: Option<SchemaConfig>,
    max_identifier_length: i64,
    primary_key_constraint: Option<PrimaryKeyConstraint>,
    failed_to_parse_primary_key_constraint: bool,
}

/// The parts of `new Table(...)` after the name; `Default` gives PHP's
/// default arguments.
#[derive(Debug, Clone, Default)]
pub struct TableParts {
    pub columns: Vec<Column>,
    pub indexes: Vec<Index>,
    pub unique_constraints: Vec<UniqueConstraint>,
    pub fk_constraints: Vec<ForeignKeyConstraint>,
    pub options: PhpArray,
    pub configuration: Option<TableConfiguration>,
    pub primary_key_constraint: Option<PrimaryKeyConstraint>,
    /// New column name => old column name.
    pub renamed_columns: Vec<(String, String)>,
}

/// `preg_match('(([^a-zA-Z0-9_]+))', $name) === 1`.
fn has_invalid_index_name_chars(name: &str) -> bool {
    name.bytes()
        .any(|b| !(b.is_ascii_alphanumeric() || b == b'_'))
}

impl Table {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\Table";

    /// `new Table($name)`.
    pub fn new(name: &str) -> Result<Self, Exception> {
        Self::with_parts(name, TableParts::default())
    }

    /// `new Table($name, $columns, $indexes, $uniqueConstraints,
    /// $fkConstraints, $options, $configuration, $primaryKeyConstraint,
    /// $renamedColumns)`.
    pub fn with_parts(name: &str, parts: TableParts) -> Result<Self, Exception> {
        if name.is_empty() {
            return Err(exception::invalid_table_name(name));
        }
        let asset = AbstractAsset::new(
            Self::CLASS,
            AssetKind::Named,
            NameParser::OptionallyQualified,
            Some(name),
        );
        let configuration = parts
            .configuration
            .unwrap_or_else(|| SchemaConfig::new().to_table_configuration());

        let mut options = PhpArray::new();
        options.insert("create_options", Value::Array(PhpArray::new()));

        let mut table = Self {
            asset,
            columns: ArrayMap::new(),
            renamed_columns: ArrayMap::new(),
            indexes: ArrayMap::new(),
            implicit_index_names: ArrayMap::new(),
            primary_key_name: None,
            unique_constraints: ArrayMap::new(),
            fk_constraints: ArrayMap::new(),
            options,
            schema_config: None,
            max_identifier_length: configuration.get_max_identifier_length(),
            primary_key_constraint: None,
            failed_to_parse_primary_key_constraint: false,
        };

        for column in parts.columns {
            table.add_column_object(column)?;
        }
        for index in parts.indexes {
            table.add_index_object(index)?;
        }
        if let Some(constraint) = parts.primary_key_constraint {
            table.add_primary_key_constraint_inner(constraint)?;
        }
        for constraint in parts.unique_constraints {
            table.add_unique_constraint_object(constraint)?;
        }
        for constraint in parts.fk_constraints {
            table.add_foreign_key_constraint_object(constraint)?;
        }
        array_merge_into(&mut table.options, &parts.options);
        table.renamed_columns = parts.renamed_columns.into_iter().collect();

        Ok(table)
    }

    /// `getObjectName()`.
    pub fn get_object_name(&self) -> Result<OptionallyQualifiedName, Exception> {
        match self.asset.get_object_name()? {
            Some(Name::OptionallyQualified(name)) => Ok(name.clone()),
            _ => Err(invalid_state::object_name_not_initialized()),
        }
    }

    /// `setSchemaConfig($schemaConfig)` (deprecated).
    pub fn set_schema_config(&mut self, schema_config: SchemaConfig) {
        deprecations::trigger_if_called_from_outside(
            "https://github.com/doctrine/dbal/pull/6635",
            || {
                "Doctrine\\DBAL\\Schema\\Table::setSchemaConfig is deprecated. Pass TableConfiguration to the constructor instead.".to_owned()
            },
        );
        self.max_identifier_length = schema_config.get_max_identifier_length();
        self.schema_config = Some(schema_config);
    }

    /// `setPrimaryKey($columnNames, $indexName)` (deprecated).
    pub fn set_primary_key(
        &mut self,
        column_names: &[&str],
        index_name: Option<&str>,
    ) -> Result<&mut Self, Exception> {
        deprecations::trigger_if_called_from_outside(
            "https://github.com/doctrine/dbal/pull/6867",
            || {
                "Doctrine\\DBAL\\Schema\\Table::setPrimaryKey() is deprecated. Use Table::addPrimaryKeyConstraint() instead.".to_owned()
            },
        );
        let column_names: Vec<String> = column_names.iter().map(|s| (*s).to_owned()).collect();
        self.set_primary_key_inner(&column_names, index_name)?;
        Ok(self)
    }

    fn set_primary_key_inner(
        &mut self,
        column_names: &[String],
        index_name: Option<&str>,
    ) -> Result<(), Exception> {
        let index_name = index_name.unwrap_or("primary");
        let index = self.create_index(
            column_names,
            index_name,
            true,
            true,
            Vec::new(),
            PhpArray::new(),
        )?;
        self.add_index_object(index)?;

        let table_name = self.get_name();
        for column_name in column_names {
            let column = self.get_column_mut(column_name)?;
            if !column.get_notnull() {
                deprecations::trigger("https://github.com/doctrine/dbal/pull/6787", || {
                    format!(
                        "Using nullable columns ({table_name}.{column_name}) in a primary key index is deprecated."
                    )
                });
            }
            column.set_notnull(true);
        }
        Ok(())
    }

    /// `addPrimaryKeyConstraint($primaryKeyConstraint)` (deprecated).
    pub fn add_primary_key_constraint(
        &mut self,
        constraint: PrimaryKeyConstraint,
    ) -> Result<&mut Self, Exception> {
        deprecations::trigger_if_called_from_outside(LINK_EDIT, || {
            "Doctrine\\DBAL\\Schema\\Table::addPrimaryKeyConstraint is deprecated. Use Table::edit() and TableEditor::addPrimaryKeyConstraint() instead.".to_owned()
        });
        self.add_primary_key_constraint_inner(constraint)?;
        Ok(self)
    }

    fn add_primary_key_constraint_inner(
        &mut self,
        constraint: PrimaryKeyConstraint,
    ) -> Result<(), Exception> {
        let column_names: Vec<String> = constraint
            .get_column_names()
            .iter()
            .map(ToString::to_string)
            .collect();
        let name = constraint.get_object_name().map(ToString::to_string);
        self.set_primary_key_inner(&column_names, name.as_deref())?;

        // there is no way to set a primary index with flags. we have to
        // set it and then add the flag
        if !constraint.is_clustered()
            && let Some(index) = self.primary_key_index_mut()
        {
            index.add_flag("nonclustered");
        }
        self.primary_key_constraint = Some(constraint);
        Ok(())
    }

    /// `addUniqueConstraint($columnNames, $indexName, $flags, $options)`
    /// (deprecated).
    pub fn add_unique_constraint(
        &mut self,
        column_names: &[&str],
        index_name: Option<&str>,
        flags: Vec<String>,
        options: PhpArray,
    ) -> Result<&mut Self, Exception> {
        deprecations::trigger(
            LINK_EDIT,
            edit_deprecated("addUniqueConstraint", "addUniqueConstraint"),
        );
        let column_names: Vec<String> = column_names.iter().map(|s| (*s).to_owned()).collect();
        let index_name = match index_name {
            Some(name) => name.to_owned(),
            None => self.generate_name(&column_names, "uniq"),
        };
        let constraint =
            self.create_unique_constraint(&column_names, &index_name, flags, options)?;
        self.add_unique_constraint_object(constraint)?;
        Ok(self)
    }

    /// `addIndex($columnNames, $indexName, $flags, $options)`
    /// (deprecated).
    pub fn add_index<C>(
        &mut self,
        column_names: C,
        index_name: Option<&str>,
        flags: Vec<String>,
        options: PhpArray,
    ) -> Result<&mut Self, Exception>
    where
        C: IntoIterator,
        C::Item: Into<String>,
    {
        deprecations::trigger_if_called_from_outside(
            LINK_EDIT,
            edit_deprecated("addIndex", "addIndex"),
        );
        let column_names: Vec<String> = column_names.into_iter().map(Into::into).collect();
        self.add_index_inner(&column_names, index_name, flags, options)?;
        Ok(self)
    }

    pub(crate) fn add_index_inner(
        &mut self,
        column_names: &[String],
        index_name: Option<&str>,
        flags: Vec<String>,
        options: PhpArray,
    ) -> Result<(), Exception> {
        let index_name = match index_name {
            Some(name) => name.to_owned(),
            None => self.generate_name(column_names, "idx"),
        };
        let index = self.create_index(column_names, &index_name, false, false, flags, options)?;
        self.add_index_object(index)
    }

    /// `dropPrimaryKey()` (deprecated).
    pub fn drop_primary_key(&mut self) -> Result<(), Exception> {
        deprecations::trigger_if_called_from_outside(
            LINK_EDIT,
            edit_deprecated("dropPrimaryKey", "dropPrimaryKeyConstraint"),
        );
        self.drop_primary_key_inner()
    }

    fn drop_primary_key_inner(&mut self) -> Result<(), Exception> {
        self.primary_key_constraint = None;
        self.failed_to_parse_primary_key_constraint = false;
        let Some(name) = self.primary_key_name.take() else {
            return Ok(());
        };
        self.drop_index_inner(&name)
    }

    /// `dropIndex($name)` (deprecated).
    pub fn drop_index(&mut self, name: &str) -> Result<(), Exception> {
        deprecations::trigger_if_called_from_outside(
            LINK_EDIT,
            edit_deprecated("dropIndex", "dropIndex"),
        );
        self.drop_index_inner(name)
    }

    fn drop_index_inner(&mut self, name: &str) -> Result<(), Exception> {
        let name = normalize_identifier(name);
        if !self.has_index(&name) {
            return Err(exception::index_does_not_exist(&name, &self.asset.name));
        }
        self.indexes.remove(&name);
        Ok(())
    }

    /// `addUniqueIndex($columnNames, $indexName, $options)` (deprecated).
    pub fn add_unique_index<C>(
        &mut self,
        column_names: C,
        index_name: Option<&str>,
        options: PhpArray,
    ) -> Result<&mut Self, Exception>
    where
        C: IntoIterator,
        C::Item: Into<String>,
    {
        deprecations::trigger_if_called_from_outside(
            LINK_EDIT,
            edit_deprecated("addUniqueIndex", "addIndex"),
        );
        let column_names: Vec<String> = column_names.into_iter().map(Into::into).collect();
        self.add_unique_index_inner(&column_names, index_name, options)?;
        Ok(self)
    }

    pub(crate) fn add_unique_index_inner(
        &mut self,
        column_names: &[String],
        index_name: Option<&str>,
        options: PhpArray,
    ) -> Result<(), Exception> {
        let index_name = match index_name {
            Some(name) => name.to_owned(),
            None => self.generate_name(column_names, "uniq"),
        };
        let index =
            self.create_index(column_names, &index_name, true, false, Vec::new(), options)?;
        self.add_index_object(index)
    }

    /// `renameIndex($oldName, $newName)` (deprecated).
    pub fn rename_index(
        &mut self,
        old_name: &str,
        new_name: Option<&str>,
    ) -> Result<&mut Self, Exception> {
        deprecations::trigger(LINK_EDIT, edit_deprecated("renameIndex", "renameIndex"));
        if !self.has_index(old_name) {
            return Err(exception::index_does_not_exist(old_name, &self.asset.name));
        }
        let normalized_old_name = normalize_identifier(old_name);
        if let Some(new_name) = new_name {
            if normalized_old_name == normalize_identifier(new_name) {
                return Ok(self);
            }
            if self.has_index(new_name) {
                return Err(exception::index_already_exists(new_name, &self.asset.name));
            }
        }

        let Some(old_index) = self.indexes.get(&normalized_old_name).cloned() else {
            return Err(exception::index_does_not_exist(old_name, &self.asset.name));
        };
        if old_index.is_primary() {
            deprecations::trigger_if_called_from_outside(
                "https://github.com/doctrine/dbal/pull/6867",
                || {
                    "Renaming primary key constraint via Doctrine\\DBAL\\Schema\\Table::renameIndex() is deprecated. \
                 Use Table::dropPrimaryKey() and  Table::addPrimaryKeyConstraint() instead."
                    .to_owned()
                },
            );
            self.drop_primary_key_inner()?;
            self.set_primary_key_inner(&old_index.get_columns(), new_name)?;
            return Ok(self);
        }

        self.indexes.remove(&normalized_old_name);
        if old_index.is_unique() {
            self.add_unique_index_inner(
                &old_index.get_columns(),
                new_name,
                old_index.get_options().clone(),
            )?;
            return Ok(self);
        }
        self.add_index_inner(
            &old_index.get_columns(),
            new_name,
            old_index.get_flags(),
            old_index.get_options().clone(),
        )?;
        Ok(self)
    }

    /// `columnsAreIndexed($columnNames)` (deprecated).
    pub fn columns_are_indexed(&self, column_names: &[String]) -> bool {
        deprecations::trigger("https://github.com/doctrine/dbal/pull/6710", || {
            "Doctrine\\DBAL\\Schema\\Table::columnsAreIndexed is deprecated.".to_owned()
        });
        self.indexes
            .values()
            .any(|index| index.spans_columns(column_names))
    }

    /// `addColumn($name, $typeName, $options)` (deprecated).
    pub fn add_column(
        &mut self,
        name: &str,
        type_name: &str,
        options: &PhpArray,
    ) -> Result<&mut Column, Exception> {
        deprecations::trigger(LINK_EDIT, edit_deprecated("addColumn", "addColumn"));
        let column = Column::new(name, type_name, options)?;
        let key = self.add_column_object(column)?;
        self.columns
            .get_mut(&key)
            .ok_or_else(|| exception::column_does_not_exist(name, &self.asset.name))
    }

    /// `getRenamedColumns()`: new column name => old column name.
    pub fn get_renamed_columns(&self) -> &ArrayMap<String> {
        &self.renamed_columns
    }

    /// `renameColumn($oldName, $newName)` (deprecated).
    pub fn rename_column(
        &mut self,
        old_name: &str,
        new_name: &str,
    ) -> Result<&mut Column, Exception> {
        deprecations::trigger(LINK_EDIT, edit_deprecated("renameColumn", "renameColumn"));
        let mut old_name = normalize_identifier(old_name);
        let new_name = normalize_identifier(new_name);

        if old_name == new_name {
            return Err(Exception::new(
                "LogicException",
                format!(
                    "Attempt to rename column \"{}.{old_name}\" to the same name.",
                    self.get_object_name()?
                ),
            ));
        }

        let mut column = self.get_column(&old_name)?.clone();
        column.asset_mut().name.clone_from(&new_name);
        self.columns.remove(&old_name);
        self.add_column_object(column)?;

        self.rename_column_in_indexes(&old_name, &new_name)?;
        self.rename_column_in_foreign_key_constraints(&old_name, &new_name)?;
        self.rename_column_in_unique_constraints(&old_name, &new_name);

        // If a column is renamed multiple times, we only want to know the
        // original and last new name
        if let Some(original) = self.renamed_columns.remove(&old_name) {
            old_name = original;
        }
        if new_name != old_name {
            self.renamed_columns.insert(new_name.clone(), old_name);
        }

        self.get_column_mut(&new_name)
    }

    /// `modifyColumn($name, $options)` (deprecated).
    pub fn modify_column(
        &mut self,
        name: &str,
        options: &PhpArray,
    ) -> Result<&mut Self, Exception> {
        deprecations::trigger(LINK_EDIT, edit_deprecated("modifyColumn", "modifyColumn"));
        self.get_column_mut(name)?.set_options(options)?;
        Ok(self)
    }

    /// `dropColumn($name)` (deprecated).
    pub fn drop_column(&mut self, name: &str) -> &mut Self {
        deprecations::trigger(LINK_EDIT, edit_deprecated("dropColumn", "dropColumn"));
        let name = normalize_identifier(name);

        let foreign_key_constraint_names: Vec<String> = self
            .fk_constraints
            .iter()
            .filter(|(_, c)| c.local_columns().contains(&name))
            .map(|(k, _)| k.to_owned())
            .collect();
        let unique_constraint_names: Vec<String> = self
            .unique_constraints
            .iter()
            .filter(|(_, c)| c.columns().contains(&name))
            .map(|(k, _)| k.to_owned())
            .collect();

        if !foreign_key_constraint_names.is_empty() || !unique_constraint_names.is_empty() {
            let mut constraints = Vec::new();
            if !foreign_key_constraint_names.is_empty() {
                constraints.push(format!(
                    "foreign key constraints: {}",
                    foreign_key_constraint_names.join(", ")
                ));
            }
            if !unique_constraint_names.is_empty() {
                constraints.push(format!(
                    "unique constraints: {}",
                    unique_constraint_names.join(", ")
                ));
            }
            let constraints = constraints.join("; ");
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6559", || {
                format!(
                    "Dropping columns referenced by constraints is deprecated. \
                     Column {name} is used by the following constraints: {constraints} "
                )
            });
        }

        self.columns.remove(&name);
        self
    }

    /// `addForeignKeyConstraint($foreignTableName, $localColumnNames,
    /// $foreignColumnNames, $options, $name)` (deprecated).
    pub fn add_foreign_key_constraint(
        &mut self,
        foreign_table_name: &str,
        local_column_names: &[&str],
        foreign_column_names: &[&str],
        options: PhpArray,
        name: Option<&str>,
    ) -> Result<&mut Self, Exception> {
        deprecations::trigger(
            LINK_EDIT,
            edit_deprecated("addForeignKeyConstraint", "addForeignKeyConstraint"),
        );
        let local: Vec<String> = local_column_names.iter().map(|s| (*s).to_owned()).collect();
        let name = match name {
            Some(name) => name.to_owned(),
            None => self.generate_name(&local, "fk"),
        };
        for column_name in &local {
            if !self.has_column(column_name) {
                return Err(exception::column_does_not_exist(
                    column_name,
                    &self.asset.name,
                ));
            }
        }
        let constraint = ForeignKeyConstraint::new(
            local,
            foreign_table_name,
            foreign_column_names.iter().copied(),
            &name,
            options,
        )?;
        self.add_foreign_key_constraint_object(constraint)?;
        Ok(self)
    }

    /// `addOption($name, $value)` (deprecated).
    pub fn add_option(&mut self, name: &str, value: Value) -> &mut Self {
        deprecations::trigger_if_called_from_outside(
            LINK_EDIT,
            edit_deprecated("addOption", "setOptions"),
        );
        self.add_option_inner(name, value);
        self
    }

    pub(crate) fn add_option_inner(&mut self, name: &str, value: Value) {
        self.options.insert(name, value);
    }

    pub fn has_foreign_key(&self, name: &str) -> bool {
        self.fk_constraints
            .contains_key(&normalize_identifier(name))
    }

    pub fn get_foreign_key(&self, name: &str) -> Result<&ForeignKeyConstraint, Exception> {
        let name = normalize_identifier(name);
        self.fk_constraints
            .get(&name)
            .ok_or_else(|| exception::foreign_key_does_not_exist(&name, &self.asset.name))
    }

    /// `removeForeignKey($name)` (deprecated).
    pub fn remove_foreign_key(&mut self, name: &str) -> Result<(), Exception> {
        deprecations::trigger("https://github.com/doctrine/dbal/pull/6560", || {
            "Table::removeForeignKey() is deprecated. Use Table::dropForeignKey() instead."
                .to_owned()
        });
        self.drop_foreign_key_inner(name)
    }

    /// `dropForeignKey($name)` (deprecated).
    pub fn drop_foreign_key(&mut self, name: &str) -> Result<(), Exception> {
        deprecations::trigger_if_called_from_outside(
            LINK_EDIT,
            edit_deprecated("dropForeignKey", "dropForeignKeyConstraint"),
        );
        self.drop_foreign_key_inner(name)
    }

    fn drop_foreign_key_inner(&mut self, name: &str) -> Result<(), Exception> {
        let name = normalize_identifier(name);
        if self.fk_constraints.remove(&name).is_none() {
            return Err(exception::foreign_key_does_not_exist(
                &name,
                &self.asset.name,
            ));
        }
        Ok(())
    }

    pub fn has_unique_constraint(&self, name: &str) -> bool {
        self.unique_constraints
            .contains_key(&normalize_identifier(name))
    }

    pub fn get_unique_constraint(&self, name: &str) -> Result<&UniqueConstraint, Exception> {
        let name = normalize_identifier(name);
        self.unique_constraints
            .get(&name)
            .ok_or_else(|| exception::unique_constraint_does_not_exist(&name, &self.asset.name))
    }

    /// `removeUniqueConstraint($name)` (deprecated).
    pub fn remove_unique_constraint(&mut self, name: &str) -> Result<(), Exception> {
        deprecations::trigger("https://github.com/doctrine/dbal/pull/6560", || {
            "Table::removeUniqueConstraint() is deprecated. Use Table::dropUniqueConstraint() instead."
                .to_owned()
        });
        self.drop_unique_constraint_inner(name)
    }

    /// `dropUniqueConstraint($name)` (deprecated).
    pub fn drop_unique_constraint(&mut self, name: &str) -> Result<(), Exception> {
        deprecations::trigger_if_called_from_outside(
            LINK_EDIT,
            edit_deprecated("dropUniqueConstraint", "dropUniqueConstraint"),
        );
        self.drop_unique_constraint_inner(name)
    }

    fn drop_unique_constraint_inner(&mut self, name: &str) -> Result<(), Exception> {
        let name = normalize_identifier(name);
        if self.unique_constraints.remove(&name).is_none() {
            return Err(exception::unique_constraint_does_not_exist(
                &name,
                &self.asset.name,
            ));
        }
        Ok(())
    }

    /// `getColumns()`.
    pub fn get_columns(&self) -> Vec<&Column> {
        self.columns.values().collect()
    }

    pub fn has_column(&self, name: &str) -> bool {
        self.columns.contains_key(&normalize_identifier(name))
    }

    pub fn get_column(&self, name: &str) -> Result<&Column, Exception> {
        let name = normalize_identifier(name);
        self.columns
            .get(&name)
            .ok_or_else(|| exception::column_does_not_exist(&name, &self.asset.name))
    }

    /// `getColumn($name)` for a caller that changes the column.
    pub fn get_column_mut(&mut self, name: &str) -> Result<&mut Column, Exception> {
        let name = normalize_identifier(name);
        let table_name = self.asset.name.clone();
        self.columns
            .get_mut(&name)
            .ok_or_else(|| exception::column_does_not_exist(&name, &table_name))
    }

    /// `getPrimaryKey()` (deprecated).
    pub fn get_primary_key(&self) -> Option<&Index> {
        deprecations::trigger_if_called_from_outside(
            "https://github.com/doctrine/dbal/pull/6867",
            || {
                "Doctrine\\DBAL\\Schema\\Table::getPrimaryKey() is deprecated. Use Table::getPrimaryKeyConstraint() instead.".to_owned()
            },
        );
        self.primary_key_index()
    }

    pub(crate) fn primary_key_index(&self) -> Option<&Index> {
        self.primary_key_name
            .as_ref()
            .and_then(|name| self.indexes.get(name))
    }

    fn primary_key_index_mut(&mut self) -> Option<&mut Index> {
        let name = self.primary_key_name.clone()?;
        self.indexes.get_mut(&name)
    }

    /// `getPrimaryKeyConstraint()`.
    pub fn get_primary_key_constraint(&self) -> Result<Option<&PrimaryKeyConstraint>, Exception> {
        if self.failed_to_parse_primary_key_constraint {
            return Err(invalid_state::table_has_invalid_primary_key_constraint(
                &self.get_name(),
            ));
        }
        Ok(self.primary_key_constraint.as_ref())
    }

    pub fn has_index(&self, name: &str) -> bool {
        self.indexes.contains_key(&normalize_identifier(name))
    }

    pub fn get_index(&self, name: &str) -> Result<&Index, Exception> {
        let name = normalize_identifier(name);
        self.indexes
            .get(&name)
            .ok_or_else(|| exception::index_does_not_exist(&name, &self.asset.name))
    }

    /// `getIndex($name)` for a caller that changes the index.
    pub fn get_index_mut(&mut self, name: &str) -> Result<&mut Index, Exception> {
        let name = normalize_identifier(name);
        let table_name = self.asset.name.clone();
        self.indexes
            .get_mut(&name)
            .ok_or_else(|| exception::index_does_not_exist(&name, &table_name))
    }

    /// `getIndexes()`, by normalized name.
    pub fn get_indexes(&self) -> &ArrayMap<Index> {
        &self.indexes
    }

    pub fn get_unique_constraints(&self) -> &ArrayMap<UniqueConstraint> {
        &self.unique_constraints
    }

    pub fn get_foreign_keys(&self) -> &ArrayMap<ForeignKeyConstraint> {
        &self.fk_constraints
    }

    /// `hasOption($name)`: `isset()`.
    pub fn has_option(&self, name: &str) -> bool {
        !matches!(self.options.get(&Key::from(name)), None | Some(Value::Null))
    }

    /// `getOption($name)`.
    pub fn get_option(&self, name: &str) -> Value {
        self.options
            .get(&Key::from(name))
            .cloned()
            .unwrap_or(Value::Null)
    }

    pub fn get_options(&self) -> &PhpArray {
        &self.options
    }

    /// `_getMaxIdentifierLength()` (deprecated).
    pub fn get_max_identifier_length(&self) -> i64 {
        deprecations::trigger_if_called_from_outside(
            "https://github.com/doctrine/dbal/pull/6635",
            || "Doctrine\\DBAL\\Schema\\Table::_getMaxIdentifierLength is deprecated.".to_owned(),
        );
        self.max_identifier_length
    }

    fn generate_name(&self, column_names: &[String], prefix: &str) -> String {
        let mut names = vec![self.get_name()];
        names.extend_from_slice(column_names);
        generate_identifier_name(
            &names,
            prefix,
            usize::try_from(self.max_identifier_length).unwrap_or(0),
        )
    }

    /// `_addColumn($column)`: the key the column is stored under.
    fn add_column_object(&mut self, column: Column) -> Result<String, Exception> {
        let column_name = normalize_identifier(&column.get_name());
        if self.columns.contains_key(&column_name) {
            return Err(exception::column_already_exists(
                &self.get_name(),
                &column_name,
            ));
        }
        self.columns.insert(column_name.clone(), column);
        Ok(column_name)
    }

    /// `_addIndex($index)`.
    fn add_index_object(&mut self, index: Index) -> Result<(), Exception> {
        let index_name = normalize_identifier(&index.get_name());

        let replaced_implicit_index_names: Vec<String> = self
            .implicit_index_names
            .keys()
            .filter(|name| {
                self.indexes
                    .get(name)
                    .is_some_and(|implicit| implicit.is_fulfilled_by(&index))
            })
            .map(str::to_owned)
            .collect();

        if self.primary_key_name.is_some() && index.is_primary() {
            return Err(exception::primary_key_already_exists(&self.asset.name));
        }

        if self.indexes.contains_key(&index_name)
            && !replaced_implicit_index_names.contains(&index_name)
        {
            return Err(exception::index_already_exists(
                &index_name,
                &self.asset.name,
            ));
        }

        for name in &replaced_implicit_index_names {
            self.indexes.remove(name);
            self.implicit_index_names.remove(name);
        }

        if index.is_primary() {
            self.primary_key_name = Some(index_name.clone());
            match parse_primary_key_constraint(&index) {
                Ok(constraint) => {
                    self.primary_key_constraint = constraint;
                    self.failed_to_parse_primary_key_constraint = false;
                }
                Err(e) if e.class() == exception::INVALID_STATE => {
                    self.primary_key_constraint = None;
                    self.failed_to_parse_primary_key_constraint = true;
                }
                Err(e) => return Err(e),
            }
        }

        self.indexes.insert(index_name, index);
        Ok(())
    }

    /// `_addUniqueConstraint($constraint)`.
    fn add_unique_constraint_object(
        &mut self,
        constraint: UniqueConstraint,
    ) -> Result<(), Exception> {
        let columns = constraint.columns();
        let name = if constraint.get_name().is_empty() {
            self.generate_name(&columns, "fk")
        } else {
            constraint.get_name()
        };
        let name = normalize_identifier(&name);
        self.unique_constraints.insert(name, constraint);

        // If there is already an index that fulfills this requirements
        // drop the request. In the case of __construct calling this
        // method during hydration from schema-details all the explicitly
        // added indexes lead to duplicates. This creates computation
        // overhead in this case, however no duplicate indexes are ever
        // added (column based).
        let index_name = self.generate_name(&columns, "idx");
        let index_candidate = self.create_index(
            &columns,
            &index_name,
            true,
            false,
            Vec::new(),
            PhpArray::new(),
        )?;
        if self
            .indexes
            .values()
            .any(|existing| index_candidate.is_fulfilled_by(existing))
        {
            return Ok(());
        }
        self.implicit_index_names
            .insert(normalize_identifier(&index_name), true);
        Ok(())
    }

    /// `_addForeignKeyConstraint($constraint)`.
    fn add_foreign_key_constraint_object(
        &mut self,
        constraint: ForeignKeyConstraint,
    ) -> Result<(), Exception> {
        let local_columns = constraint.local_columns();
        let name = if constraint.get_name().is_empty() {
            self.generate_name(&local_columns, "fk")
        } else {
            constraint.get_name()
        };
        let name = normalize_identifier(&name);
        if self.fk_constraints.contains_key(&name) {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/7125", || {
                format!(
                    "Overwriting an existing foreign key constraint (\"{name}\") is deprecated."
                )
            });
        }
        self.fk_constraints.insert(name, constraint);

        // add an explicit index on the foreign key columns. If there is
        // already an index that fulfills this requirements drop the
        // request (see _addUniqueConstraint()).
        let index_name = self.generate_name(&local_columns, "idx");
        let index_candidate = self.create_index(
            &local_columns,
            &index_name,
            false,
            false,
            Vec::new(),
            PhpArray::new(),
        )?;
        if self
            .indexes
            .values()
            .any(|existing| index_candidate.is_fulfilled_by(existing))
        {
            return Ok(());
        }
        self.add_index_object(index_candidate)?;
        self.implicit_index_names
            .insert(normalize_identifier(&index_name), true);
        Ok(())
    }

    /// `setComment($comment)` (deprecated).
    pub fn set_comment(&mut self, comment: &str) -> &mut Self {
        deprecations::trigger(LINK_EDIT, edit_deprecated("setComment", "setComment"));
        // For keeping backward compatibility with MySQL in previous
        // releases, table comments are stored as options.
        self.options.insert("comment", Value::string(comment));
        self
    }

    /// `getComment()`.
    pub fn get_comment(&self) -> Option<String> {
        match self.options.get(&Key::from("comment")) {
            Some(Value::String(s)) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        }
    }

    /// `Table::editor()`.
    pub fn editor() -> TableEditor {
        TableEditor::new()
    }

    /// `edit()`.
    pub fn edit(&self) -> Result<TableEditor, Exception> {
        let mut options = self.options.clone();
        let comment = options.remove(&Key::from("comment"));

        let indexes: Vec<Index> = self
            .indexes
            .iter()
            .filter(|(k, _)| !self.implicit_index_names.contains_key(k))
            .map(|(_, index)| index.clone())
            .collect();

        let mut editor = Self::editor()
            .set_name(self.get_object_name()?)
            .set_columns(self.columns.values().cloned().collect())
            .set_indexes(indexes)
            .set_primary_key_constraint(self.primary_key_constraint.clone())
            .set_unique_constraints(self.unique_constraints.values().cloned().collect())
            .set_foreign_key_constraints(self.fk_constraints.values().cloned().collect());

        match comment {
            None | Some(Value::Null) => {}
            Some(Value::String(comment)) => {
                editor = editor.set_comment(&comment.to_string_lossy());
            }
            Some(other) => {
                return Err(Exception::new(
                    Exception::TYPE_ERROR,
                    format!(
                        "Doctrine\\DBAL\\Schema\\TableEditor::setComment(): Argument #1 ($comment) must be of type string, {} given",
                        other.value_name()
                    ),
                ));
            }
        }

        Ok(editor
            .set_options(options)
            .set_configuration(TableConfiguration::new(self.max_identifier_length)))
    }

    fn create_unique_constraint(
        &self,
        columns: &[String],
        index_name: &str,
        flags: Vec<String>,
        options: PhpArray,
    ) -> Result<UniqueConstraint, Exception> {
        if has_invalid_index_name_chars(&normalize_identifier(index_name)) {
            return Err(exception::index_name_invalid(index_name));
        }
        for column_name in columns {
            if !self.has_column(column_name) {
                return Err(exception::column_does_not_exist(
                    column_name,
                    &self.asset.name,
                ));
            }
        }
        Ok(UniqueConstraint::new(
            index_name,
            columns.iter().cloned(),
            flags,
            options,
        ))
    }

    fn create_index(
        &self,
        columns: &[String],
        index_name: &str,
        is_unique: bool,
        is_primary: bool,
        flags: Vec<String>,
        options: PhpArray,
    ) -> Result<Index, Exception> {
        if has_invalid_index_name_chars(&normalize_identifier(index_name)) {
            return Err(exception::index_name_invalid(index_name));
        }
        for column_name in columns {
            if !self.has_column(column_name) {
                return Err(exception::column_does_not_exist(
                    column_name,
                    &self.asset.name,
                ));
            }
        }
        Index::new(
            Some(index_name),
            columns.iter().cloned(),
            is_unique,
            is_primary,
            flags,
            options,
        )
    }

    fn rename_column_in_indexes(
        &mut self,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), Exception> {
        let keys: Vec<String> = self.indexes.keys().map(str::to_owned).collect();
        for key in keys {
            let Some(index) = self.indexes.get(&key) else {
                continue;
            };
            let columns = index.get_columns();
            if !columns.iter().any(|c| c == old_name) {
                continue;
            }
            let columns: Vec<String> = columns
                .into_iter()
                .map(|c| {
                    if c == old_name {
                        new_name.to_owned()
                    } else {
                        c
                    }
                })
                .collect();
            let renamed = Index::new(
                Some(&index.get_name()),
                columns,
                index.is_unique(),
                index.is_primary(),
                index.get_flags(),
                index.get_options().clone(),
            )?;
            self.indexes.insert(key, renamed);
        }
        Ok(())
    }

    fn rename_column_in_foreign_key_constraints(
        &mut self,
        old_name: &str,
        new_name: &str,
    ) -> Result<(), Exception> {
        let keys: Vec<String> = self.fk_constraints.keys().map(str::to_owned).collect();
        for key in keys {
            let Some(constraint) = self.fk_constraints.get(&key) else {
                continue;
            };
            let local_columns = constraint.local_columns();
            if !local_columns.iter().any(|c| c == old_name) {
                continue;
            }
            let local_columns: Vec<String> = local_columns
                .into_iter()
                .map(|c| {
                    if c == old_name {
                        new_name.to_owned()
                    } else {
                        c
                    }
                })
                .collect();
            let renamed = ForeignKeyConstraint::new(
                local_columns,
                &constraint.foreign_table_name(),
                constraint.foreign_columns(),
                &constraint.get_name(),
                constraint.options().clone(),
            )?;
            self.fk_constraints.insert(key, renamed);
        }
        Ok(())
    }

    fn rename_column_in_unique_constraints(&mut self, old_name: &str, new_name: &str) {
        let keys: Vec<String> = self.unique_constraints.keys().map(str::to_owned).collect();
        for key in keys {
            let Some(constraint) = self.unique_constraints.get(&key) else {
                continue;
            };
            let columns = constraint.columns();
            if !columns.iter().any(|c| c == old_name) {
                continue;
            }
            let columns: Vec<String> = columns
                .into_iter()
                .map(|c| {
                    if c == old_name {
                        new_name.to_owned()
                    } else {
                        c
                    }
                })
                .collect();
            let renamed = UniqueConstraint::new(
                &constraint.get_name(),
                columns,
                constraint.flags(),
                constraint.options().clone(),
            );
            self.unique_constraints.insert(key, renamed);
        }
    }
}

impl Deref for Table {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}

/// `normalizeIdentifier()`: `trimQuotes(strtolower($identifier))`.
fn normalize_identifier(identifier: &str) -> String {
    trim_quotes(&identifier.to_ascii_lowercase())
}

/// `parsePrimaryKeyConstraint($index)`.
fn parse_primary_key_constraint(index: &Index) -> Result<Option<PrimaryKeyConstraint>, Exception> {
    let indexed_columns = index.get_indexed_columns()?;
    let mut column_names = Vec::new();
    for indexed_column in indexed_columns {
        if indexed_column.get_length().is_some() {
            return Ok(None);
        }
        column_names.push(indexed_column.get_column_name().clone());
    }
    // Do not derive the constraint name from the index name in the
    // upgrade path. The primary index name defaults to "PRIMARY", while
    // the default constraint name is null (unspecified, to be generated
    // by the database platform).
    PrimaryKeyConstraint::new(None, column_names, !index.has_flag("nonclustered")).map(Some)
}

impl NamedObject for Column {
    fn named_object_name(&self) -> Result<UnqualifiedName, Exception> {
        self.get_object_name()
    }
}

impl NamedObject for Index {
    fn named_object_name(&self) -> Result<UnqualifiedName, Exception> {
        self.get_object_name()
    }
}

impl OptionallyNamedObject for UniqueConstraint {
    fn optional_object_name(&self) -> Result<Option<UnqualifiedName>, Exception> {
        self.get_object_name()
    }
}

impl OptionallyNamedObject for ForeignKeyConstraint {
    fn optional_object_name(&self) -> Result<Option<UnqualifiedName>, Exception> {
        self.get_object_name()
    }
}
