//! Port of Doctrine\DBAL\Types\IntegerType.php, SmallIntType.php and
//! BigIntType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/IntegerType.php src/Types/SmallIntType.php
//! Upstream: doctrine/dbal src/Types/BigIntType.php

use php_value::{
    PhpArray, PhpString, Thrown, Value,
    numeric::{Number, int_cast, is_numeric, numeric_value},
};

use super::{ConversionError, PhpMappingType, Type};
use crate::{ParameterType, exception::Exception, platforms::AbstractPlatform};

/// `$value === null ? null : (int) $value`.
fn to_int(value: &Value) -> Value {
    if value.is_null() {
        Value::Null
    } else {
        Value::Int(int_cast(value))
    }
}

/// Type that maps an SQL INT to a PHP integer.
#[derive(Debug, Default)]
pub struct IntegerType;

impl Type for IntegerType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\IntegerType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_integer_type_declaration_sql(column)
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        Ok(to_int(value))
    }

    fn get_binding_type(&self) -> ParameterType {
        ParameterType::Integer
    }

    fn php_mapping_type(&self) -> Option<PhpMappingType> {
        Some(PhpMappingType::Integer)
    }
}

/// Type that maps a database SMALLINT to a PHP integer.
#[derive(Debug, Default)]
pub struct SmallIntType;

impl Type for SmallIntType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\SmallIntType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_small_int_type_declaration_sql(column)
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        Ok(to_int(value))
    }

    fn get_binding_type(&self) -> ParameterType {
        ParameterType::Integer
    }

    fn php_mapping_type(&self) -> Option<PhpMappingType> {
        Some(PhpMappingType::Integer)
    }
}

/// Type that attempts to map a database BIGINT to a PHP int; a value out
/// of the int range stays a string.
#[derive(Debug, Default)]
pub struct BigIntType;

impl Type for BigIntType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\BigIntType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_big_int_type_declaration_sql(column)
    }

    fn get_binding_type(&self) -> ParameterType {
        ParameterType::String
    }

    /// Upstream asserts that any other value is a numeric string; the
    /// failed assertion is an `AssertionError` (assertions on, as under
    /// PHPUnit).
    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        match value {
            Value::Null | Value::Int(_) => Ok(value.clone()),
            Value::String(s) if is_numeric(s.as_bytes()) => match numeric_value(s.as_bytes()) {
                Some(Number::Int(i)) => Ok(Value::Int(i)),
                _ => Ok(value.clone()),
            },
            _ => Err(ConversionError::Thrown(Thrown::new(
                "AssertionError",
                "DBAL assumes values outside of the integer range to be returned as string by the database driver.",
                0,
            ))),
        }
    }

    fn php_mapping_type(&self) -> Option<PhpMappingType> {
        Some(PhpMappingType::Integer)
    }
}
