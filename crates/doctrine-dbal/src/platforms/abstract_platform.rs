//! Port of Doctrine\DBAL\Platforms\AbstractPlatform.php, one group of
//! methods at a time.
//!
//! The PHP class is abstract: its abstract methods are required trait
//! methods, its concrete methods are default methods with upstream's body.
//! A platform (`SQLitePlatform`, ...) implements the trait and overrides
//! what its PHP class overrides. Tests stand in for PHPUnit's
//! `createMock(AbstractPlatform::class)` with their own implementation.
//!
//! Methods are added with the slice that first needs them, each group under
//! a comment that names the slice (Query, Types, ...), so slices merge
//! without reordering one another.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/AbstractPlatform.php

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use php_value::{Key, PhpArray, PhpString, Value};

use crate::exception::Exception;
use crate::exception::InvalidArgumentException;
use crate::sql::builder::{
    DefaultSelectSQLBuilder, DefaultUnionSQLBuilder, SelectSQLBuilder, UnionSQLBuilder,
    WithSQLBuilder,
};

// Slice SQLite platform: imports.
use super::date_interval_unit::{DateIntervalUnit, TrimMode};
use super::exception::NotSupported;
use super::keywords::KeywordList;
use super::no_columns_specified_for_table::no_columns_specified_for_table;
use super::platform_state::{
    CreateTableOptions, PlatformState, concat_value, string_argument, utf8,
};
use crate::deprecations;
use crate::schema::column_array_merge as array_merge_into;
use crate::schema::name::UnquotedIdentifierFolding;
use crate::schema::{
    Column, DefaultExpressionObject, ForeignKeyConstraint, Identifier, Index, Table, TableDiff,
    TypeObject, UniqueConstraint,
};
use crate::sql::Parser;
use crate::types::{self, PhpMappingType, Type, TypesException};
use crate::{LockMode, TransactionIsolationLevel};

/// `$this` as a `&dyn AbstractPlatform`, for the trait's default methods
/// that pass the platform on (`Type::getSQLDeclaration($column, $this)`).
pub trait AsDynPlatform {
    fn as_dyn(&self) -> &dyn AbstractPlatform;
}

impl<T: AbstractPlatform> AsDynPlatform for T {
    fn as_dyn(&self) -> &dyn AbstractPlatform {
        self
    }
}

/// Base contract for the SQL dialects (`AbstractPlatform`).
pub trait AbstractPlatform: AsDynPlatform {
    // Slice Types: the column declarations, boolean conversions and date
    // formats that Types\*Type call.

    /// `get_debug_type($platform)`: the class name PHP's messages show
    /// (`Doctrine\DBAL\Platforms\SQLitePlatform`, ...).
    fn class(&self) -> &str;

    /// Returns the SQL snippet that declares a boolean column.
    fn get_boolean_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception>;

    /// Returns the SQL snippet that declares a 4 byte integer column.
    fn get_integer_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception>;

    /// Returns the SQL snippet that declares an 8 byte integer column.
    fn get_big_int_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception>;

    /// Returns the SQL snippet that declares a 2 byte integer column.
    fn get_small_int_type_declaration_sql(&self, column: &PhpArray)
    -> Result<PhpString, Exception>;

