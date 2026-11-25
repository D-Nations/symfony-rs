//! EnumNodeDefinitionTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; the fixture enums are registered in a
//! Registry (`common::enums()`).
//!
//! Upstream: symfony/config Tests/Definition/Builder/EnumNodeDefinitionTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, enums, expect_exception, fixture};
use php_value::{PhpArray, Value, vectors::phpunit_equals};
use symfony_config::{Class, definition::builder::EnumNodeDefinition};

fn list(values: Vec<Value>) -> Value {
    Value::Array(PhpArray::list(values))
}

#[test]
fn testWithOneValue() -> TestResult {
    let def = EnumNodeDefinition::new("foo");
    def.values(vec![Value::from("foo")])?;

    let node = def.get_node(false)?;
    assert!(phpunit_equals(
        &list(vec![Value::from("foo")]),
        &list(node.get_values()?)
    ));
    Ok(())
}

#[test]
fn testWithUnitEnumFqcn() -> TestResult {
    let def = EnumNodeDefinition::new("foo");
    def.enum_fqcn(&fixture("TestEnum"), &enums())?;

    let node = def.get_node(false)?;
    assert_eq!(node.get_enum_fqcn()?, Some(fixture("TestEnum")));
    Ok(())
}

#[test]
fn testNoValuesPassed() {
    let def = EnumNodeDefinition::new("foo");
    expect_exception(
        def.get_node(false),
        Class::Runtime,
        "You must call either ->values() or ->enumFqcn() on enum nodes.",
    );
}

#[test]
fn testBothValuesAndEnumFqcnPassed() -> TestResult {
    let def = EnumNodeDefinition::new("foo");
    def.values(vec![Value::Int(123)])?
        .enum_fqcn(&fixture("StringBackedTestEnum"), &enums())?;

    expect_exception(
        def.get_node(false),
        Class::Runtime,
        "You must call either ->values() or ->enumFqcn() on enum nodes but not both.",
    );
    Ok(())
}

#[test]
fn testWithNoValues() {
    let def = EnumNodeDefinition::new("foo");
    expect_exception(
        def.values(Vec::new()),
        Class::InvalidArgument,
        "->values() must be called with at least one value.",
    );
}

#[test]
fn testGetNode() -> TestResult {
    let def = EnumNodeDefinition::new("foo");
    def.values(vec![Value::from("foo"), Value::from("bar")])?;

    let node = def.get_node(false)?;
    assert!(phpunit_equals(
        &list(vec![Value::from("foo"), Value::from("bar")]),
        &list(node.get_values()?)
    ));
    Ok(())
}

#[test]
fn testSetDeprecated() -> TestResult {
    let def = EnumNodeDefinition::new("foo");
    def.values(vec![Value::from("foo"), Value::from("bar")])?;
    def.set_deprecated(
        "vendor/package",
        "1.1",
        "The \"%path%\" node is deprecated.",
    );

    let node = def.get_node(false)?;

    assert!(node.is_deprecated());
    let deprecation = node.get_deprecation(&node.get_name(), &node.get_path())?;
    assert_eq!(deprecation.message, b"The \"foo\" node is deprecated.");
    assert_eq!(deprecation.package, b"vendor/package");
    assert_eq!(deprecation.version, b"1.1");
    assert_eq!(
        node.get_deprecation_message(None)?,
        b"Since vendor/package 1.1: The \"foo\" node is deprecated."
    );
    Ok(())
}

#[test]
fn testSameStringCoercedValuesAreDifferent() -> TestResult {
    let def = EnumNodeDefinition::new("ccc");
    def.values(vec![Value::from(""), Value::Bool(false), Value::Null])?;

    // assertSame
    assert!(
        list(vec![Value::from(""), Value::Bool(false), Value::Null])
            .identical(&list(def.get_node(false)?.get_values()?))
    );
    Ok(())
}
