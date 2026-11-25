//! Port of tests/Types/VarDateTimeImmutableTypeTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/VarDateTimeImmutableTypeTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::ParameterType;
use doctrine_dbal::types::{ConversionError, Type, VarDateTimeImmutableType};
use php_value::{Value, datetime::TimeZone};
use types_support::StubPlatform;

fn is_conversion_exception(result: &Result<Value, ConversionError>) -> bool {
    matches!(result, Err(ConversionError::Conversion(_)))
}

/// `new DateTimeImmutable($text, new DateTimeZone($zone))`.
fn date_in(text: &str, zone: TimeZone) -> php_value::datetime::DateTimeImmutable {
    php_value::date_create::date_create(text.as_bytes(), Some(zone))
        .unwrap()
        .unwrap()
}

#[test]
fn testReturnsBindingType() {
    let type_ = VarDateTimeImmutableType;
    assert_eq!(ParameterType::String, type_.get_binding_type());
}

#[test]
fn testConvertsDateTimeImmutableInstanceToDatabaseValue() {
    let type_ = VarDateTimeImmutableType;
    let platform = StubPlatform::new();
    platform.will_return("getDateTimeFormatString", "Y-m-d H:i:s");

    let date = date_in("2016-01-01 15:58:59", TimeZone::Utc).object();

    assert_eq!(
        Value::from("2016-01-01 15:58:59"),
        type_.convert_to_database_value(&date, &platform).unwrap()
    );
}

#[test]
fn testConvertsNullToDatabaseValue() {
    let type_ = VarDateTimeImmutableType;
    assert!(
        type_
            .convert_to_database_value(&Value::Null, &StubPlatform::new())
            .unwrap()
            .is_null()
    );
}

#[test]
fn testDoesNotSupportMutableDateTimeToDatabaseValueConversion() {
    let type_ = VarDateTimeImmutableType;
    assert!(is_conversion_exception(&type_.convert_to_database_value(
        &types_support::new_date_time(),
        &StubPlatform::new()
    )));
}

#[test]
fn testConvertsDateTimeImmutableInstanceToPHPValue() {
    let type_ = VarDateTimeImmutableType;
    let date = types_support::new_date_time_immutable();

    assert_eq!(
        date,
        type_
            .convert_to_php_value(&date, &StubPlatform::new())
            .unwrap()
    );
}

#[test]
fn testConvertsNullToPHPValue() {
    let type_ = VarDateTimeImmutableType;
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &StubPlatform::new())
            .unwrap()
            .is_null()
    );
}

#[test]
fn testConvertsDateishStringToPHPValue() {
    let type_ = VarDateTimeImmutableType;
    let date = type_
        .convert_to_php_value(
            &Value::from("2016-01-01 15:58:59.123456 UTC"),
            &StubPlatform::new(),
        )
        .unwrap();

    assert!(types_support::is_date_time_immutable(&date));
    assert_eq!(
        "2016-01-01 15:58:59.123456 UTC",
        types_support::format(&date, "Y-m-d H:i:s.u T")
    );
}

#[test]
fn testThrowsExceptionDuringConversionToPHPValueWithInvalidDateishString() {
    let type_ = VarDateTimeImmutableType;
    assert!(is_conversion_exception(&type_.convert_to_php_value(
        &Value::from("invalid date-ish string"),
        &StubPlatform::new()
    )));
}
