//! Tests/ParameterBag/ParameterBagTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's. `testSetNumericName`
//! and `testConstructorNumericName` pass the names as PHP converts them
//! to strings (`1001`, `10.0` as "10").
//!
//! Upstream: symfony/dependency-injection Tests/ParameterBag/ParameterBagTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::too_many_lines
)]

use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::{ErrorKind, ParameterBag};
use symfony_deprecation_contracts::collect;

fn bag(parameters: PhpArray) -> ParameterBag {
    ParameterBag::new(&parameters).unwrap()
}

fn s(text: &str) -> Value {
    Value::from(text)
}

fn messages(deprecations: &[symfony_deprecation_contracts::Deprecation]) -> Vec<String> {
    deprecations
        .iter()
        .map(|d| String::from_utf8_lossy(&d.message).into_owned())
        .collect()
}

#[test]
fn testConstructor() {
    let parameters = php_array!["foo" => "foo", "bar" => "bar"];
    let bag = bag(parameters.clone());
    assert_eq!(
        &parameters,
        bag.all(),
        "__construct() takes an array of parameters as its first argument"
    );
}

#[test]
fn testClear() {
    let mut bag = bag(php_array!["foo" => "foo", "bar" => "bar"]);
    bag.deprecate("foo", "symfony/test", "6.3", None).unwrap();
    bag.resolve().unwrap();
    bag.clear().unwrap();
    assert!(bag.all().is_empty(), "->clear() removes all parameters");
    assert!(
        bag.all_deprecated().is_empty(),
        "->clear() removes all deprecated parameters"
    );
    assert!(!bag.is_resolved(), "->clear() resets the resolved state");

    bag.set("base", s("value")).unwrap();
    bag.set("ref", s("%base%")).unwrap();
    bag.set("escaped", s("foo %%bar%% baz")).unwrap();
    bag.resolve().unwrap();
    assert_eq!(
        s("value"),
        bag.get("ref").unwrap(),
        "->resolve() resolves parameters added after clear()"
    );
    assert_eq!(
        s("foo %bar% baz"),
        bag.get("escaped").unwrap(),
        "->resolve() unescapes parameters added after clear() only once"
    );
    assert!(bag.is_resolved());
}

#[test]
fn testRemove() {
    let mut bag = bag(php_array!["foo" => "foo", "bar" => "bar"]);
    bag.remove("foo").unwrap();
    assert_eq!(
        &php_array!["bar" => "bar"],
        bag.all(),
        "->remove() removes a parameter"
    );
}

#[test]
fn testRemoveWithDeprecation() {
    let mut bag = bag(php_array!["foo" => "foo", "bar" => "bar"]);
    bag.deprecate("foo", "symfony/test", "6.3", None).unwrap();
    bag.remove("foo").unwrap();
    assert_eq!(
        &php_array!["bar" => "bar"],
        bag.all(),
        "->remove() removes a parameter"
    );
    assert!(bag.all_deprecated().is_empty());
}

#[test]
fn testGetSet() {
    let mut bag = bag(php_array!["foo" => "bar"]);
    bag.set("bar", s("foo")).unwrap();
    assert_eq!(
        s("foo"),
        bag.get("bar").unwrap(),
        "->set() sets the value of a new parameter"
    );

    bag.set("foo", s("baz")).unwrap();
    assert_eq!(
        s("baz"),
        bag.get("foo").unwrap(),
        "->set() overrides previously set parameter"
    );

    let e = bag.get("baba").unwrap_err();
    assert!(e.is(&ErrorKind::ParameterNotFound));
    assert_eq!(
        "You have requested a non-existent parameter \"baba\".",
        e.message()
    );
}

#[test]
fn testSetNumericName() {
    for name in ["1001", "10"] {
        let mut bag = ParameterBag::default();
        let e = bag.set(name, s("foo")).unwrap_err();
        assert!(e.is(&ErrorKind::InvalidArgument));
        assert_eq!(
            format!("The parameter name \"{name}\" cannot be numeric."),
            e.message()
        );
    }
}

#[test]
fn testConstructorNumericName() {
    // `[10.0 => 'foo']` has the int key 10.
    for (key, name) in [(1001, "1001"), (10, "10")] {
        let e = ParameterBag::new(&php_array![key => "foo"]).unwrap_err();
        assert!(e.is(&ErrorKind::InvalidArgument));
        assert_eq!(
            format!("The parameter name \"{name}\" cannot be numeric."),
            e.message()
        );
    }
}

