//! MergeTest.php at symfony/config v8.1.10. Names and assertions are
//! PHPUnit's; `assertEquals()` is `phpunit_equals()`.
//!
//! Upstream: symfony/config Tests/Definition/MergeTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_class};
use php_value::{PhpArray, Value, php_array, vectors::phpunit_equals};
use symfony_config::{
    Class,
    definition::builder::{TreeBuilder, class},
};

fn array(a: PhpArray) -> Value {
    Value::Array(a)
}

#[track_caller]
fn assert_equals(expected: Value, actual: Value) {
    assert!(
        phpunit_equals(&expected, &actual),
        "expected {expected:?}, got {actual:?}"
    );
}

#[test]
fn testForbiddenOverwrite() -> TestResult {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .children()
        .node("foo", "scalar")?
        .cannot_be_overwritten(true)
        .end()
        .end()
        .end();
    let tree = tb.build_tree()?;

    let a = array(php_array!["foo" => "bar"]);
    let b = array(php_array!["foo" => "moo"]);

    expect_class(tree.merge(a, b), Class::ForbiddenOverwrite);
    Ok(())
}

#[test]
fn testUnsetKey() -> TestResult {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .children()
        .node("foo", "scalar")?
        .end()
        .node("bar", "scalar")?
        .end()
        .node("unsettable", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .can_be_unset(true)
        .children()
        .node("foo", "scalar")?
        .end()
        .node("bar", "scalar")?
        .end()
        .end()
        .end()
        .node("unsetted", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .can_be_unset(true)
        .prototype("scalar")?
        .end()
        .end()
        .end()
        .end();
    let tree = tb.build_tree()?;

    let a = array(php_array![
        "foo" => "bar",
        "unsettable" => array(php_array!["foo" => "a", "bar" => "b"]),
        "unsetted" => false,
    ]);
    let b = array(php_array![
        "foo" => "moo",
        "bar" => "b",
        "unsettable" => false,
        "unsetted" => array(php_array!["a", "b"]),
    ]);

    assert_equals(
        array(php_array![
            "foo" => "moo",
            "bar" => "b",
            "unsettable" => false,
            "unsetted" => array(php_array!["a", "b"]),
        ]),
        tree.merge(a, b)?,
    );
    Ok(())
}

#[test]
fn testDoesNotAllowNewKeysInSubsequentConfigs() -> TestResult {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .children()
        .node("test", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .disallow_new_keys_in_subsequent_configs()
        .use_attribute_as_key("key", true)
        .prototype("array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .children()
        .node("value", "scalar")?
        .end()
        .end()
        .end()
        .end()
        .end()
        .end();
    let tree = tb.build_tree()?;

    let a = array(php_array![
        "test" => array(php_array!["a" => array(php_array!["value" => "foo"])])
    ]);
    let b = array(php_array![
        "test" => array(php_array!["b" => array(php_array!["value" => "foo"])])
    ]);

    expect_class(tree.merge(a, b), Class::InvalidConfiguration);
    Ok(())
}

#[test]
fn testPerformsNoDeepMerging() -> TestResult {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .children()
        .node("no_deep_merging", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .perform_no_deep_merging()
        .children()
        .node("foo", "scalar")?
        .end()
        .node("bar", "scalar")?
        .end()
        .end()
        .end()
        .end()
        .end();
    let tree = tb.build_tree()?;

    let a = array(php_array![
        "no_deep_merging" => array(php_array!["foo" => "a", "bar" => "b"])
    ]);
    let b = array(php_array!["no_deep_merging" => array(php_array!["c" => "d"])]);

    assert_equals(
        array(php_array!["no_deep_merging" => array(php_array!["c" => "d"])]),
        tree.merge(a, b)?,
    );
    Ok(())
}

#[test]
fn testPrototypeWithoutAKeyAttribute() -> TestResult {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .children()
        .array_node("append_elements", None)?
        .prototype("scalar")?
        .end()
        .end()
        .end()
        .end();
    let tree = tb.build_tree()?;

    let a = array(php_array!["append_elements" => array(php_array!["a", "b"])]);
    let b = array(php_array!["append_elements" => array(php_array!["c", "d"])]);

    assert_equals(
        array(php_array!["append_elements" => array(php_array!["a", "b", "c", "d"])]),
        tree.merge(a, b)?,
    );
    Ok(())
}
