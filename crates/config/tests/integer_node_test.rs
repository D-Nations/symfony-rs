//! IntegerNodeTest.php at symfony/config v8.1.10. Names and assertions
//! are PHPUnit's; provider rows are upstream's, written as literals.
//! Placeholders are thread-local here, so each test starts from none.
//!
//! Upstream: symfony/config Tests/Definition/IntegerNodeTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_class};
use php_value::{Number, PhpArray, Value, object::StdClass, php_array};
use symfony_config::{
    Class,
    definition::{IntegerNode, reset_placeholders, set_placeholder},
};

fn get_valid_values() -> Vec<i64> {
    vec![1798, -678, 0]
}

#[test]
fn testNormalize() -> TestResult {
    for value in get_valid_values() {
        let node = IntegerNode::new("test", None, None, None)?;
        assert_eq!(node.normalize(Value::Int(value))?, Value::Int(value));
    }
    Ok(())
}

#[test]
fn testValidNonEmptyValues() -> TestResult {
    for value in get_valid_values() {
        let node = IntegerNode::new("test", None, None, None)?;
        node.set_allow_empty_value(false)?;
        assert_eq!(node.finalize(Value::Int(value))?, Value::Int(value));
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
        let node = IntegerNode::new("test", None, None, None)?;
        expect_class(node.normalize(value), Class::InvalidType);
    }
    Ok(())
}

#[test]
fn testFinalizeAcceptsEnvPlaceholderBelowMin() -> TestResult {
    let node = IntegerNode::new("max_retries", None, Some(Number::Int(1)), None)?;
    set_placeholder(b"env_BAR", php_array!["int" => 0])?;
    let result = node.finalize(Value::from("env_BAR"));
    reset_placeholders();
    assert_eq!(result?, Value::from("env_BAR"));
    Ok(())
}
