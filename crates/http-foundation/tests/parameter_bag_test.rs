//! ParameterBagTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Names and assertions are
//! PHPUnit's; a data provider's cases run in a loop, each named.
//!
//! Closures passed with `FILTER_CALLBACK` are php-value `Closure`s; a
//! Stringable test class is a `StringableObject`. `FooEnum` is registered in
//! a `Registry`, as PHP's class table holds it. testFilterArrayWithCycle
//! and testFilterArrayWithSharedScalarReference build PHP references
//! (`&$value`), which php-value arrays cannot hold (`n/a` in the manifest).
//!
//! Upstream: symfony/http-foundation Tests/ParameterBagTest.php
#![allow(non_snake_case)]

mod common;

use common::{
    FOO_ENUM, foo_bar, foo_enum_registry, provide_invalid_filter_array_values,
    provide_valid_filter_array_values,
};
use php_value::{
    Key, PhpArray, PhpString, Value,
    filter::{
        FILTER_CALLBACK, FILTER_DEFAULT, FILTER_FLAG_ALLOW_HEX, FILTER_FLAG_PATH_REQUIRED,
        FILTER_NULL_ON_FAILURE, FILTER_SANITIZE_NUMBER_INT, FILTER_VALIDATE_EMAIL,
        FILTER_VALIDATE_INT, FILTER_VALIDATE_URL,
    },
    object::{Closure, GenericObject, StringableObject},
    php_array,
};
use symfony_http_foundation::{
    exception::{Class, Error},
    parameter_bag::ParameterBag,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const BAD_REQUEST: &str = "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException";
const UNEXPECTED_VALUE: &str =
    "Symfony\\Component\\HttpFoundation\\Exception\\UnexpectedValueException";

fn s(text: &str) -> Value {
    Value::from(text)
}

fn bytes(text: &str) -> PhpString {
    PhpString::from(text)
}

/// `expectException($class)` and `expectExceptionMessage($message)`.
fn expect<T: std::fmt::Debug>(result: Result<T, Error>, class: &str, message: Option<&str>) {
    match result {
        Ok(value) => panic!("expected {class}, got {value:?}"),
        Err(error) => {
            assert!(error.is_a(class), "expected {class}, got {error}");
            if let Some(message) = message {
                assert_eq!(error.message().to_string_lossy(), message);
            }
        }
    }
}

#[test]
fn testConstructor() {
    testAll();
}

#[test]
fn testAll() {
    let bag = ParameterBag::new(php_array!["foo" => "bar"]);
    assert_eq!(
        bag.all(),
        &php_array!["foo" => "bar"],
        "->all() gets all the input"
    );
}

#[test]
fn testAllWithInputKey() -> TestResult {
    let mut parameters = php_array!["foo" => php_array!["bar", "baz"]];
    parameters.insert("null", Value::Null);
    let bag = ParameterBag::new(parameters);

    assert_eq!(
        bag.all_of("foo")?,
        php_array!["bar", "baz"],
        "->all() gets the value of a parameter"
    );
    assert_eq!(
        bag.all_of("unknown")?,
        PhpArray::new(),
        "->all() returns an empty array if a parameter is not defined"
    );
    Ok(())
}

#[test]
fn testAllThrowsForNonArrayValues() {
    let mut parameters = php_array!["foo" => "bar"];
    parameters.insert("null", Value::Null);
    let bag = ParameterBag::new(parameters);
    expect(bag.all_of("foo"), BAD_REQUEST, None);
}

#[test]
fn testKeys() {
    let bag = ParameterBag::new(php_array!["foo" => "bar"]);
    assert_eq!(bag.keys(), vec![Key::from("foo")]);
}

#[test]
fn testAdd() {
    let mut bag = ParameterBag::new(php_array!["foo" => "bar"]);
    bag.add(php_array!["bar" => "bas"]);
    assert_eq!(bag.all(), &php_array!["foo" => "bar", "bar" => "bas"]);
}

#[test]
fn testRemove() {
    let mut bag = ParameterBag::new(php_array!["foo" => "bar"]);
    bag.add(php_array!["bar" => "bas"]);
    assert_eq!(bag.all(), &php_array!["foo" => "bar", "bar" => "bas"]);
    bag.remove("bar");
    assert_eq!(bag.all(), &php_array!["foo" => "bar"]);
}

#[test]
fn testReplace() {
    let mut bag = ParameterBag::new(php_array!["foo" => "bar"]);

    bag.replace(php_array!["FOO" => "BAR"]);
    assert_eq!(
        bag.all(),
        &php_array!["FOO" => "BAR"],
        "->replace() replaces the input with the argument"
    );
    assert!(
        !bag.has("foo"),
        "->replace() overrides previously set the input"
    );
}

#[test]
fn testGet() {
    let mut parameters = php_array!["foo" => "bar"];
    parameters.insert("null", Value::Null);
    let bag = ParameterBag::new(parameters);

    assert_eq!(
        bag.get("foo", Value::Null),
        s("bar"),
        "->get() gets the value of a parameter"
    );
    assert_eq!(
        bag.get("unknown", s("default")),
        s("default"),
        "->get() returns second argument as default if a parameter is not defined"
    );
    assert!(
        bag.get("null", s("default")).is_null(),
        "->get() returns null if null is set"
    );
}

#[test]
fn testGetDoesNotUseDeepByDefault() {
    let bag = ParameterBag::new(php_array!["foo" => php_array!["bar" => "moo"]]);

    assert!(bag.get("foo[bar]", Value::Null).is_null());
}

#[test]
fn testSet() {
    let mut bag = ParameterBag::new(PhpArray::new());

    bag.set("foo", s("bar"));
    assert_eq!(
        bag.get("foo", Value::Null),
        s("bar"),
        "->set() sets the value of parameter"
    );

    bag.set("foo", s("baz"));
    assert_eq!(
        bag.get("foo", Value::Null),
        s("baz"),
        "->set() overrides previously set parameter"
    );
}

#[test]
fn testHas() {
    let bag = ParameterBag::new(php_array!["foo" => "bar"]);

    assert!(
        bag.has("foo"),
        "->has() returns true if a parameter is defined"
    );
    assert!(
        !bag.has("unknown"),
        "->has() return false if a parameter is not defined"
    );
}

#[test]
fn testGetAlpha() -> TestResult {
    let bag =
        ParameterBag::new(php_array!["word" => "foo_BAR_012", "bool" => true, "integer" => 123]);

    assert_eq!(
        bag.get_alpha("word", "")?,
        bytes("fooBAR"),
        "->getAlpha() gets only alphabetic characters"
    );
    assert_eq!(
        bag.get_alpha("unknown", "")?,
        bytes(""),
        "->getAlpha() returns empty string if a parameter is not defined"
    );
    assert_eq!(
        bag.get_alpha("unknown", "abc_DEF_012")?,
        bytes("abcDEF"),
        "->getAlpha() returns filtered default if a parameter is not defined"
    );
    assert_eq!(
        bag.get_alpha("integer", "abc_DEF_012")?,
        bytes(""),
        "->getAlpha() returns empty string if a parameter is an integer"
    );
    assert_eq!(
        bag.get_alpha("bool", "abc_DEF_012")?,
        bytes(""),
        "->getAlpha() returns empty string if a parameter is a boolean"
    );
    Ok(())
}

#[test]
fn testGetAlphaExceptionWithArray() {
    let bag = ParameterBag::new(php_array!["word" => php_array!["foo_BAR_012"]]);

    expect(
        bag.get_alpha("word", ""),
        UNEXPECTED_VALUE,
        Some("Parameter value \"word\" cannot be converted to \"string\"."),
    );
}

#[test]
fn testGetAlnum() -> TestResult {
    let bag =
        ParameterBag::new(php_array!["word" => "foo_BAR_012", "bool" => true, "integer" => 123]);

    assert_eq!(
        bag.get_alnum("word", "")?,
        bytes("fooBAR012"),
        "->getAlnum() gets only alphanumeric characters"
    );
    assert_eq!(
        bag.get_alnum("unknown", "")?,
        bytes(""),
        "->getAlnum() returns empty string if a parameter is not defined"
    );
    assert_eq!(
        bag.get_alnum("unknown", "abc_DEF_012")?,
        bytes("abcDEF012"),
        "->getAlnum() returns filtered default if a parameter is not defined"
    );
    assert_eq!(
        bag.get_alnum("integer", "abc_DEF_012")?,
        bytes("123"),
        "->getAlnum() returns the number as string if a parameter is an integer"
    );
    assert_eq!(
        bag.get_alnum("bool", "abc_DEF_012")?,
        bytes("1"),
        "->getAlnum() returns 1 if a parameter is true"
    );
    Ok(())
}

#[test]
fn testGetAlnumExceptionWithArray() {
    let bag = ParameterBag::new(php_array!["word" => php_array!["foo_BAR_012"]]);

    expect(
        bag.get_alnum("word", ""),
        UNEXPECTED_VALUE,
        Some("Parameter value \"word\" cannot be converted to \"string\"."),
    );
}

#[test]
fn testGetDigits() -> TestResult {
    let bag =
        ParameterBag::new(php_array!["word" => "foo_BAR_0+1-2", "bool" => true, "integer" => 123]);

    assert_eq!(
        bag.get_digits("word", "")?,
        bytes("012"),
        "->getDigits() gets only digits as string"
    );
    assert_eq!(
        bag.get_digits("unknown", "")?,
        bytes(""),
        "->getDigits() returns empty string if a parameter is not defined"
    );
    assert_eq!(
        bag.get_digits("unknown", "abc_DEF_012")?,
        bytes("012"),
        "->getDigits() returns filtered default if a parameter is not defined"
    );
    assert_eq!(
        bag.get_digits("integer", "abc_DEF_012")?,
        bytes("123"),
        "->getDigits() returns the number as string if a parameter is an integer"
    );
    assert_eq!(
        bag.get_digits("bool", "abc_DEF_012")?,
        bytes("1"),
        "->getDigits() returns 1 if a parameter is true"
    );
    Ok(())
}

#[test]
fn testGetDigitsExceptionWithArray() {
    let bag = ParameterBag::new(php_array!["word" => php_array!["foo_BAR_012"]]);

    expect(
        bag.get_digits("word", ""),
        UNEXPECTED_VALUE,
        Some("Parameter value \"word\" cannot be converted to \"string\"."),
    );
}

#[test]
fn testGetInt() -> TestResult {
    let bag = ParameterBag::new(php_array!["digits" => "123", "bool" => true]);

    assert_eq!(
        bag.get_int("digits", 0)?,
        123,
        "->getInt() gets a value of parameter as integer"
    );
    assert_eq!(
        bag.get_int("unknown", 0)?,
        0,
        "->getInt() returns zero if a parameter is not defined"
    );
    assert_eq!(
        bag.get_int("unknown", 10)?,
        10,
        "->getInt() returns the default if a parameter is not defined"
    );
    assert_eq!(
        bag.get_int("bool", 0)?,
        1,
        "->getInt() returns 1 if a parameter is true"
    );
    Ok(())
}

#[test]
fn testGetIntExceptionWithArray() {
    let bag = ParameterBag::new(php_array!["digits" => php_array!["123"]]);

    expect(
        bag.get_int("digits", 0),
        UNEXPECTED_VALUE,
        Some("Parameter value \"digits\" cannot be converted to \"int\"."),
    );
}

#[test]
fn testGetIntExceptionWithInvalid() {
    let bag = ParameterBag::new(php_array!["word" => "foo_BAR_012"]);

    expect(
        bag.get_int("word", 0),
        UNEXPECTED_VALUE,
        Some("Parameter value \"word\" cannot be converted to \"int\"."),
    );
}

#[test]
fn testGetString() -> TestResult {
    let mut parameters =
        php_array!["integer" => 123, "bool_true" => true, "bool_false" => false, "string" => "abc"];
    parameters.insert(
        "stringable",
        StringableObject::object("class@anonymous", "strval"),
    );
    let bag = ParameterBag::new(parameters);

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
    let bag = ParameterBag::new(php_array!["key" => php_array!["abc"]]);

    expect(
        bag.get_string("key", ""),
        UNEXPECTED_VALUE,
        Some("Parameter value \"key\" cannot be converted to \"string\"."),
    );
}

#[test]
fn testGetStringExceptionWithObject() {
    let mut parameters = PhpArray::new();
    parameters.insert(
        "object",
        Value::Object(std::sync::Arc::new(GenericObject::new(
            "Symfony\\Component\\HttpFoundation\\Tests\\ParameterBagTest",
            PhpArray::new(),
        ))),
    );
    let bag = ParameterBag::new(parameters);

    expect(
        bag.get_string("object", ""),
        UNEXPECTED_VALUE,
        Some("Parameter value \"object\" cannot be converted to \"string\"."),
    );
}

/// `$bag->filter('url', '', \FILTER_VALIDATE_URL, ...)` and
/// `FILTER_VALIDATE_EMAIL` are refused (php-value does not reproduce
/// them), so testFilter checks its other assertions and that these two
/// filters are refused rather than approximated; it stays `pending` in the
/// manifest until they are ported.
#[test]
fn filter_assertions_without_email_and_url() -> TestResult {
    let bag = ParameterBag::new(php_array![
        "digits" => "0123ab",
        "email" => "example@example.com",
        "url" => "http://example.com/foo",
        "dec" => "256",
        "hex" => "0x100",
        "array" => php_array!["bang"],
    ]);

    assert_eq!(
        bag.filter(
            "nokey",
            s(""),
            FILTER_DEFAULT,
            &Value::Array(PhpArray::new())
        )?,
        s(""),
        "->filter() should return empty by default if no key is found"
    );

    assert_eq!(
        bag.filter(
            "digits",
            s(""),
            FILTER_SANITIZE_NUMBER_INT,
            &Value::Array(PhpArray::new())
        )?,
        s("0123"),
        "->filter() gets a value of parameter as integer filtering out invalid characters"
    );

    let refused = bag.filter(
        "email",
        s(""),
        FILTER_VALIDATE_EMAIL,
        &Value::Array(PhpArray::new()),
    );
    assert!(
        matches!(&refused, Err(e) if e.class() == Class::NotPorted),
        "{refused:?}"
    );
    let refused = bag.filter(
        "url",
        s(""),
        FILTER_VALIDATE_URL,
        &Value::Int(FILTER_FLAG_PATH_REQUIRED),
    );
    assert!(
        matches!(&refused, Err(e) if e.class() == Class::NotPorted),
        "{refused:?}"
    );

    let range = Value::Array(php_array![
        "flags" => FILTER_FLAG_ALLOW_HEX | FILTER_NULL_ON_FAILURE,
        "options" => php_array!["min_range" => 1, "max_range" => 0xFF],
    ]);
    assert!(
        bag.filter("dec", s(""), FILTER_VALIDATE_INT, &range)?
            .is_null(),
        "->filter() gets a value of parameter as integer between boundaries"
    );

    assert!(
        bag.filter("hex", s(""), FILTER_VALIDATE_INT, &range)?
            .is_null(),
        "->filter() gets a value of parameter as integer between boundaries"
    );

    assert_eq!(
        bag.filter(
            "array",
            s(""),
            FILTER_DEFAULT,
            &Value::Array(PhpArray::new())
        )?,
        Value::Array(php_array!["bang"]),
        "->filter() gets a value of parameter as an array"
    );
    Ok(())
}

#[test]
fn testFilterCallback() {
    let bag = ParameterBag::new(php_array!["foo" => "bar"]);
    expect(
        bag.filter(
            "foo",
            Value::Null,
            FILTER_CALLBACK,
            &Value::Array(php_array!["options" => "strtoupper"]),
        ),
        "InvalidArgumentException",
        Some(
            "A Closure must be passed to \"Symfony\\Component\\HttpFoundation\\ParameterBag::filter()\" when FILTER_CALLBACK is used, \"string\" given.",
        ),
    );
}

#[test]
fn testFilterClosure() -> TestResult {
    let bag = ParameterBag::new(php_array!["foo" => "bar"]);
    let mut options = PhpArray::new();
    options.insert("options", Closure::object(common::strtoupper));
    let result = bag.filter("foo", Value::Null, FILTER_CALLBACK, &Value::Array(options))?;

    assert_eq!(result, s("BAR"));
    Ok(())
}

#[test]
fn testFilterArrayWithInvalidValue() -> TestResult {
    for (name, value, filter, options, _expected) in provide_invalid_filter_array_values() {
        let mut parameters = PhpArray::new();
        parameters.insert("foo", value);
        let bag = ParameterBag::new(parameters);

        let result = bag.filter("foo", Value::Null, filter, &options);
        match result {
            Err(error) => {
                assert!(error.is_a("UnexpectedValueException"), "{name}: {error}");
                assert_eq!(
                    error.message().to_string_lossy(),
                    "Parameter value \"foo\" is invalid and flag \"FILTER_NULL_ON_FAILURE\" was not set.",
                    "{name}"
                );
            }
            Ok(value) => panic!("{name}: expected an exception, got {value:?}"),
        }
    }
    Ok(())
}

#[test]
fn testFilterArrayWithNullOnFailure() -> TestResult {
    for (name, value, filter, options, expected) in provide_invalid_filter_array_values() {
        let options = common::with_null_on_failure(options);
        let mut parameters = PhpArray::new();
        parameters.insert("foo", value);
        let bag = ParameterBag::new(parameters);

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
        let bag = ParameterBag::new(parameters);

        assert_eq!(
            bag.filter("foo", Value::Null, filter, &options)?,
            Value::Array(expected),
            "{name}"
        );
    }
    Ok(())
}

#[test]
fn testFilterArrayWithInvalidValueWithoutFlags() {
    let bag = ParameterBag::new(php_array!["foo" => php_array!["12", "bad"]]);

    expect(
        bag.filter(
            "foo",
            Value::Null,
            FILTER_VALIDATE_INT,
            &Value::Array(PhpArray::new()),
        ),
        "UnexpectedValueException",
        Some("Parameter value \"foo\" is invalid and flag \"FILTER_NULL_ON_FAILURE\" was not set."),
    );
}

#[test]
fn testGetIterator() {
    let parameters = php_array!["foo" => "bar", "hello" => "world"];
    let bag = ParameterBag::new(parameters.clone());

    let mut i = 0;
    for (key, val) in bag.iter() {
        i += 1;
        assert_eq!(parameters.get(key), Some(val));
    }

    assert_eq!(parameters.len(), i);
}

#[test]
fn testCount() {
    let parameters = php_array!["foo" => "bar", "hello" => "world"];
    let bag = ParameterBag::new(parameters.clone());

    assert_eq!(bag.count(), parameters.len());
}

#[test]
fn testGetBoolean() -> TestResult {
    let parameters =
        php_array!["string_true" => "true", "string_false" => "false", "string" => "abc"];
    let bag = ParameterBag::new(parameters);

    assert!(
        bag.get_boolean("string_true", false)?,
        "->getBoolean() gets the string true as boolean true"
    );
    assert!(
        !bag.get_boolean("string_false", false)?,
        "->getBoolean() gets the string false as boolean false"
    );
    assert!(
        !bag.get_boolean("unknown", false)?,
        "->getBoolean() returns false if a parameter is not defined"
    );
    assert!(
        bag.get_boolean("unknown", true)?,
        "->getBoolean() returns default if a parameter is not defined"
    );
    Ok(())
}

#[test]
fn testGetBooleanExceptionWithInvalid() {
    let bag = ParameterBag::new(php_array!["invalid" => "foo"]);

    expect(
        bag.get_boolean("invalid", false),
        UNEXPECTED_VALUE,
        Some("Parameter value \"invalid\" cannot be converted to \"bool\"."),
    );
}

#[test]
fn testGetEnum() -> TestResult {
    let enums = foo_enum_registry();
    let bag = ParameterBag::new(php_array!["valid-value" => 1]);

    assert_eq!(
        bag.get_enum("valid-value", &enums, FOO_ENUM, None)?
            .map(Value::Object),
        Some(foo_bar())
    );

    assert!(
        bag.get_enum("invalid-key", &enums, FOO_ENUM, None)?
            .is_none()
    );
    let bar = foo_bar().as_object().cloned();
    assert_eq!(
        bag.get_enum("invalid-key", &enums, FOO_ENUM, bar)?
            .map(Value::Object),
        Some(foo_bar())
    );
    Ok(())
}

#[test]
fn testGetEnumReturnsDefaultWhenStoredValueIsNull() -> TestResult {
    let enums = foo_enum_registry();
    let mut parameters = PhpArray::new();
    parameters.insert("stored-null", Value::Null);
    let bag = ParameterBag::new(parameters);

    assert!(
        bag.get_enum("stored-null", &enums, FOO_ENUM, None)?
            .is_none()
    );
    let bar = foo_bar().as_object().cloned();
    assert_eq!(
        bag.get_enum("stored-null", &enums, FOO_ENUM, bar)?
            .map(Value::Object),
        Some(foo_bar())
    );
    Ok(())
}

#[test]
fn testGetEnumThrowsExceptionWithNotBackingValue() {
    let enums = foo_enum_registry();
    let bag = ParameterBag::new(php_array!["invalid-value" => 2]);

    expect(
        bag.get_enum("invalid-value", &enums, FOO_ENUM, None),
        "UnexpectedValueException",
        Some(
            "Parameter \"invalid-value\" cannot be converted to enum: 2 is not a valid backing value for enum Symfony\\Component\\HttpFoundation\\Tests\\Fixtures\\FooEnum.",
        ),
    );
}

#[test]
fn testGetEnumThrowsExceptionWithInvalidValueType() {
    let enums = foo_enum_registry();
    let bag = ParameterBag::new(php_array!["invalid-value" => php_array!["foo"]]);

    expect(
        bag.get_enum("invalid-value", &enums, FOO_ENUM, None),
        UNEXPECTED_VALUE,
        Some(
            "Parameter \"invalid-value\" cannot be converted to enum: Symfony\\Component\\HttpFoundation\\Tests\\Fixtures\\FooEnum::from(): Argument #1 ($value) must be of type int, array given.",
        ),
    );
}
