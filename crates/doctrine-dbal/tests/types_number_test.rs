//! Port of tests/Types/NumberTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/NumberTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use std::sync::Arc;

use doctrine_dbal::types::{
    ConversionError, ConversionException, ConversionExceptionKind, NumberType, Type,
};
use php_value::{PhpArray, Value, bcmath::BcNumber, object::StdClass};
use types_support::StubPlatform;

fn set_up() -> (StubPlatform, NumberType) {
    (StubPlatform::new(), NumberType)
}

fn number(text: &str) -> BcNumber {
    BcNumber::parse(text.as_bytes()).unwrap()
}

fn kind(result: Result<Value, ConversionError>) -> Option<ConversionExceptionKind> {
    result.err()?.as_conversion().map(ConversionException::kind)
}

/// `#[TestWith(['5.5'])]`, `#[TestWith(['5.5000'])]`, `#[TestWith([5.5])]`.
#[test]
fn testDecimalConvertsToPHPValue() {
    for db_value in [Value::from("5.5"), Value::from("5.5000"), Value::Float(5.5)] {
        let (platform, type_) = set_up();
        let php_value = type_.convert_to_php_value(&db_value, &platform).unwrap();

        let object = php_value.as_object().expect("an object");
        let php_value = object.downcast_ref::<BcNumber>().expect("a BcMath\\Number");
        assert!(php_value.compare(&number("5.5")).is_eq(), "{db_value:?}");
    }
}

#[test]
fn testDecimalNullConvertsToPHPValue() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testNumberConvertsToDecimalString() {
    let (platform, type_) = set_up();
    assert_eq!(
        type_
            .convert_to_database_value(&Value::Object(Arc::new(number("5.5"))), &platform)
            .unwrap(),
        Value::from("5.5")
    );
}

#[test]
fn testNumberNullConvertsToNull() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_database_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

/// `#[TestWith(['5.5'])]`, `#[TestWith([new stdClass()])]`.
#[test]
fn testInvalidPhpValuesTriggerException() {
    for value in [Value::from("5.5"), StdClass::object(PhpArray::new())] {
        let (platform, type_) = set_up();
        assert_eq!(
            kind(type_.convert_to_database_value(&value, &platform)),
            Some(ConversionExceptionKind::InvalidType),
            "{value:?}"
        );
    }
}

/// `#[TestWith(['foo'])]`, `#[TestWith([true])]`.
#[test]
fn testUnexpectedValuesReturnedByTheDatabaseTriggerException() {
    for value in [Value::from("foo"), Value::Bool(true)] {
        let (platform, type_) = set_up();
        assert_eq!(
            kind(type_.convert_to_php_value(&value, &platform)),
            Some(ConversionExceptionKind::ValueNotConvertible),
            "{value:?}"
        );
    }
}