#[test]
fn testGetThrowParameterNotFoundException() {
    for (parameter_key, exception_message) in [
        (
            "foo1",
            "You have requested a non-existent parameter \"foo1\". Did you mean this: \"foo\"?",
        ),
        (
            "bag",
            "You have requested a non-existent parameter \"bag\". Did you mean one of these: \"bar\", \"baz\"?",
        ),
        ("", "You have requested a non-existent parameter \"\"."),
        (
            "fiz.bar.boo",
            "You have requested a non-existent parameter \"fiz.bar.boo\". You cannot access nested array items, do you want to inject \"fiz\" instead?",
        ),
        (
            ".foo",
            "Parameter \".foo\" not found. It was probably deleted during the compilation of the container. Did you mean this: \"foo\"?",
        ),
    ] {
        let bag = bag(php_array![
            "foo" => "foo",
            "bar" => "bar",
            "baz" => "baz",
            "fiz" => php_array!["bar" => php_array!["boo" => 12]],
        ]);
        let e = bag.get(parameter_key).unwrap_err();
        assert!(e.is(&ErrorKind::ParameterNotFound));
        assert_eq!(exception_message, e.message());
    }
}

#[test]
fn testDeprecate() {
    let mut bag = bag(php_array!["foo" => "bar"]);
    bag.deprecate("foo", "symfony/test", "6.3", None).unwrap();
    let (result, deprecations) = collect(|| bag.get("foo"));
    result.unwrap();
    assert_eq!(
        vec!["Since symfony/test 6.3: The parameter \"foo\" is deprecated."],
        messages(&deprecations)
    );
}

#[test]
fn testDeprecateWithMessage() {
    let mut bag = bag(php_array!["foo" => "bar"]);
    bag.deprecate(
        "foo",
        "symfony/test",
        "6.3",
        Some("The parameter \"%s\" is deprecated, use \"new_foo\" instead."),
    )
    .unwrap();
    let (result, deprecations) = collect(|| bag.get("foo"));
    result.unwrap();
    assert_eq!(
        vec![
            "Since symfony/test 6.3: The parameter \"foo\" is deprecated, use \"new_foo\" instead."
        ],
        messages(&deprecations)
    );
}

#[test]
fn testDeprecationIsTriggeredWhenResolved() {
    let mut bag = bag(php_array!["foo" => "%bar%", "bar" => "baz"]);
    bag.deprecate("bar", "symfony/test", "6.3", None).unwrap();
    let (result, deprecations) = collect(|| bag.resolve());
    result.unwrap();
    assert_eq!(
        vec!["Since symfony/test 6.3: The parameter \"bar\" is deprecated."],
        messages(&deprecations)
    );
}

#[test]
fn testDeprecateThrowsWhenParameterIsUndefined() {
    let mut bag = ParameterBag::default();
    let e = bag
        .deprecate("foo", "symfony/test", "6.3", None)
        .unwrap_err();
    assert!(e.is(&ErrorKind::ParameterNotFound));
    assert_eq!(
        "You have requested a non-existent parameter \"foo\".",
        e.message()
    );
}

#[test]
fn testGetMissingRequiredParameter() {
    let mut bag = ParameterBag::default();
    bag.cannot_be_empty("bar", "Did you forget to configure the \"foo.bar\" option?")
        .unwrap();
    let e = bag.get("bar").unwrap_err();
    assert!(e.is(&ErrorKind::ParameterNotFound));
    assert_eq!(
        "You have requested a non-existent parameter \"bar\". Did you forget to configure the \"foo.bar\" option?",
        e.message()
    );
}

fn assert_empty_value_refused(value: Value) {
    let mut bag = ParameterBag::default();
    bag.set("bar", value).unwrap();
    bag.cannot_be_empty("bar", "Did you forget to configure the \"foo.bar\" option?")
        .unwrap();
    let e = bag.get("bar").unwrap_err();
    assert!(e.is(&ErrorKind::EmptyParameterValue));
    assert_eq!(
        "Did you forget to configure the \"foo.bar\" option?",
        e.message()
    );
}

#[test]
fn testGetNonEmptyParameterThrowsWhenNullValue() {
    assert_empty_value_refused(Value::Null);
}

#[test]
fn testGetNonEmptyParameterThrowsWhenEmptyStringValue() {
    assert_empty_value_refused(s(""));
}

#[test]
fn testGetNonEmptyParameterThrowsWhenEmptyArrayValue() {
    assert_empty_value_refused(Value::from(PhpArray::new()));
}

#[test]
fn testHas() {
    let bag = bag(php_array!["foo" => "bar"]);
    assert!(
        bag.has("foo"),
        "->has() returns true if a parameter is defined"
    );
    assert!(
        !bag.has("bar"),
        "->has() returns false if a parameter is not defined"
    );
}

#[test]
fn testMixedCase() {
    let mut bag = bag(php_array!["foo" => "foo", "bar" => "bar", "BAR" => "baz"]);
    bag.remove("BAR").unwrap();
    assert_eq!(&php_array!["foo" => "foo", "bar" => "bar"], bag.all());

    bag.set("Foo", s("baz1")).unwrap();
    assert_eq!(s("foo"), bag.get("foo").unwrap());
    assert_eq!(s("baz1"), bag.get("Foo").unwrap());
}

