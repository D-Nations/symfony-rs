//! Port of tests/Types/DateIntervalTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/DateIntervalTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod types_support;

use std::sync::Arc;

use common::provider;
use doctrine_dbal::types::{ConversionError, DateIntervalType, Type};
use php_value::{Value, date_interval::DateInterval};
use types_support::{StubPlatform, provider_value};

fn set_up() -> (StubPlatform, DateIntervalType) {
    (StubPlatform::new(), DateIntervalType)
}

fn is_conversion_exception(result: &Result<Value, ConversionError>) -> bool {
    matches!(result, Err(ConversionError::Conversion(_)))
}

/// `new DateInterval($duration)`.
fn interval(duration: &str) -> DateInterval {
    DateInterval::new(duration.as_bytes()).unwrap().unwrap()
}

/// `$value instanceof DateInterval ? $value : null`.
fn as_interval(value: &Value) -> Option<DateInterval> {
    value
        .as_object()
        .and_then(|o| o.downcast_ref::<DateInterval>())
        .copied()
}

#[test]
fn testDateIntervalConvertsToDatabaseValue() {
    let (platform, type_) = set_up();
    let interval = Value::Object(Arc::new(interval("P2Y1DT1H2M3S")));

    let expected = Value::from("+P02Y00M01DT01H02M03S");
    let actual = type_
        .convert_to_database_value(&interval, &platform)
        .unwrap();

    assert_eq!(expected, actual);
}

#[test]
fn testDateIntervalConvertsToPHPValue() {
    let (platform, type_) = set_up();
    let interval = type_
        .convert_to_php_value(&Value::from("+P02Y00M01DT01H02M03S"), &platform)
        .unwrap();

    let interval = as_interval(&interval).expect("a DateInterval");
    assert_eq!(
        "+P02Y00M01DT01H02M03S",
        interval.format(DateIntervalType::FORMAT)
    );
}

#[test]
fn testNegativeDateIntervalConvertsToDatabaseValue() {
    let (platform, type_) = set_up();
    let mut interval = interval("P2Y1DT1H2M3S");
    interval.invert = true;

    let actual = type_
        .convert_to_database_value(&Value::Object(Arc::new(interval)), &platform)
        .unwrap();

    assert_eq!(Value::from("-P02Y00M01DT01H02M03S"), actual);
}

#[test]
fn testNegativeDateIntervalConvertsToPHPValue() {
    let (platform, type_) = set_up();
    let interval = type_
        .convert_to_php_value(&Value::from("-P02Y00M01DT01H02M03S"), &platform)
        .unwrap();

    let interval = as_interval(&interval).expect("a DateInterval");
    assert_eq!(
        "-P02Y00M01DT01H02M03S",
        interval.format(DateIntervalType::FORMAT)
    );
}

#[test]
fn testDateIntervalFormatWithoutSignConvertsToPHPValue() {
    let (platform, type_) = set_up();
    let interval = type_
        .convert_to_php_value(&Value::from("P02Y00M01DT01H02M03S"), &platform)
        .unwrap();

    let interval = as_interval(&interval).expect("a DateInterval");
    assert_eq!(
        "+P02Y00M01DT01H02M03S",
        interval.format(DateIntervalType::FORMAT)
    );
}

#[test]
fn testInvalidDateIntervalFormatConversion() {
    let (platform, type_) = set_up();
    assert!(is_conversion_exception(
        &type_.convert_to_php_value(&Value::from("abcdefg"), &platform)
    ));
}

#[test]
fn testDateIntervalNullConversion() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testDateIntervalEmptyStringConversion() {
    let (platform, type_) = set_up();
    assert!(is_conversion_exception(
        &type_.convert_to_php_value(&Value::from(""), &platform)
    ));
}

#[test]
fn testInvalidTypeConversionToDatabaseValue() {
    let rows = provider(
        "Types/DateIntervalTest",
        "testInvalidTypeConversionToDatabaseValue",
    );
    assert_eq!(rows.len(), 13);
    for (name, args) in rows {
        let (platform, type_) = set_up();
        let value = provider_value(&args[0]);
        let result = type_.convert_to_database_value(&value, &platform);
        assert!(is_conversion_exception(&result), "{name}: {result:?}");
    }
}
