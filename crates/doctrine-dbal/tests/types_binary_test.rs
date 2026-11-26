//! Port of tests/Types/BinaryTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/BinaryTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod types_support;

use common::provider;
use doctrine_dbal::ParameterType;
use doctrine_dbal::types::{BinaryType, ConversionError, Type};
use php_value::{PhpString, Value};
use types_support::{StubPlatform, provider_value};

fn set_up() -> (StubPlatform, BinaryType) {
    (StubPlatform::new(), BinaryType)
}

/// Creates a binary string containing all possible byte values.
fn get_binary_string() -> Value {
    Value::String(PhpString::from((0..=255u8).collect::<Vec<u8>>()))
}

#[test]
fn testReturnsBindingType() {
    let (_, type_) = set_up();
    assert_eq!(type_.get_binding_type(), ParameterType::Binary);
}

#[test]
fn testBinaryNullConvertsToPHPValue() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testBinaryStringConvertsToPHPValue() {
    let (platform, type_) = set_up();
    let database_value = get_binary_string();
    let php_value = type_
        .convert_to_php_value(&database_value, &platform)
        .unwrap();

    assert_eq!(database_value, php_value);
}

#[test]
fn testThrowsConversionExceptionOnInvalidDatabaseValue() {
    let rows = provider(
        "Types/BinaryTest",
        "testThrowsConversionExceptionOnInvalidDatabaseValue",
    );
    assert_eq!(rows.len(), 8);
    for (name, args) in rows {
        let (platform, type_) = set_up();
        let value = provider_value(&args[0]);
        let result = type_.convert_to_php_value(&value, &platform);
        assert!(
            matches!(result, Err(ConversionError::Conversion(_))),
            "{name}: {result:?}"
        );
    }
}
