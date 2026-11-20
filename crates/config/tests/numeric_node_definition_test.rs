//! NumericNodeDefinitionTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's.
//!
//! Upstream: symfony/config Tests/Definition/Builder/NumericNodeDefinitionTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_exception};
use php_value::{Value, vectors::phpunit_equals};
use symfony_config::{
    Class,
    definition::builder::{FloatNodeDefinition, IntegerNodeDefinition},
};

#[test]
fn testIncoherentMinAssertion() {
    let node = IntegerNodeDefinition::new("foo");
    expect_exception(
        node.max(3).and_then(|n| n.min(4)),
        Class::InvalidArgument,
        "You cannot define a min(4) as you already have a max(3)",
    );
}

#[test]
fn testIncoherentMaxAssertion() {
    let node = IntegerNodeDefinition::new("foo");
    expect_exception(
        node.min(3).and_then(|n| n.max(2)),
        Class::InvalidArgument,
        "You cannot define a max(2) as you already have a min(3)",
    );
}

#[test]
fn testIntegerMinAssertion() {
    let node = IntegerNodeDefinition::new("foo");
    expect_exception(
        node.min(5)
            .and_then(|n| n.get_node(false))
            .and_then(|n| n.finalize(Value::Int(4))),
        Class::InvalidConfiguration,
        "The value 4 is too small for path \"foo\". Should be greater than or equal to 5",
    );
}

#[test]
fn testIntegerMaxAssertion() {
    let node = IntegerNodeDefinition::new("foo");
    expect_exception(
        node.max(3)
            .and_then(|n| n.get_node(false))
            .and_then(|n| n.finalize(Value::Int(4))),
        Class::InvalidConfiguration,
        "The value 4 is too big for path \"foo\". Should be less than or equal to 3",
    );
}

#[test]
fn testIntegerValidMinMaxAssertion() -> TestResult {
    let node = IntegerNodeDefinition::new("foo");
    let node = node.min(3)?.max(7)?.get_node(false)?;
    assert!(phpunit_equals(
        &Value::Int(4),
        &node.finalize(Value::Int(4))?
    ));
    Ok(())
}

#[test]
fn testFloatMinAssertion() {
    let node = FloatNodeDefinition::new("foo");
    expect_exception(
        node.min(5e2)
            .and_then(|n| n.get_node(false))
            .and_then(|n| n.finalize(Value::Float(4e2))),
        Class::InvalidConfiguration,
        "The value 400 is too small for path \"foo\". Should be greater than or equal to 500",
    );
}

#[test]
fn testFloatMaxAssertion() {
    let node = FloatNodeDefinition::new("foo");
    expect_exception(
        node.max(0.3)
            .and_then(|n| n.get_node(false))
            .and_then(|n| n.finalize(Value::Float(4.3))),
        Class::InvalidConfiguration,
        "The value 4.3 is too big for path \"foo\". Should be less than or equal to 0.3",
    );
}

#[test]
fn testFloatValidMinMaxAssertion() -> TestResult {
    let node = FloatNodeDefinition::new("foo");
    let node = node.min(3.0)?.max(7e2)?.get_node(false)?;
    assert!(phpunit_equals(
        &Value::Float(4.5),
        &node.finalize(Value::Float(4.5))?
    ));
    Ok(())
}

#[test]
fn testCannotBeEmptyThrowsAnException() {
    let node = IntegerNodeDefinition::new("foo");
    expect_exception(
        node.cannot_be_empty(),
        Class::InvalidDefinition,
        "->cannotBeEmpty() is not applicable to NumericNodeDefinition.",
    );
}
