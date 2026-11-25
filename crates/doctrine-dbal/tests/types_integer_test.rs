//! Port of tests/Types/IntegerTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/IntegerTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::types::{IntegerType, Type};
use php_value::Value;
use types_support::StubPlatform;

#[test]
fn testIntegerConvertsToPHPValue() {
    let (platform, type_) = (StubPlatform::new(), IntegerType);
    assert!(matches!(
        type_
            .convert_to_php_value(&Value::from("1"), &platform)
            .unwrap(),
        Value::Int(_)
    ));
    assert!(matches!(
        type_
            .convert_to_php_value(&Value::from("0"), &platform)
            .unwrap(),
        Value::Int(_)
    ));
}

#[test]
fn testIntegerNullConvertsToPHPValue() {
    let (platform, type_) = (StubPlatform::new(), IntegerType);
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}
