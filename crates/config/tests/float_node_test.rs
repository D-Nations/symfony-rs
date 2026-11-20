//! FloatNodeTest.php at symfony/config v8.1.10. Names and assertions are
//! PHPUnit's; provider rows are upstream's, written as literals.
//!
//! Upstream: symfony/config Tests/Definition/FloatNodeTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_class};
use php_value::{Number, PhpArray, Value, object::StdClass, php_array};
use symfony_config::{
    Class,
    definition::{FloatNode, reset_placeholders, set_placeholder},
};

fn get_valid_values() -> Vec<Value> {
    vec![
        Value::Float(1798.0),
        Value::Float(-678.987),
        Value::Float(12.56E45),
        Value::Float(0.0),
        // Integer are accepted too, they will be cast
        Value::Int(17),
        Value::Int(-10),
        Value::Int(0),
    ]
}

#[test]
fn testNormalize() -> TestResult {
    for value in get_valid_values() {
        let node = FloatNode::new("test", None, None, None)?;
        assert_eq!(node.normalize(value.clone())?, value);
    }
    Ok(())
}

#[test]
fn testValidNonEmptyValues() -> TestResult {
    for value in get_valid_values() {
        let node = FloatNode::new("test", None, None, None)?;
        node.set_allow_empty_value(false)?;
        assert_eq!(node.finalize(value.clone())?, value);
    }
    Ok(())
}

fn get_invalid_values() -> Vec<Value> {
    vec![
        Value::Null,
        Value::from(""),
        Value::from("foo"),
        Value::Bool(true),
        Value::Bool(false),
        Value::Array(PhpArray::new()),
        Value::Array(php_array!["foo" => "bar"]),
        StdClass::object(PhpArray::new()),
    ]
}

#[test]
fn testNormalizeThrowsExceptionOnInvalidValues() -> TestResult {
    for value in get_invalid_values() {
        let node = FloatNode::new("test", None, None, None)?;
        expect_class(node.normalize(value), Class::InvalidType);
    }
    Ok(())
}

#[test]
fn testFinalizeAcceptsEnvPlaceholderBelowMin() -> TestResult {
    let node = FloatNode::new("multiplier", None, Some(Number::Int(1)), None)?;
    set_placeholder(b"env_FOO", php_array!["float" => 0.0])?;
    let result = node.finalize(Value::from("env_FOO"));
    reset_placeholders();
    assert_eq!(result?, Value::from("env_FOO"));
    Ok(())
}
