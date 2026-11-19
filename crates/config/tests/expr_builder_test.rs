//! ExprBuilderTest.php at symfony/config v8.1.10. Names, assertions and
//! helpers are PHPUnit's; provider rows are upstream's, written as
//! literals.
//!
//! Upstream: symfony/config Tests/Definition/Builder/ExprBuilderTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_class, expect_exception};
use php_value::{PhpArray, Value, php_array, vectors::phpunit_equals};
use symfony_config::{
    Class, Error,
    definition::builder::{
        ArrayNodeDefinition, ExprBuilder, NodeBuilder, TreeBuilder, VariableNodeDefinition,
    },
};

type Key = VariableNodeDefinition<NodeBuilder<ArrayNodeDefinition<TreeBuilder>>>;

/// Create a test treebuilder with a variable node, and init the validation.
fn get_test_builder() -> Result<ExprBuilder<Key>, Error> {
    let builder = TreeBuilder::new("test")?;
    Ok(builder
        .get_root_node()
        .children()
        .variable_node("key")?
        .validate())
}

/// Close the validation process and finalize with the given config
/// (`['key' => 'value']` when none is given).
fn finalize_test_builder(node_definition: Key, config: Option<Value>) -> Result<Value, Error> {
    node_definition
        .end()
        .end()
        .end()
        .build_tree()?
        .finalize(config.unwrap_or_else(|| php_array!["key" => "value"].into()))
}

/// A closure that returns the given value.
fn return_closure(val: Value) -> impl Fn(Value) -> Result<Value, Error> + Send + Sync + 'static {
    move |_| Ok(val.clone())
}

#[track_caller]
fn assert_finalized_value_is(value: Value, node_definition: Key, config: Option<Value>) {
    let mut expected = PhpArray::new();
    expected.insert("key", value);
    match finalize_test_builder(node_definition, config) {
        Ok(actual) => assert!(
            phpunit_equals(&Value::Array(expected.clone()), &actual),
            "expected {expected:?}, got {actual:?}"
        ),
        Err(e) => panic!("finalize failed: {e}"),
    }
}

fn key(value: Value) -> Option<Value> {
    let mut config = PhpArray::new();
    config.insert("key", value);
    Some(Value::Array(config))
}

fn new_value() -> Value {
    Value::from("new_value")
}

#[test]
fn testAlwaysExpression() -> TestResult {
    let test = get_test_builder()?
        .always_then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);
    Ok(())
}

#[test]
fn testIfTrueExpression() -> TestResult {
    let test = get_test_builder()?
        .if_true()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, key(Value::Bool(true)));

    let test = get_test_builder()?
        .if_true_with(|_| true)
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);

    let test = get_test_builder()?
        .if_true_with(|_| 1)
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);

    let test = get_test_builder()?
        .if_true_with(|_| false)
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::from("value"), test, None);

    let test = get_test_builder()?
        .if_true_with(|_| 0)
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::from("value"), test, None);
    Ok(())
}

#[test]
fn testIfFalseExpression() -> TestResult {
    let test = get_test_builder()?
        .if_false()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, key(Value::Bool(false)));

    let test = get_test_builder()?
        .if_false_with(|v| v.identical(&Value::from("value")))
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::from("value"), test, None);

    let test = get_test_builder()?
        .if_false_with(|_| 1)
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::from("value"), test, None);

    let test = get_test_builder()?
        .if_false_with(|v| v.identical(&Value::from("other_value")))
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);

    let test = get_test_builder()?
        .if_false_with(|_| 0)
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);
    Ok(())
}

#[test]
fn testIfStringExpression() -> TestResult {
    let test = get_test_builder()?
        .if_string()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);

    let test = get_test_builder()?
        .if_string()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::Int(45), test, key(Value::Int(45)));
    Ok(())
}

#[test]
fn testIfNullExpression() -> TestResult {
    let test = get_test_builder()?
        .if_null()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, key(Value::Null));

    let test = get_test_builder()?
        .if_null()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::from("value"), test, None);
    Ok(())
}

#[test]
fn testIfEmptyExpression() -> TestResult {
    let test = get_test_builder()?
        .if_empty()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, key(Value::Array(PhpArray::new())));

    let test = get_test_builder()?
        .if_empty()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::from("value"), test, None);
    Ok(())
}

#[test]
fn testIfArrayExpression() -> TestResult {
    let test = get_test_builder()?
        .if_array()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, key(Value::Array(PhpArray::new())));

    let test = get_test_builder()?
        .if_array()
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::from("value"), test, None);
    Ok(())
}

#[test]
fn testIfInArrayExpression() -> TestResult {
    let test = get_test_builder()?
        .if_in_array(vec!["foo".into(), "bar".into(), "value".into()])
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);

    let test = get_test_builder()?
        .if_in_array(vec!["foo".into(), "bar".into()])
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(Value::from("value"), test, None);
    Ok(())
}

#[test]
fn testIfNotInArrayExpression() -> TestResult {
    let test = get_test_builder()?
        .if_not_in_array(vec!["foo".into(), "bar".into()])
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);

    let test = get_test_builder()?
        .if_not_in_array(vec!["foo".into(), "bar".into(), "value_from_config".into()])
        .then(return_closure(new_value()))
        .end()?;
    assert_finalized_value_is(new_value(), test, None);
    Ok(())
}

#[test]
fn testThenEmptyArrayExpression() -> TestResult {
    let test = get_test_builder()?.if_string().then_empty_array().end()?;
    assert_finalized_value_is(Value::Array(PhpArray::new()), test, None);
    Ok(())
}

#[allow(clippy::approx_constant, reason = "upstream's provider row is -3.14")]
fn cast_to_array_values() -> Vec<(Value, PhpArray)> {
    vec![
        (Value::from("value"), PhpArray::list([Value::from("value")])),
        (Value::Float(-3.14), PhpArray::list([Value::Float(-3.14)])),
        (Value::Null, PhpArray::list([Value::Null])),
        (
            Value::Array(PhpArray::list([Value::from("value")])),
            PhpArray::list([Value::from("value")]),
        ),
    ]
}

#[test]
fn testCastToArrayExpression() -> TestResult {
    for (config_value, expected_value) in cast_to_array_values() {
        let test = get_test_builder()?.cast_to_array().end()?;
        assert_finalized_value_is(Value::Array(expected_value), test, key(config_value));
    }
    Ok(())
}

#[test]
fn testThenInvalid() -> TestResult {
    let test = get_test_builder()?
        .if_string()
        .then_invalid("Invalid value")
        .end()?;
    expect_class(
        finalize_test_builder(test, None),
        Class::InvalidConfiguration,
    );
    Ok(())
}

#[test]
fn testThenUnsetExpression() -> TestResult {
    let test = get_test_builder()?.if_string().then_unset().end()?;
    assert!(phpunit_equals(
        &Value::Array(PhpArray::new()),
        &finalize_test_builder(test, None)?
    ));
    Ok(())
}

#[test]
fn testEndIfPartNotSpecified() -> TestResult {
    expect_exception(
        get_test_builder()?.end(),
        Class::Runtime,
        "You must specify an if part.",
    );
    Ok(())
}

#[test]
fn testEndThenPartNotSpecified() -> TestResult {
    let builder = get_test_builder()?;
    builder.set_if_part(|_| false);
    expect_exception(
        builder.end(),
        Class::Runtime,
        "You must specify a then part.",
    );
    Ok(())
}
