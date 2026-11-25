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

use php_value::{Key, PhpArray, PhpString, Value};

use crate::exception::Exception;

/// Base contract for the SQL dialects (`AbstractPlatform`).
pub trait AbstractPlatform {
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
