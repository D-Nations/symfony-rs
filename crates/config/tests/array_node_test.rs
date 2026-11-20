//! ArrayNodeTest.php at symfony/config v8.1.10. Names and assertions are
//! PHPUnit's; the provider rows are upstream's, written as literals. The
//! protected methods upstream calls through ReflectionMethod are public
//! here. The deprecation handler is [`symfony_deprecation_contracts`]'s.
//!
//! Upstream: symfony/config Tests/Definition/ArrayNodeTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_class, expect_exception};
use php_value::{PhpArray, Value, php_array, vectors::phpunit_equals};
use symfony_config::{
    Class,
    definition::{ArrayNode, ScalarNode},
};
use symfony_deprecation_contracts::collect;

#[test]
fn testNormalizeThrowsExceptionWhenFalseIsNotAllowed() -> TestResult {
    let node = ArrayNode::new("root", None)?;
    expect_class(node.normalize(Value::Bool(false)), Class::InvalidType);
    Ok(())
}

#[test]
fn testExceptionThrownOnUnrecognizedChild() -> TestResult {
    let node = ArrayNode::new("root", None)?;
    expect_exception(
        node.normalize(php_array!["foo" => "bar"].into()),
        Class::InvalidConfiguration,
        "Unrecognized option \"foo\" under \"root\"",
    );
    Ok(())
}

#[test]
fn testNormalizeWithProposals() -> TestResult {
    let node = ArrayNode::new("root", None)?;
    node.add_child(&ArrayNode::new("alpha1", None)?)?;
    node.add_child(&ArrayNode::new("alpha2", None)?)?;
    node.add_child(&ArrayNode::new("beta", None)?)?;
    expect_exception(
        node.normalize(php_array!["alpha3" => "foo"].into()),
        Class::InvalidConfiguration,
        "Did you mean \"alpha1\", \"alpha2\"?",
    );
    Ok(())
}

#[test]
fn testNormalizeWithoutProposals() -> TestResult {
    let node = ArrayNode::new("root", None)?;
    node.add_child(&ArrayNode::new("alpha1", None)?)?;
    node.add_child(&ArrayNode::new("alpha2", None)?)?;
    expect_exception(
        node.normalize(php_array!["beta" => "foo"].into()),
        Class::InvalidConfiguration,
        "Available options are \"alpha1\", \"alpha2\".",
    );
    Ok(())
}

#[test]
fn testIgnoreAndRemoveBehaviors() -> TestResult {
    let unrecognized = "Unrecognized option \"foo\" under \"root\"";
    let rows: Vec<(bool, bool, Result<PhpArray, &str>, &str)> = vec![
        (
            true,
            true,
            Ok(PhpArray::new()),
            "no exception is thrown for an unrecognized child if the ignoreExtraKeys option is set to true",
        ),
        (
            true,
            false,
            Ok(php_array!["foo" => "bar"]),
            "extra keys are not removed when ignoreExtraKeys second option is set to false",
        ),
        (false, true, Err(unrecognized), ""),
        (false, false, Err(unrecognized), ""),
    ];
    for (ignore, remove, expected, message) in rows {
        let node = ArrayNode::new("root", None)?;
        node.set_ignore_extra_keys(ignore, remove)?;
        let result = node.normalize(php_array!["foo" => "bar"].into());
        match expected {
            Ok(expected) => assert_eq!(result?, Value::Array(expected), "{message}"),
            Err(text) => expect_exception(result, Class::InvalidConfiguration, text),
        }
    }
    Ok(())
}

#[test]
fn testPreNormalize() -> TestResult {
    let rows = vec![
        (
            php_array!["foo-bar" => "foo"],
            php_array!["foo_bar" => "foo"],
        ),
        (
            php_array!["foo-bar_moo" => "foo"],
            php_array!["foo-bar_moo" => "foo"],
        ),
        (
            php_array!["anything-with-dash-and-no-underscore" => "first", "no_dash" => "second"],
            php_array!["anything_with_dash_and_no_underscore" => "first", "no_dash" => "second"],
        ),
        (
            php_array!["foo-bar" => Value::Null, "foo_bar" => "foo"],
            php_array!["foo-bar" => Value::Null, "foo_bar" => "foo"],
        ),
    ];
    for (denormalized, normalized) in rows {
        let node = ArrayNode::new("foo", None)?;
        assert_eq!(
            node.pre_normalize(denormalized.into()),
            Value::Array(normalized)
        );
    }
    Ok(())
}

fn get_zero_named_node_examples_data() -> Vec<(PhpArray, PhpArray)> {
    let data = php_array![
        0 => php_array!["name" => "something"],
        5 => php_array![0 => "this won't work too", "new_key" => "some other value"],
        "string_key" => "just value",
    ];
    vec![(data.clone(), data)]
}

