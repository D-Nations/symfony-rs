//! Port of tests/Types/JsonTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/JsonTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod types_support;

use common::{bytes, provider};
use doctrine_dbal::ParameterType;
use doctrine_dbal::types::{ConversionError, JsonType, Type};
use php_value::{PhpArray, PhpString, Value, php_array, vectors::phpunit_equals};
use types_support::StubPlatform;

fn set_up() -> (StubPlatform, JsonType) {
    (StubPlatform::new(), JsonType)
}

#[test]
fn testReturnsBindingType() {
    let (_, type_) = set_up();
    assert_eq!(type_.get_binding_type(), ParameterType::String);
}

#[test]
fn testReturnsSQLDeclaration() {
    let (_, type_) = set_up();
    let platform = StubPlatform::new();
    platform.will_return("getJsonTypeDeclarationSQL", "TEST_JSON");

    assert_eq!(
        type_
            .get_sql_declaration(&PhpArray::new(), &platform)
            .unwrap(),
        PhpString::from("TEST_JSON")
    );
    assert_eq!(platform.calls("getJsonTypeDeclarationSQL"), 1);
}

#[test]
fn testJsonNullConvertsToPHPValue() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testJsonEmptyStringConvertsToPHPValue() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_php_value(&Value::from(""), &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testJsonStringConvertsToPHPValue() {
    let (platform, type_) = set_up();
    let value = Value::Array(php_array!["foo" => "bar", "bar" => "foo"]);

    let database_value = Value::from(r#"{"foo":"bar","bar":"foo"}"#);

    let php_value = type_
        .convert_to_php_value(&database_value, &platform)
        .unwrap();

    assert!(phpunit_equals(&value, &php_value), "{php_value:?}");
}

#[test]
fn testConversionFailure() {
    let rows = provider("Types/JsonTest", "testConversionFailure");
    assert_eq!(rows.len(), 2);
    for (name, args) in rows {
        let (platform, type_) = set_up();
        let data = Value::String(bytes(&args[0]).into());
        let result = type_.convert_to_php_value(&data, &platform);
        assert!(
            matches!(result, Err(ConversionError::Conversion(_))),
            "{name}: {result:?}"
        );
    }
}

#[test]
fn testPHPNullValueConvertsToJsonNull() {
    let (platform, type_) = set_up();
    assert!(
        type_
            .convert_to_database_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testPHPValueConvertsToJsonString() {
    let (platform, type_) = set_up();
    let source = Value::Array(php_array!["foo" => "bar", "bar" => "foo"]);
    let database_value = type_.convert_to_database_value(&source, &platform).unwrap();

    assert_eq!(database_value, Value::from(r#"{"foo":"bar","bar":"foo"}"#));
}

#[test]
fn testPHPFloatValueConvertsToJsonString() {
    let (platform, type_) = set_up();
    let source = Value::Array(php_array!["foo" => 11.4, "bar" => 10.0]);
    let database_value = type_.convert_to_database_value(&source, &platform).unwrap();

    assert_eq!(database_value, Value::from(r#"{"foo":11.4,"bar":10.0}"#));
}
