//! Port of Doctrine\DBAL\Platforms\SQLitePlatform.php: the `SQLite`
//! dialect.
//!
//! Not ported: `createSchemaManager()` and `createMetadataProvider()`
//! (`SQLiteSchemaManager` and `SQLiteMetadataProvider` need a Connection).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/SQLitePlatform.php

use std::fmt::Write as _;

use php_value::{Key, PhpArray, PhpString, Value};

use crate::TransactionIsolationLevel;
use crate::deprecations;
use crate::exception::Exception;
use crate::schema::exception::column_does_not_exist;
use crate::schema::name::UnquotedIdentifierFolding;
use crate::schema::{
    ArrayMap, Column, DefaultExpressionObject, ForeignKeyConstraint, Identifier, Index, Table,
    TableDiff, TableParts,
};
use crate::sql::builder::{DefaultSelectSQLBuilder, SelectSQLBuilder};
use crate::types::PhpMappingType;

use super::abstract_platform::{AbstractPlatform, array_unique, validate_create_table_options};
use super::date_interval_unit::{DateIntervalUnit, TrimMode};
use super::exception::NotSupported;
use super::keywords::{KeywordList, SQLiteKeywords};
use super::platform_state::{CreateTableOptions, PlatformState, string_argument};

/// PHP's global `\InvalidArgumentException` (which `SQLitePlatform`
/// throws, unlike `AbstractPlatform`'s DBAL one).
pub const GLOBAL_INVALID_ARGUMENT_EXCEPTION: &str = "InvalidArgumentException";

/// The `SQLitePlatform` class describes the specifics and dialects of the
/// `SQLite` database platform.
#[derive(Debug, Default)]
pub struct SQLitePlatform {
    state: PlatformState,
}

fn not_supported(method: &str) -> Exception {
    NotSupported::new(&format!(
        "Doctrine\\DBAL\\Platforms\\SQLitePlatform::{method}"
    ))
    .into()
}

fn flag(column: &PhpArray, name: &str) -> bool {
    column.get(&Key::from(name)).is_some_and(Value::is_truthy)
}

