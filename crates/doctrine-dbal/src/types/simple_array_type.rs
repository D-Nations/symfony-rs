//! Port of Doctrine\DBAL\Types\SimpleArrayType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/SimpleArrayType.php

use php_value::{PhpArray, PhpString, Thrown, Value};

use super::{ConversionError, Type};
use crate::{exception::Exception, platforms::AbstractPlatform};

/// Array Type which can be used for simple values: `implode(',')` in,
/// `explode(',')` out. Only use this type if you are sure that your
/// values cannot contain a ",".
#[derive(Debug, Default)]
pub struct SimpleArrayType;

impl Type for SimpleArrayType {
    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Types\\SimpleArrayType"
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        platform.get_clob_type_declaration_sql(column)
    }

    /// `implode(',', $value)`: an array element is "Array" (PHP also
    /// warns); an object without `__toString()` is an `Error`.
    fn convert_to_database_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        let Value::Array(array) = value else {
            return Ok(Value::Null);
        };
        if array.is_empty() {
            return Ok(Value::Null);
        }
        let mut out = Vec::new();
        for (i, element) in array.values().enumerate() {
            if i > 0 {
                out.push(b',');
            }
            match element {
                Value::Array(_) => out.extend_from_slice(b"Array"),
                other => {
                    let text = other.to_php_string_or_stringable().ok_or_else(|| {
                        Thrown::new(
                            "Error",
                            format!(
                                "Object of class {} could not be converted to string",
                                other.debug_type()
                            ),
                            0,
                        )
                    })?;
                    out.extend_from_slice(text.as_bytes());
                }
            }
        }
        Ok(Value::String(PhpString::from(out)))
    }

    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        let text = match value {
            Value::Null => return Ok(Value::Array(PhpArray::new())),
            Value::String(s) => s,
            other => {
                return Err(Thrown::new(
                    "TypeError",
                    format!(
                        "explode(): Argument #2 ($string) must be of type string, {} given",
                        other.value_name()
                    ),
                    0,
                )
                .into());
            }
        };
        Ok(Value::Array(PhpArray::list(
            text.as_bytes()
                .split(|&b| b == b',')
                .map(|piece| Value::String(PhpString::from(piece))),
        )))
    }
}
