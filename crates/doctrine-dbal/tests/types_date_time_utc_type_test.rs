//! Port of tests/Types/DateTimeUtcTypeTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! PHP's tests build dates in `America/New_York` (UTC-5 in January);
//! `php_value` has no IANA zones, so they use the fixed offset -05:00.
//!
//! Upstream: doctrine/dbal tests/Types/DateTimeUtcTypeTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::ParameterType;
use doctrine_dbal::types::{ConversionError, DateTimeUtcType, Type};
use php_value::{
    Value,
    datetime::{DateTime, TimeZone},
};
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

fn set_up() -> (StubPlatform, DateTimeUtcType) {
    let platform = StubPlatform::new();
    platform.will_return("getDateTimeFormatString", "Y-m-d H:i:s");
    (platform, DateTimeUtcType)
}

/// `new DateTimeZone('America/New_York')` in January.
const NEW_YORK: TimeZone = TimeZone::Offset(-5 * 3600);

#[test]
fn testReturnsBindingType() {
    let (_, type_) = set_up();
    assert_eq!(ParameterType::String, type_.get_binding_type());
}

#[test]
fn testDelegatesSQLDeclarationToTheDateTimeTypeDeclaration() {
    let (_, type_) = set_up();
    let platform = StubPlatform::new();
    platform.will_return("getDateTimeTypeDeclarationSQL", "DATETIME");
    let column = php_value::php_array!["foo" => "bar"];

    assert_eq!(
        php_value::PhpString::from("DATETIME"),
        type_.get_sql_declaration(&column, &platform).unwrap()
    );
    assert_eq!(
        platform.arguments("getDateTimeTypeDeclarationSQL"),
        vec![vec![Value::Array(column)]]
    );
}

#[test]
fn testConvertsNonUtcDateTimeToUtcDatabaseValue() {
    let (platform, type_) = set_up();
    // America/New_York is UTC-5 in January (no DST).
    let date = DateTime::object(date_in("2016-01-01 15:58:59", NEW_YORK));

    assert_eq!(
        Value::from("2016-01-01 20:58:59"),
        type_.convert_to_database_value(&date, &platform).unwrap()
    );
}

#[test]
fn testDoesNotMutateTheGivenDateTime() {
    let (platform, type_) = set_up();
    let timezone = NEW_YORK;
    let date = DateTime::object(date_in("2016-01-01 15:58:59", timezone));

    type_.convert_to_database_value(&date, &platform).unwrap();

    let object = date
        .as_object()
        .unwrap()
        .downcast_ref::<DateTime>()
        .unwrap();
    assert_eq!(timezone.name(), object.get().timezone().name());
    assert_eq!("2016-01-01 15:58:59", object.format("Y-m-d H:i:s"));
}

#[test]
fn testConvertsNullToDatabaseValue() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_database_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testThrowsForInvalidValueOnConversionToDatabaseValue() {
    let (platform, type_) = set_up();
    assert!(is_conversion_exception(&type_.convert_to_database_value(
        &types_support::new_date_time_immutable(),
        &platform
    )));
}

#[test]
fn testConvertsDatabaseValueToUtcDateTime() {
    let (platform, type_) = set_up();
    let date = type_
        .convert_to_php_value(&Value::from("2016-01-01 20:58:59"), &platform)
        .unwrap();

    assert!(types_support::is_date_time(&date));
    assert_eq!(
        "2016-01-01 20:58:59",
        types_support::format(&date, "Y-m-d H:i:s")
    );
    assert_eq!("UTC", types_support::format(&date, "e"));
    assert_eq!("0", types_support::format(&date, "Z"));
}

#[test]
fn testConvertsNullToPHPValue() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testConvertsDateTimeInstanceToPHPValueUnchanged() {
    let (platform, type_) = set_up();
    let date = types_support::new_date_time();

    assert_eq!(date, type_.convert_to_php_value(&date, &platform).unwrap());
}

#[test]
fn testThrowsExceptionForInvalidDateTimeString() {
    let (platform, type_) = set_up();
    assert!(is_conversion_exception(&type_.convert_to_php_value(
        &Value::from("invalid datetime string"),
        &platform
    )));
}
