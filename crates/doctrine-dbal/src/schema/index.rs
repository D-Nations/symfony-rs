//! Port of Doctrine\DBAL\Schema\Index.php, Index\IndexType.php and
//! Index\IndexedColumn.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Schema/Index.php src/Schema/Index/IndexType.php
//! Upstream: doctrine/dbal src/Schema/Index/IndexedColumn.php

use std::ops::Deref;

use php_value::{Key, PhpArray, Value};

use crate::deprecations;
use crate::exception::Exception;
use crate::platforms::AbstractPlatform;

use super::abstract_asset::{AbstractAsset, AssetKind, NameParser, trim_quotes};
use super::array_map::ArrayMap;
use super::exception::{self, invalid_state};
use super::identifier::Identifier;
use super::index_editor::IndexEditor;
use super::name::{Name, UnqualifiedName, parse_unqualified_name};

/// The type of an index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IndexType {
    Regular,
    Unique,
    Fulltext,
    Spatial,
}

/// An indexed column: its name and an optional length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedColumn {
    column_name: UnqualifiedName,
    length: Option<i64>,
}

impl IndexedColumn {
    /// `new IndexedColumn($columnName, $length)`: the length must be
    /// positive.
    pub fn new(column_name: UnqualifiedName, length: Option<i64>) -> Result<Self, Exception> {
        if let Some(length) = length
            && length <= 0
        {
            return Err(exception::index_non_positive_column_length(
                &column_name,
                length,
            ));
        }
        Ok(Self {
            column_name,
            length,
        })
    }

    pub fn get_column_name(&self) -> &UnqualifiedName {
        &self.column_name
    }

    pub fn get_length(&self) -> Option<i64> {
        self.length
    }
}

/// `Doctrine\DBAL\Schema\Index`.
#[derive(Debug, Clone, PartialEq)]
pub struct Index {
    asset: AbstractAsset,
    /// `$_columns`: the column names as given, each an `Identifier`.
    columns_map: ArrayMap<Identifier>,
    is_unique: bool,
    is_primary: bool,
    /// `$_flags`: the lowercase flags (as keys).
    flags: ArrayMap<bool>,
    columns: Vec<IndexedColumn>,
    type_: Option<IndexType>,
    predicate: Option<String>,
    failed_to_parse_predicate: bool,
    options: PhpArray,
}

const LINK_FLAGS: &str = "https://github.com/doctrine/dbal/pull/6886";

impl Index {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Schema\\Index";

