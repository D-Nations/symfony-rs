//! BooleanNodeDefinitionTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's.
//!
//! Upstream: symfony/config Tests/Definition/Builder/BooleanNodeDefinitionTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_exception};
use php_value::Value;
use symfony_config::{Class, definition::builder::BooleanNodeDefinition};

#[test]
fn testCannotBeEmptyThrowsAnException() {
    let def = BooleanNodeDefinition::new("foo");
    expect_exception(
        def.cannot_be_empty(),
        Class::InvalidDefinition,
        "->cannotBeEmpty() is not applicable to BooleanNodeDefinition.",
    );
}

#[test]
fn testBooleanNodeWithDefaultNull() -> TestResult {
    let def = BooleanNodeDefinition::new("foo");
    def.default_null()?;

    let node = def.get_node(false)?;
    assert!(node.has_default_value());
    assert!(node.get_default_value()?.is_null());

    assert!(node.normalize(Value::Null)?.is_null());
    Ok(())
}

#[test]
fn testBooleanNodeWithDefaultValueAtNull() -> TestResult {
    let def = BooleanNodeDefinition::new("foo");
    def.default_value(Value::Null)?;

    let node = def.get_node(false)?;
    assert!(node.has_default_value());
    assert!(node.get_default_value()?.is_null());

    assert!(node.normalize(Value::Null)?.is_null());
    Ok(())
}

#[test]
fn testSetDeprecated() -> TestResult {
    let def = BooleanNodeDefinition::new("foo");
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
