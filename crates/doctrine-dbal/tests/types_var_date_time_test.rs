//! Port of tests/Types/VarDateTimeTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/VarDateTimeTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::types::{ConversionError, Type, VarDateTimeType};
use php_value::{Value, datetime::DateTime};
use types_support::StubPlatform;

fn is_conversion_exception(result: &Result<Value, ConversionError>) -> bool {
    matches!(result, Err(ConversionError::Conversion(_)))
}

fn set_up() -> (StubPlatform, VarDateTimeType) {
    let platform = StubPlatform::new();
    platform.will_return("getDateTimeFormatString", "Y-m-d H:i:s");
    (platform, VarDateTimeType)
}

#[test]
fn testDateTimeConvertsToDatabaseValue() {
    let (platform, type_) = set_up();
    let date = types_support::date("1985-09-01 10:10:10");

    let expected = date.format(
        &doctrine_dbal::platforms::AbstractPlatform::get_date_time_format_string(&platform),
    );
    let actual = type_
        .convert_to_database_value(&DateTime::object(date), &platform)
        .unwrap();

    assert_eq!(Value::from(expected), actual);
}

#[test]
fn testDateTimeConvertsToPHPValue() {
    let (platform, type_) = set_up();
    // Birthday of jwage and also birthday of Doctrine. Send him a present ;)
    let date = type_
        .convert_to_php_value(&Value::from("1985-09-01 00:00:00"), &platform)
        .unwrap();
    assert!(types_support::is_date_time(&date));
    assert_eq!(
        "1985-09-01 00:00:00",
        types_support::format(&date, "Y-m-d H:i:s")
    );
    assert_eq!("000000", types_support::format(&date, "u"));
}

#[test]
fn testInvalidDateTimeFormatConversion() {
    let (platform, type_) = set_up();
    assert!(is_conversion_exception(
        &type_.convert_to_php_value(&Value::from("abcdefg"), &platform)
    ));
}

#[test]
fn testConversionWithMicroseconds() {
    let (platform, type_) = set_up();
    let date = type_
        .convert_to_php_value(&Value::from("1985-09-01 00:00:00.123456"), &platform)
        .unwrap();
    assert!(types_support::is_date_time(&date));
    assert_eq!(
        "1985-09-01 00:00:00",
        types_support::format(&date, "Y-m-d H:i:s")
    );
    assert_eq!("123456", types_support::format(&date, "u"));
}

#[test]
fn testNullConversion() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testConvertDateTimeToPHPValue() {
    let (platform, type_) = set_up();
    let date = DateTime::object(types_support::date("now"));
    assert_eq!(date, type_.convert_to_php_value(&date, &platform).unwrap());
}