    /// Returns the SQL snippet used to declare a column that can store
    /// characters in the ASCII character set.
    fn get_ascii_string_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        self.get_string_type_declaration_sql(column)
    }

    /// Returns the SQL snippet used to declare a string column type.
    fn get_string_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let length = column_length(
            column,
            "getVarcharTypeDeclarationSQLSnippet",
            "getCharTypeDeclarationSQLSnippet",
        )?;
        if !column_flag(column, "fixed") {
            return self
                .get_varchar_type_declaration_sql_snippet(length)
                .or_else(|e| invalid_column_declaration(column, e));
        }
        self.get_char_type_declaration_sql_snippet(length)
    }

    /// Returns the SQL snippet used to declare a binary string column type.
    fn get_binary_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let length = column_length(
            column,
            "getVarbinaryTypeDeclarationSQLSnippet",
            "getBinaryTypeDeclarationSQLSnippet",
        )?;
        let snippet = if column_flag(column, "fixed") {
            self.get_binary_type_declaration_sql_snippet(length)
        } else {
            self.get_varbinary_type_declaration_sql_snippet(length)
        };
        snippet.or_else(|e| invalid_column_declaration(column, e))
    }

    /// Returns the SQL snippet to declare an ENUM column: by default a
    /// VARCHAR as long as the longest value.
    fn get_enum_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let values = match column.get(&Key::from("values")) {
            Some(Value::Array(values)) if !values.is_empty() => values,
            _ => return Err(Exception::column_values_required(self.class(), "ENUM")),
        };
        let mut length = 0;
        for value in values.values() {
            length = length.max(mb_strlen(value)?);
        }
        if let Some(given) = column.get(&Key::from("length")).filter(|v| !v.is_null()) {
            let Value::Int(given) = given else {
                return Err(Exception::new(
                    Exception::UNSUPPORTED,
                    format!("an ENUM length of type {}", given.debug_type()),
                ));
            };
            if length > *given {
                return Err(Exception::invalid_argument(format!(
                    "Specified column length ({given}) is less than the maximum length of provided values ({length})."
                )));
            }
            length = *given;
        }
        let mut string_column = PhpArray::new();
        string_column.insert("length", Value::Int(length));
        self.get_string_type_declaration_sql(&string_column)
    }

    /// Returns the SQL snippet to declare a GUID/UUID column: by default a
    /// CHAR(36).
    fn get_guid_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let mut column = column.clone();
        column.insert("length", Value::Int(36));
        column.insert("fixed", Value::Bool(true));
        self.get_string_type_declaration_sql(&column)
    }

    /// Returns the SQL snippet to declare a JSON column: by default a CLOB.
    /// (Upstream also reports the deprecated `jsonb` column option through
    /// doctrine/deprecations, which is silent unless enabled; not ported.)
    fn get_json_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.get_clob_type_declaration_sql(column)
    }

    /// Returns the SQL snippet to declare a JSONB column.
    fn get_jsonb_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        self.get_json_type_declaration_sql(column)
    }

    /// `CHAR` or `CHAR(n)`.
    fn get_char_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from(match length {
            Some(length) => format!("CHAR({length})"),
            None => "CHAR".to_owned(),
        }))
    }

    /// `VARCHAR(n)`; a length is required.
    fn get_varchar_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        match length {
            Some(length) => Ok(PhpString::from(format!("VARCHAR({length})"))),
            None => Err(Exception::column_length_required(self.class(), "VARCHAR")),
        }
    }

    /// Returns the SQL snippet used to declare a fixed length binary column
    /// type.
    fn get_binary_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from(match length {
            Some(length) => format!("BINARY({length})"),
            None => "BINARY".to_owned(),
        }))
    }

    /// Returns the SQL snippet used to declare a variable length binary
    /// column type; a length is required.
    fn get_varbinary_type_declaration_sql_snippet(
        &self,
        length: Option<i64>,
    ) -> Result<PhpString, Exception> {
        match length {
            Some(length) => Ok(PhpString::from(format!("VARBINARY({length})"))),
            None => Err(Exception::column_length_required(self.class(), "VARBINARY")),
        }
    }

    /// Returns the SQL snippet used to declare a CLOB column type.
    fn get_clob_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception>;

    /// Returns the SQL Snippet used to declare a BLOB column type.
    fn get_blob_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception>;

    /// Returns the SQL snippet that declares a floating point column of
    /// arbitrary precision: `NUMERIC(precision, scale)`.
    fn get_decimal_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception> {
        let set = |name: &str| column.get(&Key::from(name)).filter(|v| !v.is_null());
        let Some(precision) = set("precision") else {
            return invalid_column_declaration(column, Exception::column_precision_required());
        };
        let Some(scale) = set("scale") else {
            return invalid_column_declaration(column, Exception::column_scale_required());
        };
        let (Some(precision), Some(scale)) = (precision.to_php_string(), scale.to_php_string())
        else {
            return Err(Exception::new(
                Exception::UNSUPPORTED,
                "a NUMERIC precision or scale that is an array or object",
            ));
        };
        let mut sql = b"NUMERIC(".to_vec();
        sql.extend_from_slice(precision.as_bytes());
        sql.extend_from_slice(b", ");
        sql.extend_from_slice(scale.as_bytes());
        sql.push(b')');
        Ok(PhpString::from(sql))
    }

    /// Converts a boolean, or the booleans of an array, to ints (the
    /// literal case); any other value is returned as it is.
    fn convert_booleans(&self, item: &Value) -> Value {
        match item {
            Value::Array(array) => {
                let mut converted = array.clone();
                for (key, value) in array.iter() {
                    if let Value::Bool(b) = value {
                        converted.insert(key.clone(), Value::Int(i64::from(*b)));
                    }
                }
                Value::Array(converted)
            }
            Value::Bool(b) => Value::Int(i64::from(*b)),
            other => other.clone(),
        }
    }

    /// `(bool) $item`, `null` staying `null`.
    fn convert_from_boolean(&self, item: &Value) -> Option<bool> {
        if item.is_null() {
            return None;
        }
        Some(item.is_truthy())
    }

    /// Converts booleans for a prepared statement.
    fn convert_booleans_to_database_value(&self, item: &Value) -> Value {
        self.convert_booleans(item)
    }

    /// Obtains DBMS specific SQL to be used to create datetime columns.
    fn get_date_time_type_declaration_sql(&self, column: &PhpArray)
    -> Result<PhpString, Exception>;

    /// Obtains DBMS specific SQL to be used to create datetime with timezone
    /// offset columns.
    fn get_date_time_tz_type_declaration_sql(
        &self,
        column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        self.get_date_time_type_declaration_sql(column)
    }

    /// Obtains DBMS specific SQL to be used to create date columns.
    fn get_date_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception>;

    /// Obtains DBMS specific SQL to be used to create time columns.
    fn get_time_type_declaration_sql(&self, column: &PhpArray) -> Result<PhpString, Exception>;

    fn get_float_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DOUBLE PRECISION"))
    }

    fn get_small_float_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("REAL"))
    }

    /// The `date()` format of a stored datetime value.
    fn get_date_time_format_string(&self) -> String {
        "Y-m-d H:i:s".to_owned()
    }

    /// The `date()` format of a stored datetime with timezone value.
    fn get_date_time_tz_format_string(&self) -> String {
        "Y-m-d H:i:s".to_owned()
    }

    /// The `date()` format of a stored date value.
    fn get_date_format_string(&self) -> String {
        "Y-m-d".to_owned()
    }

    /// The `date()` format of a stored time value.
    fn get_time_format_string(&self) -> String {
        "H:i:s".to_owned()
    }

    // End of slice Types.

    // Slice Query: the SQL builders and LIMIT/UNION methods that
    // Query\QueryBuilder and SQL\Builder\Default*SQLBuilder use.

    /// `createSelectSQLBuilder()`.
    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(
            self,
            Some("FOR UPDATE"),
            Some("SKIP LOCKED"),
        ))
    }

    /// `createUnionSQLBuilder()`.
    fn create_union_sql_builder(&self) -> Box<dyn UnionSQLBuilder + '_> {
        Box::new(DefaultUnionSQLBuilder::new(self))
    }

    /// `createWithSQLBuilder()`.
    fn create_with_sql_builder(&self) -> WithSQLBuilder {
        WithSQLBuilder::new()
    }

    /// Adds a driver-specific LIMIT clause to the query
    /// (`modifyLimitQuery()`). Final in PHP: platforms override
    /// [`Self::do_modify_limit_query`] instead.
    fn modify_limit_query(
        &self,
        query: &str,
        limit: Option<i64>,
        offset: i64,
    ) -> Result<String, InvalidArgumentException> {
        if offset < 0 {
            return Err(InvalidArgumentException::new(format!(
                "Offset must be a positive integer or zero, {offset} given."
            )));
        }

        Ok(self.do_modify_limit_query(query, limit, offset))
    }

    /// Adds an platform-specific LIMIT clause to the query
    /// (`doModifyLimitQuery()`, protected in PHP).
    fn do_modify_limit_query(&self, query: &str, limit: Option<i64>, offset: i64) -> String {
        let mut query = query.to_owned();

        if let Some(limit) = limit {
            query = format!("{query} LIMIT {limit}");
        }

        if offset > 0 {
            query = format!("{query} OFFSET {offset}");
        }

        query
    }

    /// Returns the SQL of a part of a UNION query
    /// (`getUnionSelectPartSQL()`).
    fn get_union_select_part_sql(&self, sub_query: &str) -> String {
        format!("({sub_query})")
    }

    /// Returns the `UNION ALL` keyword.
    fn get_union_all_sql(&self) -> String {
        "UNION ALL".to_owned()
    }

    /// Returns the compatible `UNION DISTINCT` keyword.
    fn get_union_distinct_sql(&self) -> String {
        "UNION".to_owned()
    }

    // End of slice Query.

    // Slice SQLite platform: the rest of the class: expressions, type
    // mappings, the CREATE/ALTER/DROP SQL of schema objects, column
    // declarations and the platform's capabilities. The PHP-abstract
    // methods of this slice have a default that throws `NotSupported`
    // (their body is the subclass's), so test doubles need not define
    // them. `createSchemaManager()`, `createMetadataProvider()` and
    // `getAlterSchemaSQL()` / the sequence methods taking a `Sequence`
    // are not ported (they need a Connection, a SchemaDiff or a
    // Sequence).

    /// The instance state (type mapping, keyword list); `None` for a
    /// platform that keeps none (then nothing is cached and
    /// `registerDoctrineTypeMapping()` is lost).
    fn platform_state(&self) -> Option<&PlatformState> {
        None
    }

    /// `getUnquotedIdentifierFolding()`: as the constructor was given
    /// (`UPPER` by default).
    fn get_unquoted_identifier_folding(&self) -> UnquotedIdentifierFolding {
        UnquotedIdentifierFolding::Upper
    }

    /// `_getCommonIntegerTypeDeclarationSQL()` (abstract).
    fn get_common_integer_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Err(not_supported("_getCommonIntegerTypeDeclarationSQL"))
    }

    /// `initializeDoctrineTypeMappings()` (abstract): the platform's own
    /// database type => Doctrine type pairs.
    fn initialize_doctrine_type_mappings(&self) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }

    /// `initializeAllDoctrineTypeMappings()`.
    fn initialize_all_doctrine_type_mappings(&self) -> Result<BTreeMap<String, String>, Exception> {
        let mut mapping: BTreeMap<String, String> = self
            .initialize_doctrine_type_mappings()
            .into_iter()
            .map(|(db_type, type_name)| (db_type.to_owned(), type_name.to_owned()))
            .collect();
        for (type_name, _) in types::get_types_map() {
            let type_ = types::get_type(&type_name).map_err(Exception::from)?;
            for db_type in type_.get_mapped_database_types(self.as_dyn()) {
                mapping.insert(utf8(&db_type)?.to_ascii_lowercase(), type_name.clone());
            }
        }
        Ok(mapping)
    }

    #[doc(hidden)]
    fn with_doctrine_type_mapping<T>(
        &self,
        f: impl FnOnce(&mut BTreeMap<String, String>) -> T,
    ) -> Result<T, Exception>
    where
        Self: Sized,
    {
        if let Some(state) = self.platform_state() {
            state.with_type_mapping(|| self.initialize_all_doctrine_type_mappings(), f)
        } else {
            let mut mapping = self.initialize_all_doctrine_type_mappings()?;
            Ok(f(&mut mapping))
        }
    }

    /// Registers a doctrine type to be used in conjunction with a column
    /// type of this platform.
    fn register_doctrine_type_mapping(
        &self,
        db_type: &str,
        doctrine_type: &str,
    ) -> Result<(), Exception>
    where
        Self: Sized,
    {
        self.with_doctrine_type_mapping(|_| ())?;
        if !types::has_type(doctrine_type) {
            return Err(TypesException::TypeNotFound(doctrine_type.to_owned()).into());
        }
        self.with_doctrine_type_mapping(|mapping| {
            mapping.insert(db_type.to_ascii_lowercase(), doctrine_type.to_owned());
        })
    }

    /// Gets the Doctrine type that is mapped for the given database
    /// column type.
    fn get_doctrine_type_mapping(&self, db_type: &str) -> Result<String, Exception>
    where
        Self: Sized,
    {
        let db_type = db_type.to_ascii_lowercase();
        let class = self.class().to_owned();
        self.with_doctrine_type_mapping(|mapping| mapping.get(&db_type).cloned())?
            .ok_or_else(|| {
                Exception::invalid_argument(format!(
                    "Unknown database type \"{db_type}\" requested, {class} may not support it."
                ))
            })
    }

    /// Checks if a database type is currently supported by this platform.
    fn has_doctrine_type_mapping_for(&self, db_type: &str) -> Result<bool, Exception>
    where
        Self: Sized,
    {
        let db_type = db_type.to_ascii_lowercase();
        self.with_doctrine_type_mapping(|mapping| mapping.contains_key(&db_type))
    }

    /// Returns the regular expression operator.
    fn get_regexp_expression(&self) -> Result<String, Exception> {
        Err(not_supported("getRegexpExpression"))
    }

    /// Returns the SQL snippet to get the length of a text column in
    /// characters.
    fn get_length_expression(&self, string: &str) -> String {
        format!("LENGTH({string})")
    }

    /// Returns the SQL snippet to get the remainder of the operation of
    /// division of dividend by divisor.
    fn get_mod_expression(&self, dividend: &str, divisor: &str) -> String {
        format!("MOD({dividend}, {divisor})")
    }

    /// Returns the SQL snippet to trim a string.
    fn get_trim_expression(&self, str: &str, mode: TrimMode, char: Option<&str>) -> String {
        let mut tokens: Vec<&str> = Vec::new();
        match mode {
            TrimMode::Unspecified => {}
            TrimMode::Leading => tokens.push("LEADING"),
            TrimMode::Trailing => tokens.push("TRAILING"),
            TrimMode::Both => tokens.push("BOTH"),
        }
        if let Some(char) = char {
            tokens.push(char);
        }
        if !tokens.is_empty() {
            tokens.push("FROM");
        }
        tokens.push(str);
        format!("TRIM({})", tokens.join(" "))
    }

    /// Returns the SQL snippet to get the position of the first
    /// occurrence of the substring in the string (abstract).
    fn get_locate_expression(
        &self,
        _string: &str,
        _substring: &str,
        _start: Option<&str>,
    ) -> Result<String, Exception> {
        Err(not_supported("getLocateExpression"))
    }

    /// Returns an SQL snippet to get a substring inside the string.
    fn get_substring_expression(&self, string: &str, start: &str, length: Option<&str>) -> String {
        match length {
            None => format!("SUBSTRING({string} FROM {start})"),
            Some(length) => format!("SUBSTRING({string} FROM {start} FOR {length})"),
        }
    }

    /// Returns a SQL snippet to concatenate the given strings.
    fn get_concat_expression(&self, strings: &[&str]) -> String {
        strings.join(" || ")
    }

    /// Returns the SQL to calculate the difference in days between the
    /// two passed dates (abstract).
    fn get_date_diff_expression(&self, _date1: &str, _date2: &str) -> Result<String, Exception> {
        Err(not_supported("getDateDiffExpression"))
    }

    fn get_date_add_seconds_expression(
        &self,
        date: &str,
        seconds: &str,
    ) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "+", seconds, DateIntervalUnit::Second)
    }

    fn get_date_sub_seconds_expression(
        &self,
        date: &str,
        seconds: &str,
    ) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "-", seconds, DateIntervalUnit::Second)
    }

    fn get_date_add_minutes_expression(
        &self,
        date: &str,
        minutes: &str,
    ) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "+", minutes, DateIntervalUnit::Minute)
    }

    fn get_date_sub_minutes_expression(
        &self,
        date: &str,
        minutes: &str,
    ) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "-", minutes, DateIntervalUnit::Minute)
    }

    fn get_date_add_hour_expression(&self, date: &str, hours: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "+", hours, DateIntervalUnit::Hour)
    }

    fn get_date_sub_hour_expression(&self, date: &str, hours: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "-", hours, DateIntervalUnit::Hour)
    }

    fn get_date_add_days_expression(&self, date: &str, days: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "+", days, DateIntervalUnit::Day)
    }

    fn get_date_sub_days_expression(&self, date: &str, days: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "-", days, DateIntervalUnit::Day)
    }

    fn get_date_add_weeks_expression(&self, date: &str, weeks: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "+", weeks, DateIntervalUnit::Week)
    }

    fn get_date_sub_weeks_expression(&self, date: &str, weeks: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "-", weeks, DateIntervalUnit::Week)
    }

    fn get_date_add_month_expression(&self, date: &str, months: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "+", months, DateIntervalUnit::Month)
    }

    fn get_date_sub_month_expression(&self, date: &str, months: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "-", months, DateIntervalUnit::Month)
    }

    fn get_date_add_quarters_expression(
        &self,
        date: &str,
        quarters: &str,
    ) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "+", quarters, DateIntervalUnit::Quarter)
    }

    fn get_date_sub_quarters_expression(
        &self,
        date: &str,
        quarters: &str,
    ) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "-", quarters, DateIntervalUnit::Quarter)
    }

    fn get_date_add_years_expression(&self, date: &str, years: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "+", years, DateIntervalUnit::Year)
    }

    fn get_date_sub_years_expression(&self, date: &str, years: &str) -> Result<String, Exception> {
        self.get_date_arithmetic_interval_expression(date, "-", years, DateIntervalUnit::Year)
    }

    /// `getDateArithmeticIntervalExpression()` (abstract, protected).
    fn get_date_arithmetic_interval_expression(
        &self,
        _date: &str,
        _operator: &str,
        _interval: &str,
        _unit: DateIntervalUnit,
    ) -> Result<String, Exception> {
        Err(not_supported("getDateArithmeticIntervalExpression"))
    }

    /// Generates the SQL expression which represents the given date
    /// interval multiplied by a number (protected).
    fn multiply_interval(&self, interval: &str, multiplier: i64) -> String {
        format!("({interval} * {multiplier})")
    }

    /// Returns the SQL bit AND comparison expression.
    fn get_bit_and_comparison_expression(&self, value1: &str, value2: &str) -> String {
        format!("({value1} & {value2})")
    }

    /// Returns the SQL bit OR comparison expression.
    fn get_bit_or_comparison_expression(&self, value1: &str, value2: &str) -> String {
        format!("({value1} | {value2})")
    }

    /// Returns the SQL expression which represents the currently
    /// selected database (abstract).
    fn get_current_database_expression(&self) -> Result<String, Exception> {
        Err(not_supported("getCurrentDatabaseExpression"))
    }

    /// Honors that some SQL vendors such as `MsSql` use table hints for
    /// locking instead of the `FOR UPDATE` and `FOR SHARE` clause.
    fn append_lock_hint(&self, from_clause: &str, _lock_mode: LockMode) -> String {
        from_clause.to_owned()
    }

    /// Returns the SQL snippet to drop an existing table.
    fn get_drop_table_sql(&self, table: &str) -> String {
        format!("DROP TABLE {table}")
    }

    /// Returns the SQL to safely drop a temporary table WITHOUT
    /// implicitly committing an open transaction.
    fn get_drop_temporary_table_sql(&self, table: &str) -> String {
        self.get_drop_table_sql(table)
    }

    /// Returns the SQL to drop an index from a table.
    fn get_drop_index_sql(&self, name: &str, _table: &str) -> String {
        format!("DROP INDEX {name}")
    }

    /// Returns the SQL to drop a constraint (protected).
    fn get_drop_constraint_sql(&self, name: &str, table: &str) -> String {
        format!("ALTER TABLE {table} DROP CONSTRAINT {name}")
    }

    /// Returns the SQL to drop a foreign key.
    fn get_drop_foreign_key_sql(
        &self,
        foreign_key: &str,
        table: &str,
    ) -> Result<String, Exception> {
        Ok(format!(
            "ALTER TABLE {table} DROP FOREIGN KEY {foreign_key}"
        ))
    }

    /// Returns the SQL to drop a unique constraint.
    fn get_drop_unique_constraint_sql(&self, name: &str, table_name: &str) -> String {
        self.get_drop_constraint_sql(name, table_name)
    }

    /// Returns the SQL statement(s) to create a table with the specified
    /// name, columns and constraints on this platform.
    fn get_create_table_sql(&self, table: &Table) -> Result<Vec<String>, Exception> {
        self.build_create_table_sql(table, true)
    }

    /// `getCreateTableWithoutForeignKeysSQL()` (final, protected).
    fn get_create_table_without_foreign_keys_sql(
        &self,
        table: &Table,
    ) -> Result<Vec<String>, Exception> {
        self.build_create_table_sql(table, false)
    }

    /// `getColumnType($column)` of a column definition array (final,
    /// protected).
    fn get_column_type(&self, column: &PhpArray) -> Result<Arc<dyn Type>, Exception> {
        if let Some(type_) = self.get_column_type_or_null(column)? {
            Ok(type_)
        } else {
            let name = match column.get(&Key::from("name")) {
                Some(Value::String(name)) => name.as_bytes().to_vec(),
                _ => Vec::new(),
            };
            Err(Exception::missing_column_type(&name))
        }
    }

    /// `getColumnTypeOrNull($column)` (final, protected).
    fn get_column_type_or_null(
        &self,
        column: &PhpArray,
    ) -> Result<Option<Arc<dyn Type>>, Exception> {
        if let Some(Value::String(type_name)) = column.get(&Key::from("typeName")) {
            let type_name = utf8(type_name)?;
            return types::get_type(&type_name)
                .map(Some)
                .map_err(Exception::from);
        }
        if let Some(type_) = column.get(&Key::from("type")).and_then(TypeObject::of) {
            deprecations::trigger("https://github.com/doctrine/dbal/pull/7490", || {
                "Providing a Doctrine\\DBAL\\Types\\Type instance under the \"type\" key of a column definition array is deprecated. Provide the type name under the \"typeName\" key instead.".to_owned()
            });
            return Ok(Some(Arc::clone(type_)));
        }
        Ok(None)
    }

    /// `buildCreateTableSQL($table, $createForeignKeys)` (private).
    fn build_create_table_sql(
        &self,
        table: &Table,
        create_foreign_keys: bool,
    ) -> Result<Vec<String>, Exception> {
        if table.get_columns().is_empty() {
            return Err(no_columns_specified_for_table(&table.get_name()));
        }

        let table_name = table.get_quoted_name(self);
        let mut options = CreateTableOptions {
            table: table.get_options().clone(),
            primary: Some(Vec::new()),
            primary_index: None,
            indexes: Some(Vec::new()),
            unique_constraints: Some(Vec::new()),
            foreign_keys: Some(Vec::new()),
        };

        for index in table.get_indexes().values() {
            if !index.is_primary() {
                options
                    .indexes
                    .get_or_insert_with(Vec::new)
                    .push(index.clone());
                continue;
            }
            options.primary = Some(index.get_quoted_columns(self));
            options.primary_index = Some(index.clone());
        }

        options.unique_constraints =
            Some(table.get_unique_constraints().values().cloned().collect());

        if create_foreign_keys {
            options.foreign_keys = Some(table.get_foreign_keys().values().cloned().collect());
        }

        let columns = table
            .get_columns()
            .into_iter()
            .map(|column| self.column_to_array(column))
            .collect::<Result<Vec<_>, _>>()?;

        let mut sql = self.get_create_table_sql_internal(&table_name, &columns, options)?;

        if self.supports_comment_on_statement() {
            if table.has_option("comment") {
                let comment = string_argument(
                    &table.get_option("comment"),
                    "Doctrine\\DBAL\\Platforms\\AbstractPlatform::getCommentOnTableSQL",
                    2,
                    "comment",
                )?;
                sql.push(self.get_comment_on_table_sql(&table_name, &comment));
            }
            for column in table.get_columns() {
                let comment = column.get_comment();
                if !comment.is_empty() {
                    sql.push(self.get_comment_on_column_sql(
                        &table_name,
                        &column.get_quoted_name(self),
                        comment,
                    ));
                }
            }
        }

        Ok(sql)
    }

    /// Returns the SQL to create the given tables (the foreign keys
    /// after all tables).
    fn get_create_tables_sql(&self, tables: &[Table]) -> Result<Vec<String>, Exception> {
        let mut sql = Vec::new();
        for table in tables {
            sql.extend(self.get_create_table_without_foreign_keys_sql(table)?);
        }
        for table in tables {
            for foreign_key in table.get_foreign_keys().values() {
                sql.push(
                    self.get_create_foreign_key_sql(foreign_key, &table.get_quoted_name(self))?,
                );
            }
        }
        Ok(sql)
    }

    /// Returns the SQL to drop the given tables (the foreign keys first).
    fn get_drop_tables_sql(&self, tables: &[Table]) -> Result<Vec<String>, Exception> {
        let mut sql = Vec::new();
        for table in tables {
            for foreign_key in table.get_foreign_keys().values() {
                sql.push(self.get_drop_foreign_key_sql(
                    &foreign_key.get_quoted_name(self),
                    &table.get_quoted_name(self),
                )?);
            }
        }
        for table in tables {
            sql.push(self.get_drop_table_sql(&table.get_quoted_name(self)));
        }
        Ok(sql)
    }

    /// `getCommentOnTableSQL()` (protected).
    fn get_comment_on_table_sql(&self, table_name: &str, comment: &str) -> String {
        let table_name = Identifier::new(table_name, false);
        format!(
            "COMMENT ON TABLE {} IS {}",
            table_name.get_quoted_name(self),
            self.quote_string_literal(comment)
        )
    }

    /// `getCommentOnColumnSQL()`.
    fn get_comment_on_column_sql(
        &self,
        table_name: &str,
        column_name: &str,
        comment: &str,
    ) -> String {
        let table_name = Identifier::new(table_name, false);
        let column_name = Identifier::new(column_name, false);
        format!(
            "COMMENT ON COLUMN {}.{} IS {}",
            table_name.get_quoted_name(self),
            column_name.get_quoted_name(self),
            self.quote_string_literal(comment)
        )
    }

    /// Returns the SQL to create inline comment on a column.
    fn get_inline_column_comment_sql(&self, comment: &str) -> Result<String, Exception> {
        if !self.supports_inline_column_comments() {
            return Err(not_supported("getInlineColumnCommentSQL"));
        }
        Ok(format!("COMMENT {}", self.quote_string_literal(comment)))
    }

    /// `_getCreateTableSQL($name, $columns, $options)` (protected):
    /// the SQL used to create a table.
    fn get_create_table_sql_internal(
        &self,
        name: &str,
        columns: &[PhpArray],
        options: CreateTableOptions,
    ) -> Result<Vec<String>, Exception> {
        validate_create_table_options(
            &options,
            "Doctrine\\DBAL\\Platforms\\AbstractPlatform::_getCreateTableSQL",
        );

        let mut column_list_sql = self.get_column_declaration_list_sql(columns)?;

        for definition in options.unique_constraints.iter().flatten() {
            column_list_sql.push_str(", ");
            column_list_sql.push_str(&self.get_unique_constraint_declaration_sql(definition)?);
        }

        if let Some(primary) = options.primary.as_ref().filter(|p| !p.is_empty()) {
            let _ = write!(
                column_list_sql,
                ", PRIMARY KEY ({})",
                array_unique(primary).join(", ")
            );
        }

        for definition in options.indexes.iter().flatten() {
            column_list_sql.push_str(", ");
            column_list_sql.push_str(&self.get_index_declaration_sql(definition)?);
        }

        let mut query = format!("CREATE TABLE {name} ({column_list_sql}");
        let definitions: Vec<Value> = columns.iter().cloned().map(Value::Array).collect();
        let check = self.get_check_declaration_sql(&definitions)?;
        if !check.is_empty() {
            query.push_str(", ");
            query.push_str(&check);
        }
        query.push(')');

        let mut sql = vec![query];
        for definition in options.foreign_keys.iter().flatten() {
            sql.push(self.get_create_foreign_key_sql(definition, name)?);
        }
        Ok(sql)
    }

    /// `getCreateTemporaryTableSnippetSQL()`.
    fn get_create_temporary_table_snippet_sql(&self) -> String {
        "CREATE TEMPORARY TABLE".to_owned()
    }

    /// Returns the SQL to drop an existing sequence.
    fn get_drop_sequence_sql(&self, name: &str) -> Result<String, Exception> {
        if !self.supports_sequences() {
            return Err(not_supported("getDropSequenceSQL"));
        }
        Ok(format!("DROP SEQUENCE {name}"))
    }

    /// Returns the SQL to create an index on a table on this platform.
    fn get_create_index_sql(&self, index: &Index, table: &str) -> Result<String, Exception> {
        let name = index.get_quoted_name(self);
        if index.get_columns().is_empty() {
            return Err(Exception::invalid_argument(format!(
                "Incomplete or invalid index definition {name} on table {table}"
            )));
        }
        if index.is_primary() {
            return self.get_create_primary_key_sql(index, table);
        }
        Ok(format!(
            "CREATE {}INDEX {name} ON {table} ({}){}",
            self.get_create_index_sql_flags(index),
            index.get_quoted_columns(self).join(", "),
            self.get_partial_index_sql(index)?
        ))
    }

    /// Adds condition for partial index (protected).
    fn get_partial_index_sql(&self, index: &Index) -> Result<String, Exception> {
        if self.supports_partial_indexes() && index.has_option("where") {
            let predicate = index.get_option("where").cloned().unwrap_or(Value::Null);
            return Ok(format!(" WHERE {}", concat_value(&predicate)?));
        }
        Ok(String::new())
    }

    /// Adds additional flags for index generation (protected).
    fn get_create_index_sql_flags(&self, index: &Index) -> String {
        if index.is_unique() {
            "UNIQUE ".to_owned()
        } else {
            String::new()
        }
    }

    /// Returns the SQL to create an unnamed primary key constraint
    /// (deprecated).
    fn get_create_primary_key_sql(&self, index: &Index, table: &str) -> Result<String, Exception> {
        Ok(format!(
            "ALTER TABLE {table} ADD PRIMARY KEY ({})",
            index.get_quoted_columns(self).join(", ")
        ))
    }

    /// Returns the SQL to create a named schema.
    fn get_create_schema_sql(&self, schema_name: &str) -> Result<String, Exception> {
        if !self.supports_schemas() {
            return Err(not_supported("getCreateSchemaSQL"));
        }
        Ok(format!("CREATE SCHEMA {schema_name}"))
    }

    /// Returns the SQL to create a unique constraint on a table on this
    /// platform.
    fn get_create_unique_constraint_sql(
        &self,
        constraint: &UniqueConstraint,
        table_name: &str,
    ) -> Result<String, Exception> {
        Ok(format!(
            "ALTER TABLE {table_name} ADD {}",
            self.get_unique_constraint_declaration_sql(constraint)?
        ))
    }

    /// Returns the SQL snippet to drop a schema.
    fn get_drop_schema_sql(&self, schema_name: &str) -> Result<String, Exception> {
        if !self.supports_schemas() {
            return Err(not_supported("getDropSchemaSQL"));
        }
        Ok(format!("DROP SCHEMA {schema_name}"))
    }

    /// Quotes a string so that it can be safely used as a table or
    /// column name, even if it is a reserved word of the platform
    /// (deprecated).
    fn quote_identifier(&self, identifier: &str) -> String {
        deprecations::trigger("https://github.com/doctrine/dbal/pull/6590", || {
            "Method Doctrine\\DBAL\\Platforms\\AbstractPlatform::quoteIdentifier is deprecated and will be removed in 5.0.\nUse quoteSingleIdentifier() individually for each part of a qualified name instead.".to_owned()
        });
        identifier
            .split('.')
            .map(|part| self.quote_single_identifier(part))
            .collect::<Vec<_>>()
            .join(".")
    }

    /// Quotes a single identifier (no dot chain separation).
    fn quote_single_identifier(&self, str: &str) -> String {
        format!("\"{}\"", str.replace('"', "\"\""))
    }

    /// Returns the SQL to create a new foreign key.
    fn get_create_foreign_key_sql(
        &self,
        foreign_key: &ForeignKeyConstraint,
        table: &str,
    ) -> Result<String, Exception> {
        Ok(format!(
            "ALTER TABLE {table} ADD {}",
            self.get_foreign_key_declaration_sql(foreign_key)?
        ))
    }

    /// Gets the SQL statements for altering an existing table (abstract).
    fn get_alter_table_sql(&self, _diff: &TableDiff) -> Result<Vec<String>, Exception> {
        Err(not_supported("getAlterTableSQL"))
    }

    /// `getRenameTableSQL()`.
    fn get_rename_table_sql(&self, old_name: &str, new_name: &str) -> String {
        format!("ALTER TABLE {old_name} RENAME TO {new_name}")
    }

    /// `getPreAlterTableIndexForeignKeySQL()` (protected).
    fn get_pre_alter_table_index_foreign_key_sql(
        &self,
        diff: &TableDiff,
    ) -> Result<Vec<String>, Exception> {
        let table_name_sql = diff.get_old_table().get_quoted_name(self);
        let mut sql = Vec::new();
        for foreign_key in diff.get_dropped_foreign_keys() {
            sql.push(
                self.get_drop_foreign_key_sql(&foreign_key.get_quoted_name(self), &table_name_sql)?,
            );
        }
        for foreign_key in diff.modified_foreign_keys() {
            sql.push(
                self.get_drop_foreign_key_sql(&foreign_key.get_quoted_name(self), &table_name_sql)?,
            );
        }
        for index in diff.get_dropped_indexes() {
            sql.push(self.get_drop_index_sql(&index.get_quoted_name(self), &table_name_sql));
        }
        for index in diff.modified_indexes() {
            sql.push(self.get_drop_index_sql(&index.get_quoted_name(self), &table_name_sql));
        }
        Ok(sql)
    }

    /// `getPostAlterTableIndexForeignKeySQL()` (protected).
    fn get_post_alter_table_index_foreign_key_sql(
        &self,
        diff: &TableDiff,
    ) -> Result<Vec<String>, Exception> {
        let mut sql = Vec::new();
        let table_name_sql = diff.get_old_table().get_quoted_name(self);
        for foreign_key in diff.get_added_foreign_keys() {
            sql.push(self.get_create_foreign_key_sql(foreign_key, &table_name_sql)?);
        }
        for foreign_key in diff.modified_foreign_keys() {
            sql.push(self.get_create_foreign_key_sql(foreign_key, &table_name_sql)?);
        }
        for index in diff.get_added_indexes() {
            sql.push(self.get_create_index_sql(index, &table_name_sql)?);
        }
        for index in diff.modified_indexes() {
            sql.push(self.get_create_index_sql(index, &table_name_sql)?);
        }
        for (old_index_name, index) in diff.renamed_indexes() {
            let old_index_name = Identifier::new(old_index_name, false);
            sql.extend(self.get_rename_index_sql(
                &old_index_name.get_quoted_name(self),
                index,
                &table_name_sql,
            )?);
        }
        Ok(sql)
    }

    /// Returns the SQL for renaming an index on a table (protected).
    fn get_rename_index_sql(
        &self,
        old_index_name: &str,
        index: &Index,
        table_name: &str,
    ) -> Result<Vec<String>, Exception> {
        Ok(vec![
            self.get_drop_index_sql(old_index_name, table_name),
            self.get_create_index_sql(index, table_name)?,
        ])
    }

    /// Returns the SQL for renaming a column (protected).
    fn get_rename_column_sql(
        &self,
        table_name: &str,
        old_column_name: &str,
        new_column_name: &str,
    ) -> Vec<String> {
        vec![format!(
            "ALTER TABLE {table_name} RENAME COLUMN {old_column_name} TO {new_column_name}"
        )]
    }

    /// Gets declaration of a number of columns in bulk.
    fn get_column_declaration_list_sql(&self, columns: &[PhpArray]) -> Result<String, Exception> {
        let mut declarations = Vec::new();
        for column in columns {
            let name = match column.get(&Key::from("name")) {
                Some(name) => string_argument(
                    name,
                    "Doctrine\\DBAL\\Platforms\\AbstractPlatform::getColumnDeclarationSQL",
                    1,
                    "name",
                )?,
                None => string_argument(
                    &Value::Null,
                    "Doctrine\\DBAL\\Platforms\\AbstractPlatform::getColumnDeclarationSQL",
                    1,
                    "name",
                )?,
            };
            declarations.push(self.get_column_declaration_sql(&name, column)?);
        }
        Ok(declarations.join(", "))
    }

    /// Obtains DBMS specific SQL code portion needed to declare a generic
    /// type column to be used in statements like CREATE TABLE.
    fn get_column_declaration_sql(
        &self,
        name: &str,
        column: &PhpArray,
    ) -> Result<String, Exception> {
        let declaration = match column.get(&Key::from("columnDefinition")) {
            Some(definition) if !definition.is_null() => concat_value(definition)?,
            _ => {
                let default = self.get_default_value_declaration_sql(column)?;
                let charset = match column.get(&Key::from("charset")) {
                    Some(charset) if charset.is_truthy() => format!(
                        " {}",
                        self.get_column_charset_declaration_sql(&string_argument(
                            charset,
                            "Doctrine\\DBAL\\Platforms\\AbstractPlatform::getColumnCharsetDeclarationSQL",
                            1,
                            "charset",
                        )?)
                    ),
                    _ => String::new(),
                };
                let collation = match column.get(&Key::from("collation")) {
                    Some(collation) if collation.is_truthy() => format!(
                        " {}",
                        self.get_column_collation_declaration_sql(&string_argument(
                            collation,
                            "Doctrine\\DBAL\\Platforms\\AbstractPlatform::getColumnCollationDeclarationSQL",
                            1,
                            "collation",
                        )?)
                    ),
                    _ => String::new(),
                };
                let notnull = if column_flag(column, "notnull") {
                    " NOT NULL"
                } else {
                    ""
                };
                let type_decl = utf8(
                    &self
                        .get_column_type(column)?
                        .get_sql_declaration(column, self.as_dyn())?,
                )?;
                let mut declaration = format!("{type_decl}{charset}{default}{notnull}{collation}");
                if self.supports_inline_column_comments()
                    && let Some(comment) = column.get(&Key::from("comment"))
                {
                    let is_empty = matches!(comment, Value::String(s) if s.is_empty());
                    if !comment.is_null() && !is_empty {
                        let comment = string_argument(
                            comment,
                            "Doctrine\\DBAL\\Platforms\\AbstractPlatform::getInlineColumnCommentSQL",
                            1,
                            "comment",
                        )?;
                        declaration.push(' ');
                        declaration.push_str(&self.get_inline_column_comment_sql(&comment)?);
                    }
                }
                declaration
            }
        };
        Ok(format!("{name} {declaration}"))
    }

    /// Obtains DBMS specific SQL code portion needed to set a default
    /// value declaration to be used in statements like CREATE TABLE.
    fn get_default_value_declaration_sql(&self, column: &PhpArray) -> Result<String, Exception> {
        let default = match column.get(&Key::from("default")) {
            None | Some(Value::Null) => {
                return Ok(if column_flag(column, "notnull") {
                    String::new()
                } else {
                    " DEFAULT NULL".to_owned()
                });
            }
            Some(default) => default,
        };

        if let Some(expression) = DefaultExpressionObject::of(default) {
            return Ok(format!(" DEFAULT {}", expression.to_sql(self.as_dyn())));
        }

        let Some(type_) = self.get_column_type_or_null(column)? else {
            return Ok(format!(" DEFAULT '{}'", concat_value(default)?));
        };

        let mapping = type_.php_mapping_type();
        if mapping == Some(PhpMappingType::Integer) {
            return Ok(format!(" DEFAULT {}", concat_value(default)?));
        }

        let is =
            |sql: String| matches!(default, Value::String(s) if s.as_bytes() == sql.as_bytes());
        let current = [
            (
                PhpMappingType::DateTime,
                self.get_current_timestamp_sql(),
                "CurrentTimestamp",
            ),
            (
                PhpMappingType::Time,
                self.get_current_time_sql(),
                "CurrentTime",
            ),
            (
                PhpMappingType::Date,
                self.get_current_date_sql(),
                "CurrentDate",
            ),
        ];
        for (kind, sql, class) in current {
            if mapping == Some(kind) && is(sql.clone()) {
                deprecations::trigger("https://github.com/doctrine/dbal/pull/7195", || {
                    format!(
                        "Using \"{sql}\" as a column default value is deprecated. Use a {class} instance instead."
                    )
                });
                return Ok(format!(" DEFAULT {sql}"));
            }
        }

        if type_.class() == "Doctrine\\DBAL\\Types\\BooleanType" {
            return Ok(format!(
                " DEFAULT {}",
                concat_value(&self.convert_booleans(default))?
            ));
        }

        if matches!(default, Value::Int(_) | Value::Float(_)) {
            return Ok(format!(" DEFAULT {}", concat_value(default)?));
        }

        if mapping == Some(PhpMappingType::Float) {
            return Ok(format!(
                " DEFAULT {}",
                php_value::numeric::float_to_string(php_value::numeric::float_cast(default))
            ));
        }

        let default = string_argument(
            default,
            "Doctrine\\DBAL\\Platforms\\AbstractPlatform::quoteStringLiteral",
            1,
            "str",
        )?;
        Ok(format!(" DEFAULT {}", self.quote_string_literal(&default)))
    }

    /// Obtains DBMS specific SQL code portion needed to set a CHECK
    /// constraint declaration to be used in statements like CREATE
    /// TABLE (a definition is a column array, or a string: deprecated).
    fn get_check_declaration_sql(&self, definition: &[Value]) -> Result<String, Exception> {
        let mut constraints = Vec::new();
        for def in definition {
            match def {
                Value::String(def) => {
                    deprecations::trigger("https://github.com/doctrine/dbal/pull/6805", || {
                        "Passing column definition to Doctrine\\DBAL\\Platforms\\AbstractPlatform::getCheckDeclarationSQL() as string is deprecated. Pass the definition as array instead.".to_owned()
                    });
                    constraints.push(format!("CHECK ({})", utf8(def)?));
                }
                Value::Array(def) => {
                    let name = def.get(&Key::from("name")).cloned().unwrap_or(Value::Null);
                    if let Some(min) = def.get(&Key::from("min")).filter(|v| !v.is_null()) {
                        constraints.push(format!(
                            "CHECK ({} >= {})",
                            concat_value(&name)?,
                            concat_value(min)?
                        ));
                    }
                    if let Some(max) = def.get(&Key::from("max")).filter(|v| !v.is_null()) {
                        constraints.push(format!(
                            "CHECK ({} <= {})",
                            concat_value(&name)?,
                            concat_value(max)?
                        ));
                    }
                }
                other => {
                    return Err(Exception::new(
                        Exception::UNSUPPORTED,
                        format!("a check definition of type {}", other.debug_type()),
                    ));
                }
            }
        }
        Ok(constraints.join(", "))
    }

    /// Obtains DBMS specific SQL code portion needed to set a unique
    /// constraint declaration to be used in statements like CREATE
    /// TABLE.
    fn get_unique_constraint_declaration_sql(
        &self,
        constraint: &UniqueConstraint,
    ) -> Result<String, Exception> {
        let columns = constraint.quoted_columns(self);
        if columns.is_empty() {
            return Err(Exception::invalid_argument(
                "Incomplete definition. \"columns\" required.",
            ));
        }
        let mut chunks = Vec::new();
        if !constraint.get_name().is_empty() {
            chunks.push("CONSTRAINT".to_owned());
            chunks.push(constraint.get_quoted_name(self));
        }
        chunks.push("UNIQUE".to_owned());
        if constraint.has_flag_inner("clustered") {
            chunks.push("CLUSTERED".to_owned());
        }
        chunks.push(format!("({})", columns.join(", ")));
        Ok(chunks.join(" "))
    }

    /// Obtains DBMS specific SQL code portion needed to set an index
    /// declaration to be used in statements like CREATE TABLE.
    fn get_index_declaration_sql(&self, index: &Index) -> Result<String, Exception> {
        if index.get_columns().is_empty() {
            return Err(Exception::invalid_argument(
                "Incomplete definition. \"columns\" required.",
            ));
        }
        Ok(format!(
            "{}INDEX {} ({}){}",
            self.get_create_index_sql_flags(index),
            index.get_quoted_name(self),
            index.get_quoted_columns(self).join(", "),
            self.get_partial_index_sql(index)?
        ))
    }

    /// Some vendors require temporary table names to be qualified
    /// specially.
    fn get_temporary_table_name(&self, table_name: &str) -> String {
        table_name.to_owned()
    }

    /// Obtain DBMS specific SQL code portion needed to set the FOREIGN
    /// KEY constraint of a column declaration to be used in statements
    /// like CREATE TABLE.
    fn get_foreign_key_declaration_sql(
        &self,
        foreign_key: &ForeignKeyConstraint,
    ) -> Result<String, Exception> {
        Ok(format!(
            "{}{}",
            self.get_foreign_key_base_declaration_sql(foreign_key)?,
            self.get_advanced_foreign_key_options_sql(foreign_key)?
        ))
    }

    /// Returns the FOREIGN KEY query section dealing with non-standard
    /// options as MATCH, INITIALLY DEFERRED, ON UPDATE, ...
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
        Ok(query)
    }

    /// `getConstraintDeferrabilitySQL()` (protected).
    fn get_constraint_deferrability_sql(&self, foreign_key: &ForeignKeyConstraint) -> String {
        let mut sql = String::new();
        if foreign_key.has_option_inner("deferrable") {
            if matches!(foreign_key.option("deferrable"), Value::Bool(false)) {
                sql.push_str(" NOT DEFERRABLE");
            } else {
                sql.push_str(" DEFERRABLE");
            }
        }
        if foreign_key.has_option_inner("deferred") {
            if matches!(foreign_key.option("deferred"), Value::Bool(false)) {
                sql.push_str(" INITIALLY IMMEDIATE");
            } else {
                sql.push_str(" INITIALLY DEFERRED");
            }
        }
        sql
    }

    /// Returns the given referential action in uppercase if valid,
    /// otherwise throws an exception.
    fn get_foreign_key_referential_action_sql(&self, action: &str) -> Result<String, Exception> {
        let upper = action.to_ascii_uppercase();
        match upper.as_str() {
            "CASCADE" | "SET NULL" | "NO ACTION" | "RESTRICT" | "SET DEFAULT" => Ok(upper),
            _ => Err(Exception::invalid_argument(format!(
                "Invalid foreign key action \"{upper}\"."
            ))),
        }
    }

    /// Obtains DBMS specific SQL code portion needed to set the FOREIGN
    /// KEY constraint of a column declaration to be used in statements
    /// like CREATE TABLE.
    fn get_foreign_key_base_declaration_sql(
        &self,
        foreign_key: &ForeignKeyConstraint,
    ) -> Result<String, Exception> {
        let mut sql = String::new();
        if !foreign_key.get_name().is_empty() {
            let _ = write!(sql, "CONSTRAINT {} ", foreign_key.get_quoted_name(self));
        }
        sql.push_str("FOREIGN KEY (");

        if foreign_key.local_columns().is_empty() {
            return Err(Exception::invalid_argument(
                "Incomplete definition. \"local\" required.",
            ));
        }
        if foreign_key.foreign_columns().is_empty() {
            return Err(Exception::invalid_argument(
                "Incomplete definition. \"foreign\" required.",
            ));
        }
        if foreign_key.foreign_table_name().is_empty() {
            return Err(Exception::invalid_argument(
                "Incomplete definition. \"foreignTable\" required.",
            ));
        }

        Ok(format!(
            "{sql}{}) REFERENCES {} ({})",
            foreign_key.quoted_local_columns(self).join(", "),
            foreign_key.quoted_foreign_table_name(self),
            foreign_key.quoted_foreign_columns(self).join(", ")
        ))
    }

    /// Obtains DBMS specific SQL code portion needed to set the CHARACTER
    /// SET of a column declaration.
    fn get_column_charset_declaration_sql(&self, _charset: &str) -> String {
        String::new()
    }

    /// Obtains DBMS specific SQL code portion needed to set the COLLATION
    /// of a column declaration.
    fn get_column_collation_declaration_sql(&self, collation: &str) -> String {
        if self.supports_column_collation() {
            format!("COLLATE {}", self.quote_single_identifier(collation))
        } else {
            String::new()
        }
    }

    /// Returns the SQL specific for the platform to get the current date.
    fn get_current_date_sql(&self) -> String {
        "CURRENT_DATE".to_owned()
    }

    /// Returns the SQL specific for the platform to get the current time.
    fn get_current_time_sql(&self) -> String {
        "CURRENT_TIME".to_owned()
    }

    /// Returns the SQL specific for the platform to get the current
    /// timestamp.
    fn get_current_timestamp_sql(&self) -> String {
        "CURRENT_TIMESTAMP".to_owned()
    }

    /// `_getTransactionIsolationLevelSQL()` (protected).
    fn get_transaction_isolation_level_sql(&self, level: TransactionIsolationLevel) -> String {
        match level {
            TransactionIsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            TransactionIsolationLevel::ReadCommitted => "READ COMMITTED",
            TransactionIsolationLevel::RepeatableRead => "REPEATABLE READ",
            TransactionIsolationLevel::Serializable => "SERIALIZABLE",
        }
        .to_owned()
    }

    fn get_list_databases_sql(&self) -> Result<String, Exception> {
        Err(not_supported("getListDatabasesSQL"))
    }

    fn get_list_sequences_sql(&self, _database: &str) -> Result<String, Exception> {
        Err(not_supported("getListSequencesSQL"))
    }

    /// `getListViewsSQL()` (abstract).
    fn get_list_views_sql(&self, _database: &str) -> Result<String, Exception> {
        Err(not_supported("getListViewsSQL"))
    }

    fn get_create_view_sql(&self, name: &str, sql: &str) -> String {
        format!("CREATE VIEW {name} AS {sql}")
    }

    fn get_drop_view_sql(&self, name: &str) -> String {
        format!("DROP VIEW {name}")
    }

    fn get_sequence_next_val_sql(&self, _sequence: &str) -> Result<String, Exception> {
        Err(not_supported("getSequenceNextValSQL"))
    }

    /// Returns the SQL snippet to create a new database.
    fn get_create_database_sql(&self, name: &str) -> Result<String, Exception> {
        Ok(format!("CREATE DATABASE {name}"))
    }

    /// Returns the SQL snippet to drop an existing database.
    fn get_drop_database_sql(&self, name: &str) -> Result<String, Exception> {
        Ok(format!("DROP DATABASE {name}"))
    }

    /// Returns the SQL to set the transaction isolation level
    /// (abstract).
    fn get_set_transaction_isolation_sql(
        &self,
        _level: TransactionIsolationLevel,
    ) -> Result<String, Exception> {
        Err(not_supported("getSetTransactionIsolationSQL"))
    }

    /// Gets the default transaction isolation level of the platform.
    fn get_default_transaction_isolation_level(&self) -> TransactionIsolationLevel {
        TransactionIsolationLevel::ReadCommitted
    }

    fn supports_sequences(&self) -> bool {
        false
    }

    fn supports_identity_columns(&self) -> bool {
        false
    }

    fn supports_partial_indexes(&self) -> bool {
        false
    }

    /// Whether the platform supports indexes with column length
    /// definitions (deprecated upstream; the deprecation a caller outside
    /// the package raises is not reproduced).
    fn supports_column_length_indexes(&self) -> bool {
        false
    }

    fn supports_savepoints(&self) -> bool {
        true
    }

    fn supports_release_savepoints(&self) -> bool {
        self.supports_savepoints()
    }

    fn supports_schemas(&self) -> bool {
        false
    }

    fn supports_inline_column_comments(&self) -> bool {
        false
    }

    fn supports_comment_on_statement(&self) -> bool {
        false
    }

    fn supports_column_collation(&self) -> bool {
        false
    }

    /// Maximum length of any given database identifier, like tables or
    /// column names.
    fn get_max_identifier_length(&self) -> i64 {
        63
    }

    /// Returns the insert SQL for an empty insert statement.
    fn get_empty_identity_insert_sql(
        &self,
        quoted_table_name: &str,
        quoted_identifier_column_name: &str,
    ) -> String {
        format!("INSERT INTO {quoted_table_name} ({quoted_identifier_column_name}) VALUES (null)")
    }

    /// Generates a Truncate Table SQL statement for a given table.
    fn get_truncate_table_sql(&self, table_name: &str, _cascade: bool) -> String {
        let table_identifier = Identifier::new(table_name, false);
        format!("TRUNCATE {}", table_identifier.get_quoted_name(self))
    }

    /// This is for test reasons, many vendors have special requirements
    /// for dummy statements.
    fn get_dummy_select_sql(&self, expression: &str) -> String {
        format!("SELECT {expression}")
    }

    fn create_save_point(&self, savepoint: &str) -> String {
        format!("SAVEPOINT {savepoint}")
    }

    fn release_save_point(&self, savepoint: &str) -> String {
        format!("RELEASE SAVEPOINT {savepoint}")
    }

    fn rollback_save_point(&self, savepoint: &str) -> String {
        format!("ROLLBACK TO SAVEPOINT {savepoint}")
    }

    /// Returns the keyword list instance of this platform (final; the
    /// 6607 deprecation a caller outside the package raises is not
    /// reproduced).
    fn get_reserved_keywords_list(&self) -> Arc<KeywordList> {
        match self.platform_state() {
            Some(state) => state.keywords(|| self.create_reserved_keywords_list()),
            None => Arc::new(self.create_reserved_keywords_list()),
        }
    }

    /// `createReservedKeywordsList()` (abstract, protected; here an empty
    /// list by default).
    fn create_reserved_keywords_list(&self) -> KeywordList {
        KeywordList::new("Doctrine\\DBAL\\Platforms\\Keywords\\KeywordList", || &[])
    }

    /// Quotes a literal string.
    fn quote_string_literal(&self, str: &str) -> String {
        format!("'{}'", str.replace('\'', "''"))
    }

    /// Escapes metacharacters in a string intended to be used with a
    /// LIKE operator (final).
    fn escape_string_for_like(&self, input_string: &str, escape_char: &str) -> String {
        let special: Vec<char> = self
            .get_like_wildcard_characters()
            .chars()
            .chain(escape_char.chars())
            .collect();
        let mut sql = String::new();
        for c in input_string.chars() {
            if special.contains(&c) {
                sql.push_str(escape_char);
            }
            sql.push(c);
        }
        sql
    }

    /// `columnToArray($column)` (private).
    fn column_to_array(&self, column: &Column) -> Result<PhpArray, Exception> {
        let mut array = column.to_array(true)?;
        let mut extra = PhpArray::new();
        extra.insert("name", Value::string(column.get_quoted_name(self)));
        extra.insert(
            "version",
            if column.has_platform_option("version") {
                column
                    .get_platform_option("version")
                    .cloned()
                    .unwrap_or(Value::Null)
            } else {
                Value::Bool(false)
            },
        );
        extra.insert("comment", Value::string(column.get_comment()));
        array_merge_into(&mut array, &extra);
        Ok(array)
    }

    /// `createSQLParser()`.
    fn create_sql_parser(&self) -> Parser {
        Parser::new(false)
    }

    /// `getLikeWildcardCharacters()` (protected).
    fn get_like_wildcard_characters(&self) -> String {
        "%_".to_owned()
    }

    /// Compares the definitions of the given columns in the context of
    /// this platform.
    fn columns_equal(&self, column1: &Column, column2: &Column) -> Result<bool, Exception> {
        let mut column1_array = self.column_to_array(column1)?;
        let mut column2_array = self.column_to_array(column2)?;

        // ignore explicit columnDefinition since it's not set on the
        // Column generated by the SchemaManager
        column1_array.insert("columnDefinition", Value::Null);
        column2_array.insert("columnDefinition", Value::Null);

        if self.get_column_declaration_sql("", &column1_array)?
            != self.get_column_declaration_sql("", &column2_array)?
        {
            return Ok(false);
        }

        // If the platform supports inline comments, all comparison is
        // already done above
        if self.supports_inline_column_comments() {
            return Ok(true);
        }

        Ok(column1.get_comment() == column2.get_comment())
    }

    // End of slice SQLite platform.
}

