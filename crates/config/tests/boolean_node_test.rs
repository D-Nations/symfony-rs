//! BooleanNodeTest.php at symfony/config v8.1.10. Names and assertions
//! are PHPUnit's; provider rows are upstream's, written as literals.
//!
//! Upstream: symfony/config Tests/Definition/BooleanNodeTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_class, expect_exception};
use php_value::{PhpArray, Value, object::StdClass, php_array};
use symfony_config::{Class, definition::BooleanNode};

fn get_valid_values() -> Vec<bool> {
    vec![false, true]
}

#[test]
fn testNormalize() -> TestResult {
    for value in get_valid_values() {
        let node = BooleanNode::new("test", None)?;
        assert_eq!(node.normalize(Value::Bool(value))?, Value::Bool(value));
    }
    Ok(())
}

#[test]
fn testNullValueOnNullable() -> TestResult {
    let node = BooleanNode::with_options("test", None, b".", true)?;
    assert!(node.normalize(Value::Null)?.is_null());
    Ok(())
}

#[test]
fn testNullValueOnNotNullable() -> TestResult {
    let node = BooleanNode::with_options("test", None, b".", false)?;
    expect_exception(
        node.normalize(Value::Null),
        Class::InvalidType,
        "Invalid type for path \"test\". Expected \"bool\", but got \"null\".",
    );
    Ok(())
}

#[test]
fn testInvalidValueOnNullable() -> TestResult {
    let node = BooleanNode::with_options("test", None, b".", true)?;
    expect_exception(
        node.normalize(Value::Int(123)),
        Class::InvalidType,
        "Invalid type for path \"test\". Expected \"bool\" or \"null\", but got \"int\".",
    );
    Ok(())
}

#[test]
fn testValidNonEmptyValues() -> TestResult {
    for value in get_valid_values() {
        let node = BooleanNode::new("test", None)?;
        node.set_allow_empty_value(false)?;
        assert_eq!(node.finalize(Value::Bool(value))?, Value::Bool(value));
    }
    Ok(())
}

fn get_invalid_values() -> Vec<Value> {
    vec![
        Value::from(""),
        Value::from("foo"),
        Value::Int(0),
        Value::Int(1),
        Value::Float(0.0),
        Value::Float(0.1),
        Value::Array(PhpArray::new()),
        Value::Array(php_array!["foo" => "bar"]),
        StdClass::object(PhpArray::new()),
    ]
}

#[test]
fn testNormalizeThrowsExceptionOnInvalidValues() -> TestResult {
    for value in get_invalid_values() {
        let node = BooleanNode::new("test", None)?;
        expect_class(node.normalize(value), Class::InvalidType);
    }
    Ok(())
}
