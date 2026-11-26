//! Port of Doctrine\DBAL\Schema\ColumnDiff.php: the changes of a column.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/ColumnDiff.php

use php_value::Value;
use php_value::compare::loose_equals;

use super::column::Column;

/// Represents the change of a column.
#[derive(Debug, Clone)]
pub struct ColumnDiff {
    old_column: Column,
    new_column: Column,
}

impl ColumnDiff {
    /// `@internal` The diff can be only instantiated by a `Comparator`.
    pub fn new(old_column: Column, new_column: Column) -> Self {
        Self {
            old_column,
            new_column,
        }
    }

    pub fn count_changed_properties(&self) -> usize {
        [
            self.has_unsigned_changed(),
            self.has_auto_increment_changed(),
            self.has_default_changed(),
            self.has_fixed_changed(),
            self.has_precision_changed(),
            self.has_scale_changed(),
            self.has_length_changed(),
            self.has_not_null_changed(),
            self.has_name_changed(),
            self.has_type_changed(),
            self.has_platform_options_changed(),
            self.has_comment_changed(),
        ]
        .into_iter()
        .filter(|changed| *changed)
        .count()
    }

    pub fn get_old_column(&self) -> &Column {
        &self.old_column
    }

    pub fn get_new_column(&self) -> &Column {
        &self.new_column
    }

    /// Column names are case insensitive (`strcasecmp()`).
    pub fn has_name_changed(&self) -> bool {
        !self
            .old_column
            .get_name()
            .eq_ignore_ascii_case(&self.new_column.get_name())
    }

    pub fn has_type_changed(&self) -> bool {
        self.new_column.get_type_name() != self.old_column.get_type_name()
    }

    pub fn has_length_changed(&self) -> bool {
        self.new_column.get_length() != self.old_column.get_length()
    }

    pub fn has_precision_changed(&self) -> bool {
        self.new_column.get_precision() != self.old_column.get_precision()
    }

    pub fn has_scale_changed(&self) -> bool {
        self.new_column.get_scale() != self.old_column.get_scale()
    }

    pub fn has_unsigned_changed(&self) -> bool {
        self.new_column.get_unsigned() != self.old_column.get_unsigned()
    }

    pub fn has_fixed_changed(&self) -> bool {
        self.new_column.get_fixed() != self.old_column.get_fixed()
    }

    pub fn has_not_null_changed(&self) -> bool {
        self.new_column.get_notnull() != self.old_column.get_notnull()
    }

    /// Null values tell whether to create or drop a default value; other
    /// defaults are compared loosely (`!=`).
    pub fn has_default_changed(&self) -> bool {
        let old_default = self.old_column.get_default();
        let new_default = self.new_column.get_default();
        if new_default.is_null() != old_default.is_null() {
            return true;
        }
        !loose_equals(new_default, old_default)
    }

    pub fn has_auto_increment_changed(&self) -> bool {
        self.new_column.get_autoincrement() != self.old_column.get_autoincrement()
    }

    pub fn has_comment_changed(&self) -> bool {
        self.new_column.get_comment() != self.old_column.get_comment()
    }

    pub fn has_platform_options_changed(&self) -> bool {
        !Value::Array(self.new_column.get_platform_options().clone()).identical(&Value::Array(
            self.old_column.get_platform_options().clone(),
        ))
    }
}
