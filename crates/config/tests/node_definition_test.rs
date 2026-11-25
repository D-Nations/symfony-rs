//! NodeDefinitionTest.php at symfony/config v8.1.10. Names and assertions
//! are PHPUnit's; provider rows are upstream's, written as literals.
//! The mock of testSetPathSeparatorChangesChildren is a
//! [`CustomDefinition`] that overrides `setParent()` and
//! `setPathSeparator()` and counts the calls. `ReflectionProperty` reads
//! are `property()`; Composer's installed phpunit/phpunit is registered
//! with the version the oracle reports.
//!
//! Upstream: symfony/config Tests/Definition/Builder/NodeDefinitionTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::{Arc, Mutex};

use common::{TestResult, expect_exception};
use php_value::{Key, PhpArray, Value};
use symfony_config::{
    Class, Error, Node, composer,
    definition::builder::{
        ArrayNodeDefinition, Builtin, Call, CustomDefinition, NodeClass, NodeDefinition, Parent,
        node_definition::NODE_DEFINITION,
    },
};

#[derive(Default)]
struct MockDefinition {
    path_separators: Mutex<Vec<Vec<u8>>>,
    parents: Mutex<Vec<Parent>>,
}

struct Mock(Arc<MockDefinition>);

impl CustomDefinition for Mock {
    fn class(&self) -> &'static str {
        "MockObject_NodeDefinition"
    }

    fn create_node(&self, _definition: &NodeDefinition) -> Result<Node, Error> {
        Err(Error::new(Class::Error, "not expected"))
    }

    fn set_parent(&self, _definition: &NodeDefinition, parent: &Parent) -> Call {
        if let Ok(mut p) = self.0.parents.lock() {
            p.push(parent.clone());
        }
        Call::Done
    }

    fn set_path_separator(&self, _definition: &NodeDefinition, separator: &[u8]) -> Call {
        if let Ok(mut s) = self.0.path_separators.lock() {
            s.push(separator.to_vec());
        }
        Call::Done
    }
}

#[test]
fn testSetPathSeparatorChangesChildren() -> TestResult {
    let parent_node = ArrayNodeDefinition::new("name");
    let calls = Arc::new(MockDefinition::default());
    let child_node = NodeDefinition::of_class(
        &NodeClass::Custom(Arc::new(Mock(calls.clone()))),
        None::<&str>,
    )?;

    parent_node.append(&child_node);
    parent_node.set_path_separator("/");

    let parents = calls.parents.lock().map_err(|_| "poisoned")?;
    assert_eq!(parents.len(), 1);
    assert!(matches!(&parents[0], Parent::Definition(d) if d.same(&parent_node)));
    let separators = calls.path_separators.lock().map_err(|_| "poisoned")?;
    assert_eq!(*separators, vec![b"/".to_vec()]);
    Ok(())
}

fn doc_url(node: &ArrayNodeDefinition) -> Result<Vec<u8>, Error> {
    let attributes = node.property("attributes")?;
    Ok(attributes
        .as_array()
        .and_then(|a| a.get(&Key::from("docUrl")))
        .and_then(Value::as_string)
        .map(|s| s.to_vec())
        .unwrap_or_default())
}

#[test]
fn testDocUrl() -> TestResult {
    composer::set_installed_versions([("phpunit/phpunit", Some("11.5.57.0"))]);
    let node = ArrayNodeDefinition::new("node");
    node.doc_url(
        "https://example.com/doc/{package}/{version:major}.{version:minor}",
        Some("phpunit/phpunit"),
    )?;

    // ~^https://example.com/doc/phpunit/phpunit/\d+\.\d+$~
    let url = String::from_utf8(doc_url(&node)?)?;
    let rest = url
        .strip_prefix("https://example.com/doc/phpunit/phpunit/")
        .ok_or("prefix")?;
    let (major, minor) = rest.split_once('.').ok_or("no dot")?;
    assert!(!major.is_empty() && major.bytes().all(|b| b.is_ascii_digit()));
    assert!(!minor.is_empty() && minor.bytes().all(|b| b.is_ascii_digit()));
    Ok(())
}

#[test]
fn testDocUrlWithoutPackage() -> TestResult {
    let node = ArrayNodeDefinition::new("node");
    node.doc_url(
        "https://example.com/doc/empty{version:major}.empty{version:minor}",
        None,
    )?;
    assert_eq!(doc_url(&node)?, b"https://example.com/doc/empty.empty");
    Ok(())
}

#[test]
fn testUnknownPackageThrowsException() {
    let node = ArrayNodeDefinition::new("node");
    expect_exception(
        node.doc_url(
            "https://example.com/doc/{package}/{version:major}.{version:minor}",
            Some("phpunit/invalid"),
        ),
        Class::Other {
            class: "OutOfBoundsException".to_owned(),
            exception: true,
        },
        "Package \"phpunit/invalid\" is not installed",
    );
}

fn provide_definition_classes_and_default_values() -> Vec<(Builtin, Value)> {
    vec![
        (Builtin::Array, Value::Array(PhpArray::new())),
        (Builtin::Scalar, Value::Null),
        (Builtin::Boolean, Value::Bool(false)),
        (Builtin::String, Value::from("default")),
        (Builtin::Variable, Value::from("default")),
    ]
}

#[test]
fn testIncoherentRequiredAndDefaultValue() -> TestResult {
    for (class, default_value) in provide_definition_classes_and_default_values() {
        let node = NodeDefinition::of_class(&NodeClass::Builtin(class), "foo")?;
        assert!(node.is_instance_of(NODE_DEFINITION));
        expect_exception(
            node.default_value(default_value)
                .and_then(|n| n.is_required()),
            Class::InvalidDefinition,
            "The node \"foo\" cannot be required and have a default value.",
        );
    }
    Ok(())
}

#[test]
fn testIncoherentDefaultValueAndRequired() -> TestResult {
    for (class, default_value) in provide_definition_classes_and_default_values() {
        let node = NodeDefinition::of_class(&NodeClass::Builtin(class), "foo")?;
        assert!(node.is_instance_of(NODE_DEFINITION));
        expect_exception(
            node.is_required()
                .and_then(|n| n.default_value(default_value)),
            Class::InvalidDefinition,
            "The node \"foo\" cannot be required and have a default value.",
        );
    }
    Ok(())
}