#[test]
fn testResolveValue() {
    let bag_ = bag(PhpArray::new());
    assert_eq!(
        s("foo"),
        bag_.resolve_value(&s("foo"), &[]).unwrap(),
        "->resolveValue() returns its argument unmodified if no placeholders are found"
    );

    let bag_ = bag(php_array!["foo" => "bar"]);
    assert_eq!(
        s("I'm a bar"),
        bag_.resolve_value(&s("I'm a %foo%"), &[]).unwrap(),
        "->resolveValue() replaces placeholders by their values"
    );
    assert_eq!(
        Value::from(php_array!["bar" => "bar"]),
        bag_.resolve_value(&Value::from(php_array!["%foo%" => "%foo%"]), &[])
            .unwrap(),
        "->resolveValue() replaces placeholders in keys and values of arrays"
    );
    assert_eq!(
        Value::from(php_array!["bar" => php_array!["bar" => php_array!["bar" => "bar"]]]),
        bag_.resolve_value(
            &Value::from(
                php_array!["%foo%" => php_array!["%foo%" => php_array!["%foo%" => "%foo%"]]]
            ),
            &[]
        )
        .unwrap(),
        "->resolveValue() replaces placeholders in nested arrays"
    );
    assert_eq!(
        s("I'm a %%foo%%"),
        bag_.resolve_value(&s("I'm a %%foo%%"), &[]).unwrap(),
        "->resolveValue() supports % escaping by doubling it"
    );
    assert_eq!(
        s("I'm a bar %%foo bar"),
        bag_.resolve_value(&s("I'm a %foo% %%foo %foo%"), &[])
            .unwrap(),
        "->resolveValue() supports % escaping by doubling it"
    );
    let nested = Value::from(
        php_array!["foo" => php_array!["bar" => php_array!["ding" => "I'm a bar %%foo %%bar"]]],
    );
    assert_eq!(
        nested,
        bag_.resolve_value(&nested, &[]).unwrap(),
        "->resolveValue() supports % escaping by doubling it"
    );

    let bag_ = bag(php_array!["foo" => true]);
    assert_eq!(
        Value::Bool(true),
        bag_.resolve_value(&s("%foo%"), &[]).unwrap(),
        "->resolveValue() replaces arguments that are just a placeholder by their value without casting them to strings"
    );
    let bag_ = bag(php_array!["foo" => Value::Null]);
    assert_eq!(
        Value::Null,
        bag_.resolve_value(&s("%foo%"), &[]).unwrap(),
        "->resolveValue() replaces arguments that are just a placeholder by their value without casting them to strings"
    );

    let bag_ = bag(php_array!["foo" => "bar", "baz" => "%%%foo% %foo%%% %%foo%% %%%foo%%%"]);
    assert_eq!(
        s("%%bar bar%% %%foo%% %%bar%%"),
        bag_.resolve_value(&s("%baz%"), &[]).unwrap(),
        "->resolveValue() replaces params placed besides escaped %"
    );

    let bag_ = bag(php_array!["baz" => "%%s?%%s"]);
    assert_eq!(
        s("%%s?%%s"),
        bag_.resolve_value(&s("%baz%"), &[]).unwrap(),
        "->resolveValue() is not replacing greedily"
    );

    let bag_ = bag(PhpArray::new());
    let e = bag_.resolve_value(&s("%foobar%"), &[]).unwrap_err();
    assert!(e.is(&ErrorKind::ParameterNotFound));
    assert_eq!(
        "You have requested a non-existent parameter \"foobar\".",
        e.message(),
        "->resolveValue() throws a ParameterNotFoundException if a placeholder references a non-existent parameter"
    );

    let e = bag_.resolve_value(&s("foo %foobar% bar"), &[]).unwrap_err();
    assert!(e.is(&ErrorKind::ParameterNotFound));
    assert_eq!(
        "You have requested a non-existent parameter \"foobar\".",
        e.message(),
        "->resolveValue() throws a ParameterNotFoundException if a placeholder references a non-existent parameter"
    );

    let bag_ = bag(php_array!["foo" => "a %bar%", "bar" => PhpArray::new()]);
    let e = bag_.resolve_value(&s("%foo%"), &[]).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "A string value must be composed of strings and/or numbers, but found parameter \"bar\" of type \"array\" inside string value \"a %bar%\".",
        e.message(),
        "->resolveValue() throws a RuntimeException when a parameter embeds another non-string parameter"
    );

    let bag_ = bag(php_array!["foo" => "%bar%", "bar" => "%foobar%", "foobar" => "%foo%"]);
    let e = bag_.resolve_value(&s("%foo%"), &[]).unwrap_err();
    assert!(e.is(&ErrorKind::ParameterCircularReference));
    assert_eq!(
        "Circular reference detected for parameter \"foo\" (\"foo\" > \"bar\" > \"foobar\" > \"foo\").",
        e.message(),
        "->resolveValue() throws a ParameterCircularReferenceException when a parameter has a circular reference"
    );

    let bag_ = bag(php_array!["foo" => "a %bar%", "bar" => "a %foobar%", "foobar" => "a %foo%"]);
    let e = bag_.resolve_value(&s("%foo%"), &[]).unwrap_err();
    assert!(e.is(&ErrorKind::ParameterCircularReference));
    assert_eq!(
        "Circular reference detected for parameter \"foo\" (\"foo\" > \"bar\" > \"foobar\" > \"foo\").",
        e.message(),
        "->resolveValue() throws a ParameterCircularReferenceException when a parameter has a circular reference"
    );

    let bag_ = bag(php_array!["host" => "foo.bar", "port" => 1337]);
    assert_eq!(
        s("foo.bar:1337"),
        bag_.resolve_value(&s("%host%:%port%"), &[]).unwrap()
    );
}

