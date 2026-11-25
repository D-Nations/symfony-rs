//! Port of tests/Types/BooleanTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/BooleanTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::types::{BooleanType, Type};
use php_value::Value;
use types_support::StubPlatform;

#[test]
fn testBooleanConvertsToDatabaseValue() {
    let type_ = BooleanType;
    let platform = StubPlatform::new();
    platform.will_return("convertBooleansToDatabaseValue", 1);

    assert_eq!(
        type_
            .convert_to_database_value(&Value::Bool(true), &platform)
            .unwrap(),
        Value::Int(1)
    );
    assert_eq!(
        platform.arguments("convertBooleansToDatabaseValue"),
        vec![vec![Value::Bool(true)]]
    );
}

#[test]
fn testBooleanConvertsToPHPValue() {
    let type_ = BooleanType;
    let platform = StubPlatform::new();
    platform.will_return("convertFromBoolean", false);

    assert_eq!(
        type_
            .convert_to_php_value(&Value::Int(0), &platform)
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        platform.arguments("convertFromBoolean"),
        vec![vec![Value::Int(0)]]
    );
}

#[test]
fn testBooleanNullConvertsToPHPValue() {
    let type_ = BooleanType;
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &StubPlatform::new())
            .unwrap()
            .is_null()
    );
}