/// `empty($column[$name])` negated.
fn column_flag(column: &PhpArray, name: &str) -> bool {
    column.get(&Key::from(name)).is_some_and(Value::is_truthy)
}

/// `$column['length'] ?? null` passed to a `?int` parameter of `method`
/// (the varying or the fixed snippet, as `fixed` chooses) under
/// `strict_types`.
fn column_length(column: &PhpArray, varying: &str, fixed: &str) -> Result<Option<i64>, Exception> {
    match column.get(&Key::from("length")) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Int(length)) => Ok(Some(*length)),
        Some(other) => {
            let method = if column_flag(column, "fixed") {
                fixed
            } else {
                varying
            };
            Err(Exception::new(
                Exception::TYPE_ERROR,
                format!(
                    "Doctrine\\DBAL\\Platforms\\AbstractPlatform::{method}(): Argument #1 ($length) must be of type ?int, {} given",
                    other.value_name()
                ),
            ))
        }
    }
}

/// `throw InvalidColumnDeclaration::fromInvalidColumnType($column['name'],
/// $e)`; the name must be a string (`strict_types`).
fn invalid_column_declaration<T>(column: &PhpArray, e: Exception) -> Result<T, Exception> {
    match column.get(&Key::from("name")) {
        Some(Value::String(name)) => Err(Exception::invalid_column_declaration(name.as_bytes(), e)),
        other => Err(Exception::new(
            Exception::TYPE_ERROR,
            format!(
                "Doctrine\\DBAL\\Exception\\InvalidColumnDeclaration::fromInvalidColumnType(): Argument #1 ($columnName) must be of type string, {} given",
                other.map_or_else(|| "null".to_owned(), Value::value_name)
            ),
        )),
    }
}