#[test]
fn testResolveIndicatesWhyAParameterIsNeeded() {
    let mut bag_ = bag(php_array!["foo" => "%bar%"]);
    let e = bag_.resolve().unwrap_err();
    assert_eq!(
        "The parameter \"foo\" has a dependency on a non-existent parameter \"bar\".",
        e.message()
    );

    let mut bag_ = bag(php_array!["foo" => "%bar%"]);
    let e = bag_.resolve().unwrap_err();
    assert_eq!(
        "The parameter \"foo\" has a dependency on a non-existent parameter \"bar\".",
        e.message()
    );
}

#[test]
fn testResolveUnescapesValue() {
    let mut bag_ = bag(php_array![
        "foo" => php_array!["bar" => php_array!["ding" => "I'm a bar %%foo %%bar"]],
        "bar" => "I'm a %%foo%%",
    ]);
    bag_.resolve().unwrap();
    assert_eq!(
        s("I'm a %foo%"),
        bag_.get("bar").unwrap(),
        "->resolveValue() supports % escaping by doubling it"
    );
    assert_eq!(
        Value::from(php_array!["bar" => php_array!["ding" => "I'm a bar %foo %bar"]]),
        bag_.get("foo").unwrap(),
        "->resolveValue() supports % escaping by doubling it"
    );
}

#[test]
fn testResolveValueKeepsEscapingOnAResolvedBag() {
    let mut bag_ = bag(php_array!["foo" => "I'm a %%bar%%"]);
    bag_.resolve().unwrap();
    assert_eq!(
        s("I'm a %%bar%%"),
        bag_.resolve_value(&s("%foo%"), &[]).unwrap(),
        "->resolveValue() returns escaped values once the bag is resolved"
    );
    assert_eq!(
        s("ding I'm a %%bar%% dong"),
        bag_.resolve_value(&s("ding %foo% dong"), &[]).unwrap(),
        "->resolveValue() returns escaped values once the bag is resolved"
    );
    assert_eq!(
        s("I'm a %bar%"),
        bag_.unescape_value(&bag_.resolve_value(&s("%foo%"), &[]).unwrap())
    );
}

#[test]
fn testEscapeValue() {
    let mut bag_ = ParameterBag::default();
    let foo = bag_.escape_value(&Value::from(
        php_array!["bar" => php_array!["ding" => "I'm a bar %foo %bar", "zero" => Value::Null]],
    ));
    let bar = bag_.escape_value(&s("I'm a %foo%"));
    bag_.add(&php_array!["foo" => foo, "bar" => bar]).unwrap();
    assert_eq!(
        s("I'm a %%foo%%"),
        bag_.get("bar").unwrap(),
        "->escapeValue() escapes % by doubling it"
    );
    assert_eq!(
        Value::from(
            php_array!["bar" => php_array!["ding" => "I'm a bar %%foo %%bar", "zero" => Value::Null]]
        ),
        bag_.get("foo").unwrap(),
        "->escapeValue() escapes % by doubling it"
    );
}

#[test]
fn testResolveStringWithSpacesReturnsString() {
    for (expected, test, description) in [
        ("bar", "%foo%", "Parameters must be wrapped by %."),
        ("% foo %", "% foo %", "Parameters should not have spaces."),
        (
            "{% set my_template = \"foo\" %}",
            "{% set my_template = \"foo\" %}",
            "Twig-like strings are not parameters.",
        ),
        (
            "50% is less than 100%",
            "50% is less than 100%",
            "Text between % signs is allowed, if there are spaces.",
        ),
    ] {
        let bag_ = bag(php_array!["foo" => "bar"]);
        let result = bag_.resolve_string(&test.into(), &[]);
        assert_eq!(s(expected), result.unwrap(), "{description}");
    }
}
