//! Port of Doctrine\DBAL\Types\DateType.php, DateImmutableType.php,
//! TimeType.php and TimeImmutableType.php, and the date helpers the
//! date-time types share.
//!
//! Dates are `php_value::datetime`'s `DateTime` (mutable) and
//! `DateTimeImmutable` objects; texts are read by
//! `php_value::date_create`, which refuses what it does not read
//! ([`ConversionError::Unsupported`]).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Types/DateType.php src/Types/DateImmutableType.php
//! Upstream: doctrine/dbal src/Types/TimeType.php src/Types/TimeImmutableType.php

use php_value::{
    PhpArray, PhpString, Thrown, Value,
    date_create::create_from_format,
    datetime::{DateTime, DateTimeImmutable, TimeZone},
};

use super::{ConversionError, ConversionException, PhpMappingType, Type};
use crate::{exception::Exception, platforms::AbstractPlatform};

/// `$value instanceof DateTimeImmutable`.
pub(super) fn immutable(value: &Value) -> Option<DateTimeImmutable> {
    value
        .as_object()
        .and_then(|o| o.downcast_ref::<DateTimeImmutable>())
        .copied()
}

/// `$value instanceof DateTime`.
pub(super) fn mutable(value: &Value) -> Option<&DateTime> {
    value.as_object().and_then(|o| o.downcast_ref::<DateTime>())
}

/// The `$datetime` string argument of a PHP date method: anything else is
/// a `TypeError` (`strict_types`).
pub(super) fn datetime_argument<'a>(
    value: &'a Value,
    method: &str,
    position: u8,
) -> Result<&'a [u8], ConversionError> {
    match value {
        Value::String(s) => Ok(s.as_bytes()),
        other => Err(Thrown::new(
            "TypeError",
            format!(
                "{method}(): Argument #{position} ($datetime) must be of type string, {} given",
                other.value_name()
            ),
            0,
        )
        .into()),
    }
}

/// The class of a mutable or immutable date object.
pub(super) fn date_class(immutable: bool) -> &'static str {
    if immutable {
        DateTimeImmutable::CLASS
    } else {
        DateTime::CLASS
    }
}

/// A date object of the class asked for.
pub(super) fn date_object(date: DateTimeImmutable, immutable: bool) -> Value {
    if immutable {
        date.object()
    } else {
        DateTime::object(date)
    }
}

/// `$value->format($platform->get...FormatString())` for a date of the
/// class the type takes, else `InvalidType`.
pub(super) fn format_date(
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
        Some(date) => Ok(Value::from(date.format(&format()))),
        None => Err(ConversionException::invalid_type(
            value,
            class,
            &["null", date_class(immutable_class)],
            None,
        )
        .into()),
    }
}

/// The value itself when it is null or a date of the type's class.
pub(super) fn passes_through(value: &Value, immutable_class: bool) -> bool {
    value.is_null()
        || if immutable_class {
            immutable(value).is_some()
        } else {
            mutable(value).is_some()
        }
}

/// `DateTime(Immutable)::createFromFormat($format, $value[, $zone])` or
/// `InvalidFormat` naming `expected` (the platform's format, asked again
/// as PHP asks it).
pub(super) fn from_format_or_invalid(
    value: &Value,
    class: &str,
    immutable_class: bool,
    format: &str,
    expected: impl FnOnce() -> String,
) -> Result<Value, ConversionError> {
    let method = format!("{}::createFromFormat", date_class(immutable_class));
    let text = datetime_argument(value, &method, 2)?;
    match create_from_format(format.as_bytes(), text, None)? {
        Some(date) => Ok(date_object(date, immutable_class)),
        None => {
            Err(ConversionException::invalid_format(text, class, Some(&expected()), None).into())
        }
    }
}

macro_rules! date_type {
    (
        $(#[$doc:meta])* $name:ident, $class:literal, $immutable:literal,
        $declaration:ident, $format:ident, $mapping:ident
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
                format_date(value, $class, $immutable, || platform.$format())
            }

            /// Read with `'!' . $format`: fields the format does not set
            /// are reset to the Unix epoch's.
            fn convert_to_php_value(
                &self,
                value: &Value,
                platform: &dyn AbstractPlatform,
            ) -> Result<Value, ConversionError> {
                if passes_through(value, $immutable) {
                    return Ok(value.clone());
                }
                let format = format!("!{}", platform.$format());
                from_format_or_invalid(value, $class, $immutable, &format, || platform.$format())
            }

            fn php_mapping_type(&self) -> Option<PhpMappingType> {
                Some(PhpMappingType::$mapping)
            }
        }
    };
}

date_type!(
    /// Type that maps an SQL DATE to a PHP `DateTime` object.
    DateType,
    "Doctrine\\DBAL\\Types\\DateType",
    false,
    get_date_type_declaration_sql,
    get_date_format_string,
    Date
);

date_type!(
    /// Immutable type of [`DateType`].
    DateImmutableType,
    "Doctrine\\DBAL\\Types\\DateImmutableType",
    true,
    get_date_type_declaration_sql,
    get_date_format_string,
    Date
);

date_type!(
    /// Type that maps an SQL TIME to a PHP `DateTime` object.
    TimeType,
    "Doctrine\\DBAL\\Types\\TimeType",
    false,
    get_time_type_declaration_sql,
    get_time_format_string,
    Time
);

date_type!(
    /// Immutable type of [`TimeType`].
    TimeImmutableType,
    "Doctrine\\DBAL\\Types\\TimeImmutableType",
    true,
    get_time_type_declaration_sql,
    get_time_format_string,
    Time
);

/// `new DateTimeZone('UTC')`.
pub(super) const UTC: TimeZone = TimeZone::Utc;
