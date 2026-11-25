//! Port of tests/Types/JsonObjectTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/JsonObjectTest.php

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
use doctrine_dbal::types::{ConversionError, JsonObjectType, Type};
use php_value::{PhpArray, PhpString, Value, object::StdClass, php_array, vectors::phpunit_equals};
use types_support::{StubPlatform, provider_value};

fn set_up() -> (StubPlatform, JsonObjectType) {
    (StubPlatform::new(), JsonObjectType)
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
    let rows = provider("Types/JsonObjectTest", "testJsonStringConvertsToPHPValue");
    assert_eq!(rows.len(), 3);
    for (name, args) in rows {
        let (platform, type_) = set_up();
        let database_value = provider_value(&args[0]);
        let expected_value = provider_value(&args[1]);
        let php_value = type_
            .convert_to_php_value(&database_value, &platform)
            .unwrap();

        assert!(
            phpunit_equals(&expected_value, &php_value),
            "{name}: {php_value:?} != {expected_value:?}"
        );
    }
}

#[test]
fn testConversionFailure() {
    let rows = provider("Types/JsonObjectTest", "testConversionFailure");
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
    let rows = provider("Types/JsonObjectTest", "testPHPValueConvertsToJsonString");
    assert_eq!(rows.len(), 4);
    for (name, args) in rows {
        let (platform, type_) = set_up();
        let php_value = provider_value(&args[0]);
        let expected_value = provider_value(&args[1]);
        let database_value = type_
            .convert_to_database_value(&php_value, &platform)
            .unwrap();

        assert_eq!(expected_value, database_value, "{name}");
    }
}

#[test]
fn testPHPFloatValueConvertsToJsonString() {
    let (platform, type_) = set_up();
    let source = StdClass::object(php_array!["foo" => 11.4, "bar" => 10.0]);

    let database_value = type_.convert_to_database_value(&source, &platform).unwrap();

    assert_eq!(database_value, Value::from(r#"{"foo":11.4,"bar":10.0}"#));
}
