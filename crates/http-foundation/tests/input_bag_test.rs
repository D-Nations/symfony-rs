//! InputBagTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Names and assertions are
//! PHPUnit's; the providers are ParameterBagTest's (tests/common).
//! testFilterArrayWithCycle and testFilterArrayWithSharedScalarReference
//! build PHP references, which php-value arrays cannot hold (`n/a`).
//!
//! Upstream: symfony/http-foundation Tests/InputBagTest.php
#![allow(non_snake_case)]

mod common;

use common::{
    FOO_ENUM, foo_bar, foo_enum_registry, provide_invalid_filter_array_values,
    provide_valid_filter_array_values,
};
use php_value::{
    PhpArray, PhpString, Value,
    filter::{FILTER_CALLBACK, FILTER_DEFAULT, FILTER_FORCE_ARRAY, FILTER_VALIDATE_INT},
    object::{Closure, GenericObject, StringableObject},
    php_array,
};
use symfony_http_foundation::{exception::Error, input_bag::InputBag};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const BAD_REQUEST: &str = "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException";

fn s(text: &str) -> Value {
    Value::from(text)
}

fn bytes(text: &str) -> PhpString {
    PhpString::from(text)
}

fn expect<T: std::fmt::Debug>(result: Result<T, Error>, class: &str, message: &str) {
    match result {
        Ok(value) => panic!("expected {class}, got {value:?}"),
        Err(error) => {
            assert!(error.is_a(class), "expected {class}, got {error}");
            assert_eq!(error.message().to_string_lossy(), message);
        }
    }
}

fn stringable() -> Value {
    StringableObject::object("Stringable@anonymous", "strval")
}

#[test]
fn testGet() -> TestResult {
    let mut parameters = php_array!["foo" => "bar"];
    parameters.insert("null", Value::Null);
    parameters.insert("int", Value::Int(1));
    parameters.insert("float", Value::Float(1.0));
    parameters.insert("bool", Value::Bool(false));
    parameters.insert("stringable", stringable());
    let bag = InputBag::new(parameters);

    assert_eq!(
        bag.get("foo", Value::Null)?,
        s("bar"),
        "->get() gets the value of a string parameter"
    );
    assert_eq!(
        bag.get("unknown", s("default"))?,
        s("default"),
        "->get() returns second argument as default if a parameter is not defined"
    );
    assert!(
        bag.get("null", s("default"))?.is_null(),
        "->get() returns null if null is set"
    );
    assert_eq!(
        bag.get("int", Value::Null)?,
        Value::Int(1),
        "->get() gets the value of an int parameter"
    );
    assert_eq!(
        bag.get("float", Value::Null)?,
        Value::Float(1.0),
        "->get() gets the value of a float parameter"
    );
    assert_eq!(
        bag.get("stringable", Value::Null)?,
        s("strval"),
        "->get() gets the string value of a \\Stringable parameter"
    );
    assert_eq!(
        bag.get("bool", Value::Null)?,
        Value::Bool(false),
        "->get() gets the value of a bool parameter"
    );
    Ok(())
}

#[test]
fn testGetIntError() {
    let bag = InputBag::new(php_array!["foo" => "bar"]);

    expect(
        bag.get_int("foo", 0),
        BAD_REQUEST,
        "Input value \"foo\" cannot be converted to \"int\".",
    );
}

#[test]
fn testGetBooleanError() {
    let bag = InputBag::new(php_array!["foo" => "bar"]);

    expect(
        bag.get_boolean("foo", false),
        BAD_REQUEST,
        "Input value \"foo\" cannot be converted to \"bool\".",
    );
}

#[test]
fn testGetString() -> TestResult {
    let mut parameters =
        php_array!["integer" => 123, "bool_true" => true, "bool_false" => false, "string" => "abc"];
    parameters.insert("stringable", stringable());
    let bag = InputBag::new(parameters);

    assert_eq!(
        bag.get_string("integer", "")?,
        bytes("123"),
        "->getString() gets a value of parameter as string"
    );
    assert_eq!(
        bag.get_string("string", "")?,
        bytes("abc"),
        "->getString() gets a value of parameter as string"
    );
    assert_eq!(
        bag.get_string("unknown", "")?,
        bytes(""),
        "->getString() returns zero if a parameter is not defined"
    );
    assert_eq!(
        bag.get_string("unknown", "foo")?,
        bytes("foo"),
        "->getString() returns the default if a parameter is not defined"
    );
    assert_eq!(
        bag.get_string("bool_true", "")?,
        bytes("1"),
        "->getString() returns \"1\" if a parameter is true"
    );
    assert_eq!(
        bag.get_string("bool_false", "foo")?,
        bytes(""),
        "->getString() returns an empty empty string if a parameter is false"
    );
    assert_eq!(
        bag.get_string("stringable", "")?,
        bytes("strval"),
        "->getString() gets a value of a stringable parameter as string"
    );
    Ok(())
}

