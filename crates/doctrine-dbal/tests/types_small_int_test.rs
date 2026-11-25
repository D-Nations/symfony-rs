//! Port of tests/Types/SmallIntTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/SmallIntTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::types::{SmallIntType, Type};
use php_value::Value;
use types_support::StubPlatform;

#[test]
fn testSmallIntConvertsToPHPValue() {
    let (platform, type_) = (StubPlatform::new(), SmallIntType);
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
fn testSmallIntNullConvertsToPHPValue() {
    let (platform, type_) = (StubPlatform::new(), SmallIntType);
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}