impl SQLitePlatform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\SQLitePlatform";

    pub fn new() -> Self {
        Self::default()
    }

    /// `hasAutoIncrementColumn($columns, $options)` (private).
    fn has_auto_increment_column(columns: &[PhpArray], options: &CreateTableOptions) -> bool {
        let primary_key_column_names = array_unique(options.primary.as_deref().unwrap_or(&[]));
        for column in columns {
            if !flag(column, "autoincrement") {
                continue;
            }
            let name = match column.get(&Key::from("name")) {
                Some(Value::String(name)) => name.to_string_lossy().into_owned(),
                _ => String::new(),
            };
            if !primary_key_column_names.contains(&name) {
                deprecations::trigger("https://github.com/doctrine/dbal/pull/6849", || {
                    "Declaring a column that is not part of the primary key as auto-increment is deprecated."
                        .to_owned()
                });
            } else if primary_key_column_names.len() > 1 {
                deprecations::trigger("https://github.com/doctrine/dbal/pull/6849", || {
                    "Declaring a column that is part of a composite primary key as auto-increment is deprecated."
                        .to_owned()
                });
            }
            return true;
        }
        false
    }

    /// `getInlineCommentSQL($comment)` (private).
    fn get_inline_comment_sql(comment: &str) -> String {
        if comment.is_empty() {
            return String::new();
        }
        format!("--{}\n", comment.replace('\n', "\n--"))
    }

    /// `replaceColumn()` (private): `array_combine()` of the keys with
    /// `$columnName` replaced.
    fn replace_column(
        table_name: &str,
        columns: &ArrayMap<Column>,
        column_name: &str,
        column: &Column,
    ) -> Result<ArrayMap<Column>, Exception> {
        if !columns.contains_key(column_name) {
            return Err(column_does_not_exist(column_name, table_name));
        }
        let new_key = column.get_name().to_ascii_lowercase();
        Ok(columns
            .iter()
            .map(|(key, value)| {
                if key == column_name {
                    (new_key.clone(), column.clone())
                } else {
                    (key.to_owned(), value.clone())
                }
            })
            .collect())
    }

    /// `getSimpleAlterTableSQL()` (private): `None` for PHP's `false`.
    fn get_simple_alter_table_sql(
        &self,
        diff: &TableDiff,
    ) -> Result<Option<Vec<String>>, Exception> {
        if !diff.get_changed_columns().is_empty()
            || !diff.get_dropped_columns().is_empty()
            || !diff.get_added_indexes().is_empty()
            || !diff.modified_indexes().is_empty()
            || !diff.get_dropped_indexes().is_empty()
            || !diff.renamed_indexes().is_empty()
            || !diff.get_added_foreign_keys().is_empty()
            || !diff.modified_foreign_keys().is_empty()
            || !diff.get_dropped_foreign_keys().is_empty()
        {
            return Ok(None);
        }

        let table = diff.get_old_table();
        let mut sql = Vec::new();

        for column in diff.get_added_columns() {
            let mut definition = column.to_array(true)?;
            let mapping = self.get_column_type(&definition)?.php_mapping_type();
            let default = column.get_default();
            let is = |expected: String| matches!(default, Value::String(s) if s.as_bytes() == expected.as_bytes());

            // A non-constant default expression (e.g. CURRENT_TIMESTAMP)
            // cannot be used with ALTER TABLE ... ADD COLUMN on a
            // non-empty table, so fall back to a table rebuild.
            if column.get_column_definition().is_some()
                || column.get_autoincrement()
                || !column.get_comment().is_empty()
                || DefaultExpressionObject::of(default).is_some()
                || (mapping == Some(PhpMappingType::DateTime)
                    && is(self.get_current_timestamp_sql()))
                || (mapping == Some(PhpMappingType::Date) && is(self.get_current_date_sql()))
                || (mapping == Some(PhpMappingType::Time) && is(self.get_current_time_sql()))
            {
                return Ok(None);
            }

            let name = column.get_quoted_name(self);
            definition.insert("name", Value::string(name.clone()));
            sql.push(format!(
                "ALTER TABLE {} ADD COLUMN {}",
                table.get_quoted_name(self),
                self.get_column_declaration_sql(&name, &definition)?
            ));
        }

        Ok(Some(sql))
    }

    /// `getDiffColumnNameMap()` (private): lowercased old name => new
    /// name.
    fn get_diff_column_name_map(diff: &TableDiff) -> ArrayMap<String> {
        let mut map = ArrayMap::new();
        for column in diff.get_old_table().get_columns() {
            let column_name = column.get_name();
            map.insert(column_name.to_ascii_lowercase(), column_name);
        }
        for column in diff.get_dropped_columns() {
            map.remove(&column.get_name().to_ascii_lowercase());
        }
        for column_diff in diff.get_changed_columns() {
            let column_name = column_diff.get_old_column().get_name();
            map.insert(
                column_name.to_ascii_lowercase(),
                column_diff.get_new_column().get_name(),
            );
        }
        for column in diff.get_added_columns() {
            let column_name = column.get_name();
            map.insert(column_name.to_ascii_lowercase(), column_name);
        }
        map
    }

    /// `getIndexesInAlteredTable()` (private).
    fn get_indexes_in_altered_table(diff: &TableDiff) -> Result<ArrayMap<Index>, Exception> {
        let old_table = diff.get_old_table();
        let mut indexes = old_table.get_indexes().clone();
        let name_map = Self::get_diff_column_name_map(diff);

        for (key, index) in old_table.get_indexes().iter() {
            let index_name = index.get_name();
            for (old_index_name, _) in diff.renamed_indexes() {
                if index_name.eq_ignore_ascii_case(old_index_name) {
                    indexes.remove(key);
                }
            }

            let mut changed = false;
            let mut index_columns = Vec::new();
            let mut dropped = false;
            // Use the unquoted column names so the lookup is agnostic of
            // whether the index was introspected (which marks its column
            // names as quoted) or built in memory.
            for column_name in index.get_unquoted_columns() {
                let normalized_column_name = column_name.to_ascii_lowercase();
                let Some(new_name) = name_map.get(&normalized_column_name) else {
                    indexes.remove(key);
                    dropped = true;
                    break;
                };
                index_columns.push(new_name.clone());
                if column_name != *new_name {
                    changed = true;
                }
            }
            if dropped {
                continue;
            }

            if changed {
                indexes.insert(
                    key,
                    Index::new(
                        Some(&index.get_name()),
                        index_columns,
                        index.is_unique(),
                        index.is_primary(),
                        index.get_flags(),
                        PhpArray::new(),
                    )?,
                );
            }
        }

        for index in diff.get_dropped_indexes() {
            let index_name = index.get_name();
            if !index_name.is_empty() {
                indexes.remove(&index_name.to_ascii_lowercase());
            }
        }

        let renamed = diff.renamed_indexes().iter().map(|(_, index)| index);
        for index in diff
            .modified_indexes()
            .iter()
            .chain(diff.get_added_indexes())
            .chain(renamed)
        {
            let index_name = index.get_name();
            if index_name.is_empty() {
                indexes.push(index.clone());
            } else {
                indexes.insert(index_name.to_ascii_lowercase(), index.clone());
            }
        }

        Ok(indexes)
    }

    /// `getForeignKeysInAlteredTable()` (private).
    fn get_foreign_keys_in_altered_table(
        diff: &TableDiff,
    ) -> Result<ArrayMap<ForeignKeyConstraint>, Exception> {
        let old_table = diff.get_old_table();
        let mut foreign_keys = old_table.get_foreign_keys().clone();
        let name_map = Self::get_diff_column_name_map(diff);

        for (key, constraint) in old_table.get_foreign_keys().iter() {
            let mut changed = false;
            let mut local_columns = Vec::new();
            let mut dropped = false;
            // Use the unquoted column names so the lookup is agnostic of
            // whether the constraint was introspected (which marks its
            // column names as quoted) or built in memory.
            for column_name in constraint.unquoted_local_columns() {
                let normalized_column_name = column_name.to_ascii_lowercase();
                let Some(new_name) = name_map.get(&normalized_column_name) else {
                    foreign_keys.remove(key);
                    dropped = true;
                    break;
                };
                local_columns.push(new_name.clone());
                if column_name != *new_name {
                    changed = true;
                }
            }
            if dropped {
                continue;
            }

            if changed {
                foreign_keys.insert(
                    key,
                    ForeignKeyConstraint::new(
                        local_columns,
                        &constraint.foreign_table_name(),
                        constraint.foreign_columns(),
                        &constraint.get_name(),
                        constraint.options().clone(),
                    )?,
                );
            }
        }

        for constraint in diff.get_dropped_foreign_keys() {
            let constraint_name = constraint.get_name();
            if !constraint_name.is_empty() {
                foreign_keys.remove(&constraint_name.to_ascii_lowercase());
            }
        }

        for constraint in diff
            .modified_foreign_keys()
            .iter()
            .chain(diff.get_added_foreign_keys())
        {
            let constraint_name = constraint.get_name();
            if constraint_name.is_empty() {
                foreign_keys.push(constraint.clone());
            } else {
                foreign_keys.insert(constraint_name.to_ascii_lowercase(), constraint.clone());
            }
        }

        Ok(foreign_keys)
    }

    /// `getPrimaryIndexInAlteredTable()` (private).
    fn get_primary_index_in_altered_table(diff: &TableDiff) -> Result<Vec<Index>, Exception> {
        let mut primary_index = Vec::new();
        for index in Self::get_indexes_in_altered_table(diff)?.values() {
            if index.is_primary() {
                primary_index = vec![index.clone()];
            }
        }
        Ok(primary_index)
    }
}

