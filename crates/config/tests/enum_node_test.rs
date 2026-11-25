//! EnumNodeTest.php at symfony/config v8.1.10. Names and assertions are
//! PHPUnit's. The fixtures' enums are registered in a Registry
//! (`common::enums`), as PHP's autoloader would load them. Placeholders are
//! thread-local here, so each test starts from none; the two placeholder
//! tests reset them at the end (upstream leaves them set).
//!
//! Upstream: symfony/config Tests/Definition/EnumNodeTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, case, enums, expect_exception, fixture};
use php_value::{PhpArray, Value, object::StdClass, php_array};
use symfony_config::{
    Class,
    definition::{EnumNode, reset_placeholders, set_placeholder},
};

#[test]
fn testFinalizeValue() -> TestResult {
    let e = enums();
    let node = EnumNode::new(
        "foo",
        None,
        vec!["foo".into(), "bar".into(), case(&e, "TestEnum::Bar")],
    )?;
    assert_eq!(node.finalize("foo".into())?, Value::from("foo"));
    assert_eq!(
        node.finalize(case(&e, "TestEnum::Bar"))?,
        case(&e, "TestEnum::Bar")
    );
    Ok(())
}

#[test]
fn testConstructionWithNoValues() {
    expect_exception(
        EnumNode::new("foo", None, vec![]),
        Class::InvalidArgument,
        "$values must contain at least one element.",
    );
}

#[test]
fn testConstructionWithBothValuesAndEnumFqcn() {
    let e = enums();
    expect_exception(
        EnumNode::with_options(
            "foo",
            None,
            vec![Value::Int(1), Value::Int(2)],
            b".",
            Some(&fixture("StringBackedTestEnum")),
            &e,
        ),
        Class::InvalidArgument,
        "$values or $enumFqcn cannot be both set.",
    );
}

#[test]
fn testConstructionWithInvlaidEnumFqcn() {
    let e = enums();
    expect_exception(
        EnumNode::of_enum(
            "foo",
            None,
            "Symfony\\Component\\Config\\Tests\\Definition\\InvalidEnum",
            &e,
        ),
        Class::InvalidArgument,
        "The \"Symfony\\Component\\Config\\Tests\\Definition\\InvalidEnum\" enum does not exist.",
    );
}

#[test]
fn testConstructionWithOneValue() -> TestResult {
    let node = EnumNode::new("foo", None, vec!["foo".into()])?;
    assert_eq!(node.finalize("foo".into())?, Value::from("foo"));
    Ok(())
}

#[test]
fn testConstructionWithOneDistinctValue() -> TestResult {
    let node = EnumNode::new("foo", None, vec!["foo".into(), "foo".into()])?;
    assert_eq!(node.finalize("foo".into())?, Value::from("foo"));
    Ok(())
}

#[test]
fn testConstructionWithNullName() -> TestResult {
    let node = EnumNode::new("", None, vec!["foo".into()])?;
    assert_eq!(node.finalize("foo".into())?, Value::from("foo"));
    Ok(())
}

#[test]
fn testFinalizeWithInvalidValue() -> TestResult {
    let e = enums();
    let node = EnumNode::new(
        "foo",
        None,
        vec!["foo".into(), "bar".into(), case(&e, "TestEnum::Foo")],
    )?;
    expect_exception(
        node.finalize("foobar".into()),
        Class::InvalidConfiguration,
        "The value \"foobar\" is not allowed for path \"foo\". Permissible values: \"foo\", \"bar\", Symfony\\Component\\Config\\Tests\\Fixtures\\TestEnum::Foo",
    );
    Ok(())
}