/// `mb_strlen($value)` (UTF-8) under `strict_types`. A string that is not
/// UTF-8 is refused.
fn mb_strlen(value: &Value) -> Result<i64, Exception> {
    match value {
        Value::String(s) => match s.to_str() {
            Some(text) => Ok(i64::try_from(text.chars().count()).unwrap_or(i64::MAX)),
            None => Err(Exception::new(
                Exception::UNSUPPORTED,
                "mb_strlen() of a string that is not UTF-8",
            )),
        },
        other => Err(Exception::new(
            Exception::TYPE_ERROR,
            format!(
                "mb_strlen(): Argument #1 ($string) must be of type string, {} given",
                other.value_name()
            ),
        )),
    }
}

// Slice SQLite platform: helpers.

/// `NotSupported::new('Doctrine\DBAL\Platforms\AbstractPlatform::' . $method)`.
fn not_supported(method: &str) -> Exception {
    NotSupported::new(&format!(
        "Doctrine\\DBAL\\Platforms\\AbstractPlatform::{method}"
    ))
    .into()
}

/// `validateCreateTableOptions($options, $methodName)` (final,
/// protected).
pub(crate) fn validate_create_table_options(options: &CreateTableOptions, method_name: &str) {
    if options.primary.is_none()
        || options.indexes.is_none()
        || options.unique_constraints.is_none()
        || options.foreign_keys.is_none()
    {
        deprecations::trigger("https://github.com/doctrine/dbal/pull/6805", || {
            format!(
                "Not passing $options or any of its following keys to {method_name}() is deprecated: \"primary\", \"indexes\", \"uniqueConstraints\", \"foreignKeys\"."
            )
        });
    }
}

/// `array_unique(array_values($values))`.
pub(crate) fn array_unique(values: &[String]) -> Vec<String> {
    let mut unique: Vec<String> = Vec::new();
    for value in values {
        if !unique.contains(value) {
            unique.push(value.clone());
        }
    }
    unique
}
