//! TreeBuilderTest.php at symfony/config v8.1.10. Names and assertions
//! are PHPUnit's; `assertInstanceOf()` is `is_instance_of()`.
//!
//! Upstream: symfony/config Tests/Definition/Builder/TreeBuilderTest.php
#![allow(non_snake_case)]

mod common;

use common::{
    TestResult,
    builder::{
        BAR_NODE, BAR_NODE_DEFINITION, CUSTOM_NODE_BUILDER, VARIABLE_NODE_DEFINITION, bar_node,
        custom_node_builder,
    },
};
use php_value::{Key, Value, php_array};
use symfony_config::{
    Node,
    definition::{
        base_node::BASE_NODE,
        boolean_node,
        builder::{Builtin, TreeBuilder},
    },
};

fn child(children: &[(Key, Node)], name: &str) -> Node {
    children
        .iter()
        .find(|(k, _)| *k == Key::from(name))
        .map(|(_, n)| n.clone())
        .unwrap_or_else(|| panic!("no child {name}"))
}

fn has_key(children: &[(Key, Node)], name: &str) -> bool {
    children.iter().any(|(k, _)| *k == Key::from(name))
}

#[test]
fn testUsingACustomNodeBuilder() -> TestResult {
    let builder = TreeBuilder::with_builder("custom", custom_node_builder())?;

    let node_builder = builder.get_root_node().children();
    assert!(node_builder.is_instance_of(CUSTOM_NODE_BUILDER));

    let node_builder = node_builder.array_node("deeper", None)?.children();
    assert!(node_builder.is_instance_of(CUSTOM_NODE_BUILDER));
    Ok(())
}

#[test]
fn testOverrideABuiltInNodeType() -> TestResult {
    let builder = TreeBuilder::with_builder("override", custom_node_builder())?;
    let definition = builder
        .get_root_node()
        .children()
        .variable_node("variable")?;
    assert!(definition.is_instance_of(VARIABLE_NODE_DEFINITION));
    Ok(())
}

#[test]
fn testAddANodeType() -> TestResult {
    let builder = TreeBuilder::with_builder("override", custom_node_builder())?;
    let definition = bar_node(&builder.get_root_node().children(), "variable")?;
    assert!(definition.is_instance_of(BAR_NODE_DEFINITION));
    Ok(())
}

#[test]
fn testCreateABuiltInNodeTypeWithACustomNodeBuilder() -> TestResult {
    let builder = TreeBuilder::with_builder("builtin", custom_node_builder())?;
    let definition = builder.get_root_node().children().boolean_node("boolean")?;
    assert!(definition.is_instance_of(Builtin::Boolean.class()));
    Ok(())
}

#[test]
fn testPrototypedArrayNodeUseTheCustomNodeBuilder() -> TestResult {
    let builder = TreeBuilder::with_builder("override", custom_node_builder())?;
    let root = builder.get_root_node();
    root.prototype("bar")?.end();
    assert!(
        root.get_node(true)?
            .get_prototype()?
            .is_instance_of(BAR_NODE)
    );
    Ok(())
}

#[test]
fn testAnExtendedNodeBuilderGetsPropagatedToTheChildren() -> TestResult {
    let builder = TreeBuilder::new("propagation")?;
    builder
        .get_root_node()
        .children()
        .set_node_class(
            "extended",
            "Symfony\\Component\\Config\\Definition\\Builder\\BooleanNodeDefinition",
        )
        .node("foo", "extended")?
        .end()
        .array_node("child", None)?
        .children()
        .node("foo", "extended")?
        .end()
        .end()
        .end()
        .end();

    let node = builder.build_tree()?;
    let children = node.get_children()?;
    assert!(child(&children, "foo").is_instance_of(boolean_node::CLASS));
    let child_children = child(&children, "child").get_children()?;
    assert!(child(&child_children, "foo").is_instance_of(boolean_node::CLASS));
    Ok(())
}

#[test]
fn testDefinitionInfoGetsTransferredToNode() -> TestResult {
    let builder = TreeBuilder::new("test")?;
    builder
        .get_root_node()
        .info("root info")
        .children()
        .node("child", "variable")?
        .info("child info")
        .default_value(Value::from("default"))?
        .end()
        .end();

    let tree = builder.build_tree()?;
    let children = tree.get_children()?;
    assert_eq!(tree.get_info().as_deref(), Some(&b"root info"[..]));
    assert_eq!(
        child(&children, "child").get_info().as_deref(),
        Some(&b"child info"[..])
    );
    Ok(())
}

#[test]
fn testDefinitionExampleGetsTransferredToNode() -> TestResult {
    let builder = TreeBuilder::new("test")?;
    builder
        .get_root_node()
        .example(php_array!["key" => "value"])?
        .children()
        .node("child", "variable")?
        .info("child info")
        .default_value(Value::from("default"))?
        .example("example")?
        .end()
        .end();

    let tree = builder.build_tree()?;
    let children = tree.get_children()?;
    assert!(matches!(tree.get_example(), Value::Array(_)));
    assert_eq!(
        child(&children, "child").get_example(),
        Value::from("example")
    );
    Ok(())
}

fn propagation_tree() -> Result<TreeBuilder, symfony_config::Error> {
    let builder = TreeBuilder::new("propagation")?;
    builder
        .get_root_node()
        .children()
        .node("foo", "variable")?
        .end()
        .array_node("child", None)?
        .children()
        .node("foo", "variable")?
        .end()
        .end()
        .end()
        .end();
    Ok(builder)
}

#[test]
fn testDefaultPathSeparatorIsDot() -> TestResult {
    let builder = propagation_tree()?;
    let node = builder.build_tree()?;
    let children = node.get_children()?;

    assert!(has_key(&children, "foo"));
    assert!(child(&children, "foo").is_instance_of(BASE_NODE));
    assert_eq!(&*child(&children, "foo").get_path(), b"propagation.foo");

    assert!(has_key(&children, "child"));
    let child_children = child(&children, "child").get_children()?;
    assert!(has_key(&child_children, "foo"));
    assert!(child(&child_children, "foo").is_instance_of(BASE_NODE));
    assert_eq!(
        &*child(&child_children, "foo").get_path(),
        b"propagation.child.foo"
    );
    Ok(())
}

#[test]
fn testPathSeparatorIsPropagatedToChildren() -> TestResult {
    let builder = propagation_tree()?;
    builder.set_path_separator("/");
    let node = builder.build_tree()?;
    let children = node.get_children()?;

    assert!(has_key(&children, "foo"));
    assert!(child(&children, "foo").is_instance_of(BASE_NODE));
    assert_eq!(&*child(&children, "foo").get_path(), b"propagation/foo");

    assert!(has_key(&children, "child"));
    let child_children = child(&children, "child").get_children()?;
    assert!(has_key(&child_children, "foo"));
    assert!(child(&child_children, "foo").is_instance_of(BASE_NODE));
    assert_eq!(
        &*child(&child_children, "foo").get_path(),
        b"propagation/child/foo"
    );
    Ok(())
}