#[test]
fn testFinalizeUnitEnumFqcnWithInvalidValue() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("TestEnum"), &e)?;
    expect_exception(
        node.finalize("foobar".into()),
        Class::InvalidConfiguration,
        "The value \"foobar\" is not allowed for path \"foo\". Permissible values: Symfony\\Component\\Config\\Tests\\Fixtures\\TestEnum::Foo, Symfony\\Component\\Config\\Tests\\Fixtures\\TestEnum::Bar, Symfony\\Component\\Config\\Tests\\Fixtures\\TestEnum::Ccc (cases of the \"Symfony\\Component\\Config\\Tests\\Fixtures\\TestEnum\" enum)",
    );
    Ok(())
}

#[test]
fn testFinalizeWithStringEnumFqcn() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("StringBackedTestEnum"), &e)?;
    assert_eq!(
        node.finalize(case(&e, "StringBackedTestEnum::Foo"))?,
        case(&e, "StringBackedTestEnum::Foo")
    );
    Ok(())
}

#[test]
fn testFinalizeWithIntegerEnumFqcn() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("IntegerBackedTestEnum"), &e)?;
    assert_eq!(
        node.finalize(case(&e, "IntegerBackedTestEnum::One"))?,
        case(&e, "IntegerBackedTestEnum::One")
    );
    Ok(())
}

#[test]
fn testFinalizeWithUnitEnumFqcn() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("TestEnum"), &e)?;
    assert_eq!(
        node.finalize(case(&e, "TestEnum::Foo"))?,
        case(&e, "TestEnum::Foo")
    );
    Ok(())
}

#[test]
fn testFinalizeAnotherEnumWithEnumFqcn() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("StringBackedTestEnum"), &e)?;
    expect_exception(
        node.finalize(case(&e, "StringBackedTestEnum2::Foo")),
        Class::InvalidConfiguration,
        "The value should be part of the \"Symfony\\Component\\Config\\Tests\\Fixtures\\StringBackedTestEnum\" enum, got a value from the \"Symfony\\Component\\Config\\Tests\\Fixtures\\StringBackedTestEnum2\" enum.",
    );
    Ok(())
}

#[test]
fn testFinalizeWithEnumFqcnWorksWithPlainString() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("StringBackedTestEnum"), &e)?;
    assert_eq!(
        node.finalize("foo".into())?,
        case(&e, "StringBackedTestEnum::Foo")
    );
    Ok(())
}

#[test]
fn testFinalizeWithEnumFqcnWorksWithInteger() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("IntegerBackedTestEnum"), &e)?;
    assert_eq!(
        node.finalize(Value::Int(1))?,
        case(&e, "IntegerBackedTestEnum::One")
    );
    Ok(())
}

#[test]
fn testFinalizeWithStringEnumFqcnWithWrongCase() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("StringBackedTestEnum"), &e)?;
    expect_exception(
        node.finalize("qux".into()),
        Class::InvalidConfiguration,
        "The value \"qux\" is not allowed for path \"foo\". Permissible values: \"foo\", \"bar baz\" (cases of the \"Symfony\\Component\\Config\\Tests\\Fixtures\\StringBackedTestEnum\" enum)",
    );
    Ok(())
}

#[test]
fn testFinalizeWithStringEnumFqcnWithIntegerCase() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("StringBackedTestEnum"), &e)?;
    expect_exception(
        node.finalize(Value::Int(1)),
        Class::InvalidConfiguration,
        "The value 1 is not allowed for path \"foo\". Permissible values: \"foo\", \"bar baz\" (cases of the \"Symfony\\Component\\Config\\Tests\\Fixtures\\StringBackedTestEnum\" enum).",
    );
    Ok(())
}

#[test]
fn testFinalizeWithIntegerEnumFqcnWithWrongCase() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("IntegerBackedTestEnum"), &e)?;
    expect_exception(
        node.finalize(Value::Int(3)),
        Class::InvalidConfiguration,
        "The value 3 is not allowed for path \"foo\". Permissible values: 1, 2 (cases of the \"Symfony\\Component\\Config\\Tests\\Fixtures\\IntegerBackedTestEnum\" enum).",
    );
    Ok(())
}

