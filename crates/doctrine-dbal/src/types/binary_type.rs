//! Port of Doctrine\DBAL\Types\BinaryType.php and BlobType.php.
//!
//! PHP drivers may give stream resources, which these types read
//! (`BinaryType`) or create (`BlobType` wraps a string in a `php://temp`
//! stream). `php_value::Value` has no resources: `BlobType` refuses a
//! string ([`ConversionError::Unsupported`]).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/BinaryType.php src/Types/BlobType.php

use php_value::{PhpArray, PhpString, Value};

use super::{ConversionError, ConversionException, Type, Types};
use crate::{ParameterType, exception::Exception, platforms::AbstractPlatform};

/// Type that maps an SQL BINARY/VARBINARY to a PHP string.
#[derive(Debug, Default)]
pub struct BinaryType;

impl Type for BinaryType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\BinaryType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_binary_type_declaration_sql(column)
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        match value {
            Value::Null | Value::String(_) => Ok(value.clone()),
            other => {
                Err(
                    ConversionException::value_not_convertible(other, Types::BINARY, None, None)?
                        .into(),
                )
            }
        }
    }

    fn get_binding_type(&self) -> ParameterType {
        ParameterType::Binary
    }
}

/// Type that maps an SQL BLOB to a PHP resource stream.
#[derive(Debug, Default)]
pub struct BlobType;

impl Type for BlobType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\BlobType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_blob_type_declaration_sql(column)
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        match value {
            Value::Null => Ok(Value::Null),
            Value::String(_) => Err(ConversionError::Unsupported(
                "BlobType: a php://temp stream resource".to_owned(),
            )),
            other => {
                Err(
                    ConversionException::value_not_convertible(other, Types::BLOB, None, None)?
                        .into(),
                )
            }
        }
    }

    fn get_binding_type(&self) -> ParameterType {
        ParameterType::LargeObject
    }
}
