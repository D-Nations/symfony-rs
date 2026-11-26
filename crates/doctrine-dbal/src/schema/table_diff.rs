//! Port of Doctrine\DBAL\Schema\TableDiff.php (table changes) and
//! IndexRename.php.
//!
//! PHP's lists are `Vec`s; `$renamedIndexes` (old name => new index)
//! is a list of pairs in order. `unsetAddedIndex()` and
//! `unsetDroppedIndex()` compare indexes by value: PHP's `!==` compares
//! object identity, and equal but distinct indexes are not told apart.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/TableDiff.php src/Schema/IndexRename.php

use crate::deprecations;
use crate::exception::Exception;

use super::column::Column;
use super::column_diff::ColumnDiff;
use super::exception::invalid_state;
use super::foreign_key_constraint::ForeignKeyConstraint;
use super::index::Index;
use super::name::UnqualifiedName;
use super::table::Table;

/// The arguments of `new TableDiff()` after `$oldTable`; `Default` gives
/// PHP's default arguments.
#[derive(Debug, Clone, Default)]
pub struct TableDiffParts {
    pub added_columns: Vec<Column>,
    pub changed_columns: Vec<ColumnDiff>,
    pub dropped_columns: Vec<Column>,
    pub added_indexes: Vec<Index>,
    pub modified_indexes: Vec<Index>,
    pub dropped_indexes: Vec<Index>,
    pub renamed_indexes: Vec<(String, Index)>,
    pub added_foreign_keys: Vec<ForeignKeyConstraint>,
    pub modified_foreign_keys: Vec<ForeignKeyConstraint>,
    pub dropped_foreign_keys: Vec<ForeignKeyConstraint>,
}

/// Table Diff.
#[derive(Debug, Clone)]
pub struct TableDiff {
    old_table: Table,
    parts: TableDiffParts,
}

/// `IndexRename`: the rename of an index.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexRename {
    old_name: UnqualifiedName,
    new_index: Index,
}

impl IndexRename {
    pub fn new(old_name: UnqualifiedName, new_index: Index) -> Self {
        Self { old_name, new_index }
    }

    pub fn get_old_name(&self) -> &UnqualifiedName {
        &self.old_name
    }

    pub fn get_new_index(&self) -> &Index {
        &self.new_index
    }
}

const METHOD: &str = "Doctrine\\DBAL\\Schema\\TableDiff";