impl AbstractPlatform for SQLitePlatform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn platform_state(&self) -> Option<&PlatformState> {
        Some(&self.state)
    }

    /// `parent::__construct(UnquotedIdentifierFolding::NONE)`.
    fn get_unquoted_identifier_folding(&self) -> UnquotedIdentifierFolding {
        UnquotedIdentifierFolding::None
    }

    fn get_create_database_sql(&self, _name: &str) -> Result<String, Exception> {
        Err(not_supported("getCreateDatabaseSQL"))
    }

    fn get_drop_database_sql(&self, _name: &str) -> Result<String, Exception> {
        Err(not_supported("getDropDatabaseSQL"))
    }

    fn get_regexp_expression(&self) -> Result<String, Exception> {
        Ok("REGEXP".to_owned())
    }

    fn get_mod_expression(&self, dividend: &str, divisor: &str) -> String {
        format!("{dividend} % {divisor}")
    }

    fn get_trim_expression(&self, str: &str, mode: TrimMode, char: Option<&str>) -> String {
        let trim_fn = match mode {
            TrimMode::Unspecified | TrimMode::Both => "TRIM",
            TrimMode::Leading => "LTRIM",
            TrimMode::Trailing => "RTRIM",
        };
        let mut arguments = vec![str];
        if let Some(char) = char {
            arguments.push(char);
        }
        format!("{trim_fn}({})", arguments.join(", "))
    }

    fn get_substring_expression(&self, string: &str, start: &str, length: Option<&str>) -> String {
        match length {
            None => format!("SUBSTR({string}, {start})"),
            Some(length) => format!("SUBSTR({string}, {start}, {length})"),
        }
    }

    fn get_locate_expression(
        &self,
        string: &str,
        substring: &str,
        start: Option<&str>,
    ) -> Result<String, Exception> {
        Ok(match start {
            None | Some("1") => format!("INSTR({string}, {substring})"),
            Some(start) => format!(
                "CASE WHEN INSTR(SUBSTR({string}, {start}), {substring}) > 0 THEN INSTR(SUBSTR({string}, {start}), {substring}) + {start} - 1 ELSE 0 END"
            ),
        })
    }

    fn get_date_arithmetic_interval_expression(
        &self,
        date: &str,
        operator: &str,
        interval: &str,
        unit: DateIntervalUnit,
    ) -> Result<String, Exception> {
        let (interval, unit) = match unit {
            DateIntervalUnit::Week => (self.multiply_interval(interval, 7), DateIntervalUnit::Day),
            DateIntervalUnit::Quarter => {
                (self.multiply_interval(interval, 3), DateIntervalUnit::Month)
            }
            unit => (interval.to_owned(), unit),
        };
        Ok(format!(
            "DATETIME({date},{})",
            self.get_concat_expression(&[
                &self.quote_string_literal(operator),
                &interval,
                &self.quote_string_literal(&format!(" {}", unit.value())),
            ])
        ))
    }

    fn get_date_diff_expression(&self, date1: &str, date2: &str) -> Result<String, Exception> {
        Ok(format!(
            "JULIANDAY({date1}, 'start of day') - JULIANDAY({date2}, 'start of day')"
        ))
    }

    fn get_current_database_expression(&self) -> Result<String, Exception> {
        Ok("'main'".to_owned())
    }

    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(self, None, None))
    }

    fn get_transaction_isolation_level_sql(&self, level: TransactionIsolationLevel) -> String {
        match level {
            TransactionIsolationLevel::ReadUncommitted => "0",
            TransactionIsolationLevel::ReadCommitted
            | TransactionIsolationLevel::RepeatableRead
            | TransactionIsolationLevel::Serializable => "1",
        }
        .to_owned()
    }

    fn get_set_transaction_isolation_sql(
        &self,
        level: TransactionIsolationLevel,
    ) -> Result<String, Exception> {
        Ok(format!(
            "PRAGMA read_uncommitted = {}",
            self.get_transaction_isolation_level_sql(level)
        ))
    }

    fn get_boolean_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BOOLEAN"))
    }

    fn get_integer_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let mut sql = b"INTEGER".to_vec();
        sql.extend_from_slice(
            self.get_common_integer_type_declaration_sql(column)?
                .as_bytes(),
        );
        Ok(PhpString::from(sql))
    }

    fn get_big_int_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        // SQLite autoincrement is implicit for INTEGER PKs, but not for
        // BIGINT fields.
        if flag(column, "autoincrement") {
            return self.get_integer_type_declaration_sql(column);
        }
        let mut sql = b"BIGINT".to_vec();
        sql.extend_from_slice(
            self.get_common_integer_type_declaration_sql(column)?
                .as_bytes(),
        );
        Ok(PhpString::from(sql))
    }

    fn get_small_int_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        // SQLite autoincrement is implicit for INTEGER PKs, but not for
        // SMALLINT fields.
        if flag(column, "autoincrement") {
            return self.get_integer_type_declaration_sql(column);
        }
        let mut sql = b"SMALLINT".to_vec();
        sql.extend_from_slice(
            self.get_common_integer_type_declaration_sql(column)?
                .as_bytes(),
        );
        Ok(PhpString::from(sql))
    }

    fn get_date_time_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DATETIME"))
    }

    fn get_date_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DATE"))
    }

    fn get_time_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TIME"))
    }

    fn get_common_integer_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        // SQLite autoincrement is only possible for the primary key
        if flag(column, "autoincrement") {
            return Ok(PhpString::from(" PRIMARY KEY AUTOINCREMENT"));
        }
        Ok(PhpString::from(if flag(column, "unsigned") {
            " UNSIGNED"
        } else {
            ""
        }))
    }

    fn get_create_table_sql_internal(
        &self,
        name: &str,
        columns: &[PhpArray],
        mut options: CreateTableOptions,
    ) -> Result<Vec<String>, Exception> {
        validate_create_table_options(
            &options,
            "Doctrine\\DBAL\\Platforms\\SQLitePlatform::_getCreateTableSQL",
        );

        if Self::has_auto_increment_column(columns, &options) {
            options.primary = Some(Vec::new());
        }

        let mut query_fields = self.get_column_declaration_list_sql(columns)?;

        for definition in options.unique_constraints.iter().flatten() {
            query_fields.push_str(", ");
            query_fields.push_str(&self.get_unique_constraint_declaration_sql(definition)?);
        }

        if let Some(primary) = options.primary.as_ref().filter(|p| !p.is_empty()) {
            let _ = write!(
                query_fields,
                ", PRIMARY KEY ({})",
                array_unique(primary).join(", ")
            );
        }

        for foreign_key in options.foreign_keys.iter().flatten() {
            query_fields.push_str(", ");
            query_fields.push_str(&self.get_foreign_key_declaration_sql(foreign_key)?);
        }

        let mut table_comment = String::new();
        if let Some(comment) = options
            .table
            .get(&Key::from("comment"))
            .filter(|c| !c.is_null())
        {
            let comment = string_argument(
                comment,
                "Doctrine\\DBAL\\Platforms\\SQLitePlatform::getInlineCommentSQL",
                1,
                "comment",
            )?;
            table_comment = Self::get_inline_comment_sql(&comment);
        }

        let mut sql = format!("CREATE TABLE {name} {table_comment}({query_fields})");

        if matches!(
            options.table.get(&Key::from("without_rowid")),
            Some(Value::Bool(true))
        ) {
            sql.push_str(" WITHOUT ROWID");
        }

        let mut query = vec![sql];

        if matches!(
            options.table.get(&Key::from("alter")),
            Some(Value::Bool(true))
        ) {
            return Ok(query);
        }

        for index in options.indexes.iter().flatten() {
            query.push(self.get_create_index_sql(index, name)?);
        }

        Ok(query)
    }

    fn get_binary_type_declaration_sql_snippet(
        &self,
        _length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BLOB"))
    }

    fn get_varchar_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from(match length {
            Some(length) => format!("VARCHAR({length})"),
            None => "VARCHAR".to_owned(),
        }))
    }

    fn get_varbinary_type_declaration_sql_snippet(
        &self,
        _length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BLOB"))
    }

    fn get_clob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("CLOB"))
    }

    fn get_list_views_sql(&self, _database: &str) -> Result<String, Exception> {
        Ok("SELECT name, sql FROM sqlite_master WHERE type='view' AND sql NOT NULL".to_owned())
    }

    fn get_advanced_foreign_key_options_sql(
        &self,
        foreign_key: &ForeignKeyConstraint,
    ) -> Result<String, Exception> {
        let mut query = String::new();
        for (option, clause) in [("onUpdate", "ON UPDATE"), ("onDelete", "ON DELETE")] {
            if foreign_key.has_option_inner(option) {
                let action = string_argument(
                    &foreign_key.option(option),
                    "Doctrine\\DBAL\\Platforms\\AbstractPlatform::getForeignKeyReferentialActionSQL",
                    1,
                    "action",
                )?;
                let _ = write!(
                    query,
                    " {clause} {}",
                    self.get_foreign_key_referential_action_sql(&action)?
                );
            }
        }

        if !foreign_key.has_option_inner("deferrable")
            || matches!(foreign_key.option("deferrable"), Value::Bool(false))
        {
            query.push_str(" NOT");
        }
        query.push_str(" DEFERRABLE INITIALLY");
        if foreign_key.has_option_inner("deferred")
            && !matches!(foreign_key.option("deferred"), Value::Bool(false))
        {
            query.push_str(" DEFERRED");
        } else {
            query.push_str(" IMMEDIATE");
        }
        Ok(query)
    }

    fn supports_identity_columns(&self) -> bool {
        true
    }

    fn supports_column_collation(&self) -> bool {
        true
    }

    fn supports_inline_column_comments(&self) -> bool {
        true
    }

    fn get_truncate_table_sql(&self, table_name: &str, _cascade: bool) -> String {
        let table_identifier = Identifier::new(table_name, false);
        format!("DELETE FROM {}", table_identifier.get_quoted_name(self))
    }

    fn get_inline_column_comment_sql(&self, comment: &str) -> Result<String, Exception> {
        Ok(Self::get_inline_comment_sql(comment))
    }

    fn initialize_doctrine_type_mappings(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("bigint", "bigint"),
            ("bigserial", "bigint"),
            ("blob", "blob"),
            ("boolean", "boolean"),
            ("char", "string"),
            ("clob", "text"),
            ("date", "date"),
            ("datetime", "datetime"),
            ("decimal", "decimal"),
            ("double", "float"),
            ("double precision", "float"),
            ("float", "float"),
            ("image", "string"),
            ("int", "integer"),
            ("integer", "integer"),
            ("longtext", "text"),
            ("longvarchar", "string"),
            ("mediumint", "integer"),
            ("mediumtext", "text"),
            ("ntext", "string"),
            ("numeric", "decimal"),
            ("nvarchar", "string"),
            ("real", "smallfloat"),
            ("serial", "integer"),
            ("smallint", "smallint"),
            ("string", "string"),
            ("text", "text"),
            ("time", "time"),
            ("timestamp", "datetime"),
            ("tinyint", "boolean"),
            ("tinytext", "text"),
            ("varchar", "string"),
            ("varchar2", "string"),
        ]
    }

    fn create_reserved_keywords_list(&self) -> KeywordList {
        SQLiteKeywords::new()
    }

    fn get_pre_alter_table_index_foreign_key_sql(
        &self,
        _diff: &TableDiff,
    ) -> Result<Vec<String>, Exception> {
        Ok(Vec::new())
    }

    fn get_post_alter_table_index_foreign_key_sql(
        &self,
        diff: &TableDiff,
    ) -> Result<Vec<String>, Exception> {
        let table = diff.get_old_table();
        let mut sql = Vec::new();
        for index in Self::get_indexes_in_altered_table(diff)?.values() {
            if !index.is_primary() {
                sql.push(self.get_create_index_sql(index, &table.get_quoted_name(self))?);
            }
        }
        Ok(sql)
    }

    fn do_modify_limit_query(&self, query: &str, limit: Option<i64>, offset: i64) -> String {
        let limit = if limit.is_none() && offset > 0 {
            Some(-1)
        } else {
            limit
        };
        let mut query = query.to_owned();
        if let Some(limit) = limit {
            query = format!("{query} LIMIT {limit}");
        }
        if offset > 0 {
            query = format!("{query} OFFSET {offset}");
        }
        query
    }

    fn get_blob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BLOB"))
    }

    fn get_temporary_table_name(&self, table_name: &str) -> String {
        table_name.to_owned()
    }

    fn get_create_tables_sql(&self, tables: &[Table]) -> Result<Vec<String>, Exception> {
        let mut sql = Vec::new();
        for table in tables {
            sql.extend(self.get_create_table_sql(table)?);
        }
        Ok(sql)
    }

    fn get_create_index_sql(&self, index: &Index, table: &str) -> Result<String, Exception> {
        let mut name = index.get_quoted_name(self);
        if index.get_columns().is_empty() {
            return Err(Exception::new(
                GLOBAL_INVALID_ARGUMENT_EXCEPTION,
                format!("Incomplete or invalid index definition {name} on table {table}"),
            ));
        }
        if index.is_primary() {
            return self.get_create_primary_key_sql(index, table);
        }
        let mut table = table.to_owned();
        if table.contains('.') {
            // [$schema, $table] = explode('.', $table)
            let mut parts = table.split('.');
            let schema = parts.next().unwrap_or("").to_owned();
            let unqualified = parts.next().unwrap_or("").to_owned();
            name = format!("{schema}.{name}");
            table = unqualified;
        }
        Ok(format!(
            "CREATE {}INDEX {name} ON {table} ({}){}",
            self.get_create_index_sql_flags(index),
            index.get_quoted_columns(self).join(", "),
            self.get_partial_index_sql(index)?
        ))
    }

    fn get_drop_tables_sql(&self, tables: &[Table]) -> Result<Vec<String>, Exception> {
        Ok(tables
            .iter()
            .map(|table| self.get_drop_table_sql(&table.get_quoted_name(self)))
            .collect())
    }

    fn get_create_primary_key_sql(
        &self,
        _index: &Index,
        _table: &str,
    ) -> Result<String, Exception> {
        Err(not_supported("getCreatePrimaryKeySQL"))
    }

    fn get_create_foreign_key_sql(
        &self,
        _foreign_key: &ForeignKeyConstraint,
        _table: &str,
    ) -> Result<String, Exception> {
        Err(not_supported("getCreateForeignKeySQL"))
    }

    fn get_drop_foreign_key_sql(
        &self,
        _foreign_key: &str,
        _table: &str,
    ) -> Result<String, Exception> {
        Err(not_supported("getDropForeignKeySQL"))
    }

    fn get_alter_table_sql(&self, diff: &TableDiff) -> Result<Vec<String>, Exception> {
        if let Some(sql) = self.get_simple_alter_table_sql(diff)? {
            return Ok(sql);
        }

        let table = diff.get_old_table();

        let mut columns: ArrayMap<Column> = ArrayMap::new();
        let mut old_column_names: ArrayMap<String> = ArrayMap::new();
        let mut new_column_names: ArrayMap<String> = ArrayMap::new();

        for column in table.get_columns() {
            let column_name = column.get_name().to_ascii_lowercase();
            columns.insert(column_name.clone(), column.clone());
            let quoted = column.get_quoted_name(self);
            old_column_names.insert(column_name.clone(), quoted.clone());
            new_column_names.insert(column_name, quoted);
        }

        for column in diff.get_dropped_columns() {
            let column_name = column.get_name().to_ascii_lowercase();
            if columns.contains_key(&column_name) {
                columns.remove(&column_name);
                old_column_names.remove(&column_name);
                new_column_names.remove(&column_name);
            }
        }

        for column_diff in diff.get_changed_columns() {
            let old_column_name = column_diff.get_old_column().get_name().to_ascii_lowercase();
            let new_column = column_diff.get_new_column();
            columns =
                Self::replace_column(&table.get_name(), &columns, &old_column_name, new_column)?;
            if let Some(name) = new_column_names.get_mut(&old_column_name) {
                *name = new_column.get_quoted_name(self);
            }
        }

        for column in diff.get_added_columns() {
            columns.insert(column.get_name().to_ascii_lowercase(), column.clone());
        }

        let mut table_name = table.get_name();
        if let Some(pos) = table_name.find('.') {
            table_name = table_name[pos + 1..].to_owned();
        }

        let data_table = Table::new(&format!("__temp__{table_name}"))?;

        let mut new_table = Table::with_parts(
            &table.get_quoted_name(self),
            TableParts {
                columns: columns.values().cloned().collect(),
                indexes: Self::get_primary_index_in_altered_table(diff)?,
                fk_constraints: Self::get_foreign_keys_in_altered_table(diff)?
                    .values()
                    .cloned()
                    .collect(),
                options: table.get_options().clone(),
                ..TableParts::default()
            },
        )?;
        new_table.add_option_inner("alter", Value::Bool(true));

        let old_names: Vec<&str> = old_column_names.values().map(String::as_str).collect();
        let new_names: Vec<&str> = new_column_names.values().map(String::as_str).collect();

        let mut sql = self.get_pre_alter_table_index_foreign_key_sql(diff)?;
        sql.push(format!(
            "CREATE TEMPORARY TABLE {} AS SELECT {} FROM {}",
            data_table.get_quoted_name(self),
            old_names.join(", "),
            table.get_quoted_name(self)
        ));
        sql.push(self.get_drop_table_sql(&table.get_quoted_name(self)));
        sql.extend(self.get_create_table_sql(&new_table)?);
        sql.push(format!(
            "INSERT INTO {} ({}) SELECT {} FROM {}",
            new_table.get_quoted_name(self),
            new_names.join(", "),
            old_names.join(", "),
            data_table.get_quoted_name(self)
        ));
        sql.push(self.get_drop_table_sql(&data_table.get_quoted_name(self)));
        sql.extend(self.get_post_alter_table_index_foreign_key_sql(diff)?);
        Ok(sql)
    }

    fn get_union_select_part_sql(&self, sub_query: &str) -> String {
        sub_query.to_owned()
    }
}