#[test]
fn testFinalizeWithIntegerEnumFqcnWithStringCase() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("IntegerBackedTestEnum"), &e)?;
    expect_exception(
        node.finalize("my string".into()),
        Class::InvalidConfiguration,
        "The value \"my string\" is not allowed for path \"foo\". Permissible values: 1, 2 (cases of the \"Symfony\\Component\\Config\\Tests\\Fixtures\\IntegerBackedTestEnum\" enum).",
    );
    Ok(())
}

#[test]
fn testFinalizeWithEnumFqcnWithWrongType() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("foo", None, &fixture("StringBackedTestEnum"), &e)?;
    expect_exception(
        node.finalize(Value::Bool(true)),
        Class::InvalidConfiguration,
        "Only strings and integers can be cast to a case of the \"Symfony\\Component\\Config\\Tests\\Fixtures\\StringBackedTestEnum\" enum, got value of type \"bool\".",
    );
    Ok(())
}

#[test]
fn testWithPlaceHolderWithValidValue() -> TestResult {
    let node = EnumNode::new(
        "cookie_samesite",
        None,
        vec!["lax".into(), "strict".into(), "none".into()],
    )?;
    set_placeholder(b"custom", php_array!["string" => "lax"])?;
    let result = node.finalize("custom".into());
    reset_placeholders();
    assert_eq!(result?, Value::from("custom"));
    Ok(())
}

#[test]
fn testWithPlaceHolderWithInvalidValue() -> TestResult {
    let node = EnumNode::new(
        "cookie_samesite",
        None,
        vec!["lax".into(), "strict".into(), "none".into()],
    )?;
    set_placeholder(b"custom", php_array!["string" => "foo"])?;
    let result = node.finalize("custom".into());
    reset_placeholders();
    expect_exception(
        result,
        Class::InvalidConfiguration,
        "The value \"foo\" is not allowed for path \"cookie_samesite\". Permissible values: \"lax\", \"strict\", \"none\"",
    );
    Ok(())
}

#[test]
fn testSameStringCoercedValuesAreDifferent() -> TestResult {
    let node = EnumNode::new(
        "ccc",
        None,
        vec!["".into(), Value::Bool(false), Value::Null],
    )?;
    assert_eq!(node.finalize("".into())?, Value::from(""));
    assert_eq!(node.finalize(Value::Bool(false))?, Value::Bool(false));
    assert!(node.finalize(Value::Null)?.is_null());
    Ok(())
}

#[test]
fn testNonScalarOrEnumOrNullValueThrows() {
    expect_exception(
        EnumNode::new("ccc", None, vec![StdClass::object(PhpArray::new())]),
        Class::InvalidArgument,
        "\"Symfony\\Component\\Config\\Definition\\EnumNode\" only supports scalar, enum, or null values, \"stdClass\" given.",
    );
}

#[test]
fn testTwoDifferentEnumsThrows() {
    let e = enums();
    let mut values: Vec<Value> = ["Foo", "Bar", "Ccc"]
        .iter()
        .map(|c| case(&e, &format!("TestEnum::{c}")))
        .collect();
    values.push(case(&e, "TestEnum2::Ccc"));
    expect_exception(
        EnumNode::new("ccc", None, values),
        Class::InvalidArgument,
        "\"Symfony\\Component\\Config\\Definition\\EnumNode\" only supports one type of enum, \"Symfony\\Component\\Config\\Tests\\Fixtures\\TestEnum\" and \"Symfony\\Component\\Config\\Tests\\Fixtures\\TestEnum2\" passed.",
    );
}

#[test]
fn testFinalizeNullableBackedEnum() -> TestResult {
    let e = enums();
    let node = EnumNode::of_enum("status", None, &fixture("StringBackedTestEnum"), &e)?;
    node.set_allow_empty_value(true)?; // Equivalent to ->defaultNull() in the builder
    assert!(node.finalize(Value::Null)?.is_null());
    Ok(())
}
