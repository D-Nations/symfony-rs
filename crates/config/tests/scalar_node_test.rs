//! ScalarNodeTest.php at symfony/config v8.1.10. Names and assertions are
//! PHPUnit's; provider rows are upstream's, written as literals. The
//! deprecation handler is [`symfony_deprecation_contracts`]'s.
//!
//! Upstream: symfony/config Tests/Definition/ScalarNodeTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_class, expect_exception};
use php_value::{PhpArray, Value, object::StdClass, php_array};
use symfony_config::{
    Class,
    definition::{ArrayNode, ScalarNode},
};
use symfony_deprecation_contracts::collect;

fn get_valid_values() -> Vec<Value> {
    vec![
        Value::Bool(false),
        Value::Bool(true),
        Value::Null,
        Value::from(""),
        Value::from("foo"),
        Value::Int(0),
        Value::Int(1),
        Value::Float(0.0),
        Value::Float(0.1),
    ]
}

#[test]
fn testNormalize() -> TestResult {
    for value in get_valid_values() {
        let node = ScalarNode::new("test", None)?;
        assert_eq!(node.normalize(value.clone())?, value);
    }
    Ok(())
}

#[test]
fn testSetDeprecated() -> TestResult {
    let child_node = ScalarNode::new("foo", None)?;
    child_node.set_deprecated(b"vendor/package", b"1.1", b"\"%node%\" is deprecated");

    assert!(child_node.is_deprecated());
    let deprecation = child_node.get_deprecation(&child_node.get_name(), &child_node.get_path())?;
    assert_eq!(deprecation.message, b"\"foo\" is deprecated");
    assert_eq!(deprecation.package, b"vendor/package");
    assert_eq!(deprecation.version, b"1.1");
    assert_eq!(
        child_node.get_deprecation_message(None)?,
        b"Since vendor/package 1.1: \"foo\" is deprecated"
    );

    let node = ArrayNode::new("root", None)?;
    node.add_child(&child_node)?;

    let (result, deprecations) = collect(|| node.finalize(PhpArray::new().into()));
    result?;
    assert_eq!(
        0,
        deprecations.len(),
        "->finalize() should not trigger if the deprecated node is not set"
    );

    let (result, deprecations) = collect(|| node.finalize(php_array!["foo" => ""].into()));
    result?;
    assert_eq!(
        1,
        deprecations.len(),
        "->finalize() should trigger if the deprecated node is set"
    );
    Ok(())
}

#[test]
fn testNotDeprecatedException() -> TestResult {
    let child_node = ScalarNode::new("foo", None)?;
    assert!(!child_node.is_deprecated());
    expect_exception(
        child_node.get_deprecation(b"node", b"path"),
        Class::Logic,
        "The node \"foo\" is not deprecated.",
    );
    Ok(())
}

fn get_invalid_values() -> Vec<Value> {
    vec![
        Value::Array(PhpArray::new()),
        Value::Array(php_array!["foo" => "bar"]),
        StdClass::object(PhpArray::new()),
    ]
}

#[test]
fn testNormalizeThrowsExceptionOnInvalidValues() -> TestResult {
    for value in get_invalid_values() {
        let node = ScalarNode::new("test", None)?;
        expect_class(node.normalize(value), Class::InvalidType);
    }
    Ok(())
}

#[test]
fn testNormalizeThrowsExceptionWithoutHint() -> TestResult {
    let node = ScalarNode::new("test", None)?;
    expect_exception(
        node.normalize(PhpArray::new().into()),
        Class::InvalidType,
        "Invalid type for path \"test\". Expected \"scalar\", but got \"array\".",
    );
    Ok(())
}

#[test]
fn testNormalizeThrowsExceptionWithErrorMessage() -> TestResult {
    let node = ScalarNode::new("test", None)?;
    node.set_info(b"\"the test value\"");
    expect_exception(
        node.normalize(PhpArray::new().into()),
        Class::InvalidType,
        "Invalid type for path \"test\". Expected \"scalar\", but got \"array\".\nHint: \"the test value\"",
    );
    Ok(())
}

fn get_valid_non_empty_values() -> Vec<Value> {
    vec![
        Value::Bool(false),
        Value::Bool(true),
        Value::from("foo"),
        Value::Int(0),
        Value::Int(1),
        Value::Float(0.0),
        Value::Float(0.1),
    ]
}

#[test]
fn testValidNonEmptyValues() -> TestResult {
    for value in get_valid_non_empty_values() {
        let node = ScalarNode::new("test", None)?;
        node.set_allow_empty_value(false)?;
        assert_eq!(node.finalize(value.clone())?, value);
    }
    Ok(())
}

fn get_empty_values() -> Vec<Value> {
    vec![Value::Null, Value::from("")]
}

#[test]
fn testNotAllowedEmptyValuesThrowException() -> TestResult {
    for value in get_empty_values() {
        let node = ScalarNode::new("test", None)?;
        node.set_allow_empty_value(false)?;
        expect_class(node.finalize(value), Class::InvalidConfiguration);
    }
    Ok(())
}
