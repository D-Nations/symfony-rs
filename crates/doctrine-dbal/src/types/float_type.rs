//! Port of Doctrine\DBAL\Types\FloatType.php, SmallFloatType.php,
//! DecimalType.php and NumberType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/FloatType.php src/Types/SmallFloatType.php
//! Upstream: doctrine/dbal src/Types/DecimalType.php src/Types/NumberType.php

use std::sync::Arc;

use php_value::{
    PhpArray, PhpString, Value,
    bcmath::BcNumber,
    numeric::{float_cast, float_to_string},
};

use super::{ConversionError, ConversionException, PhpMappingType, Type};
use crate::{exception::Exception, platforms::AbstractPlatform};

/// `$value === null ? null : (float) $value`.
fn to_float(value: &Value) -> Value {
    if value.is_null() {
        Value::Null
    } else {
        Value::Float(float_cast(value))
    }
}

/// Type that maps an SQL DOUBLE PRECISION to a PHP float.
#[derive(Debug, Default)]
pub struct FloatType;

impl Type for FloatType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\FloatType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_float_declaration_sql(column)
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        Ok(to_float(value))
    }

    fn php_mapping_type(&self) -> Option<PhpMappingType> {
        Some(PhpMappingType::Float)
    }
}

/// Type that maps an SQL REAL to a PHP float.
#[derive(Debug, Default)]
pub struct SmallFloatType;

impl Type for SmallFloatType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\SmallFloatType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_small_float_declaration_sql(column)
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        Ok(to_float(value))
    }

    fn php_mapping_type(&self) -> Option<PhpMappingType> {
        Some(PhpMappingType::Float)
    }
}

/// Type that maps an SQL DECIMAL to a PHP string.
#[derive(Debug, Default)]
pub struct DecimalType;

impl Type for DecimalType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\DecimalType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_decimal_type_declaration_sql(column)
    }

    /// Some drivers can represent decimals as float/int
    /// (<https://github.com/doctrine/dbal/pull/4818>).
    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        Ok(match value {
            Value::Float(f) => Value::from(float_to_string(*f)),
            Value::Int(i) => Value::from(i.to_string()),
            other => other.clone(),
        })
    }
}

/// Type that maps an SQL DECIMAL to a `BcMath\Number`.
#[derive(Debug, Default)]
pub struct NumberType;

impl NumberType {
    const CLASS: &'static str = "Doctrine\\DBAL\\Types\\NumberType";
}

impl Type for NumberType {
    fn class(&self) -> &'static str {
        Self::CLASS
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_decimal_type_declaration_sql(column)
    }

    fn convert_to_database_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        if value.is_null() {
            return Ok(Value::Null);
        }
        match value.as_object().and_then(|o| o.downcast_ref::<BcNumber>()) {
            Some(number) => Ok(Value::String(number.to_php_string())),
            None => Err(ConversionException::invalid_type(
                value,
                Self::CLASS,
                &["null", BcNumber::CLASS],
                None,
            )
            .into()),
        }
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        if value.is_null() {
            return Ok(Value::Null);
        }
        // SQLite might return a decimal as float.
        let value = match value {
            Value::Float(f) => Value::from(float_to_string(*f)),
            other => other.clone(),
        };
        match BcNumber::new(&value) {
            Ok(number) => Ok(Value::Object(Arc::new(number))),
            Err(e) => Err(ConversionException::value_not_convertible(
                &value,
                Self::CLASS,
                None,
                Some(Arc::new(e)),
            )?
            .into()),
        }
    }
}
