//! Port of tests/Types/DateImmutableTypeTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/DateImmutableTypeTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::ParameterType;
use doctrine_dbal::types::{ConversionError, DateImmutableType, Type};
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
fn testFactoryCreatesCorrectType() {
    let type_ = DateImmutableType;
    assert_eq!("Doctrine\\DBAL\\Types\\DateImmutableType", type_.class());
}

#[test]
fn testReturnsBindingType() {
    let type_ = DateImmutableType;
    assert_eq!(ParameterType::String, type_.get_binding_type());
}

#[test]
fn testConvertsDateTimeImmutableInstanceToDatabaseValue() {
    let type_ = DateImmutableType;
    let date = date_in("2016-01-01 12:34:56", TimeZone::Utc).object();

    let platform = StubPlatform::new();
    platform.will_return("getDateFormatString", "Y-m-d");

    assert_eq!(
        Value::from("2016-01-01"),
        type_.convert_to_database_value(&date, &platform).unwrap()
    );
    assert_eq!(platform.calls("getDateFormatString"), 1);
}

#[test]
fn testConvertsNullToDatabaseValue() {
    let type_ = DateImmutableType;
    assert!(
        type_
            .convert_to_database_value(&Value::Null, &StubPlatform::new())
            .unwrap()
            .is_null()
    );
}

#[test]
fn testDoesNotSupportMutableDateTimeToDatabaseValueConversion() {
    let type_ = DateImmutableType;
    assert!(is_conversion_exception(&type_.convert_to_database_value(
        &types_support::new_date_time(),
        &StubPlatform::new()
    )));
}

#[test]
fn testConvertsDateTimeImmutableInstanceToPHPValue() {
    let type_ = DateImmutableType;
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
    let type_ = DateImmutableType;
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &StubPlatform::new())
            .unwrap()
            .is_null()
    );
}

#[test]
fn testConvertsDateStringToPHPValue() {
    let type_ = DateImmutableType;
    let platform = StubPlatform::new();
    platform.will_return("getDateFormatString", "Y-m-d");

    let date = type_
        .convert_to_php_value(&Value::from("2016-01-01"), &platform)
        .unwrap();

    assert!(types_support::is_date_time_immutable(&date));
    assert_eq!("2016-01-01", types_support::format(&date, "Y-m-d"));
    assert_eq!(platform.calls("getDateFormatString"), 1);
}

#[test]
fn testResetTimeFractionsWhenConvertingToPHPValue() {
    let type_ = DateImmutableType;
    let platform = StubPlatform::new();
    platform.will_return("getDateFormatString", "Y-m-d");

    let date = type_
        .convert_to_php_value(&Value::from("2016-01-01"), &platform)
        .unwrap();

    assert!(!date.is_null());
    assert_eq!(
        "2016-01-01 00:00:00.000000",
        types_support::format(&date, "Y-m-d H:i:s.u")
    );
}

#[test]
fn testThrowsExceptionDuringConversionToPHPValueWithInvalidDateString() {
    let type_ = DateImmutableType;
    assert!(is_conversion_exception(&type_.convert_to_php_value(
        &Value::from("invalid date string"),
        &StubPlatform::new()
    )));
}
