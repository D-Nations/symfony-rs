//! Port of tests/Tools/DsnParserTest.php. The `databaseUrls` rows are
//! upstream's, recorded (tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/Tools/DsnParserTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{array, data, s};
use doctrine_dbal::tools::DsnParser;
use php_value::Value;
use php_value::vectors::decode;

/// `ksort()` both, then `assertSame()`.
fn assert_same_sorted(name: &str, expected: &php_value::PhpArray, actual: &php_value::PhpArray) {
    let mut expected = expected.clone();
    let mut actual = actual.clone();
    expected.ksort();
    actual.ksort();
    assert!(
        Value::Array(expected.clone()).identical(&Value::Array(actual.clone())),
        "{name}: expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn testDatabaseUrl() {
    let rows = data()["providers"]["Tools/DsnParserTest"]["testDatabaseUrl"]["rows"]
        .as_array()
        .unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        let name = row[0].as_str().unwrap();
        let args = row[1].as_array().unwrap();
        let Value::String(dsn) = decode(&args[0]).unwrap() else {
            panic!("dsn");
        };
        let Value::Array(expected) = decode(&args[1]).unwrap() else {
            panic!("expected");
        };
        let parser = DsnParser::new([("mysql", "mysqli"), ("sqlite", "sqlite3")]);
        let actual = parser.parse(dsn.as_bytes()).unwrap();
        assert_same_sorted(name, &expected, &actual);
    }
}

/// The PHP test maps the scheme to a `Driver` stub's class; here the
/// parser is told that the class implements `Driver`.
#[test]
fn testDriverClassScheme() {
    let driver_class = "Mock_Driver_12345678";
    let parser = DsnParser::new([("custom", driver_class)]).with_driver_class(driver_class);
    let actual = parser.parse(b"custom://foo:bar@localhost/baz").unwrap();
    let expected = array([
        ("host", s("localhost")),
        ("user", s("foo")),
        ("password", s("bar")),
        ("driverClass", s(driver_class)),
        ("dbname", s("baz")),
    ]);
    assert!(
        Value::Array(expected.clone()).identical(&Value::Array(actual.clone())),
        "expected {expected:?}, got {actual:?}"
    );
}
