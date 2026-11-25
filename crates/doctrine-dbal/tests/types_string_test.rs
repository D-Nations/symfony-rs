//! Port of tests/Types/StringTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/StringTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::types::{StringType, Type};
use php_value::{PhpArray, PhpString, Value};
use types_support::StubPlatform;

#[test]
fn testReturnsSQLDeclaration() {
    let type_ = StringType;
    let platform = StubPlatform::new();
    platform.will_return("getStringTypeDeclarationSQL", "TEST_STRING");

    assert_eq!(
        type_
            .get_sql_declaration(&PhpArray::new(), &platform)
            .unwrap(),
        PhpString::from("TEST_STRING")
    );
    assert_eq!(platform.calls("getStringTypeDeclarationSQL"), 1);
}

#[test]
fn testConvertToPHPValue() {
    let (platform, type_) = (StubPlatform::new(), StringType);
    assert!(matches!(
        type_
            .convert_to_php_value(&Value::from("foo"), &platform)
            .unwrap(),
        Value::String(_)
    ));
    assert!(matches!(
        type_
            .convert_to_php_value(&Value::from(""), &platform)
            .unwrap(),
        Value::String(_)
    ));
}

#[test]
fn testNullConversion() {
    let (platform, type_) = (StubPlatform::new(), StringType);
    assert!(
        type_
            .convert_to_php_value(&Value::Null, &platform)
            .unwrap()
            .is_null()
    );
}

#[test]
fn testSQLConversion() {
    let (platform, type_) = (StubPlatform::new(), StringType);
    assert_eq!(
        type_.convert_to_database_value_sql(b"t.foo", &platform),
        PhpString::from("t.foo")
    );
    assert_eq!(
        type_.convert_to_php_value_sql(b"t.foo", &platform),
        PhpString::from("t.foo")
    );
}
