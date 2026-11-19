//! NodeBuilderTest.php at symfony/config v8.1.10. Names and assertions
//! are PHPUnit's. The test's `SomeNodeDefinition extends
//! VariableNodeDefinition {}` is a [`NodeClass::Subclass`].
//!
//! Upstream: symfony/config Tests/Definition/Builder/NodeBuilderTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::{TestResult, expect_class};
use symfony_config::{
    Class,
    definition::builder::{Builtin, NodeBuilder, NodeClass},
};

const SOME_NODE_DEFINITION: &str =
    "Symfony\\Component\\Config\\Tests\\Definition\\Builder\\SomeNodeDefinition";

fn some_node_definition() -> NodeClass {
    NodeClass::Subclass {
        class: Arc::from(SOME_NODE_DEFINITION),
        of: Builtin::Variable,
    }
}

#[test]
fn testThrowsAnExceptionWhenTryingToCreateANonRegisteredNodeType() {
    let builder = NodeBuilder::new();
    expect_class(builder.node("", "foobar"), Class::Runtime);
}

#[test]
fn testThrowsAnExceptionWhenTheNodeClassIsNotFound() {
    let builder = NodeBuilder::new();
    expect_class(
        builder
            .set_node_class("noclasstype", "\\foo\\bar\\noclass")
            .node("", "noclasstype"),
        Class::Runtime,
    );
}

#[test]
fn testAddingANewNodeType() -> TestResult {
    let builder = NodeBuilder::new();
    let node = builder
        .set_node_class("newtype", some_node_definition())
        .node("", "newtype")?;
    assert!(node.is_instance_of(SOME_NODE_DEFINITION));
    Ok(())
}

#[test]
fn testOverridingAnExistingNodeType() -> TestResult {
    let builder = NodeBuilder::new();
    let node = builder
        .set_node_class("variable", some_node_definition())
        .node("", "variable")?;
    assert!(node.is_instance_of(SOME_NODE_DEFINITION));
    Ok(())
}

#[test]
fn testNodeTypesAreNotCaseSensitive() -> TestResult {
    let builder = NodeBuilder::new();

    let node1 = builder.node("", "VaRiAbLe")?;
    let node2 = builder.node("", "variable")?;
    assert!(node2.is_instance_of(&node1.class()));

    builder.set_node_class("CuStOm", some_node_definition());

    let node1 = builder.node("", "CUSTOM")?;
    let node2 = builder.node("", "custom")?;
    assert!(node2.is_instance_of(&node1.class()));
    Ok(())
}

#[test]
fn testNumericNodeCreation() -> TestResult {
    let builder = NodeBuilder::new();

    let node = builder.integer_node("foo")?.min(3)?.max(5)?;
    assert!(node.is_instance_of(Builtin::Integer.class()));

    let node = builder.float_node("bar")?.min(3.0)?.max(5.0)?;
    assert!(node.is_instance_of(Builtin::Float.class()));
    Ok(())
}

#[test]
fn testStringNodeCreation() -> TestResult {
    let builder = NodeBuilder::new();
    let node = builder.string_node("foo bar")?;
    assert!(node.is_instance_of(Builtin::String.class()));
    Ok(())
}
