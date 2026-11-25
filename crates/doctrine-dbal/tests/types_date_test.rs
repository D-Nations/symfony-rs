//! Port of tests/Types/DateTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! The first four tests are inherited from BaseDateTypeTestCase.php, as
//! PHPUnit runs them for this class.
//!
//! Upstream: doctrine/dbal tests/Types/DateTest.php tests/Types/BaseDateTypeTestCase.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod types_support;

use common::provider;
use doctrine_dbal::types::{ConversionError, DateType, Type};
use php_value::Value;
use types_support::{StubPlatform, provider_value};

fn set_up() -> (StubPlatform, DateType) {
    let platform = StubPlatform::new();
    platform.will_return("getDateFormatString", "Y-m-d");
    (platform, DateType)
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
    let rows = provider("Types/DateTest", "testInvalidTypeConversionToDatabaseValue");
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
fn testDateConvertsToPHPValue() {
    let (platform, type_) = set_up();
    // Birthday of jwage and also birthday of Doctrine. Send him a present ;)
    assert!(types_support::is_date_time(
        &type_
            .convert_to_php_value(&Value::from("1985-09-01"), &platform)
            .unwrap()
    ));
}

#[test]
fn testDateResetsNonDatePartsToZeroUnixTimeValues() {
    let (platform, type_) = set_up();
    let date = type_
        .convert_to_php_value(&Value::from("1985-09-01"), &platform)
        .unwrap();

    assert_eq!("00:00:00", types_support::format(&date, "H:i:s"));
}

#[test]
fn testInvalidDateFormatConversion() {
    let (platform, type_) = set_up();
    assert!(is_conversion_exception(
        &type_.convert_to_php_value(&Value::from("abcdefg"), &platform)
    ));
}
