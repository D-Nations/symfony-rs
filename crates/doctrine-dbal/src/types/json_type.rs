//! Port of Doctrine\DBAL\Types\JsonType.php, JsonObjectType.php,
//! JsonbType.php, JsonbObjectType.php and the trait they use,
//! JsonTypeConvert.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/JsonTypeConvert.php src/Types/JsonType.php
//! Upstream: doctrine/dbal src/Types/JsonObjectType.php src/Types/JsonbType.php
//! Upstream: doctrine/dbal src/Types/JsonbObjectType.php

use std::sync::Arc;

use php_value::{
    PhpArray, PhpString, Thrown, Value,
    json::{
        EncodeFailure, JSON_PRESERVE_ZERO_FRACTION, JSON_THROW_ON_ERROR, decode_assoc,
        decode_object, encode,
    },
};

use super::{ConversionError, ConversionException, Type};
use crate::{exception::Exception, platforms::AbstractPlatform};

/// `JsonTypeConvert::convertToDatabaseValue()`.
fn convert_to_database_value(value: &Value) -> Result<Value, ConversionError> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    match encode(
        value,
        JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION,
        512,
    ) {
        Ok(encoded) => Ok(encoded.json.map_or(Value::Bool(false), Value::from)),
        Err(EncodeFailure::Json { error, .. }) => Err(ConversionException::serialization_failed(
            value,
            "json",
            error.message(),
            Some(Arc::new(Thrown::new(
                "JsonException",
                error.message(),
                error.code(),
            ))),
        )
        .into()),
        Err(EncodeFailure::Thrown(thrown)) => Err(thrown.into()),
    }
}

/// `JsonTypeConvert::convertToPHPValue()`: arrays for JSON objects when
/// `associative`, `stdClass` objects otherwise.
fn convert_to_php_value(value: &Value, associative: bool) -> Result<Value, ConversionError> {
    let json = match value {
        Value::Null => return Ok(Value::Null),
        Value::String(s) if s.is_empty() => return Ok(Value::Null),
        Value::String(s) => s,
        other => {
            return Err(Thrown::new(
                "TypeError",
                format!(
                    "json_decode(): Argument #1 ($json) must be of type string, {} given",
                    other.value_name()
                ),
                0,
            )
            .into());
        }
    };
    let decoded = if associative {
        decode_assoc(json.as_bytes(), 512, false)
    } else {
        decode_object(json.as_bytes(), 512, false)
    };
    decoded.map_err(|error| {
        match ConversionException::value_not_convertible(
            value,
            "json",
            Some(error.message().as_bytes()),
            Some(Arc::new(Thrown::new(
                "JsonException",
                error.message(),
                error.code(),
            ))),
        ) {
            Ok(e) => e.into(),
            Err(thrown) => thrown.into(),
        }
    })
}

macro_rules! json_type {
    ($(#[$doc:meta])* $name:ident, $class:literal, $declaration:ident, $associative:literal) => {
        $(#[$doc])*
        #[derive(Debug, Default)]
        pub struct $name;

        impl Type for $name {
            fn class(&self) -> &'static str {
                $class
            }

            fn get_sql_declaration(
                &self,
                column: &PhpArray,
                platform: &dyn AbstractPlatform,
            ) -> Result<PhpString, Exception> {
                platform.$declaration(column)
            }

            fn convert_to_database_value(
                &self,
                value: &Value,
                _platform: &dyn AbstractPlatform,
            ) -> Result<Value, ConversionError> {
                convert_to_database_value(value)
            }

            fn convert_to_php_value(
                &self,
                value: &Value,
                _platform: &dyn AbstractPlatform,
            ) -> Result<Value, ConversionError> {
                convert_to_php_value(value, $associative)
            }
        }
    };
}

json_type!(
    /// Type generating json objects values; JSON objects are read as
    /// arrays.
    JsonType,
    "Doctrine\\DBAL\\Types\\JsonType",
    get_json_type_declaration_sql,
    true
);

json_type!(
    /// Type generating json objects values; JSON objects are read as
    /// `stdClass` objects.
    JsonObjectType,
    "Doctrine\\DBAL\\Types\\JsonObjectType",
    get_json_type_declaration_sql,
    false
);

json_type!(
    /// `JsonType` on a JSONB column (`extends JsonType`).
    JsonbType,
    "Doctrine\\DBAL\\Types\\JsonbType",
    get_jsonb_type_declaration_sql,
    true
);

json_type!(
    /// `JsonObjectType` on a JSONB column.
    JsonbObjectType,
    "Doctrine\\DBAL\\Types\\JsonbObjectType",
    get_jsonb_type_declaration_sql,
    false
);
