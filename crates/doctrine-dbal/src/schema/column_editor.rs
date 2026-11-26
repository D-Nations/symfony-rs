//! Port of Doctrine\DBAL\Schema\ColumnEditor.php.
//!
//! The editor is a by-value builder. `setUnquotedName('')` throws in PHP
//! (`InvalidIdentifier`); here the error is kept and returned by
//! `create()`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/ColumnEditor.php

use std::sync::Arc;

use php_value::{PhpArray, Value};

use crate::exception::Exception;
use crate::types::{self, Type};

use super::column::Column;
use super::exception;
use super::name::UnqualifiedName;

#[derive(Debug, Clone)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the properties of ColumnEditor.php"
)]
pub struct ColumnEditor {
    error: Option<Exception>,
    name: Option<UnqualifiedName>,
    type_name: Option<String>,
    length: Option<i64>,
    precision: Option<i64>,
    scale: i64,
    unsigned: bool,
    fixed: bool,
    not_null: bool,
    default_value: Value,
    minimum_value: Value,
    maximum_value: Value,
    enum_type: Option<String>,
    autoincrement: bool,
    comment: String,
    values: PhpArray,
    charset: Option<String>,
    collation: Option<String>,
    default_constraint_name: Option<String>,
    column_definition: Option<String>,
}

impl Default for ColumnEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl ColumnEditor {
    /// `@internal` Use `Column::editor()` or `Column::edit()`.
    pub fn new() -> Self {
        Self {
            error: None,
            name: None,
            type_name: None,
            length: None,
            precision: None,
            scale: 0,
            unsigned: false,
            fixed: false,
            not_null: true,
            default_value: Value::Null,
            minimum_value: Value::Null,
            maximum_value: Value::Null,
            enum_type: None,
            autoincrement: false,
            comment: String::new(),
            values: PhpArray::new(),
            charset: None,
            collation: None,
            default_constraint_name: None,
            column_definition: None,
        }
    }

    #[must_use]
    pub fn set_name(mut self, name: UnqualifiedName) -> Self {
        self.name = Some(name);
        self
    }

    #[must_use]
    pub fn set_unquoted_name(mut self, name: &str) -> Self {
        match UnqualifiedName::unquoted(name) {
            Ok(name) => self.name = Some(name),
            Err(e) => {
                self.error.get_or_insert(e);
            }
        }
        self
    }

    #[must_use]
    pub fn set_quoted_name(mut self, name: &str) -> Self {
        match UnqualifiedName::quoted(name) {
            Ok(name) => self.name = Some(name),
            Err(e) => {
                self.error.get_or_insert(e);
            }
        }
        self
    }

    /// `setType($type)` (deprecated upstream: use `setTypeName()`).
    #[must_use]
    pub fn set_type(mut self, type_: &Arc<dyn Type>) -> Self {
        crate::deprecations::trigger("https://github.com/doctrine/dbal/pull/7490", || {
            "Doctrine\\DBAL\\Schema\\ColumnEditor::setType is deprecated. Use ColumnEditor::setTypeName() instead.".to_owned()
        });
        match types::get_type_registry().lookup_name(type_) {
            Ok(name) => self.type_name = Some(name),
            Err(e) => {
                self.error.get_or_insert(e.into());
            }
        }
        self
    }

    #[must_use]
    pub fn set_type_name(mut self, type_name: &str) -> Self {
        self.type_name = Some(type_name.to_owned());
        self
    }

    #[must_use]
    pub fn set_length(mut self, length: Option<i64>) -> Self {
        self.length = length;
        self
    }

    #[must_use]
    pub fn set_precision(mut self, precision: Option<i64>) -> Self {
        self.precision = precision;
        self
    }

    #[must_use]
    pub fn set_scale(mut self, scale: i64) -> Self {
        self.scale = scale;
        self
    }

    #[must_use]
    pub fn set_unsigned(mut self, unsigned: bool) -> Self {
        self.unsigned = unsigned;
        self
    }