#[test]
fn testNodeNameCanBeZero() -> TestResult {
    for (denormalized, normalized) in get_zero_named_node_examples_data() {
        let zero_node = ArrayNode::new("0", None)?;
        zero_node.add_child(&ScalarNode::new("name", None)?)?;
        let five_node = ArrayNode::new("5", None)?;
        five_node.add_child(&ScalarNode::new("0", None)?)?;
        five_node.add_child(&ScalarNode::new("new_key", None)?)?;
        let root_node = ArrayNode::new("root", None)?;
        root_node.add_child(&zero_node)?;
        root_node.add_child(&five_node)?;
        root_node.add_child(&ScalarNode::new("string_key", None)?)?;
        assert_eq!(
            root_node.normalize_value(denormalized.into())?,
            Value::Array(normalized)
        );
    }
    Ok(())
}

fn get_pre_normalized_normalized_ordered_data() -> Vec<(PhpArray, PhpArray)> {
    let data = php_array!["2" => "two", "1" => "one", "3" => "three"];
    vec![(data.clone(), data)]
}

#[test]
fn testChildrenOrderIsMaintainedOnNormalizeValue() -> TestResult {
    for (prenormalized, normalized) in get_pre_normalized_normalized_ordered_data() {
        let scalar1 = ScalarNode::new("1", None)?;
        let scalar2 = ScalarNode::new("2", None)?;
        let scalar3 = ScalarNode::new("3", None)?;
        let node = ArrayNode::new("foo", None)?;
        node.add_child(&scalar1)?;
        node.add_child(&scalar3)?;
        node.add_child(&scalar2)?;
        assert_eq!(
            node.normalize_value(prenormalized.into())?,
            Value::Array(normalized)
        );
    }
    Ok(())
}

#[test]
fn testAddChildEmptyName() -> TestResult {
    let node = ArrayNode::new("root", None)?;
    let child_node = ArrayNode::new("", None)?;
    expect_exception(
        node.add_child(&child_node),
        Class::InvalidArgument,
        "Child nodes must be named.",
    );
    Ok(())
}

#[test]
fn testAddChildNameAlreadyExists() -> TestResult {
    let node = ArrayNode::new("root", None)?;
    let child_node = ArrayNode::new("foo", None)?;
    node.add_child(&child_node)?;
    let child_node_with_same_name = ArrayNode::new("foo", None)?;
    expect_exception(
        node.add_child(&child_node_with_same_name),
        Class::InvalidArgument,
        "A child node named \"foo\" already exists.",
    );
    Ok(())
}

#[test]
fn testGetDefaultValueWithoutDefaultValue() -> TestResult {
    let node = ArrayNode::new("foo", None)?;
    expect_exception(
        node.get_default_value(),
        Class::Runtime,
        "The node at path \"foo\" has no default value.",
    );
    Ok(())
}

#[test]
fn testSetDeprecated() -> TestResult {
    let child_node = ArrayNode::new("foo", None)?;
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
    assert!(
        deprecations.is_empty(),
        "->finalize() should not trigger if the deprecated node is not set"
    );

    let (result, deprecations) =
        collect(|| node.finalize(php_array!["foo" => PhpArray::new()].into()));
    result?;
    assert!(
        !deprecations.is_empty(),
        "->finalize() should trigger if the deprecated node is set"
    );
    Ok(())
}

fn get_data_with_included_extra_keys() -> Vec<((PhpArray, PhpArray), PhpArray)> {
    vec![(
        (
            php_array!["foo" => "bar", "baz" => "not foo"],
            php_array!["bar" => "baz", "baz" => "foo"],
        ),
        php_array!["foo" => "bar", "bar" => "baz", "baz" => "foo"],
    )]
}

fn foo_bar_root() -> Result<symfony_config::Node, symfony_config::Error> {
    let node = ArrayNode::new("root", None)?;
    node.add_child(&ScalarNode::new("foo", None)?)?;
    node.add_child(&ScalarNode::new("bar", None)?)?;
    Ok(node)
}

#[test]
fn testMergeWithoutIgnoringExtraKeys() -> TestResult {
    for ((left, right), _merged) in get_data_with_included_extra_keys() {
        let node = foo_bar_root()?;
        node.set_ignore_extra_keys(false, true)?;
        expect_exception(
            node.merge_values(left.into(), right.into()),
            Class::Runtime,
            "merge() expects a normalized config array.",
        );
    }
    Ok(())
}

#[test]
fn testMergeWithIgnoringAndRemovingExtraKeys() -> TestResult {
    for ((left, right), _merged) in get_data_with_included_extra_keys() {
        let node = foo_bar_root()?;
        node.set_ignore_extra_keys(true, true)?;
        expect_exception(
            node.merge_values(left.into(), right.into()),
            Class::Runtime,
            "merge() expects a normalized config array.",
        );
    }
    Ok(())
}

#[test]
fn testMergeWithIgnoringExtraKeys() -> TestResult {
    for ((left, right), merged) in get_data_with_included_extra_keys() {
        let node = foo_bar_root()?;
        node.set_ignore_extra_keys(true, false)?;
        let result = node.merge_values(left.into(), right.into())?;
        assert!(phpunit_equals(&Value::Array(merged), &result), "{result:?}");
    }
    Ok(())
}