    /// `new Index($name, $columns, $isUnique, $isPrimary, $flags, $options)`.
    pub fn new<C, F>(
        name: Option<&str>,
        columns: C,
        is_unique: bool,
        is_primary: bool,
        flags: F,
        options: PhpArray,
    ) -> Result<Self, Exception>
    where
        C: IntoIterator,
        C::Item: Into<String>,
        F: IntoIterator,
        F::Item: Into<String>,
    {
        let columns: Vec<String> = columns.into_iter().map(Into::into).collect();
        let flags: Vec<String> = flags.into_iter().map(Into::into).collect();
        let asset = AbstractAsset::new(
            Self::CLASS,
            AssetKind::Named,
            NameParser::Unqualified,
            Some(name.unwrap_or("")),
        );

        if columns.is_empty() {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/6787", || {
                "Instantiation of an index without column names is deprecated.".to_owned()
            });
        }

        let mut index = Self {
            asset,
            columns_map: ArrayMap::new(),
            is_unique: is_unique || is_primary,
            is_primary,
            flags: ArrayMap::new(),
            columns: Vec::new(),
            type_: None,
            predicate: None,
            failed_to_parse_predicate: false,
            options,
        };

        for column in &columns {
            index.add_column(column);
        }

        match index.options.get(&Key::from("where")) {
            None | Some(Value::Null) => {}
            Some(Value::String(predicate)) => {
                if predicate.is_empty() {
                    deprecations::trigger("https://github.com/doctrine/dbal/pull/6886", || {
                        "Passing an empty string as index predicate is deprecated.".to_owned()
                    });
                    index.failed_to_parse_predicate = true;
                } else {
                    index.predicate = Some(predicate.to_string_lossy().into_owned());
                }
            }
            Some(other) => {
                return Err(Exception::new(
                    Exception::TYPE_ERROR,
                    format!(
                        "strlen(): Argument #1 ($string) must be of type string, {} given",
                        other.value_name()
                    ),
                ));
            }
        }

        for flag in &flags {
            index.add_flag_inner(flag);
        }

        if flags.is_empty() {
            index.type_ = index.infer_type();
        }

        let lengths = match index.options.get(&Key::from("lengths")) {
            Some(Value::Array(lengths)) => lengths.values().cloned().collect(),
            _ => Vec::new(),
        };
        index.columns = parse_columns(is_primary, &columns, lengths);

        Ok(index)
    }

    /// `getObjectName()`.
    pub fn get_object_name(&self) -> Result<UnqualifiedName, Exception> {
        match self.asset.get_object_name()? {
            Some(Name::Unqualified(name)) => Ok(name.clone()),
            _ => Err(invalid_state::object_name_not_initialized()),
        }
    }

    /// `getType()`.
    pub fn get_type(&self) -> Result<IndexType, Exception> {
        self.type_
            .ok_or_else(|| invalid_state::index_has_invalid_type(&self.get_name()))
    }

    /// `getIndexedColumns()`: the indexed columns, never empty.
    pub fn get_indexed_columns(&self) -> Result<&[IndexedColumn], Exception> {
        if self.columns.is_empty() {
            return Err(invalid_state::index_has_invalid_columns(&self.get_name()));
        }
        Ok(&self.columns)
    }

    /// Returns whether the index is clustered.
    pub fn is_clustered(&self) -> bool {
        self.has_flag("clustered")
    }

    /// `getPredicate()`: the index predicate, if any.
    pub fn get_predicate(&self) -> Result<Option<String>, Exception> {
        if self.failed_to_parse_predicate {
            return Err(invalid_state::index_has_invalid_predicate(&self.get_name()));
        }
        Ok(match self.get_option("where") {
            Some(Value::String(predicate)) => Some(predicate.to_string_lossy().into_owned()),
            _ => None,
        })
    }

    fn add_column(&mut self, column: &str) {
        self.columns_map
            .insert(column, Identifier::new(column, false));
    }

    /// `getColumns()`: the names of the referencing table columns the
    /// constraint is associated with.
    pub fn get_columns(&self) -> Vec<String> {
        self.columns_map.keys().map(str::to_owned).collect()
    }

    /// `getQuotedColumns($platform)`: the quoted representation of the
    /// column names, with their lengths where the platform supports them.
    pub fn get_quoted_columns(&self, platform: &(impl AbstractPlatform + ?Sized)) -> Vec<String> {
        let mut sub_parts: Vec<Value> =
            if platform.supports_column_length_indexes() && self.has_option("lengths") {
                match self.get_option("lengths") {
                    Some(Value::Array(lengths)) => lengths.values().cloned().collect(),
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            }
            .into_iter()
            .rev()
            .collect();

        self.columns_map
            .values()
            .map(|column| {
                let length = sub_parts.pop().unwrap_or(Value::Null);
                let mut quoted_column = column.get_quoted_name(platform);
                if let Some(length) = length.to_php_string().filter(|_| !length.is_null()) {
                    quoted_column.push('(');
                    quoted_column.push_str(&length.to_string_lossy());
                    quoted_column.push(')');
                }
                quoted_column
            })
            .collect()
    }

    /// `getUnquotedColumns()`.
    pub fn get_unquoted_columns(&self) -> Vec<String> {
        self.get_columns().iter().map(|c| trim_quotes(c)).collect()
    }

    /// Is the index neither unique nor primary key?
    pub fn is_simple_index(&self) -> bool {
        !self.is_primary && !self.is_unique
    }

    pub fn is_unique(&self) -> bool {
        self.is_unique
    }

    pub fn is_primary(&self) -> bool {
        self.is_primary
    }

    /// `hasColumnAtPosition($name, $pos)`.
    pub fn has_column_at_position(&self, name: &str, pos: usize) -> bool {
        let name = trim_quotes(&name.to_ascii_lowercase());
        let index_columns: Vec<String> = self
            .get_unquoted_columns()
            .iter()
            .map(|c| c.to_ascii_lowercase())
            .collect();
        index_columns.iter().position(|c| *c == name) == Some(pos)
    }

    /// Checks if this index exactly spans the given column names in the
    /// correct order.
    pub fn spans_columns(&self, column_names: &[String]) -> bool {
        let columns = self.get_columns();
        let mut same_columns = true;
        for (i, column) in columns.iter().enumerate() {
            match column_names.get(i) {
                Some(other)
                    if trim_quotes(&column.to_ascii_lowercase())
                        == trim_quotes(&other.to_ascii_lowercase()) => {}
                _ => same_columns = false,
            }
        }
        same_columns
    }

    /// Checks if the other index already fulfills all the indexing and
    /// constraint needs of the current one.
    pub fn is_fulfilled_by(&self, other: &Index) -> bool {
        // allow the other index to be equally large only. It being larger is an option
        // but it creates a problem with scenarios of the kind PRIMARY KEY(foo,bar) UNIQUE(foo)
        if other.get_columns().len() != self.get_columns().len() {
            return false;
        }

        // Check if columns are the same, and even in the same order
        if self.spans_columns(&other.get_columns()) {
            if !self.same_partial_index(other) {
                return false;
            }

            if !self.has_same_column_lengths(other) {
                return false;
            }

            if !self.is_unique() && !self.is_primary() {
                // this is a special case: If the current key is neither primary or unique, any unique or
                // primary key will always have the same effect for the index and there cannot be any constraint
                // overlaps. This means a primary or unique index can always fulfill the requirements of just an
                // index that has no constraints.
                return true;
            }

            if other.is_primary() != self.is_primary() {
                return false;
            }

            return other.is_unique() == self.is_unique();
        }

        false
    }

    /// Detects if the other index is a non-unique, non primary index that
    /// can be overwritten by this one (deprecated upstream).
    pub fn overrules(&self, other: &Index) -> bool {
        deprecations::trigger("https://github.com/doctrine/dbal/pull/6886", || {
            "Doctrine\\DBAL\\Schema\\Index::overrules is deprecated.".to_owned()
        });
        if other.is_primary() {
            return false;
        }
        if self.is_simple_index() && other.is_unique() {
            return false;
        }
        self.spans_columns(&other.get_columns())
            && (self.is_primary() || self.is_unique())
            && self.same_partial_index(other)
    }

    /// `getFlags()`: the platform specific flags for indexes.
    pub fn get_flags(&self) -> Vec<String> {
        self.flags.keys().map(str::to_owned).collect()
    }

    /// `addFlag($flag)`: adds a flag for a platform specific index.
    pub fn add_flag(&mut self, flag: &str) -> &mut Self {
        self.add_flag_inner(flag);
        self
    }

    fn add_flag_inner(&mut self, flag: &str) {
        self.flags.insert(flag.to_ascii_lowercase(), true);
        self.validate_flags();
        self.type_ = self.infer_type();
    }

    /// Does this index have a specific flag?
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.contains_key(&flag.to_ascii_lowercase())
    }

    /// Removes a flag.
    pub fn remove_flag(&mut self, flag: &str) {
        self.flags.remove(&flag.to_ascii_lowercase());
        self.type_ = self.infer_type();
    }

    /// `isset($this->options[strtolower($name)])`.
    pub fn has_option(&self, name: &str) -> bool {
        self.options
            .get(&Key::from(name.to_ascii_lowercase().as_str()))
            .is_some_and(|v| !v.is_null())
    }

    /// `$this->options[strtolower($name)]`.
    pub fn get_option(&self, name: &str) -> Option<&Value> {
        self.options
            .get(&Key::from(name.to_ascii_lowercase().as_str()))
    }

    pub fn get_options(&self) -> &PhpArray {
        &self.options
    }

    fn validate_flags(&self) {
        let unsupported: Vec<&str> = self
            .flags
            .keys()
            .filter(|flag| !matches!(*flag, "fulltext" | "spatial" | "clustered" | "nonclustered"))
            .collect();

        if !unsupported.is_empty() {
            let list = unsupported.join(", ");
            deprecations::trigger(LINK_FLAGS, || {
                format!("Configuring an index with non-standard flags is deprecated: {list}")
            });
        }

        if self.has_flag("clustered")
            && (self.has_flag("nonclustered")
                || self.has_flag("fulltext")
                || self.has_flag("spatial"))
        {
            deprecations::trigger(LINK_FLAGS, || {
                "A fulltext, spatial or non-clustered index cannot be clustered.".to_owned()
            });
        }

        if self.predicate.is_some()
            && (self.has_flag("fulltext") || self.has_flag("spatial") || self.has_flag("clustered"))
        {
            deprecations::trigger(LINK_FLAGS, || {
                "A fulltext, spatial or clustered index cannot be partial.".to_owned()
            });
        }
    }

    fn infer_type(&self) -> Option<IndexType> {
        let mut type_ = IndexType::Regular;
        let mut matches = Vec::new();

        if self.is_unique {
            type_ = IndexType::Unique;
            matches.push("unique");
        }

        if self.has_flag("fulltext") {
            type_ = IndexType::Fulltext;
            matches.push("fulltext");
        }

        if self.has_flag("spatial") {
            type_ = IndexType::Spatial;
            matches.push("spatial");
        }

        if matches.len() > 1 {
            let list = matches.join(", ");
            deprecations::trigger(LINK_FLAGS, || {
                format!(
                    "Configuring an index with mutually exclusive properties is deprecated: {list}"
                )
            });
            return None;
        }

        Some(type_)
    }

    fn same_partial_index(&self, other: &Index) -> bool {
        if self.has_option("where")
            && other.has_option("where")
            && let (Some(a), Some(b)) = (self.get_option("where"), other.get_option("where"))
            && a.identical(b)
        {
            return true;
        }
        !self.has_option("where") && !other.has_option("where")
    }

    fn has_same_column_lengths(&self, other: &Index) -> bool {
        let filter = |options: &PhpArray| {
            let mut filtered = PhpArray::new();
            if let Some(Value::Array(lengths)) = options.get(&Key::from("lengths")) {
                for (key, length) in lengths.iter() {
                    if !length.is_null() {
                        filtered.insert(key.clone(), length.clone());
                    }
                }
            }
            Value::Array(filtered)
        };
        filter(&self.options).identical(&filter(&other.options))
    }

    /// `Index::editor()`.
    pub fn editor() -> IndexEditor {
        IndexEditor::new()
    }

    /// `edit()`: an editor holding this index's definition.
    pub fn edit(&self) -> Result<IndexEditor, Exception> {
        Ok(Self::editor()
            .set_name(Some(self.get_object_name()?))
            .set_type(self.get_type()?)
            .set_columns(self.get_indexed_columns()?.to_vec())
            .set_is_clustered(self.is_clustered())
            .set_predicate(self.get_predicate()?.as_deref()))
    }
}

