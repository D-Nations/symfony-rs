//! Port of tests/Types/AsciiStringTest.php at doctrine/dbal
//! c435cd7ea33e17d2a89288caf994cf3d4549b879. Names and assertions are
//! PHPUnit's; `createStub()`/`createMock()` platforms are
//! `types_support::StubPlatform`.
//!
//! Upstream: doctrine/dbal tests/Types/AsciiStringTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod types_support;

use doctrine_dbal::ParameterType;
use doctrine_dbal::types::{AsciiStringType, Type};
use php_value::{Value, php_array};
use types_support::StubPlatform;

#[test]
fn testReturnCorrectBindingType() {
    let type_ = AsciiStringType;
    assert_eq!(type_.get_binding_type(), ParameterType::Ascii);
}

#[test]
fn testDelegateToPlatformForSqlDeclaration() {
    let type_ = AsciiStringType;
    let column_definitions = [
        php_array!["length" => 12, "fixed" => true],
        php_array!["length" => 14],
    ];

    for column in column_definitions {
        let platform = StubPlatform::new();
        type_.get_sql_declaration(&column, &platform).unwrap();

        assert_eq!(platform.calls("getAsciiStringTypeDeclarationSQL"), 1);
        assert_eq!(
            platform.arguments("getAsciiStringTypeDeclarationSQL"),
            vec![vec![Value::Array(column)]]
        );
    }
}
