//! PrototypedArrayNodeTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; the provider rows are upstream's, written as
//! literals (their node objects created once, as PHPUnit's providers are).
//!
//! Upstream: symfony/config Tests/Definition/PrototypedArrayNodeTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::needless_pass_by_value,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

mod common;

use common::TestResult;
use php_value::{PhpArray, Value, php_array, vectors::phpunit_equals};
use symfony_config::{
    Error, Node,
    definition::{ArrayNode, PrototypedArrayNode, ScalarNode, VariableNode},
};

#[track_caller]
fn assert_equals(expected: impl Into<Value>, actual: Value) {
    let expected = expected.into();
    assert!(
        phpunit_equals(&expected, &actual),
        "expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn testGetDefaultValueReturnsAnEmptyArrayForPrototypes() -> TestResult {
    let node = PrototypedArrayNode::new("root", None)?;
    let prototype = ArrayNode::new("", Some(&node))?;
    node.set_prototype(&prototype)?;
    assert_eq!(node.get_default_value()?, Value::Array(PhpArray::new()));
    Ok(())
}

#[test]
fn testGetDefaultValueReturnsDefaultValueForPrototypes() -> TestResult {
    let node = PrototypedArrayNode::new("root", None)?;
    let prototype = ArrayNode::new("", Some(&node))?;
    node.set_prototype(&prototype)?;
    node.set_default_value(Value::Array(php_array!["test"]))?;
    assert_equals(php_array!["test"], node.get_default_value()?);
    Ok(())
}

// a remapped key (e.g. "mapping" -> "mappings") should be unset after being used
#[test]
fn testRemappedKeysAreUnset() -> TestResult {
    let node = ArrayNode::new("root", None)?;
    let mappings_node = PrototypedArrayNode::new("mappings", None)?;
    node.add_child(&mappings_node)?;

    // each item under mappings is just a scalar
    let prototype = ScalarNode::new("", Some(&mappings_node))?;
    mappings_node.set_prototype(&prototype)?;

    node.set_xml_remappings(vec![("mapping".into(), "mappings".into())])?;

    let normalized = node.normalize(php_array!["mapping" => php_array!["foo", "bar"]].into())?;
    assert_equals(
        php_array!["mappings" => php_array!["foo", "bar"]],
        normalized,
    );
    Ok(())
}

/// Tests that when a key attribute is mapped, that key is removed from the array.
#[test]
fn testMappedAttributeKeyIsRemoved() -> TestResult {
    let node = PrototypedArrayNode::new("root", None)?;
    node.set_key_attribute(b"id", true)?;

    // each item under the root is an array, with one scalar item
    let prototype = ArrayNode::new("", Some(&node))?;
    prototype.add_child(&ScalarNode::new("foo", None)?)?;
    node.set_prototype(&prototype)?;

    let children = php_array![php_array!["id" => "item_name", "foo" => "bar"]];
    let normalized = node.normalize(children.into())?;

    assert_equals(
        php_array!["item_name" => php_array!["foo" => "bar"]],
        normalized,
    );
    Ok(())
}

/// Tests the opposite of the testMappedAttributeKeyIsRemoved because
/// the removal can be toggled with an option.
#[test]
fn testMappedAttributeKeyNotRemoved() -> TestResult {
    let node = PrototypedArrayNode::new("root", None)?;
    node.set_key_attribute(b"id", false)?;

    // each item under the root is an array, with two scalar items
    let prototype = ArrayNode::new("", Some(&node))?;
    prototype.add_child(&ScalarNode::new("foo", None)?)?;
    prototype.add_child(&ScalarNode::new("id", None)?)?; // the key attribute will remain
    node.set_prototype(&prototype)?;

    let children = php_array![php_array!["id" => "item_name", "foo" => "bar"]];
    let normalized = node.normalize(children.into())?;

    assert_equals(
        php_array!["item_name" => php_array!["id" => "item_name", "foo" => "bar"]],
        normalized,
    );
    Ok(())
}

#[test]
fn testAddDefaultChildren() -> TestResult {
    let foo_bar = || php_array!["foo" => "bar"];

    let node = get_prototype_node_with_default_children()?;
    node.set_add_children_if_none_set(Value::Null)?;
    assert!(node.has_default_value());
    assert_equals(php_array![foo_bar()], node.get_default_value()?);

    let node = get_prototype_node_with_default_children()?;
    node.set_key_attribute(b"foobar", true)?;
    node.set_add_children_if_none_set(Value::Null)?;
    assert!(node.has_default_value());
    assert_equals(
        php_array!["defaults" => foo_bar()],
        node.get_default_value()?,
    );

    let node = get_prototype_node_with_default_children()?;
    node.set_key_attribute(b"foobar", true)?;
    node.set_add_children_if_none_set(Value::from("defaultkey"))?;
    assert!(node.has_default_value());
    assert_equals(
        php_array!["defaultkey" => foo_bar()],
        node.get_default_value()?,
    );

    let node = get_prototype_node_with_default_children()?;
    node.set_key_attribute(b"foobar", true)?;
    node.set_add_children_if_none_set(Value::Array(php_array!["defaultkey"]))?;
    assert!(node.has_default_value());
    assert_equals(
        php_array!["defaultkey" => foo_bar()],
        node.get_default_value()?,
    );

    let node = get_prototype_node_with_default_children()?;
    node.set_key_attribute(b"foobar", true)?;
    node.set_add_children_if_none_set(Value::Array(php_array!["dk1", "dk2"]))?;
    assert!(node.has_default_value());
    assert_equals(
        php_array!["dk1" => foo_bar(), "dk2" => foo_bar()],
        node.get_default_value()?,
    );

    let node = get_prototype_node_with_default_children()?;
    node.set_add_children_if_none_set(Value::Array(php_array![5, 6]))?;
    assert!(node.has_default_value());
    assert_equals(
        php_array![0 => foo_bar(), 1 => foo_bar()],
        node.get_default_value()?,
    );

    let node = get_prototype_node_with_default_children()?;
    node.set_add_children_if_none_set(Value::Int(2))?;
    assert!(node.has_default_value());
    assert_equals(php_array![foo_bar(), foo_bar()], node.get_default_value()?);
    Ok(())
}

#[test]
fn testDefaultChildrenWinsOverDefaultValue() -> TestResult {
    let node = get_prototype_node_with_default_children()?;
    node.set_add_children_if_none_set(Value::Null)?;
    node.set_default_value(Value::Array(php_array!["bar" => "foo"]))?;
    assert!(node.has_default_value());
    assert_equals(
        php_array![php_array!["foo" => "bar"]],
        node.get_default_value()?,
    );
    Ok(())
}

fn get_prototype_node_with_default_children() -> Result<Node, Error> {
    let node = PrototypedArrayNode::new("root", None)?;
    let prototype = ArrayNode::new("", Some(&node))?;
    let child = ScalarNode::new("foo", None)?;
    child.set_default_value(Value::from("bar"))?;
    prototype.add_child(&child)?;
    prototype.set_add_if_not_set(true)?;
    node.set_prototype(&prototype)?;
    Ok(node)
}

/// Tests that when a key attribute is mapped, that key is removed from the array.
/// And if only 'value' element is left in the array, it will replace its wrapper array.
/// (Upstream's docblock gives the XML and array forms of each row.)
#[test]
fn testMappedAttributeKeyIsRemovedLeftValueOnly() -> TestResult {
    for (value, children, expected) in get_data_for_key_removed_left_value_only()? {
        let node = PrototypedArrayNode::new("root", None)?;
        node.set_key_attribute(b"id", true)?;

        // each item under the root is an array, with one scalar item
        let prototype = ArrayNode::new("", Some(&node))?;
        prototype.add_child(&ScalarNode::new("id", None)?)?;
        prototype.add_child(&ScalarNode::new("foo", None)?)?;
        prototype.add_child(&value)?;
        node.set_prototype(&prototype)?;

        let normalized = node.normalize(children.into())?;
        assert_equals(expected, normalized);
    }
    Ok(())
}

fn get_data_for_key_removed_left_value_only() -> Result<Vec<(Node, PhpArray, PhpArray)>, Error> {
    let scalar_value = ScalarNode::new("value", None)?;

    let array_value = ArrayNode::new("value", None)?;
    array_value.add_child(&ScalarNode::new("foo", None)?)?;
    array_value.add_child(&ScalarNode::new("bar", None)?)?;

    let variable_value = VariableNode::new("value", None)?;

    Ok(vec![
        (
            scalar_value.clone(),
            php_array![php_array!["id" => "option1", "value" => "value1"]],
            php_array!["option1" => "value1"],
        ),
        (
            scalar_value,
            php_array![
                php_array!["id" => "option1", "value" => "value1"],
                php_array!["id" => "option2", "value" => "value2", "foo" => "foo2"],
            ],
            php_array![
                "option1" => "value1",
                "option2" => php_array!["value" => "value2", "foo" => "foo2"],
            ],
        ),
        (
            array_value,
            php_array![php_array![
                "id" => "option1",
                "value" => php_array!["foo" => "foo1", "bar" => "bar1"],
            ]],
            php_array!["option1" => php_array!["foo" => "foo1", "bar" => "bar1"]],
        ),
        (
            variable_value,
            php_array![
                php_array!["id" => "option1", "value" => php_array!["foo" => "foo1", "bar" => "bar1"]],
                php_array!["id" => "option2", "value" => "value2"],
            ],
            php_array![
                "option1" => php_array!["foo" => "foo1", "bar" => "bar1"],
                "option2" => "value2",
            ],
        ),
    ])
}

#[test]
fn testPrototypedArrayNodeMerge() -> TestResult {
    for (left, right, expected) in get_prototyped_array_node_data_to_merge() {
        let node = PrototypedArrayNode::new("options", None)?;
        node.set_normalize_keys(false)?;
        node.set_prototype(&VariableNode::new("value", None)?)?;
        node.set_default_value(Value::Array(PhpArray::new()))?;

        let result = node.merge(left.into(), right.into())?;

        assert_eq!(result, Value::Array(expected));
    }
    Ok(())
}

fn get_prototyped_array_node_data_to_merge() -> Vec<(PhpArray, PhpArray, PhpArray)> {
    vec![
        // data to merged is a plain array
        (
            php_array!["foo", "bar"],
            php_array!["foo", "baz", "qux"],
            php_array!["foo", "bar", "foo", "baz", "qux"],
        ),
        // data to be merged is an associative array
        (
            php_array!["option1" => true, "option2" => "foo"],
            php_array![
                "option2" => "bar",
                "option3" => 42,
                "option4" => php_array![
                    "option41" => "baz",
                    "option42" => php_array!["option423" => "qux"],
                ],
            ],
            php_array![
                "option1" => true,
                "option2" => "bar",
                "option3" => 42,
                "option4" => php_array![
                    "option41" => "baz",
                    "option42" => php_array!["option423" => "qux"],
                ],
            ],
        ),
    ]
}