impl Deref for Index {
    type Target = AbstractAsset;

    fn deref(&self) -> &AbstractAsset {
        &self.asset
    }
}

/// `parseColumns()`: the indexed columns, or none (with a deprecation)
/// when a name or a length is invalid.
fn parse_columns(
    is_primary: bool,
    column_names: &[String],
    lengths: Vec<Value>,
) -> Vec<IndexedColumn> {
    let mut lengths = lengths.into_iter();
    let mut columns = Vec::new();
    for column_name in column_names {
        let parsed_name = match parse_unqualified_name(column_name) {
            Ok(name) => name,
            Err(e) => {
                let message = e.message().to_string_lossy().into_owned();
                deprecations::trigger("https://github.com/doctrine/dbal/pull/6787", || {
                    format!("Unable to parse column name: {message}.")
                });
                return Vec::new();
            }
        };

        let length = match lengths.next().unwrap_or(Value::Null) {
            Value::Null => None,
            length => {
                if is_primary {
                    deprecations::trigger("https://github.com/doctrine/dbal/pull/6787", || {
                        "Declaring column length for primary key indexes is deprecated.".to_owned()
                    });
                    return Vec::new();
                }
                let length = match length {
                    Value::Int(i) => i,
                    other => {
                        let type_name = match &other {
                            Value::Object(o) => o.class().to_owned(),
                            Value::Bool(_) => "boolean".to_owned(),
                            Value::Float(_) => "double".to_owned(),
                            Value::String(_) => "string".to_owned(),
                            Value::Array(_) => "array".to_owned(),
                            Value::Int(_) => "integer".to_owned(),
                            Value::Null => "NULL".to_owned(),
                        };
                        deprecations::trigger("https://github.com/doctrine/dbal/pull/6787", || {
                            format!(
                                "Indexed column length should be an integer, {type_name} given."
                            )
                        });
                        php_value::numeric::int_cast(&other)
                    }
                };
                if length < 1 {
                    deprecations::trigger("https://github.com/doctrine/dbal/pull/6787", || {
                        format!(
                            "Indexed column length should be a positive integer, {length} given."
                        )
                    });
                    return Vec::new();
                }
                Some(length)
            }
        };

        match IndexedColumn::new(parsed_name, length) {
            Ok(column) => columns.push(column),
            Err(_) => return Vec::new(),
        }
    }
    columns
}
