//! Port of Doctrine\DBAL\Types\DateTimeType.php, DateTimeImmutableType.php,
//! DateTimeTzType.php, DateTimeTzImmutableType.php, DateTimeUtcType.php,
//! DateTimeUtcImmutableType.php, VarDateTimeType.php and
//! VarDateTimeImmutableType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/DateTimeType.php src/Types/DateTimeImmutableType.php
//! Upstream: doctrine/dbal src/Types/DateTimeTzType.php src/Types/DateTimeTzImmutableType.php
//! Upstream: doctrine/dbal src/Types/DateTimeUtcType.php src/Types/DateTimeUtcImmutableType.php
//! Upstream: doctrine/dbal src/Types/VarDateTimeType.php src/Types/VarDateTimeImmutableType.php

use std::sync::Arc;

use php_value::{
    PhpArray, PhpString, Thrown, Value,
    date_create::{create_from_format, date_create},
    datetime::{DateTime, TimeZone},
};

use super::{
    ConversionError, ConversionException, PhpMappingType, Type,
    date_type::{
        UTC, date_class, date_object, datetime_argument, format_date, from_format_or_invalid,
        immutable, mutable, passes_through,
    },
};
use crate::{exception::Exception, platforms::AbstractPlatform};

/// The `DateMalformedStringException` `new DateTime($value)` threw.
fn malformed(message: PhpString) -> Thrown {
    Thrown::new("DateMalformedStringException", message, 0)
}

/// `createFromFormat($format, $value, $zone)`, else `new
/// DateTime(Immutable)($value, $zone)`, else `InvalidFormat` naming the
/// platform's format (asked again) with the constructor's exception.
fn from_format_or_parse(
    value: &Value,
    class: &str,
    immutable_class: bool,
    zone: Option<TimeZone>,
    format: &dyn Fn() -> String,
) -> Result<Value, ConversionError> {
    let date_class = date_class(immutable_class);
    let text = datetime_argument(value, &format!("{date_class}::createFromFormat"), 2)?;
    if let Some(date) = create_from_format(format().as_bytes(), text, zone)? {
        return Ok(date_object(date, immutable_class));
    }
    match date_create(text, zone)? {
        Ok(date) => Ok(date_object(date, immutable_class)),
        Err(message) => Err(ConversionException::invalid_format(
            text,
            class,
            Some(&format()),
            Some(Arc::new(malformed(message))),
        )
        .into()),
    }
}

/// `new DateTime(Immutable)($value)`, else `ValueNotConvertible` with the
/// constructor's message.
fn parse_or_not_convertible(
    value: &Value,
    immutable_class: bool,
) -> Result<Value, ConversionError> {
    let date_class = date_class(immutable_class);
    let text = datetime_argument(value, &format!("{date_class}::__construct"), 1)?;
    match date_create(text, None)? {
        Ok(date) => Ok(date_object(date, immutable_class)),
        Err(message) => Err(ConversionException::value_not_convertible(
            value,
            date_class,
            Some(message.as_bytes()),
            Some(Arc::new(malformed(message.clone()))),
        )?
        .into()),
    }
}

/// `$value->setTimezone(UTC)->format(...)` (on a clone of a mutable
/// `DateTime`, which is left as it is).
fn format_in_utc(
    value: &Value,
    class: &str,
    immutable_class: bool,
    format: impl FnOnce() -> String,
) -> Result<Value, ConversionError> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let date = if immutable_class {
        immutable(value)
    } else {
        mutable(value).map(DateTime::get)
    };
    match date {
        Some(date) => Ok(Value::from(date.set_timezone(UTC).format(&format()))),
        None => Err(ConversionException::invalid_type(
            value,
            class,
            &["null", date_class(immutable_class)],
            None,
        )
        .into()),
    }
}

/// How a date-time type reads a database value.
#[derive(Clone, Copy)]
enum Read {
    /// The format, else the date parser (`DateTimeType`).
    FormatOrParse,
    /// The format only (`DateTimeTzType`).
    Format,
    /// The format, else the parser, in UTC (`DateTimeUtcType`).
    FormatOrParseUtc,
    /// The parser only (`VarDateTimeType`).
    Parse,
}