#[test]
fn testGetStringExceptionWithArray() {
    let bag = InputBag::new(php_array!["key" => php_array!["abc"]]);

    expect(
        bag.get_string("key", ""),
        BAD_REQUEST,
        "Input value \"key\" contains a non-scalar value.",
    );
}

#[test]
fn testGetDoesNotUseDeepByDefault() -> TestResult {
    let bag = InputBag::new(php_array!["foo" => php_array!["bar" => "moo"]]);

    assert!(bag.get("foo[bar]", Value::Null)?.is_null());
    Ok(())
}

#[test]
fn testFilterArray() -> TestResult {
    let bag = InputBag::new(php_array!["foo" => php_array!["12", "8"]]);

    let result = bag.filter(
        "foo",
        Value::Null,
        FILTER_VALIDATE_INT,
        &Value::Int(FILTER_FORCE_ARRAY),
    )?;
    assert_eq!(result, Value::Array(php_array![12, 8]));
    Ok(())
}

#[test]
fn testFilterCallback() {
    let bag = InputBag::new(php_array!["foo" => "bar"]);
    expect(
        bag.filter(
            "foo",
            Value::Null,
            FILTER_CALLBACK,
            &Value::Array(php_array!["options" => "strtoupper"]),
        ),
        "InvalidArgumentException",
        "A Closure must be passed to \"Symfony\\Component\\HttpFoundation\\InputBag::filter()\" when FILTER_CALLBACK is used, \"string\" given.",
    );
}

#[test]
fn testFilterClosure() -> TestResult {
    let bag = InputBag::new(php_array!["foo" => "bar"]);
    let mut options = PhpArray::new();
    options.insert("options", Closure::object(common::strtoupper));
    let result = bag.filter("foo", Value::Null, FILTER_CALLBACK, &Value::Array(options))?;

    assert_eq!(result, s("BAR"));
    Ok(())
}

#[test]
fn testFilterArrayWithInvalidValue() {
    for (name, value, filter, options, _expected) in provide_invalid_filter_array_values() {
        let mut parameters = PhpArray::new();
        parameters.insert("foo", value);
        let bag = InputBag::new(parameters);

        let result = bag.filter("foo", Value::Null, filter, &options);
        match result {
            Err(error) => {
                assert!(error.is_a(BAD_REQUEST), "{name}: {error}");
                assert_eq!(
                    error.message().to_string_lossy(),
                    "Input value \"foo\" is invalid and flag \"FILTER_NULL_ON_FAILURE\" was not set.",
                    "{name}"
                );
            }
            Ok(value) => panic!("{name}: expected an exception, got {value:?}"),
        }
    }
}