    #[must_use]
    pub fn set_fixed(mut self, fixed: bool) -> Self {
        self.fixed = fixed;
        self
    }

    #[must_use]
    pub fn set_not_null(mut self, not_null: bool) -> Self {
        self.not_null = not_null;
        self
    }

    #[must_use]
    pub fn set_default_value(mut self, default_value: Value) -> Self {
        self.default_value = default_value;
        self
    }

    #[must_use]
    pub fn set_minimum_value(mut self, minimum_value: Value) -> Self {
        self.minimum_value = minimum_value;
        self
    }

    #[must_use]
    pub fn set_maximum_value(mut self, maximum_value: Value) -> Self {
        self.maximum_value = maximum_value;
        self
    }

    #[must_use]
    pub fn set_enum_type(mut self, enum_type: Option<&str>) -> Self {
        self.enum_type = enum_type.map(str::to_owned);
        self
    }

    #[must_use]
    pub fn set_autoincrement(mut self, flag: bool) -> Self {
        self.autoincrement = flag;
        self
    }

    #[must_use]
    pub fn set_comment(mut self, comment: &str) -> Self {
        comment.clone_into(&mut self.comment);
        self
    }

    #[must_use]
    pub fn set_values(mut self, values: PhpArray) -> Self {
        self.values = values;
        self
    }

    #[must_use]
    pub fn set_charset(mut self, charset: Option<&str>) -> Self {
        self.charset = charset.map(str::to_owned);
        self
    }

    #[must_use]
    pub fn set_collation(mut self, collation: Option<&str>) -> Self {
        self.collation = collation.map(str::to_owned);
        self
    }

    #[must_use]
    pub fn set_default_constraint_name(mut self, name: Option<&str>) -> Self {
        self.default_constraint_name = name.map(str::to_owned);
        self
    }

    #[must_use]
    pub fn set_column_definition(mut self, column_definition: Option<&str>) -> Self {
        self.column_definition = column_definition.map(str::to_owned);
        self
    }

    /// `create()`: the column.
    pub fn create(self) -> Result<Column, Exception> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let Some(name) = self.name else {
            return Err(exception::column_name_not_specified());
        };
        let Some(type_name) = self.type_name else {
            return Err(exception::column_data_type_not_specified(&name));
        };

        let mut platform_options = PhpArray::new();
        if let Some(charset) = self.charset {
            platform_options.insert("charset", Value::string(charset));
        }
        if let Some(collation) = self.collation {
            platform_options.insert("collation", Value::string(collation));
        }
        if !self.minimum_value.is_null() {
            platform_options.insert("min", self.minimum_value);
        }
        if !self.maximum_value.is_null() {
            platform_options.insert("max", self.maximum_value);
        }
        if let Some(enum_type) = self.enum_type {
            platform_options.insert("enumType", Value::string(enum_type));
        }
        if let Some(name) = self.default_constraint_name {
            platform_options.insert("default_constraint_name", Value::string(name));
        }

        let optional_int = |v: Option<i64>| v.map_or(Value::Null, Value::Int);
        let mut options = PhpArray::new();
        options.insert("length", optional_int(self.length));
        options.insert("precision", optional_int(self.precision));
        options.insert("scale", Value::Int(self.scale));
        options.insert("unsigned", Value::Bool(self.unsigned));
        options.insert("fixed", Value::Bool(self.fixed));
        options.insert("notnull", Value::Bool(self.not_null));
        options.insert("default", self.default_value);
        options.insert("autoincrement", Value::Bool(self.autoincrement));
        options.insert("comment", Value::string(self.comment));
        options.insert("values", Value::Array(self.values));
        options.insert("platformOptions", Value::Array(platform_options));
        options.insert(
            "columnDefinition",
            self.column_definition.map_or(Value::Null, Value::string),
        );

        Column::new(&name.to_string(), type_name.as_str(), &options)
    }
}