macro_rules! date_time_type {
    (
        $(#[$doc:meta])* $name:ident, $class:literal, $immutable:literal,
        $declaration:ident, $format:ident, $read:ident
    ) => {
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
                platform: &dyn AbstractPlatform,
            ) -> Result<Value, ConversionError> {
                match Read::$read {
                    Read::FormatOrParseUtc => {
                        format_in_utc(value, $class, $immutable, || platform.$format())
                    }
                    _ => format_date(value, $class, $immutable, || platform.$format()),
                }
            }

            fn convert_to_php_value(
                &self,
                value: &Value,
                platform: &dyn AbstractPlatform,
            ) -> Result<Value, ConversionError> {
                if passes_through(value, $immutable) {
                    return Ok(value.clone());
                }
                let format = || platform.$format();
                match Read::$read {
                    Read::FormatOrParse => {
                        from_format_or_parse(value, $class, $immutable, None, &format)
                    }
                    Read::Format => {
                        let first = format();
                        from_format_or_invalid(value, $class, $immutable, &first, format)
                    }
                    Read::FormatOrParseUtc => {
                        from_format_or_parse(value, $class, $immutable, Some(UTC), &format)
                    }
                    Read::Parse => parse_or_not_convertible(value, $immutable),
                }
            }

            fn php_mapping_type(&self) -> Option<PhpMappingType> {
                Some(PhpMappingType::DateTime)
            }
        }
    };
}

date_time_type!(
    /// Type that maps an SQL DATETIME/TIMESTAMP to a PHP `DateTime` object.
    DateTimeType,
    "Doctrine\\DBAL\\Types\\DateTimeType",
    false,
    get_date_time_type_declaration_sql,
    get_date_time_format_string,
    FormatOrParse
);

date_time_type!(
    /// Immutable type of [`DateTimeType`].
    DateTimeImmutableType,
    "Doctrine\\DBAL\\Types\\DateTimeImmutableType",
    true,
    get_date_time_type_declaration_sql,
    get_date_time_format_string,
    FormatOrParse
);

date_time_type!(
    /// `DateTime` type accepting a time zone offset. Since a PHP `DateTime`
    /// knows its zone and the database (except `PostgreSQL`) does not, the
    /// value is written with the platform's zone format.
    DateTimeTzType,
    "Doctrine\\DBAL\\Types\\DateTimeTzType",
    false,
    get_date_time_tz_type_declaration_sql,
    get_date_time_tz_format_string,
    Format
);

date_time_type!(
    /// Immutable type of [`DateTimeTzType`].
    DateTimeTzImmutableType,
    "Doctrine\\DBAL\\Types\\DateTimeTzImmutableType",
    true,
    get_date_time_tz_type_declaration_sql,
    get_date_time_tz_format_string,
    Format
);

date_time_type!(
    /// Type that maps an SQL DATETIME to a PHP `DateTime` object, stored
    /// in UTC and read as UTC.
    DateTimeUtcType,
    "Doctrine\\DBAL\\Types\\DateTimeUtcType",
    false,
    get_date_time_type_declaration_sql,
    get_date_time_format_string,
    FormatOrParseUtc
);

date_time_type!(
    /// Immutable type of [`DateTimeUtcType`].
    DateTimeUtcImmutableType,
    "Doctrine\\DBAL\\Types\\DateTimeUtcImmutableType",
    true,
    get_date_time_type_declaration_sql,
    get_date_time_format_string,
    FormatOrParseUtc
);

date_time_type!(
    /// Variable `DateTime` Type using the date parser instead of a format
    /// (`extends DateTimeType`): microseconds and zones as the database
    /// gives them.
    VarDateTimeType,
    "Doctrine\\DBAL\\Types\\VarDateTimeType",
    false,
    get_date_time_type_declaration_sql,
    get_date_time_format_string,
    Parse
);

date_time_type!(
    /// Immutable type of [`VarDateTimeType`] (`extends
    /// DateTimeImmutableType`).
    VarDateTimeImmutableType,
    "Doctrine\\DBAL\\Types\\VarDateTimeImmutableType",
    true,
    get_date_time_type_declaration_sql,
    get_date_time_format_string,
    Parse
);
