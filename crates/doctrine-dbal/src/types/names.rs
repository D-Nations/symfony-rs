//! Port of Doctrine\DBAL\Types\Types.php: the names of the built-in types.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/Types.php

/// Default built-in types provided by Doctrine DBAL.
#[derive(Debug, Clone, Copy)]
pub struct Types;

impl Types {
    pub const ASCII_STRING: &'static str = "ascii_string";
    pub const BIGINT: &'static str = "bigint";
    pub const BINARY: &'static str = "binary";
    pub const BLOB: &'static str = "blob";
    pub const BOOLEAN: &'static str = "boolean";
    pub const DATE_MUTABLE: &'static str = "date";
    pub const DATE_IMMUTABLE: &'static str = "date_immutable";
    pub const DATEINTERVAL: &'static str = "dateinterval";
    pub const DATETIME_MUTABLE: &'static str = "datetime";
    pub const DATETIME_IMMUTABLE: &'static str = "datetime_immutable";
    pub const DATETIME_UTC_MUTABLE: &'static str = "datetime_utc";
    pub const DATETIME_UTC_IMMUTABLE: &'static str = "datetime_utc_immutable";
    pub const DATETIMETZ_MUTABLE: &'static str = "datetimetz";
    pub const DATETIMETZ_IMMUTABLE: &'static str = "datetimetz_immutable";
    pub const DECIMAL: &'static str = "decimal";
    pub const NUMBER: &'static str = "number";
    pub const FLOAT: &'static str = "float";
    pub const ENUM: &'static str = "enum";
    pub const GUID: &'static str = "guid";
    pub const INTEGER: &'static str = "integer";
    pub const JSON: &'static str = "json";
    pub const JSON_OBJECT: &'static str = "json_object";
    pub const JSONB: &'static str = "jsonb";
    pub const JSONB_OBJECT: &'static str = "jsonb_object";
    /// Deprecated upstream: use [`Types::JSON`] instead.
    pub const SIMPLE_ARRAY: &'static str = "simple_array";
    pub const SMALLFLOAT: &'static str = "smallfloat";
    pub const SMALLINT: &'static str = "smallint";
    pub const STRING: &'static str = "string";
    pub const TEXT: &'static str = "text";
    pub const TIME_MUTABLE: &'static str = "time";
    pub const TIME_IMMUTABLE: &'static str = "time_immutable";

    /// The public constants in declaration order, as reflection lists them.
    pub const ALL: [&'static str; 31] = [
        Self::ASCII_STRING,
        Self::BIGINT,
        Self::BINARY,
        Self::BLOB,
        Self::BOOLEAN,
        Self::DATE_MUTABLE,
        Self::DATE_IMMUTABLE,
        Self::DATEINTERVAL,
        Self::DATETIME_MUTABLE,
        Self::DATETIME_IMMUTABLE,
        Self::DATETIME_UTC_MUTABLE,
        Self::DATETIME_UTC_IMMUTABLE,
        Self::DATETIMETZ_MUTABLE,
        Self::DATETIMETZ_IMMUTABLE,
        Self::DECIMAL,
        Self::NUMBER,
        Self::FLOAT,
        Self::ENUM,
        Self::GUID,
        Self::INTEGER,
        Self::JSON,
        Self::JSON_OBJECT,
        Self::JSONB,
        Self::JSONB_OBJECT,
        Self::SIMPLE_ARRAY,
        Self::SMALLFLOAT,
        Self::SMALLINT,
        Self::STRING,
        Self::TEXT,
        Self::TIME_MUTABLE,
        Self::TIME_IMMUTABLE,
    ];
}
