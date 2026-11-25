//! Port of Doctrine\DBAL\Types\DateIntervalType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/DateIntervalType.php

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Thrown, Value, date_interval::DateInterval};

use super::{ConversionError, ConversionException, Type};
use crate::{exception::Exception, platforms::AbstractPlatform};

/// Type that maps interval string to a PHP `DateInterval` object.
#[derive(Debug, Default)]
pub struct DateIntervalType;

impl DateIntervalType {
    pub const FORMAT: &'static str = "%RP%YY%MM%DDT%HH%IM%SS";
    const CLASS: &'static str = "Doctrine\\DBAL\\Types\\DateIntervalType";
}

fn interval(value: &Value) -> Option<DateInterval> {
    value
        .as_object()
        .and_then(|o| o.downcast_ref::<DateInterval>())
        .copied()
}

impl Type for DateIntervalType {
    fn class(&self) -> &'static str {
        Self::CLASS
    }

    fn get_sql_declaration(
        &self,
        column: &PhpArray,
        platform: &dyn AbstractPlatform,
    ) -> Result<PhpString, Exception> {
        let mut column = column.clone();
        column.insert("length", Value::Int(255));
        platform.get_string_type_declaration_sql(&column)
    }

    fn convert_to_database_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        if value.is_null() {
            return Ok(Value::Null);
        }
        match interval(value) {
            Some(interval) => Ok(Value::from(interval.format(Self::FORMAT))),
            None => Err(ConversionException::invalid_type(
                value,
                Self::CLASS,
                &["null", DateInterval::CLASS],
                None,
            )
            .into()),
        }
    }

    /// A value that is not a string is refused: PHP's answer for it (a
    /// `TypeError` from the constructor turned into one from
    /// `InvalidFormat::new()`) is not reproduced.
    fn convert_to_php_value(
        &self,
        value: &Value,
        _platform: &dyn AbstractPlatform,
    ) -> Result<Value, ConversionError> {
        if value.is_null() || interval(value).is_some() {
            return Ok(value.clone());
        }
        let Value::String(text) = value else {
            return Err(ConversionError::Unsupported(format!(
                "DateIntervalType: a database value of type {}",
                value.debug_type()
            )));
        };
        let mut text = text.as_bytes();
        let mut negative = false;
        if let Some(&sign @ (b'+' | b'-')) = text.first() {
            negative = sign == b'-';
            text = &text[1..];
        }
        match DateInterval::new(text)? {
            Ok(mut interval) => {
                if negative {
                    interval.invert = true;
                }
                Ok(Value::Object(Arc::new(interval)))
            }
            Err(message) => Err(ConversionException::invalid_format(
                text,
                Self::CLASS,
                Some(Self::FORMAT),
                Some(Arc::new(Thrown::new(
                    "DateMalformedIntervalStringException",
                    message,
                    0,
                ))),
            )
            .into()),
        }
    }
}
