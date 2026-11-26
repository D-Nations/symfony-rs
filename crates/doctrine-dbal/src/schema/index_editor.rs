//! Port of Doctrine\DBAL\Schema\IndexEditor.php.
//!
//! The editor is a by-value builder; a name that `setUnquotedName()` or
//! `addUnquotedColumnName()` cannot take (PHP throws at once) is kept and
//! returned by `create()` or `add_to_table()`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/IndexEditor.php

use php_value::{PhpArray, Value};

use crate::exception::Exception;

use super::exception;
use super::index::{Index, IndexType, IndexedColumn};
use super::name::UnqualifiedName;
use super::table::Table;

#[derive(Debug, Clone)]
pub struct IndexEditor {
    error: Option<Exception>,
    name: Option<UnqualifiedName>,
    type_: IndexType,
    columns: Vec<IndexedColumn>,
    is_clustered: bool,
    predicate: Option<String>,
}

impl Default for IndexEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexEditor {
    /// `@internal` Use `Index::editor()` or `Index::edit()`.
    pub fn new() -> Self {
        Self {
            error: None,
            name: None,
            type_: IndexType::Regular,
            columns: Vec::new(),
            is_clustered: false,
            predicate: None,
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
    pub fn set_type(mut self, type_: IndexType) -> Self {
        self.type_ = type_;
        self
    }

    /// `setColumns($firstColumn, ...$otherColumns)`; the first column is
    /// required by the PHP signature, here an empty list is taken as is.
    #[must_use]
    pub fn set_columns(mut self, columns: Vec<IndexedColumn>) -> Self {
        self.columns = columns;
        self
    }

    #[must_use]
    pub fn add_column(mut self, column: IndexedColumn) -> Self {
        self.columns.push(column);
        self
    }

    #[must_use]
    pub fn add_unquoted_column_name(mut self, name: &str, length: Option<i64>) -> Self {
        let column = UnqualifiedName::unquoted(name).and_then(|n| IndexedColumn::new(n, length));
        if let Some(column) = self.keep(column) {
            self.columns.push(column);
        }
        self
    }

    #[must_use]
    pub fn add_quoted_column_name(mut self, name: &str, length: Option<i64>) -> Self {
        let column = UnqualifiedName::quoted(name).and_then(|n| IndexedColumn::new(n, length));
        if let Some(column) = self.keep(column) {
            self.columns.push(column);
        }
        self
    }

    #[must_use]
    pub fn set_column_names(mut self, names: Vec<UnqualifiedName>) -> Self {
        let columns: Result<Vec<_>, _> = names
            .into_iter()
            .map(|name| IndexedColumn::new(name, None))
            .collect();
        if let Some(columns) = self.keep(columns) {
            self.columns = columns;
        }
        self
    }

    #[must_use]
    pub fn set_unquoted_column_names(mut self, names: &[&str]) -> Self {
        let columns: Result<Vec<_>, _> = names
            .iter()
            .map(|name| UnqualifiedName::unquoted(name).and_then(|n| IndexedColumn::new(n, None)))
            .collect();
        if let Some(columns) = self.keep(columns) {
            self.columns = columns;
        }
        self
    }

    #[must_use]
    pub fn set_quoted_column_names(mut self, names: &[&str]) -> Self {
        let columns: Result<Vec<_>, _> = names
            .iter()
            .map(|name| UnqualifiedName::quoted(name).and_then(|n| IndexedColumn::new(n, None)))
            .collect();
        if let Some(columns) = self.keep(columns) {
            self.columns = columns;
        }
        self
    }

    #[must_use]
    pub fn set_is_clustered(mut self, is_clustered: bool) -> Self {
        self.is_clustered = is_clustered;
        self
    }

    #[must_use]
    pub fn set_predicate(mut self, predicate: Option<&str>) -> Self {
        self.predicate = predicate.map(str::to_owned);
        self
    }

    /// `create()`.
    pub fn create(self) -> Result<Index, Exception> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let Some(name) = &self.name else {
            return Err(exception::index_name_not_set());
        };
        let (column_names, flags, options) = self.to_index_parameters(name)?;
        Index::new(
            Some(&name.to_string()),
            column_names,
            self.type_ == IndexType::Unique,
            false,
            flags,
            options,
        )
    }

    /// `addToTable($table)`.
    pub fn add_to_table(self, table: &mut Table) -> Result<(), Exception> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let default_name;
        let name = if let Some(name) = &self.name {
            name
        } else {
            default_name = UnqualifiedName::unquoted("index")?;
            &default_name
        };
        let (column_names, flags, options) = self.to_index_parameters(name)?;
        let index_name = self.name.as_ref().map(ToString::to_string);

        if self.type_ == IndexType::Unique {
            table.add_unique_index_inner(&column_names, index_name.as_deref(), options)?;
            return Ok(());
        }
        table.add_index_inner(&column_names, index_name.as_deref(), flags, options)?;
        Ok(())
    }

    fn to_index_parameters(
        &self,
        name: &UnqualifiedName,
    ) -> Result<(Vec<String>, Vec<String>, PhpArray), Exception> {
        if self.columns.is_empty() {
            return Err(exception::index_columns_not_set(name));
        }

        let mut column_names = Vec::new();
        let mut lengths = Vec::new();
        let mut has_length = false;
        for column in &self.columns {
            column_names.push(column.get_column_name().to_string());
            let length = column.get_length();
            lengths.push(length.map_or(Value::Null, Value::Int));
            if length.is_some() {
                has_length = true;
            }
        }

        let mut options = PhpArray::new();
        if has_length {
            options.insert("lengths", Value::Array(PhpArray::list(lengths)));
        }

        let mut flags = Vec::new();
        match self.type_ {
            IndexType::Fulltext => flags.push("fulltext".to_owned()),
            IndexType::Spatial => flags.push("spatial".to_owned()),
            _ => {}
        }
        if self.is_clustered {
            flags.push("clustered".to_owned());
        }
        if let Some(predicate) = &self.predicate {
            options.insert("where", Value::string(predicate.clone()));
        }

        Ok((column_names, flags, options))
    }
}