#[test]
fn testFilterArrayWithNullOnFailure() -> TestResult {
    for (name, value, filter, options, expected) in provide_invalid_filter_array_values() {
        let options = common::with_null_on_failure(options);
        let mut parameters = PhpArray::new();
        parameters.insert("foo", value);
        let bag = InputBag::new(parameters);

        assert_eq!(
            bag.filter("foo", Value::Null, filter, &options)?,
            Value::Array(expected),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn testFilterArrayWithValidValue() -> TestResult {
    for (name, value, filter, options, expected) in provide_valid_filter_array_values() {
        let mut parameters = PhpArray::new();
        parameters.insert("foo", value);
        let bag = InputBag::new(parameters);

        assert_eq!(
            bag.filter("foo", Value::Null, filter, &options)?,
            Value::Array(expected),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn testSetWithNonScalarOrArray() {
    let mut bag = InputBag::new(PhpArray::new());
    let other = Value::Object(std::sync::Arc::new(GenericObject::new(
        "Symfony\\Component\\HttpFoundation\\InputBag",
        PhpArray::new(),
    )));
    expect(
        bag.set("foo", other),
        "InvalidArgumentException",
        "Expected a scalar, or an array as a 2nd argument to \"Symfony\\Component\\HttpFoundation\\InputBag::set()\", \"Symfony\\Component\\HttpFoundation\\InputBag\" given.",
    );
}

#[test]
fn testGettingANonStringValue() {
    let bag = InputBag::new(php_array!["foo" => php_array!["a", "b"]]);
    expect(
        bag.get("foo", Value::Null),
        BAD_REQUEST,
        "Input value \"foo\" contains a non-scalar value.",
    );
}

#[test]
fn testGetWithNonStringDefaultValue() {
    let bag = InputBag::new(php_array!["foo" => "bar"]);
    expect(
        bag.get("foo", Value::Array(php_array!["a", "b"])),
        "InvalidArgumentException",
        "Expected a scalar value as a 2nd argument to \"Symfony\\Component\\HttpFoundation\\InputBag::get()\", \"array\" given.",
    );
}

#[test]
fn testFilterArrayWithoutArrayFlag() {
    let bag = InputBag::new(php_array!["foo" => php_array!["bar", "baz"]]);
    // `filter('foo', \FILTER_VALIDATE_INT)`: the constant is the default.
    expect(
        bag.filter(
            "foo",
            Value::Int(FILTER_VALIDATE_INT),
            FILTER_DEFAULT,
            &Value::Array(PhpArray::new()),
        ),
        BAD_REQUEST,
        "Input value \"foo\" contains an array, but \"FILTER_REQUIRE_ARRAY\" or \"FILTER_FORCE_ARRAY\" flags were not set.",
    );
}

#[test]
fn testAdd() -> TestResult {
    let mut bag = InputBag::new(php_array!["foo" => "bar"]);
    bag.add(php_array!["baz" => "qux"])?;

    assert_eq!(
        bag.get("foo", Value::Null)?,
        s("bar"),
        "->add() does not remove existing parameters"
    );
    assert_eq!(
        bag.get("baz", Value::Null)?,
        s("qux"),
        "->add() adds new parameters"
    );
    Ok(())
}

#[test]
fn testReplace() -> TestResult {
    let mut bag = InputBag::new(php_array!["foo" => "bar"]);
    bag.replace(php_array!["baz" => "qux"])?;

    assert!(
        bag.get("foo", Value::Null)?.is_null(),
        "->replace() removes existing parameters"
    );
    assert_eq!(
        bag.get("baz", Value::Null)?,
        s("qux"),
        "->replace() adds new parameters"
    );
    Ok(())
}

#[test]
fn testGetEnum() -> TestResult {
    let enums = foo_enum_registry();
    let bag = InputBag::new(php_array!["valid-value" => 1]);

    assert_eq!(
        bag.get_enum("valid-value", &enums, FOO_ENUM, None)?
            .map(Value::Object),
        Some(foo_bar())
    );
    Ok(())
}

#[test]
fn testGetEnumThrowsExceptionWithInvalidValue() {
    let enums = foo_enum_registry();
    let bag = InputBag::new(php_array!["invalid-value" => 2]);

    expect(
        bag.get_enum("invalid-value", &enums, FOO_ENUM, None),
        BAD_REQUEST,
        "Parameter \"invalid-value\" cannot be converted to enum: 2 is not a valid backing value for enum Symfony\\Component\\HttpFoundation\\Tests\\Fixtures\\FooEnum.",
    );
}

#[test]
fn testGetAlnumExceptionWithArray() {
    let bag = InputBag::new(php_array!["word" => php_array!["foo_BAR_012"]]);

    expect(
        bag.get_alnum("word", ""),
        BAD_REQUEST,
        "Input value \"word\" contains a non-scalar value.",
    );
}

#[test]
fn testGetAlphaExceptionWithArray() {
    let bag = InputBag::new(php_array!["word" => php_array!["foo_BAR_012"]]);

    expect(
        bag.get_alpha("word", ""),
        BAD_REQUEST,
        "Input value \"word\" contains a non-scalar value.",
    );
}

#[test]
fn testGetDigitsExceptionWithArray() {
    let bag = InputBag::new(php_array!["word" => php_array!["foo_BAR_012"]]);

    expect(
        bag.get_digits("word", ""),
        BAD_REQUEST,
        "Input value \"word\" contains a non-scalar value.",
    );
}