impl TableDiff {
    /// `@internal` The diff can be only instantiated by a `Comparator`.
    pub fn new(old_table: Table, parts: TableDiffParts) -> Self {
        if !parts.modified_indexes.is_empty() {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6831", || {
                format!(
                    "Passing a non-empty $modifiedIndexes value to {METHOD}::__construct() is deprecated. \
                     Instead, pass dropped indexes via $droppedIndexes and added indexes via $addedIndexes."
                )
            });
        }
        if parts
            .dropped_foreign_keys
            .iter()
            .any(|fk| fk.get_name().is_empty())
        {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/7143", || {
                "Dropping a foreign key constraints without specifying its name is deprecated.".to_owned()
            });
        }
        if !parts.modified_foreign_keys.is_empty() {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6827", || {
                format!(
                    "Passing a non-empty $modifiedForeignKeys value to {METHOD}::__construct() is deprecated. \
                     Instead, pass dropped constraints via $droppedForeignKeys and added constraints via $addedForeignKeys."
                )
            });
        }
        Self { old_table, parts }
    }

    pub fn get_old_table(&self) -> &Table {
        &self.old_table
    }

    pub fn get_added_columns(&self) -> &[Column] {
        &self.parts.added_columns
    }

    pub fn get_changed_columns(&self) -> &[ColumnDiff] {
        &self.parts.changed_columns
    }

    /// `getModifiedColumns()` (deprecated).
    pub fn get_modified_columns(&self) -> Vec<&ColumnDiff> {
        deprecations::trigger_if_called_from_outside("https://github.com/doctrine/dbal/pull/6280", || {
            format!("{METHOD}::getModifiedColumns is deprecated, use `getChangedColumns()` instead.")
        });
        self.parts
            .changed_columns
            .iter()
            .filter(|diff| diff.count_changed_properties() > usize::from(diff.has_name_changed()))
            .collect()
    }

    /// `getRenamedColumns()` (deprecated): old column name => new column.
    pub fn get_renamed_columns(&self) -> Vec<(String, &Column)> {
        deprecations::trigger_if_called_from_outside("https://github.com/doctrine/dbal/pull/6280", || {
            format!("{METHOD}::getRenamedColumns is deprecated, you should use `getChangedColumns()` instead.")
        });
        let mut renamed: Vec<(String, &Column)> = Vec::new();
        for diff in &self.parts.changed_columns {
            if !diff.has_name_changed() {
                continue;
            }
            let old_column_name = diff.get_old_column().get_name();
            match renamed.iter_mut().find(|(k, _)| *k == old_column_name) {
                Some(entry) => entry.1 = diff.get_new_column(),
                None => renamed.push((old_column_name, diff.get_new_column())),
            }
        }
        renamed
    }

    pub fn get_dropped_columns(&self) -> &[Column] {
        &self.parts.dropped_columns
    }

    pub fn get_added_indexes(&self) -> &[Index] {
        &self.parts.added_indexes
    }

    pub fn unset_added_index(&mut self, index: &Index) {
        self.parts.added_indexes.retain(|added| added != index);
    }

    /// `getModifiedIndexes()` (deprecated).
    pub fn get_modified_indexes(&self) -> &[Index] {
        deprecations::trigger_if_called_from_outside("https://github.com/doctrine/dbal/pull/6831", || {
            format!("{METHOD}::getModifiedIndexes() is deprecated, use getAddedIndexes() and getDroppedIndexes() instead.")
        });
        &self.parts.modified_indexes
    }

    pub(crate) fn modified_indexes(&self) -> &[Index] {
        &self.parts.modified_indexes
    }

    pub fn get_dropped_indexes(&self) -> &[Index] {
        &self.parts.dropped_indexes
    }

    pub fn unset_dropped_index(&mut self, index: &Index) {
        self.parts.dropped_indexes.retain(|dropped| dropped != index);
    }

    /// `getRenamedIndexes()` (deprecated): old name => new index.
    pub fn get_renamed_indexes(&self) -> &[(String, Index)] {
        deprecations::trigger_if_called_from_outside("https://github.com/doctrine/dbal/pull/7368", || {
            format!("{METHOD}::getRenamedIndexes() is deprecated, use getIndexRenames() instead.")
        });
        &self.parts.renamed_indexes
    }

    pub(crate) fn renamed_indexes(&self) -> &[(String, Index)] {
        &self.parts.renamed_indexes
    }

    /// `getIndexRenames()`.
    pub fn get_index_renames(&self) -> Result<Vec<IndexRename>, Exception> {
        self.parts
            .renamed_indexes
            .iter()
            .map(|(old_name, new_index)| {
                Ok(IndexRename::new(UnqualifiedName::unquoted(old_name)?, new_index.clone()))
            })
            .collect()
    }

    pub fn get_added_foreign_keys(&self) -> &[ForeignKeyConstraint] {
        &self.parts.added_foreign_keys
    }

    /// `getModifiedForeignKeys()` (deprecated).
    pub fn get_modified_foreign_keys(&self) -> &[ForeignKeyConstraint] {
        deprecations::trigger_if_called_from_outside("https://github.com/doctrine/dbal/pull/6827", || {
            format!(
                "{METHOD}::getModifiedForeignKeys() is deprecated, use getDroppedForeignKeys() and getAddedForeignKeys() instead."
            )
        });
        &self.parts.modified_foreign_keys
    }

    pub(crate) fn modified_foreign_keys(&self) -> &[ForeignKeyConstraint] {
        &self.parts.modified_foreign_keys
    }

    pub fn get_dropped_foreign_keys(&self) -> &[ForeignKeyConstraint] {
        &self.parts.dropped_foreign_keys
    }

    /// `getDroppedForeignKeyConstraintNames()`.
    pub fn get_dropped_foreign_key_constraint_names(&self) -> Result<Vec<UnqualifiedName>, Exception> {
        self.parts
            .dropped_foreign_keys
            .iter()
            .map(|constraint| {
                constraint.get_object_name()?.ok_or_else(
                    invalid_state::table_diff_contains_unnamed_dropped_foreign_key_constraints,
                )
            })
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        let p = &self.parts;
        p.added_columns.is_empty()
            && p.changed_columns.is_empty()
            && p.dropped_columns.is_empty()
            && p.added_indexes.is_empty()
            && p.modified_indexes.is_empty()
            && p.dropped_indexes.is_empty()
            && p.renamed_indexes.is_empty()
            && p.added_foreign_keys.is_empty()
            && p.modified_foreign_keys.is_empty()
            && p.dropped_foreign_keys.is_empty()
    }
}
