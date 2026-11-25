//! Port of tests/Types/TimeTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! The first four tests are inherited from BaseDateTypeTestCase.php, as
//! PHPUnit runs them for this class.
//!
//! Upstream: doctrine/dbal tests/Types/TimeTest.php tests/Types/BaseDateTypeTestCase.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod types_support;

use common::provider;
use doctrine_dbal::types::{ConversionError, TimeType, Type};
use php_value::Value;
use types_support::{StubPlatform, provider_value};

fn set_up() -> (StubPlatform, TimeType) {
    let platform = StubPlatform::new();
    platform.will_return("getTimeFormatString", "H:i:s");
    (platform, TimeType)
}

fn is_conversion_exception(result: &Result<Value, ConversionError>) -> bool {
    matches!(result, Err(ConversionError::Conversion(_)))
}

#[test]
fn testDateConvertsToDatabaseValue() {
    let (platform, type_) = set_up();
    assert!(matches!(
        type_
            .convert_to_database_value(&types_support::new_date_time(), &platform)
            .unwrap(),
        Value::String(_)
    ));
}

#[test]
fn testInvalidTypeConversionToDatabaseValue() {
    let rows = provider("Types/TimeTest", "testInvalidTypeConversionToDatabaseValue");
    assert_eq!(rows.len(), 12);
    for (name, args) in rows {
        let (platform, type_) = set_up();
        let value = provider_value(&args[0]);
        let result = type_.convert_to_database_value(&value, &platform);
        assert!(is_conversion_exception(&result), "{name}: {result:?}");
    }
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
    let date = types_support::new_date_time();

    assert_eq!(date, type_.convert_to_php_value(&date, &platform).unwrap());
}

#[test]
fn testTimeConvertsToPHPValue() {
    let (platform, type_) = set_up();
    assert!(types_support::is_date_time(
        &type_
            .convert_to_php_value(&Value::from("5:30:55"), &platform)
            .unwrap()
    ));
}

#[test]
fn testDateFieldResetInPHPValue() {
    let (platform, type_) = set_up();
    let time = type_
        .convert_to_php_value(&Value::from("01:23:34"), &platform)
        .unwrap();

    assert_eq!("01:23:34", types_support::format(&time, "H:i:s"));
    assert_eq!("1970-01-01", types_support::format(&time, "Y-m-d"));
}

#[test]
fn testInvalidTimeFormatConversion() {
    let (platform, type_) = set_up();
    assert!(is_conversion_exception(
        &type_.convert_to_php_value(&Value::from("abcdefg"), &platform)
    ));
}
