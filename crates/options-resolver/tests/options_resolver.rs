//! OptionsResolverTest.php at symfony/options-resolver
//! 88f9c561f678a02d54b897014049fa839e33ff82. Names and assertions are
//! PHPUnit's. The data providers are replayed from
//! `tests/data/options_resolver_test.json` (`scripts/record.sh
//! options-resolver`), except provideDeprecationData, whose rows are
//! closures and are written out here. `Assert::fail()` inside a closure
//! is a panic; `assertEquals` is `phpunit_equals`, `assertSame` is `==`
//! on values (PHP's `===`).
//!
//! Upstream: symfony/options-resolver Tests/OptionsResolverTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI64, Ordering},
};

use php_value::{
    Key, PhpArray, Value,
    datetime::{DateTimeImmutable, Fields, TimeZone},
    object::{GenericObject, StdClass},
    php_array,
    vectors::{decode, phpunit_equals},
};
use symfony_options_resolver::{
    AllowedValue, AllowedValues, Class, DEFAULT_DEPRECATION_MESSAGE, DefaultValue,
    DeprecationMessage, Error, NestedClosure, OptionClosure, OptionsResolver,
    OptionsResolverIntrospector,
    php::{ClosureValue, Resource},
};

type TestResult = Result<(), Error>;

fn expect_error<T: std::fmt::Debug>(result: Result<T, Error>, class: Class, message: Option<&str>) {
    match result {
        Err(e) => {
            assert!(e.is_a(&class), "{e} is not a {}", class.name());
            if let Some(message) = message {
                assert_eq!(e.message(), message);
            }
        }
        Ok(v) => panic!("expected {}, got {v:?}", class.name()),
    }
}

fn expect<T: std::fmt::Debug>(result: Result<T, Error>, class: Class) {
    expect_error(result, class, None);
}

fn expect_message<T: std::fmt::Debug>(result: Result<T, Error>, class: Class, message: &str) {
    expect_error(result, class, Some(message));
}

fn empty() -> PhpArray {
    PhpArray::new()
}

fn arr(a: PhpArray) -> Value {
    Value::Array(a)
}

fn assert_same(expected: PhpArray, actual: PhpArray) {
    assert_eq!(arr(expected), arr(actual));
}

fn assert_equals(expected: PhpArray, actual: PhpArray) {
    assert!(
        phpunit_equals(&arr(expected.clone()), &arr(actual.clone())),
        "{expected:?} != {actual:?}"
    );
}

fn lazy(
    f: impl Fn(&OptionsResolver) -> Result<Value, Error> + Send + Sync + 'static,
) -> DefaultValue {
    DefaultValue::lazy(f)
}

fn closure(
    f: impl Fn(&OptionsResolver, Value) -> Result<Value, Error> + Send + Sync + 'static,
) -> OptionClosure {
    OptionClosure::new(f)
}

fn nested(
    f: impl Fn(&OptionsResolver, &OptionsResolver) -> Result<(), Error> + Send + Sync + 'static,
) -> NestedClosure {
    NestedClosure::new(f)
}

fn allowed(f: impl Fn(&Value) -> Result<bool, Error> + Send + Sync + 'static) -> AllowedValue {
    AllowedValue::closure(f)
}

fn std_class() -> Value {
    StdClass::object(PhpArray::new())
}

fn date_time_immutable_at(timestamp: i64) -> Value {
    Value::Object(Arc::new(DateTimeImmutable::from_fields(
        Fields {
            year: 1970,
            month: 1,
            day: 1,
            second: timestamp,
            ..Fields::default()
        },
        TimeZone::Utc,
    )))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn date_time_immutable() -> Value {
    date_time_immutable_at(now())
}

fn date_time() -> Value {
    Value::Object(Arc::new(GenericObject::new("DateTime", PhpArray::new())))
}

fn timestamp(value: &Value) -> Option<i64> {
    value
        .as_object()
        .and_then(|o| o.downcast_ref::<DateTimeImmutable>())
        .map(DateTimeImmutable::timestamp)
}

// Data providers, recorded from upstream.

struct Row {
    args: Vec<Value>,
}

fn provider(test: &str) -> Vec<Row> {
    let text = include_str!("data/options_resolver_test.json");
    let json: serde_json::Value = serde_json::from_str(text).expect("provider JSON");
    let rows = json[test]["rows"].as_array().expect("provider rows");
    rows.iter()
        .map(|row| Row {
            args: row[1]
                .as_array()
                .expect("arguments")
                .iter()
                .map(|arg| {
                    if arg[0] == "unsupported" && arg[1] == "resource (stream)" {
                        Resource::stream()
                    } else {
                        decode(arg).expect("typed value")
                    }
                })
                .collect(),
        })
        .collect()
}

fn text(value: &Value) -> String {
    value
        .as_string()
        .map(|s| s.to_string_lossy().into_owned())
        .expect("string argument")
}

fn array(value: &Value) -> PhpArray {
    value.as_array().cloned().expect("array argument")
}

#[test]
fn testResolveWithIgnoreUndefined() -> TestResult {
    for row in provider("testResolveWithIgnoreUndefined") {
        let resolver = OptionsResolver::new();
        resolver
            .set_defaults_array(&array(&row.args[0]))?
            .set_ignore_undefined(true);

        assert_same(array(&row.args[2]), resolver.resolve(&array(&row.args[1]))?);
    }
    Ok(())
}

#[test]
fn testResolveFailsIfNonExistingOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("z", "1")?;
    resolver.set_default("a", "2")?;

    expect_message(
        resolver.resolve(&php_array!["foo" => "bar"]),
        Class::UndefinedOptions,
        "The option \"foo\" does not exist. Defined options are: \"a\", \"z\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsIfMultipleNonExistingOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("z", "1")?;
    resolver.set_default("a", "2")?;

    expect_message(
        resolver.resolve(&php_array!["ping" => "pong", "foo" => "bar", "baz" => "bam"]),
        Class::UndefinedOptions,
        "The options \"baz\", \"foo\", \"ping\" do not exist. Defined options are: \"a\", \"z\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.resolve(&empty())?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testSetDefaultReturnsThis() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(std::ptr::eq(
        &raw const resolver,
        resolver.set_default("foo", "bar")?
    ));
    Ok(())
}

#[test]
fn testSetDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("one", "1")?;
    resolver.set_default("two", "20")?;

    assert_equals(
        php_array!["one" => "1", "two" => "20"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testFailIfSetDefaultFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "lazy",
        lazy(|options| {
            options.set_default("default", 42)?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testHasDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.has_default("foo"));
    resolver.set_default("foo", 42)?;
    assert!(resolver.has_default("foo"));
    Ok(())
}

#[test]
fn testHasDefaultWithNullValue() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.has_default("foo"));
    resolver.set_default("foo", Value::Null)?;
    assert!(resolver.has_default("foo"));
    Ok(())
}

#[test]
fn testSetLazyReturnsThis() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(std::ptr::eq(
        &raw const resolver,
        resolver.set_default("foo", lazy(|_| Ok(Value::Null)))?
    ));
    Ok(())
}

#[test]
fn testSetLazyClosure() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", lazy(|_| Ok("lazy".into())))?;

    assert_equals(php_array!["foo" => "lazy"], resolver.resolve(&empty())?);
    Ok(())
}

/// The closure is `function ($options)`, `function ()`, `function
/// (int|string|null $value)` or `function (\Stringable&\JsonSerializable
/// $value)`: not lazy, stored as a value and never called.
fn closure_not_invoked() -> TestResult {
    let resolver = OptionsResolver::new();
    let closure = ClosureValue::value();

    resolver.set_default("foo", closure.clone())?;

    assert_same(php_array!["foo" => closure], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testClosureWithoutTypeHintNotInvoked() -> TestResult {
    closure_not_invoked()
}

#[test]
fn testClosureWithoutParametersNotInvoked() -> TestResult {
    closure_not_invoked()
}

#[test]
fn testClosureWithUnionTypesNotInvoked() -> TestResult {
    closure_not_invoked()
}

#[test]
fn testClosureWithIntersectionTypesNotInvoked() -> TestResult {
    closure_not_invoked()
}

#[test]
fn testAccessPreviousDefaultValue() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_default("foo", "bar")?;

    // defined by subclass
    resolver.set_default(
        "foo",
        DefaultValue::lazy_with_previous(|_, previous| {
            assert!(phpunit_equals(&"bar".into(), &previous));
            Ok("lazy".into())
        }),
    )?;

    assert_equals(php_array!["foo" => "lazy"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testAccessPreviousLazyDefaultValue() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_default("foo", lazy(|_| Ok("bar".into())))?;

    // defined by subclass
    resolver.set_default(
        "foo",
        DefaultValue::lazy_with_previous(|_, previous| {
            assert!(phpunit_equals(&"bar".into(), &previous));
            Ok("lazy".into())
        }),
    )?;

    assert_equals(php_array!["foo" => "lazy"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testPreviousValueIsNotEvaluatedIfNoSecondArgument() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass: `static function () { Assert::fail(...) }`,
    // which is not lazy
    resolver.set_default("foo", ClosureValue::value())?;

    // defined by subclass, no $previousValue argument defined!
    resolver.set_default("foo", lazy(|_| Ok("lazy".into())))?;

    assert_equals(php_array!["foo" => "lazy"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testOverwrittenLazyOptionNotEvaluated() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", lazy(|_| panic!("Should not be called")))?;

    resolver.set_default("foo", "bar")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testInvokeEachLazyOptionOnlyOnce() -> TestResult {
    let resolver = OptionsResolver::new();
    let calls = Arc::new(AtomicI64::new(0));

    let c = calls.clone();
    resolver.set_default(
        "lazy1",
        lazy(move |options| {
            assert_eq!(1, c.fetch_add(1, Ordering::SeqCst) + 1);
            options.offset_get("lazy2")?;
            Ok(Value::Null)
        }),
    )?;

    let c = calls.clone();
    resolver.set_default(
        "lazy2",
        lazy(move |_| {
            assert_eq!(2, c.fetch_add(1, Ordering::SeqCst) + 1);
            Ok(Value::Null)
        }),
    )?;

    resolver.resolve(&empty())?;

    assert_eq!(2, calls.load(Ordering::SeqCst));
    Ok(())
}

#[test]
fn testSetRequiredReturnsThis() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(std::ptr::eq(
        &raw const resolver,
        resolver.set_required("foo")?
    ));
    Ok(())
}

#[test]
fn testFailIfSetRequiredFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.set_required("bar")?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testResolveFailsIfRequiredOptionMissing() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_required("foo")?;

    expect(resolver.resolve(&empty()), Class::MissingOptions);
    Ok(())
}

#[test]
fn testResolveSucceedsIfRequiredOptionSet() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_required("foo")?;
    resolver.set_default("foo", "bar")?;

    assert!(!resolver.resolve(&empty())?.is_empty());
    Ok(())
}

#[test]
fn testResolveSucceedsIfRequiredOptionPassed() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_required("foo")?;

    assert!(!resolver.resolve(&php_array!["foo" => "bar"])?.is_empty());
    Ok(())
}

#[test]
fn testIsRequired() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_required("foo"));
    resolver.set_required("foo")?;
    assert!(resolver.is_required("foo"));
    Ok(())
}

#[test]
fn testRequiredIfSetBefore() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_required("foo"));

    resolver.set_default("foo", "bar")?;
    resolver.set_required("foo")?;

    assert!(resolver.is_required("foo"));
    Ok(())
}

#[test]
fn testStillRequiredAfterSet() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_required("foo"));

    resolver.set_required("foo")?;
    resolver.set_default("foo", "bar")?;

    assert!(resolver.is_required("foo"));
    Ok(())
}

#[test]
fn testIsNotRequiredAfterRemove() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_required("foo"));
    resolver.set_required("foo")?;
    resolver.remove("foo")?;
    assert!(!resolver.is_required("foo"));
    Ok(())
}

#[test]
fn testIsNotRequiredAfterClear() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_required("foo"));
    resolver.set_required("foo")?;
    resolver.clear()?;
    assert!(!resolver.is_required("foo"));
    Ok(())
}

fn keys(names: &[&str]) -> Vec<Key> {
    names.iter().map(|n| Key::from(*n)).collect()
}

#[test]
fn testGetRequiredOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_required(["foo", "bar"])?;
    resolver.set_default("bam", "baz")?;
    resolver.set_default("foo", "boo")?;

    assert_eq!(keys(&["foo", "bar"]), resolver.get_required_options());
    Ok(())
}

#[test]
fn testIsMissingIfNotSet() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_missing("foo"));
    resolver.set_required("foo")?;
    assert!(resolver.is_missing("foo"));
    Ok(())
}

#[test]
fn testIsNotMissingIfSet() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;

    assert!(!resolver.is_missing("foo"));
    resolver.set_required("foo")?;
    assert!(!resolver.is_missing("foo"));
    Ok(())
}

#[test]
fn testIsNotMissingAfterRemove() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_required("foo")?;
    resolver.remove("foo")?;
    assert!(!resolver.is_missing("foo"));
    Ok(())
}

#[test]
fn testIsNotMissingAfterClear() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_required("foo")?;
    resolver.clear()?;
    assert!(!resolver.is_required("foo"));
    Ok(())
}

#[test]
fn testGetMissingOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_required(["foo", "bar"])?;
    resolver.set_default("bam", "baz")?;
    resolver.set_default("foo", "boo")?;

    assert_eq!(keys(&["bar"]), resolver.get_missing_options());
    Ok(())
}

#[test]
fn testFailIfSetDefinedFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.set_defined("bar")?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testDefinedOptionsNotIncludedInResolvedOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;

    assert_same(empty(), resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testDefinedOptionsIncludedIfDefaultSetBefore() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_defined("foo")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testDefinedOptionsIncludedIfDefaultSetAfter() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_default("foo", "bar")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testDefinedOptionsIncludedIfPassedToResolve() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;

    assert_same(
        php_array!["foo" => "bar"],
        resolver.resolve(&php_array!["foo" => "bar"])?,
    );
    Ok(())
}

#[test]
fn testIsDefined() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_defined("foo"));
    resolver.set_defined("foo")?;
    assert!(resolver.is_defined("foo"));
    Ok(())
}

#[test]
fn testLazyOptionsAreDefined() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_defined("foo"));
    resolver.set_default("foo", lazy(|_| Ok(Value::Null)))?;
    assert!(resolver.is_defined("foo"));
    Ok(())
}

#[test]
fn testRequiredOptionsAreDefined() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_defined("foo"));
    resolver.set_required("foo")?;
    assert!(resolver.is_defined("foo"));
    Ok(())
}

#[test]
fn testSetOptionsAreDefined() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_defined("foo"));
    resolver.set_default("foo", "bar")?;
    assert!(resolver.is_defined("foo"));
    Ok(())
}

#[test]
fn testGetDefinedOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined(["foo", "bar"])?;
    resolver.set_default("baz", "bam")?;
    resolver.set_required("boo")?;

    assert_eq!(
        keys(&["foo", "bar", "baz", "boo"]),
        resolver.get_defined_options()
    );
    Ok(())
}

#[test]
fn testRemovedOptionsAreNotDefined() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_defined("foo"));
    resolver.set_defined("foo")?;
    assert!(resolver.is_defined("foo"));
    resolver.remove("foo")?;
    assert!(!resolver.is_defined("foo"));
    Ok(())
}

#[test]
fn testClearedOptionsAreNotDefined() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(!resolver.is_defined("foo"));
    resolver.set_defined("foo")?;
    assert!(resolver.is_defined("foo"));
    resolver.clear()?;
    assert!(!resolver.is_defined("foo"));
    Ok(())
}

#[test]
fn testFailIfSetDeprecatedFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    let result = resolver
        .set_default("bar", "baz")?
        .set_default(
            "foo",
            lazy(|options| {
                options.set_deprecated(
                    "bar",
                    "vendor/package",
                    "1.1",
                    DEFAULT_DEPRECATION_MESSAGE,
                )?;
                Ok(Value::Null)
            }),
        )?
        .resolve(&empty());
    expect(result, Class::Access);
    Ok(())
}

#[test]
fn testSetDeprecatedFailsIfUnknownOption() {
    let resolver = OptionsResolver::new();
    expect(
        resolver.set_deprecated("foo", "vendor/package", "1.1", DEFAULT_DEPRECATION_MESSAGE),
        Class::UndefinedOptions,
    );
}

#[test]
fn testLazyDeprecationFailsIfInvalidDeprecationMessageType() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?.set_deprecated(
        "foo",
        "vendor/package",
        "1.1",
        DeprecationMessage::closure(|_, _| Ok(false.into())),
    )?;
    expect_message(
        resolver.resolve(&php_array!["foo" => Value::Null]),
        Class::InvalidArgument,
        "Invalid type for deprecation message, expected string but got \"bool\", return an empty string to ignore.",
    );
    Ok(())
}

#[test]
fn testFailsIfCyclicDependencyBetweenDeprecation() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .set_defined(["foo", "bar"])?
        .set_deprecated(
            "foo",
            "vendor/package",
            "1.1",
            DeprecationMessage::closure(|options, _| {
                options.offset_get("bar")?;
                Ok(Value::Null)
            }),
        )?
        .set_deprecated(
            "bar",
            "vendor/package",
            "1.1",
            DeprecationMessage::closure(|options, _| {
                options.offset_get("foo")?;
                Ok(Value::Null)
            }),
        )?;
    expect_message(
        resolver.resolve(&php_array!["foo" => Value::Null, "bar" => Value::Null]),
        Class::OptionDefinition,
        "The options \"foo\", \"bar\" have a cyclic dependency.",
    );
    Ok(())
}

#[test]
fn testIsDeprecated() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?.set_deprecated(
        "foo",
        "vendor/package",
        "1.1",
        DEFAULT_DEPRECATION_MESSAGE,
    )?;
    assert!(resolver.is_deprecated("foo"));
    Ok(())
}

#[test]
fn testIsNotDeprecatedIfEmptyString() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .set_defined("foo")?
        .set_deprecated("foo", "vendor/package", "1.1", "")?;
    assert!(!resolver.is_deprecated("foo"));
    Ok(())
}

type Configure = Box<dyn Fn(&OptionsResolver) -> TestResult>;

/// provideDeprecationData: (name, configure, options, expected last
/// error message, expected count).
fn provide_deprecation_data() -> Vec<(
    &'static str,
    Configure,
    PhpArray,
    Option<&'static str>,
    usize,
)> {
    vec![
        (
            "It deprecates an option with default message",
            Box::new(|resolver| {
                resolver.set_defined(["foo", "bar"])?.set_deprecated(
                    "foo",
                    "vendor/package",
                    "1.1",
                    "The option \"%name%\" is deprecated.",
                )?;
                Ok(())
            }),
            php_array!["foo" => "baz"],
            Some("Since vendor/package 1.1: The option \"foo\" is deprecated."),
            1,
        ),
        (
            "It deprecates an option with custom message",
            Box::new(|resolver| {
                resolver
                    .set_defined("foo")?
                    .set_default("bar", lazy(|options| options.offset_get("foo")))?
                    .set_deprecated(
                        "foo",
                        "vendor/package",
                        "1.1",
                        "The option \"foo\" is deprecated, use \"bar\" option instead.",
                    )?;
                Ok(())
            }),
            php_array!["foo" => "baz"],
            Some(
                "Since vendor/package 1.1: The option \"foo\" is deprecated, use \"bar\" option instead.",
            ),
            2,
        ),
        (
            "It deprecates an option evaluated in another definition",
            Box::new(|resolver| {
                // defined by superclass
                resolver.set_default("foo", Value::Null)?.set_deprecated(
                    "foo",
                    "vendor/package",
                    "1.1",
                    "The option \"%name%\" is deprecated.",
                )?;
                // defined by subclass
                resolver.set_default(
                    "bar",
                    lazy(|options| {
                        options.offset_get("foo") // It triggers a deprecation
                    }),
                )?;
                Ok(())
            }),
            empty(),
            Some("Since vendor/package 1.1: The option \"foo\" is deprecated."),
            1,
        ),
        (
            "It deprecates allowed type and value",
            Box::new(|resolver| {
                resolver
                    .set_default("foo", Value::Null)?
                    .set_allowed_types("foo", ["null", "string", StdClass::CLASS])?
                    .set_deprecated(
                        "foo",
                        "vendor/package",
                        "1.1",
                        DeprecationMessage::closure(|_, value| {
                            if value.as_object().is_some_and(|o| o.class() == StdClass::CLASS) {
                                return Ok(format!(
                                    "Passing an instance of \"{}\" to option \"foo\" is deprecated, pass its FQCN instead.",
                                    StdClass::CLASS
                                )
                                .into());
                            }

                            Ok("".into())
                        }),
                    )?;
                Ok(())
            }),
            php_array!["foo" => std_class()],
            Some(
                "Since vendor/package 1.1: Passing an instance of \"stdClass\" to option \"foo\" is deprecated, pass its FQCN instead.",
            ),
            1,
        ),
        (
            "It triggers a deprecation based on the value only if option is provided by the user",
            Box::new(|resolver| {
                resolver
                    .set_defined("foo")?
                    .set_allowed_types("foo", ["null", "bool"])?
                    .set_deprecated(
                        "foo",
                        "vendor/package",
                        "1.1",
                        DeprecationMessage::closure(|_, value| {
                            if !matches!(value, Value::Bool(_)) {
                                return Ok(
                                    "Passing a value different than true or false is deprecated."
                                        .into(),
                                );
                            }

                            Ok("".into())
                        }),
                    )?
                    .set_default("baz", Value::Null)?
                    .set_allowed_types("baz", ["null", "int"])?
                    .set_deprecated(
                        "baz",
                        "vendor/package",
                        "1.1",
                        DeprecationMessage::closure(|_, value| {
                            if !matches!(value, Value::Int(_)) {
                                return Ok("Not passing an integer is deprecated.".into());
                            }

                            Ok("".into())
                        }),
                    )?
                    .set_default(
                        "bar",
                        lazy(|options| {
                            options.offset_get("baz")?; // It does not triggers a deprecation

                            options.offset_get("foo") // It does not triggers a deprecation
                        }),
                    )?;
                Ok(())
            }),
            php_array!["foo" => Value::Null], // It triggers a deprecation
            Some(
                "Since vendor/package 1.1: Passing a value different than true or false is deprecated.",
            ),
            1,
        ),
        (
            "It ignores a deprecation if closure returns an empty string",
            Box::new(|resolver| {
                resolver.set_default("foo", Value::Null)?.set_deprecated(
                    "foo",
                    "vendor/package",
                    "1.1",
                    DeprecationMessage::closure(|_, _| Ok("".into())),
                )?;
                Ok(())
            }),
            php_array!["foo" => "Symfony\\Component\\OptionsResolver\\Tests\\Bar"],
            None,
            0,
        ),
        (
            "It deprecates value depending on other option value",
            Box::new(|resolver| {
                resolver
                    .set_default("widget", Value::Null)?
                    .set_default("date_format", Value::Null)?
                    .set_deprecated(
                        "date_format",
                        "vendor/package",
                        "1.1",
                        DeprecationMessage::closure(|options, date_format| {
                            if !date_format.is_null()
                                && Value::from("single_text") == options.offset_get("widget")?
                            {
                                return Ok("Using the \"date_format\" option when the \"widget\" option is set to \"single_text\" is deprecated.".into());
                            }

                            Ok("".into())
                        }),
                    )?;
                Ok(())
            }),
            php_array!["widget" => "single_text", "date_format" => 2],
            Some(
                "Since vendor/package 1.1: Using the \"date_format\" option when the \"widget\" option is set to \"single_text\" is deprecated.",
            ),
            1,
        ),
        (
            "It triggers a deprecation for each evaluation",
            Box::new(|resolver| {
                resolver
                    // defined by superclass
                    .set_defined("foo")?
                    .set_deprecated(
                        "foo",
                        "vendor/package",
                        "1.1",
                        "The option \"%name%\" is deprecated.",
                    )?
                    // defined by subclass
                    .set_default(
                        "bar",
                        lazy(|options| {
                            options.offset_get("foo") // It triggers a deprecation
                        }),
                    )?
                    .set_normalizer(
                        "bar",
                        closure(|options, value| {
                            options.offset_get("foo")?; // It triggers a deprecation
                            options.offset_get("foo")?; // It triggers a deprecation

                            Ok(value)
                        }),
                    )?;
                Ok(())
            }),
            php_array!["foo" => "baz"], // It triggers a deprecation
            Some("Since vendor/package 1.1: The option \"foo\" is deprecated."),
            4,
        ),
        (
            "It ignores a deprecation if no option is provided by the user",
            Box::new(|resolver| {
                resolver
                    .set_defined("foo")?
                    .set_default("bar", Value::Null)?
                    .set_deprecated(
                        "foo",
                        "vendor/package",
                        "1.1",
                        "The option \"%name%\" is deprecated.",
                    )?
                    .set_deprecated(
                        "bar",
                        "vendor/package",
                        "1.1",
                        "The option \"%name%\" is deprecated.",
                    )?;
                Ok(())
            }),
            empty(),
            None,
            0,
        ),
        (
            "It explicitly ignores a deprecation",
            Box::new(|resolver| {
                resolver
                    .set_default("baz", lazy(|options| options.offset_get_with("foo", false)))?
                    .set_default("foo", Value::Null)?
                    .set_deprecated(
                        "foo",
                        "vendor/package",
                        "1.1",
                        "The option \"%name%\" is deprecated.",
                    )?
                    .set_default("bar", lazy(|options| options.offset_get_with("foo", false)))?;
                Ok(())
            }),
            empty(),
            None,
            0,
        ),
    ]
}

#[test]
fn testDeprecationMessages() -> TestResult {
    for (name, configure, options, expected_error, expected_count) in provide_deprecation_data() {
        let resolver = OptionsResolver::new();
        let (result, deprecations) = symfony_deprecation_contracts::collect(|| {
            configure(&resolver)?;
            resolver.resolve(&options)
        });
        result?;

        let last_error = deprecations
            .last()
            .map(|d| String::from_utf8_lossy(&d.message).into_owned());
        assert_eq!(expected_error.map(str::to_owned), last_error, "{name}");
        assert_eq!(expected_count, deprecations.len(), "{name}");
    }
    Ok(())
}

#[test]
fn testSetAllowedTypesFailsIfUnknownOption() {
    let resolver = OptionsResolver::new();
    expect(
        resolver.set_allowed_types("foo", "string"),
        Class::UndefinedOptions,
    );
}

#[test]
fn testResolveTypedWithUnion() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "string|int")?;

    let options = resolver.resolve(&php_array!["foo" => 1])?;
    assert_same(php_array!["foo" => 1], options);

    let options = resolver.resolve(&php_array!["foo" => "1"])?;
    assert_same(php_array!["foo" => "1"], options);
    Ok(())
}

#[test]
fn testResolveTypedWithUnionAndWhitespaces() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "string | int")?;

    let options = resolver.resolve(&php_array!["foo" => 1])?;
    assert_same(php_array!["foo" => 1], options);

    let options = resolver.resolve(&php_array!["foo" => "1"])?;
    assert_same(php_array!["foo" => "1"], options);
    Ok(())
}

#[test]
fn testResolveTypedWithUnionOfClasse() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "DateTime|DateTimeImmutable")?;

    let datetime = date_time();
    let options = resolver.resolve(&php_array!["foo" => datetime.clone()])?;
    assert_same(php_array!["foo" => datetime], options);

    let datetime = date_time_immutable();
    let options = resolver.resolve(&php_array!["foo" => datetime.clone()])?;
    assert_same(php_array!["foo" => datetime], options);
    Ok(())
}

#[test]
fn testResolveTypedWithUnionOfArray() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "(string|int)[]|(bool|int)[]")?;

    let options = resolver.resolve(&php_array!["foo" => php_array![1, "1"]])?;
    assert_same(php_array!["foo" => php_array![1, "1"]], options);

    let options = resolver.resolve(&php_array!["foo" => php_array![1, true]])?;
    assert_same(php_array!["foo" => php_array![1, true]], options);
    Ok(())
}

#[test]
fn testResolveTypedArray() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "string[]")?;
    let options = resolver.resolve(&php_array!["foo" => php_array!["bar", "baz"]])?;

    assert_same(php_array!["foo" => php_array!["bar", "baz"]], options);
    Ok(())
}

#[test]
fn testResolveTypedArrayWithUnion() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "(string|int)[]")?;
    let options = resolver.resolve(&php_array!["foo" => php_array!["bar", 1]])?;

    assert_same(php_array!["foo" => php_array!["bar", 1]], options);
    Ok(())
}

#[test]
fn testFailIfSetAllowedTypesFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.set_allowed_types("bar", "string")?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default("bar", "baz")?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidTypedArray() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "int[]")?;

    expect_message(
        resolver.resolve(&php_array!["foo" => php_array![date_time_immutable()]]),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"int[]\", but one of the elements is of type \"DateTimeImmutable\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsWithNonArray() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "int[]")?;

    expect_message(
        resolver.resolve(&php_array!["foo" => "bar"]),
        Class::InvalidOptions,
        "The option \"foo\" with value \"bar\" is expected to be of type \"int[]\", but is of type \"string\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsIfTypedArrayContainsInvalidTypes() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "int[]")?;
    let mut values = php_array![1, 2, 3, 4, 5];
    let _ = values.push(std_class());
    let _ = values.push(arr(empty()));
    let _ = values.push(date_time_immutable());
    let _ = values.push(123.into());

    expect_message(
        resolver.resolve(&php_array!["foo" => values]),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"int[]\", but one of the elements is of type \"stdClass|array|DateTimeImmutable\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsWithCorrectLevelsButWrongScalar() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "int[][]")?;

    expect_message(
        resolver.resolve(&php_array!["foo" => php_array![php_array![1.2]]]),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"int[][]\", but one of the elements is of type \"float\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidType() -> TestResult {
    for row in provider("testResolveFailsIfInvalidType") {
        let resolver = OptionsResolver::new();
        resolver.set_defined("option")?;
        let allowed_types: Vec<String> = match &row.args[1] {
            Value::Array(types) => types.values().map(text).collect(),
            other => vec![text(other)],
        };
        resolver.set_allowed_types("option", allowed_types)?;

        expect_message(
            resolver.resolve(&php_array!["option" => row.args[0].clone()]),
            Class::InvalidOptions,
            &text(&row.args[2]),
        );
    }
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidType() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_types("foo", "string")?;

    assert!(!resolver.resolve(&empty())?.is_empty());
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidTypeMultiple() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.set_allowed_types("foo", ["string", "bool"])?;

    expect_message(
        resolver.resolve(&empty()),
        Class::InvalidOptions,
        "The option \"foo\" with value 42 is expected to be of type \"string\" or \"bool\", but is of type \"int\".",
    );
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidTypeMultiple() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", true)?;
    resolver.set_allowed_types("foo", ["string", "bool"])?;

    assert!(!resolver.resolve(&empty())?.is_empty());
    Ok(())
}

#[test]
fn testResolveSucceedsIfInstanceOfClass() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", std_class())?;
    resolver.set_allowed_types("foo", "\\stdClass")?;

    assert!(!resolver.resolve(&empty())?.is_empty());
    Ok(())
}

#[test]
fn testResolveSucceedsIfTypedArray() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", Value::Null)?;
    resolver.set_allowed_types("foo", ["null", "DateTimeImmutable[]"])?;

    let data = php_array![
        "foo" => php_array![date_time_immutable(), date_time_immutable()],
    ];
    let result = resolver.resolve(&data)?;
    assert_equals(data, result);
    Ok(())
}

#[test]
fn testResolveFailsIfNotInstanceOfClass() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_types("foo", "\\stdClass")?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testAddAllowedTypesFailsIfUnknownOption() {
    let resolver = OptionsResolver::new();
    expect(
        resolver.add_allowed_types("foo", "string"),
        Class::UndefinedOptions,
    );
}

#[test]
fn testFailIfAddAllowedTypesFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.add_allowed_types("bar", "string")?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default("bar", "baz")?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidAddedType() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.add_allowed_types("foo", "string")?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidAddedType() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.add_allowed_types("foo", "string")?;

    assert!(!resolver.resolve(&empty())?.is_empty());
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidAddedTypeMultiple() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.add_allowed_types("foo", ["string", "bool"])?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidAddedTypeMultiple() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.add_allowed_types("foo", ["string", "bool"])?;

    assert!(!resolver.resolve(&empty())?.is_empty());
    Ok(())
}

#[test]
fn testAddAllowedTypesDoesNotOverwrite() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_types("foo", "string")?;
    resolver.add_allowed_types("foo", "bool")?;

    resolver.set_default("foo", "bar")?;

    assert!(!resolver.resolve(&empty())?.is_empty());
    Ok(())
}

#[test]
fn testAddAllowedTypesDoesNotOverwrite2() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_types("foo", "string")?;
    resolver.add_allowed_types("foo", "bool")?;

    resolver.set_default("foo", false)?;

    assert!(!resolver.resolve(&empty())?.is_empty());
    Ok(())
}

#[test]
fn testSetAllowedValuesFailsIfUnknownOption() {
    let resolver = OptionsResolver::new();
    expect(
        resolver.set_allowed_values("foo", "bar"),
        Class::UndefinedOptions,
    );
}

#[test]
fn testFailIfSetAllowedValuesFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.set_allowed_values("bar", "baz")?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default("bar", "baz")?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_values("foo", "bar")?;

    expect_message(
        resolver.resolve(&php_array!["foo" => 42]),
        Class::InvalidOptions,
        "The option \"foo\" with value 42 is invalid. Accepted values are: \"bar\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidValueIsNull() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", Value::Null)?;
    resolver.set_allowed_values("foo", "bar")?;

    expect_message(
        resolver.resolve(&empty()),
        Class::InvalidOptions,
        "The option \"foo\" with value null is invalid. Accepted values are: \"bar\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidValueStrict() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.set_allowed_values("foo", "42")?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_values("foo", "bar")?;

    assert_equals(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidValueIsNull() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", Value::Null)?;
    resolver.set_allowed_values("foo", Value::Null)?;

    assert_equals(
        php_array!["foo" => Value::Null],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidValueMultiple() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.set_allowed_values("foo", php_array!["bar", false, Value::Null])?;

    expect_message(
        resolver.resolve(&empty()),
        Class::InvalidOptions,
        "The option \"foo\" with value 42 is invalid. Accepted values are: \"bar\", false, null.",
    );
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidValueMultiple() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "baz")?;
    resolver.set_allowed_values("foo", php_array!["bar", "baz"])?;

    assert_equals(php_array!["foo" => "baz"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testResolveFailsIfClosureReturnsFalse() -> TestResult {
    let resolver = OptionsResolver::new();
    let passed_value = Arc::new(Mutex::new(Value::Null));
    resolver.set_default("foo", 42)?;
    let passed = passed_value.clone();
    resolver.set_allowed_values(
        "foo",
        allowed(move |value| {
            *passed
                .lock()
                .map_err(|_| Error::other("Error", "poisoned"))? = value.clone();

            Ok(false)
        }),
    )?;

    match resolver.resolve(&empty()) {
        Ok(_) => panic!("Should fail"),
        Err(e) => assert!(e.is_a(&Class::InvalidOptions)),
    }

    assert_eq!(
        Value::Int(42),
        passed_value.lock().map(|v| v.clone()).unwrap_or_default()
    );
    Ok(())
}

#[test]
fn testResolveSucceedsIfClosureReturnsTrue() -> TestResult {
    let resolver = OptionsResolver::new();
    let passed_value = Arc::new(Mutex::new(Value::Null));
    resolver.set_default("foo", "bar")?;
    let passed = passed_value.clone();
    resolver.set_allowed_values(
        "foo",
        allowed(move |value| {
            *passed
                .lock()
                .map_err(|_| Error::other("Error", "poisoned"))? = value.clone();

            Ok(true)
        }),
    )?;

    assert_equals(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    assert_eq!(
        Value::from("bar"),
        passed_value.lock().map(|v| v.clone()).unwrap_or_default()
    );
    Ok(())
}

#[test]
fn testResolveFailsIfAllClosuresReturnFalse() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.set_allowed_values(
        "foo",
        [
            allowed(|_| Ok(false)),
            allowed(|_| Ok(false)),
            allowed(|_| Ok(false)),
        ],
    )?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testResolveSucceedsIfAnyClosureReturnsTrue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_values(
        "foo",
        [
            allowed(|_| Ok(false)),
            allowed(|_| Ok(true)),
            allowed(|_| Ok(false)),
        ],
    )?;

    assert_equals(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testAddAllowedValuesFailsIfUnknownOption() {
    let resolver = OptionsResolver::new();
    expect(
        resolver.add_allowed_values("foo", "bar"),
        Class::UndefinedOptions,
    );
}

#[test]
fn testFailIfAddAllowedValuesFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.add_allowed_values("bar", "baz")?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default("bar", "baz")?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidAddedValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.add_allowed_values("foo", "bar")?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidAddedValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.add_allowed_values("foo", "bar")?;

    assert_equals(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidAddedValueIsNull() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", Value::Null)?;
    resolver.add_allowed_values("foo", Value::Null)?;

    assert_equals(
        php_array!["foo" => Value::Null],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidAddedValueMultiple() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.add_allowed_values("foo", php_array!["bar", "baz"])?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testResolveSucceedsIfValidAddedValueMultiple() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.add_allowed_values("foo", php_array!["bar", "baz"])?;

    assert_equals(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testAddAllowedValuesDoesNotOverwrite() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_values("foo", "bar")?;
    resolver.add_allowed_values("foo", "baz")?;

    assert_equals(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testAddAllowedValuesDoesNotOverwrite2() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "baz")?;
    resolver.set_allowed_values("foo", "bar")?;
    resolver.add_allowed_values("foo", "baz")?;

    assert_equals(php_array!["foo" => "baz"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testResolveFailsIfAllAddedClosuresReturnFalse() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 42)?;
    resolver.set_allowed_values("foo", allowed(|_| Ok(false)))?;
    resolver.add_allowed_values("foo", allowed(|_| Ok(false)))?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testResolveSucceedsIfAnyAddedClosureReturnsTrue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_values("foo", allowed(|_| Ok(false)))?;
    resolver.add_allowed_values("foo", allowed(|_| Ok(true)))?;

    assert_equals(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testResolveSucceedsIfAnyAddedClosureReturnsTrue2() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_values("foo", allowed(|_| Ok(true)))?;
    resolver.add_allowed_values("foo", allowed(|_| Ok(false)))?;

    assert_equals(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

fn noop() -> OptionClosure {
    closure(|_, _| Ok(Value::Null))
}

#[test]
fn testSetNormalizerReturnsThis() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    assert!(std::ptr::eq(
        &raw const resolver,
        resolver.set_normalizer("foo", noop())?
    ));
    Ok(())
}

#[test]
fn testSetNormalizerClosure() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_normalizer("foo", closure(|_, _| Ok("normalized".into())))?;

    assert_equals(
        php_array!["foo" => "normalized"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testSetNormalizerFailsIfUnknownOption() {
    let resolver = OptionsResolver::new();
    expect(
        resolver.set_normalizer("foo", noop()),
        Class::UndefinedOptions,
    );
}

#[test]
fn testFailIfSetNormalizerFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.set_normalizer("foo", noop())?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default("bar", "baz")?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

/// `'normalized['.$value.']'`.
fn bracket(prefix: &str, value: &Value, suffix: &str) -> Value {
    let mut out = prefix.as_bytes().to_vec();
    out.extend(value.to_php_string().unwrap_or_default().to_vec());
    out.extend_from_slice(suffix.as_bytes());
    Value::String(out.into())
}

#[test]
fn testNormalizerReceivesSetOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;

    resolver.set_normalizer(
        "foo",
        closure(|_, value| Ok(bracket("normalized[", &value, "]"))),
    )?;

    assert_equals(
        php_array!["foo" => "normalized[bar]"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testNormalizerReceivesPassedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;

    resolver.set_normalizer(
        "foo",
        closure(|_, value| Ok(bracket("normalized[", &value, "]"))),
    )?;

    let resolved = resolver.resolve(&php_array!["foo" => "baz"])?;

    assert_equals(php_array!["foo" => "normalized[baz]"], resolved);
    Ok(())
}

#[test]
fn testValidateTypeBeforeNormalization() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;

    resolver.set_allowed_types("foo", "int")?;

    resolver.set_normalizer("foo", closure(|_, _| panic!("Should not be called.")))?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testValidateValueBeforeNormalization() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;

    resolver.set_allowed_values("foo", "baz")?;

    resolver.set_normalizer("foo", closure(|_, _| panic!("Should not be called.")))?;

    expect(resolver.resolve(&empty()), Class::InvalidOptions);
    Ok(())
}

#[test]
fn testNormalizerCanAccessOtherOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("default", "bar")?;
    resolver.set_default("norm", "baz")?;

    resolver.set_normalizer(
        "norm",
        closure(|options, _| {
            assert_eq!(Value::from("bar"), options.offset_get("default")?);

            Ok("normalized".into())
        }),
    )?;

    assert_equals(
        php_array!["default" => "bar", "norm" => "normalized"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testNormalizerCanAccessLazyOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("lazy", lazy(|_| Ok("bar".into())))?;
    resolver.set_default("norm", "baz")?;

    resolver.set_normalizer(
        "norm",
        closure(|options, _| {
            assert_eq!(Value::from("bar"), options.offset_get("lazy")?);

            Ok("normalized".into())
        }),
    )?;

    assert_equals(
        php_array!["lazy" => "bar", "norm" => "normalized"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testFailIfCyclicDependencyBetweenNormalizers() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("norm1", "bar")?;
    resolver.set_default("norm2", "baz")?;

    resolver.set_normalizer(
        "norm1",
        closure(|options, _| {
            options.offset_get("norm2")?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_normalizer(
        "norm2",
        closure(|options, _| {
            options.offset_get("norm1")?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::OptionDefinition);
    Ok(())
}

#[test]
fn testFailIfCyclicDependencyBetweenNormalizerAndLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "lazy",
        lazy(|options| {
            options.offset_get("norm")?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default("norm", "baz")?;

    resolver.set_normalizer(
        "norm",
        closure(|options, _| {
            options.offset_get("lazy")?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::OptionDefinition);
    Ok(())
}

#[test]
fn testCaughtExceptionFromNormalizerDoesNotCrashOptionResolver() -> TestResult {
    let resolver = OptionsResolver::new();
    let throw = Arc::new(AtomicBool::new(true));

    resolver.set_defaults([("catcher", Value::Null), ("thrower", Value::Null)])?;

    resolver.set_normalizer(
        "catcher",
        closure(|options, _| match options.offset_get("thrower") {
            Ok(value) => Ok(value),
            Err(e) if e.class().is_exception() => Ok(false.into()),
            Err(e) => Err(e),
        }),
    )?;

    let t = throw.clone();
    resolver.set_normalizer(
        "thrower",
        closure(move |_, _| {
            if t.load(Ordering::SeqCst) {
                t.store(false, Ordering::SeqCst);
                return Err(Error::other("UnexpectedValueException", "throwing"));
            }

            Ok(true.into())
        }),
    )?;

    assert_same(
        php_array!["catcher" => false, "thrower" => true],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testCaughtExceptionFromLazyDoesNotCrashOptionResolver() -> TestResult {
    let resolver = OptionsResolver::new();
    let throw = Arc::new(AtomicBool::new(true));

    resolver.set_default(
        "catcher",
        lazy(|options| match options.offset_get("thrower") {
            Ok(value) => Ok(value),
            Err(e) if e.class().is_exception() => Ok(false.into()),
            Err(e) => Err(e),
        }),
    )?;

    let t = throw.clone();
    resolver.set_default(
        "thrower",
        lazy(move |_| {
            if t.load(Ordering::SeqCst) {
                t.store(false, Ordering::SeqCst);
                return Err(Error::other("UnexpectedValueException", "throwing"));
            }

            Ok(true.into())
        }),
    )?;

    assert_same(
        php_array!["catcher" => false, "thrower" => true],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testInvokeEachNormalizerOnlyOnce() -> TestResult {
    let resolver = OptionsResolver::new();
    let calls = Arc::new(AtomicI64::new(0));

    resolver.set_default("norm1", "bar")?;
    resolver.set_default("norm2", "baz")?;

    let c = calls.clone();
    resolver.set_normalizer(
        "norm1",
        closure(move |options, _| {
            assert_eq!(1, c.fetch_add(1, Ordering::SeqCst) + 1);

            options.offset_get("norm2")?;
            Ok(Value::Null)
        }),
    )?;
    let c = calls.clone();
    resolver.set_normalizer(
        "norm2",
        closure(move |_, _| {
            assert_eq!(2, c.fetch_add(1, Ordering::SeqCst) + 1);
            Ok(Value::Null)
        }),
    )?;

    resolver.resolve(&empty())?;

    assert_eq!(2, calls.load(Ordering::SeqCst));
    Ok(())
}

#[test]
fn testNormalizerNotCalledForUnsetOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("norm")?;

    resolver.set_normalizer("norm", closure(|_, _| panic!("Should not be called.")))?;

    assert_same(empty(), resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testAddNormalizerReturnsThis() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;

    assert!(std::ptr::eq(
        &raw const resolver,
        resolver.add_normalizer("foo", noop(), false)?
    ));
    Ok(())
}

#[test]
fn testAddNormalizerClosure() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_default("foo", "bar")?;
    resolver.set_normalizer(
        "foo",
        closure(|_, value| Ok(bracket("1st-normalized-", &value, ""))),
    )?;
    // defined by subclass
    resolver.add_normalizer(
        "foo",
        closure(|_, value| Ok(bracket("2nd-normalized-", &value, ""))),
        false,
    )?;

    assert_equals(
        php_array!["foo" => "2nd-normalized-1st-normalized-bar"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testForcePrependNormalizerClosure() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_default("foo", "bar")?;
    resolver.set_normalizer(
        "foo",
        closure(|_, value| Ok(bracket("2nd-normalized-", &value, ""))),
    )?;
    // defined by subclass
    resolver.add_normalizer(
        "foo",
        closure(|_, value| Ok(bracket("1st-normalized-", &value, ""))),
        true,
    )?;

    assert_equals(
        php_array!["foo" => "2nd-normalized-1st-normalized-bar"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testForcePrependNormalizerForResolverWithoutPreviousNormalizers() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_default("foo", "bar")?;
    resolver.add_normalizer(
        "foo",
        closure(|_, value| Ok(bracket("1st-normalized-", &value, ""))),
        true,
    )?;

    assert_equals(
        php_array!["foo" => "1st-normalized-bar"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testAddNormalizerFailsIfUnknownOption() {
    let resolver = OptionsResolver::new();
    expect(
        resolver.add_normalizer("foo", noop(), false),
        Class::UndefinedOptions,
    );
}

#[test]
fn testFailIfAddNormalizerFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.add_normalizer("foo", noop(), false)?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testSetDefaultsReturnsThis() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(std::ptr::eq(
        &raw const resolver,
        resolver.set_defaults_array(&php_array!["foo", "bar"])?
    ));
    Ok(())
}

#[test]
fn testSetDefaults() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("one", "1")?;
    resolver.set_default("two", "bar")?;

    resolver.set_defaults([("two", "2"), ("three", "3")])?;

    assert_equals(
        php_array!["one" => "1", "two" => "2", "three" => "3"],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testFailIfSetDefaultsFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.set_defaults([("two", "2")])?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testRemoveReturnsThis() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;

    assert!(std::ptr::eq(&raw const resolver, resolver.remove("foo")?));
    Ok(())
}

#[test]
fn testRemoveSingleOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_default("baz", "boo")?;
    resolver.remove("foo")?;

    assert_same(php_array!["baz" => "boo"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testRemoveMultipleOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_default("baz", "boo")?;
    resolver.set_default("doo", "dam")?;

    resolver.remove(["foo", "doo"])?;

    assert_same(php_array!["baz" => "boo"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testRemoveLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", lazy(|_| Ok("lazy".into())))?;
    resolver.remove("foo")?;

    assert_same(empty(), resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testRemoveNormalizer() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_normalizer("foo", closure(|_, _| Ok("normalized".into())))?;
    resolver.remove("foo")?;
    resolver.set_default("foo", "bar")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testRemoveAllowedTypes() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_types("foo", "int")?;
    resolver.remove("foo")?;
    resolver.set_default("foo", "bar")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testRemoveAllowedValues() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_values("foo", php_array!["baz", "boo"])?;
    resolver.remove("foo")?;
    resolver.set_default("foo", "bar")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testFailIfRemoveFromLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.remove("bar")?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default("bar", "baz")?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testRemoveUnknownOptionIgnored() -> TestResult {
    let resolver = OptionsResolver::new();
    // assertNotNull($resolver->remove('foo'))
    resolver.remove("foo")?;
    Ok(())
}

#[test]
fn testClearReturnsThis() -> TestResult {
    let resolver = OptionsResolver::new();
    assert!(std::ptr::eq(&raw const resolver, resolver.clear()?));
    Ok(())
}

#[test]
fn testClearRemovesAllOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("one", 1)?;
    resolver.set_default("two", 2)?;

    resolver.clear()?;

    assert_same(empty(), resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testClearLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", lazy(|_| Ok("lazy".into())))?;
    resolver.clear()?;

    assert_same(empty(), resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testClearNormalizer() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_normalizer("foo", closure(|_, _| Ok("normalized".into())))?;
    resolver.clear()?;
    resolver.set_default("foo", "bar")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testClearAllowedTypes() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_types("foo", "int")?;
    resolver.clear()?;
    resolver.set_default("foo", "bar")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testClearAllowedValues() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;
    resolver.set_allowed_values("foo", "baz")?;
    resolver.clear()?;
    resolver.set_default("foo", "bar")?;

    assert_same(php_array!["foo" => "bar"], resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testFailIfClearFromLazyption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.clear()?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default("bar", "baz")?;

    expect(resolver.resolve(&empty()), Class::Access);
    Ok(())
}

#[test]
fn testClearOptionAndNormalizer() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo1", "bar")?;
    resolver.set_normalizer("foo1", closure(|_, _| Ok("".into())))?;
    resolver.set_default("foo2", "bar")?;
    resolver.set_normalizer("foo2", closure(|_, _| Ok("".into())))?;

    resolver.clear()?;
    assert_same(empty(), resolver.resolve(&empty())?);
    Ok(())
}

#[test]
fn testArrayAccess() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("default1", 0)?;
    resolver.set_default("default2", 1)?;
    resolver.set_required("required")?;
    resolver.set_defined("defined")?;
    resolver.set_default("lazy1", lazy(|_| Ok("lazy".into())))?;

    resolver.set_default(
        "lazy2",
        lazy(|options| {
            assert!(options.offset_exists("default1")?);
            assert!(options.offset_exists("default2")?);
            assert!(options.offset_exists("required")?);
            assert!(options.offset_exists("lazy1")?);
            assert!(options.offset_exists("lazy2")?);
            assert!(!options.offset_exists("defined")?);

            assert_eq!(Value::Int(0), options.offset_get("default1")?);
            assert_eq!(Value::Int(42), options.offset_get("default2")?);
            assert_eq!(Value::from("value"), options.offset_get("required")?);
            assert_eq!(Value::from("lazy"), options.offset_get("lazy1")?);

            // Obviously $options['lazy'] and $options['defined'] cannot be
            // accessed
            Ok(Value::Null)
        }),
    )?;

    resolver.resolve(&php_array!["default2" => 42, "required" => "value"])?;
    Ok(())
}

#[test]
fn testArrayAccessGetFailsOutsideResolve() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("default", 0)?;

    expect(resolver.offset_get("default"), Class::Access);
    Ok(())
}

#[test]
fn testArrayAccessExistsFailsOutsideResolve() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("default", 0)?;

    expect(resolver.offset_exists("default"), Class::Access);
    Ok(())
}

#[test]
fn testArrayAccessSetNotSupported() {
    let resolver = OptionsResolver::new();
    expect(resolver.offset_set("default", 0.into()), Class::Access);
}

#[test]
fn testArrayAccessUnsetNotSupported() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("default", 0)?;

    expect(resolver.offset_unset("default"), Class::Access);
    Ok(())
}

#[test]
fn testFailIfGetNonExisting() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", "bar")?;

    resolver.set_default(
        "lazy",
        lazy(|options| {
            options.offset_get("undefined")?;
            Ok(Value::Null)
        }),
    )?;

    expect_message(
        resolver.resolve(&empty()),
        Class::NoSuchOption,
        "The option \"undefined\" does not exist. Defined options are: \"foo\", \"lazy\".",
    );
    Ok(())
}

#[test]
fn testFailIfGetDefinedButUnset() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("defined")?;

    resolver.set_default(
        "lazy",
        lazy(|options| {
            options.offset_get("defined")?;
            Ok(Value::Null)
        }),
    )?;

    expect_message(
        resolver.resolve(&empty()),
        Class::NoSuchOption,
        "The optional option \"defined\" has no value set. You should make sure it is set with \"isset\" before reading it.",
    );
    Ok(())
}

#[test]
fn testFailIfCyclicDependency() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "lazy1",
        lazy(|options| {
            options.offset_get("lazy2")?;
            Ok(Value::Null)
        }),
    )?;

    resolver.set_default(
        "lazy2",
        lazy(|options| {
            options.offset_get("lazy1")?;
            Ok(Value::Null)
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::OptionDefinition);
    Ok(())
}

#[test]
fn testCount() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("default", 0)?;
    resolver.set_required("required")?;
    resolver.set_defined("defined")?;
    // `static function () {}`: not lazy
    resolver.set_default("lazy1", ClosureValue::value())?;

    resolver.set_default(
        "lazy2",
        lazy(|options| {
            assert_eq!(4, options.count()?);
            Ok(Value::Null)
        }),
    )?;

    assert_eq!(
        4,
        resolver.resolve(&php_array!["required" => "value"])?.len()
    );
    Ok(())
}

/// In `resolve()` we count the options that are actually set (which may be
/// only a subset of the defined options). Outside of `resolve()`, it's not
/// clear what is counted.
#[test]
fn testCountFailsOutsideResolve() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("foo", 0)?;
    resolver.set_required("bar")?;
    resolver.set_defined("bar")?;
    resolver.set_default("lazy1", ClosureValue::value())?;

    expect(resolver.count(), Class::Access);
    Ok(())
}

#[test]
fn testNestedArrays() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "int[][]")?;

    assert_equals(
        php_array!["foo" => php_array![php_array![1, 2]]],
        resolver.resolve(&php_array!["foo" => php_array![php_array![1, 2]]])?,
    );
    Ok(())
}

#[test]
fn testNestedArraysWithUnions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "(int|float|(int|float)[])[]")?;

    let value = || php_array!["foo" => php_array![1, 2.0, php_array![1, 2.0]]];
    assert_equals(value(), resolver.resolve(&value())?);
    Ok(())
}

#[test]
fn testNested2Arrays() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "int[][][][]")?;

    let value = || php_array!["foo" => php_array![php_array![php_array![php_array![1, 2]]]]];
    assert_equals(value(), resolver.resolve(&value())?);
    Ok(())
}

#[test]
fn testNestedArraysException() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "float[][][][]")?;

    expect_message(
        resolver
            .resolve(&php_array!["foo" => php_array![php_array![php_array![php_array![1, 2]]]]]),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"float[][][][]\", but one of the elements is of type \"int\".",
    );
    Ok(())
}

#[test]
fn testDeeplyNestedUnionTypes() -> TestResult {
    for row in provider("testDeeplyNestedUnionTypes") {
        let resolver = OptionsResolver::new();
        resolver.set_defined("option")?;
        resolver.set_allowed_types("option", text(&row.args[0]))?;
        let resolved = resolver.resolve(&php_array!["option" => row.args[1].clone()])?;
        let expected = php_array!["option" => row.args[1].clone()];
        // assertEquals; a resource equals only itself.
        assert!(
            phpunit_equals(&arr(expected.clone()), &arr(resolved.clone()))
                || arr(expected) == arr(resolved),
            "{}",
            text(&row.args[0])
        );
    }
    Ok(())
}

#[test]
fn testDeeplyNestedUnionTypesException() -> TestResult {
    for row in provider("testDeeplyNestedUnionTypesException") {
        let resolver = OptionsResolver::new();
        resolver.set_defined("option")?;
        resolver.set_allowed_types("option", text(&row.args[0]))?;

        expect_message(
            resolver.resolve(&php_array!["option" => row.args[1].clone()]),
            Class::InvalidOptions,
            &text(&row.args[2]),
        );
    }
    Ok(())
}

#[test]
fn testNestedArrayException1() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "int[][]")?;
    expect_message(
        resolver.resolve(
            &php_array!["foo" => php_array![php_array![1, true, "str", php_array![2, 3]]]],
        ),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"int[][]\", but one of the elements is of type \"bool|string|array\".",
    );
    Ok(())
}

#[test]
fn testNestedArrayException2() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "int[][]")?;
    expect_message(
        resolver
            .resolve(&php_array!["foo" => php_array![php_array![true, "str", php_array![2, 3]]]]),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"int[][]\", but one of the elements is of type \"bool|string|array\".",
    );
    Ok(())
}

#[test]
fn testNestedArrayException3() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "string[][][]")?;
    expect_message(
        resolver.resolve(&php_array!["foo" => php_array![php_array!["str", php_array![1, 2]]]]),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"string[][][]\", but one of the elements is of type \"string|int\".",
    );
    Ok(())
}

#[test]
fn testNestedArrayException4() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "string[][][]")?;
    expect_message(
        resolver.resolve(
            &php_array!["foo" => php_array![php_array![php_array!["str"], php_array![1, 2]]]],
        ),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"string[][][]\", but one of the elements is of type \"int\".",
    );
    Ok(())
}

#[test]
fn testNestedArrayException5() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_allowed_types("foo", "string[]")?;
    expect_message(
        resolver.resolve(
            &php_array!["foo" => php_array![php_array![php_array!["str"], php_array![1, 2]]]],
        ),
        Class::InvalidOptions,
        "The option \"foo\" with value array is expected to be of type \"string[]\", but one of the elements is of type \"array\".",
    );
    Ok(())
}

#[test]
fn testNestedClosureWithoutTypeHintNotInvoked() -> TestResult {
    // `static function ($resolver)`: not lazy
    closure_not_invoked()
}

#[test]
fn testNestedClosureWithoutTypeHint2ndArgumentNotInvoked() -> TestResult {
    // `static function (OptionsResolver $resolver, $parent)`: its first
    // parameter is not typed `Options`, so it is not lazy
    closure_not_invoked()
}

#[test]
fn testFailsIfOptionIsAlreadyDefined() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.define("foo")?;
    expect_message(
        resolver.define("foo"),
        Class::OptionDefinition,
        "The option \"foo\" is already defined.",
    );
    Ok(())
}

#[test]
fn testResolveOptionsDefinedByOptionConfigurator() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .define("foo")?
        .required()?
        .deprecated("vendor/package", "1.1", DEFAULT_DEPRECATION_MESSAGE)?
        .default("bar")?
        .allowed_types(&["string", "bool"])?
        .allowed_values(["bar".into(), "zab".into()])?
        .normalize(closure(|_, value| Ok(value)))?
        .info("info message")?;
    resolver.define("table")?.options(nested(|resolver, _| {
        resolver.set_default("ping", "pong")?;
        Ok(())
    }))?;
    let introspector = OptionsResolverIntrospector::new(&resolver);

    assert!(resolver.is_defined("foo"));
    assert!(resolver.is_deprecated("foo"));
    assert!(resolver.has_default("foo"));
    assert_eq!(Value::from("bar"), introspector.get_default("foo")?);
    assert_eq!(
        vec!["string", "bool"],
        introspector.get_allowed_types("foo")?
    );
    let allowed_values = introspector.get_allowed_values("foo")?;
    let expected = AllowedValues::list(["bar".into(), "zab".into()]);
    assert_eq!(expected.0.len(), allowed_values.0.len());
    assert!(
        expected
            .0
            .iter()
            .zip(&allowed_values.0)
            .all(|((k1, v1), (k2, v2))| k1 == k2 && v1.same(v2))
    );
    assert_eq!(1, introspector.get_normalizers("foo")?.len());
    assert_eq!(Some("info message".to_owned()), resolver.get_info("foo")?);
    assert!(resolver.is_nested("table"));
    Ok(())
}

#[test]
fn testGetInfo() -> TestResult {
    let resolver = OptionsResolver::new();
    let info = "The option info message";
    resolver.set_defined("foo")?;
    resolver.set_info("foo", info)?;

    assert_eq!(Some(info.to_owned()), resolver.get_info("foo")?);
    Ok(())
}

#[test]
fn testSetInfoOnNormalization() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_normalizer(
        "foo",
        closure(|options, _| {
            options.set_info("foo", "Info")?;
            Ok(Value::Null)
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array!["foo" => "bar"]),
        Class::Access,
        "The Info message cannot be set from a lazy option or normalizer.",
    );
    Ok(())
}

#[test]
fn testSetNestedOnNormalization() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default(
        "foo",
        lazy(|options| {
            options.set_options("foo", nested(|_, _| Ok(())))?;
            Ok(Value::Null)
        }),
    )?;

    expect_message(
        resolver.resolve(&empty()),
        Class::Access,
        "Nested options cannot be defined from a lazy option or normalizer.",
    );
    Ok(())
}

#[test]
fn testSetInfoOnUndefinedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    expect_message(
        resolver.set_info("bar", "The option info message"),
        Class::UndefinedOptions,
        "The option \"bar\" does not exist. Defined options are: \"foo\".",
    );
    Ok(())
}

#[test]
fn testGetInfoOnUndefinedOption2() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    expect_message(
        resolver.get_info("bar"),
        Class::UndefinedOptions,
        "The option \"bar\" does not exist. Defined options are: \"foo\".",
    );
    Ok(())
}

#[test]
fn testInfoOnInvalidValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .set_required("expires")?
        .set_info("expires", "A future date time")?
        .set_allowed_types("expires", DateTimeImmutable::CLASS)?
        .set_allowed_values(
            "expires",
            allowed(|value| Ok(timestamp(value).is_some_and(|t| t >= now()))),
        )?;

    expect_message(
        resolver.resolve(&php_array!["expires" => date_time_immutable_at(now() - 3600)]),
        Class::InvalidOptions,
        "The option \"expires\" with value DateTimeImmutable is invalid. Info: A future date time.",
    );
    Ok(())
}

#[test]
fn testAccessExceptionOnPrototypeDefinition() {
    let resolver = OptionsResolver::new();
    expect_message(
        resolver.set_prototype(true),
        Class::Access,
        "The prototype property cannot be set from a root definition.",
    );
}

#[test]
fn testPrototypeDefinition() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_options(
        "connections",
        nested(|resolver, _| {
            resolver
                .set_prototype(true)?
                .set_required("table")?
                .set_defaults([("user", Value::from("root")), ("password", Value::Null)])?;
            Ok(())
        }),
    )?;

    let actual_options = resolver.resolve(&php_array![
        "connections" => php_array![
            "default" => php_array!["table" => "default"],
            "custom" => php_array!["user" => "foo", "password" => "pa$$", "table" => "symfony"],
        ],
    ])?;
    let expected_options = php_array![
        "connections" => php_array![
            "default" => php_array!["user" => "root", "password" => Value::Null, "table" => "default"],
            "custom" => php_array!["user" => "foo", "password" => "pa$$", "table" => "symfony"],
        ],
    ];

    assert_same(expected_options, actual_options);
    Ok(())
}

#[test]
fn testInvalidValueForPrototypeDefinition() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_options(
        "connections",
        nested(|resolver, _| {
            resolver
                .set_prototype(true)?
                .set_defined(["table", "user", "password"])?;
            Ok(())
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array!["connections" => php_array!["foo"]]),
        Class::InvalidOptions,
        "The value of the option \"connections\" is expected to be of type array of array, but is of type array of \"string\".",
    );
    Ok(())
}

#[test]
fn testMissingOptionForPrototypeDefinition() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_options(
        "connections",
        nested(|resolver, _| {
            resolver.set_prototype(true)?.set_required("table")?;
            Ok(())
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array![
            "connections" => php_array![
                php_array!["table" => "default"],
                empty(), // <- missing required option "table"
            ],
        ]),
        Class::MissingOptions,
        "The required option \"connections[1][table]\" is missing.",
    );
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidValueFromNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver
                .set_defined("bar")?
                .set_allowed_values("bar", "valid value")?;
            Ok(())
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array!["foo" => php_array!["bar" => "invalid value"]]),
        Class::InvalidOptions,
        "The option \"foo[bar]\" with value \"invalid value\" is invalid. Accepted values are: \"valid value\".",
    );
    Ok(())
}

#[test]
fn testResolveFailsIfInvalidTypeFromNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver
                .set_defined("bar")?
                .set_allowed_types("bar", "string")?;
            Ok(())
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array!["foo" => php_array!["bar" => 1]]),
        Class::InvalidOptions,
        "The option \"foo[bar]\" with value 1 is expected to be of type \"string\", but is of type \"int\".",
    );
    Ok(())
}

#[test]
fn testIsNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_options(
        "database",
        nested(|resolver, _| {
            resolver.set_defined(["host", "port"])?;
            Ok(())
        }),
    )?;

    assert!(resolver.is_nested("database"));
    Ok(())
}

#[test]
fn testFailsIfUndefinedNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("name", "default")?.set_options(
        "database",
        nested(|resolver, _| {
            resolver.set_defined(["host", "port"])?;
            Ok(())
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array!["database" => php_array!["foo" => "bar"]]),
        Class::UndefinedOptions,
        "The option \"database[foo]\" does not exist. Defined options are: \"host\", \"port\".",
    );
    Ok(())
}

#[test]
fn testFailsIfMissingRequiredNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("name", "default")?.set_options(
        "database",
        nested(|resolver, _| {
            resolver.set_required("host")?;
            Ok(())
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array!["database" => empty()]),
        Class::MissingOptions,
        "The required option \"database[host]\" is missing.",
    );
    Ok(())
}

#[test]
fn testFailsIfInvalidTypeNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("name", "default")?.set_options(
        "database",
        nested(|resolver, _| {
            resolver
                .set_defined("logging")?
                .set_allowed_types("logging", "bool")?;
            Ok(())
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array!["database" => php_array!["logging" => Value::Null]]),
        Class::InvalidOptions,
        "The option \"database[logging]\" with value null is expected to be of type \"bool\", but is of type \"null\".",
    );
    Ok(())
}

#[test]
fn testFailsIfNotArrayIsGivenForNestedOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("name", "default")?.set_options(
        "database",
        nested(|resolver, _| {
            resolver.set_defined("host")?;
            Ok(())
        }),
    )?;

    expect_message(
        resolver.resolve(&php_array!["database" => Value::Null]),
        Class::InvalidOptions,
        "The nested option \"database\" with value null is expected to be of type array, but is of type \"null\".",
    );
    Ok(())
}

#[test]
fn testResolveNestedOptionsWithoutDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("name", "default")?.set_options(
        "database",
        nested(|resolver, _| {
            resolver.set_defined(["host", "port"])?;
            Ok(())
        }),
    )?;

    let actual_options = resolver.resolve(&empty())?;
    let expected_options = php_array!["name" => "default", "database" => empty()];

    assert_same(expected_options, actual_options);
    Ok(())
}

#[test]
fn testResolveNestedOptionsWithDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("name", "default")?.set_options(
        "database",
        nested(|resolver, _| {
            resolver.set_defaults([("host", Value::from("localhost")), ("port", 3306.into())])?;
            Ok(())
        }),
    )?;

    let actual_options = resolver.resolve(&empty())?;
    let expected_options = php_array![
        "name" => "default",
        "database" => php_array!["host" => "localhost", "port" => 3306],
    ];

    assert_same(expected_options, actual_options);
    Ok(())
}

#[test]
fn testResolveMultipleNestedOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defaults([("name", "default")])?.set_options(
        "database",
        nested(|resolver, _| {
            resolver
                .set_required(["dbname", "host"])?
                .set_defaults([("port", 3306)])?
                .set_options(
                    "replicas",
                    nested(|resolver, _| {
                        resolver.set_defaults([
                            ("host", Value::from("replica1")),
                            ("port", 3306.into()),
                        ])?;
                        Ok(())
                    }),
                )?;
            Ok(())
        }),
    )?;

    let actual_options = resolver.resolve(&php_array![
        "name" => "custom",
        "database" => php_array![
            "dbname" => "test",
            "host" => "localhost",
            "port" => Value::Null,
            "replicas" => php_array!["host" => "replica2"],
        ],
    ])?;
    let expected_options = php_array![
        "name" => "custom",
        "database" => php_array![
            "port" => Value::Null,
            "replicas" => php_array!["port" => 3306, "host" => "replica2"],
            "dbname" => "test",
            "host" => "localhost",
        ],
    ];
    assert_same(expected_options, actual_options);
    Ok(())
}

/// `$options['database']['server_version']`.
fn dig(options: &OptionsResolver, outer: &str, inner: &str) -> Result<Value, Error> {
    Ok(options
        .offset_get(outer)?
        .as_array()
        .and_then(|a| a.get(&Key::from(inner)).cloned())
        .unwrap_or_default())
}

#[test]
fn testResolveLazyOptionUsingNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .set_default(
            "version",
            lazy(|options| dig(options, "database", "server_version")),
        )?
        .set_options(
            "database",
            nested(|resolver, _| {
                resolver.set_default("server_version", "3.15")?;
                Ok(())
            }),
        )?;

    let actual_options = resolver.resolve(&empty())?;
    let expected_options = php_array![
        "database" => php_array!["server_version" => "3.15"],
        "version" => "3.15",
    ];

    assert_same(expected_options, actual_options);
    Ok(())
}

#[test]
fn testNormalizeNestedOptionValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .set_options(
            "database",
            nested(|resolver, _| {
                resolver.set_defaults([
                    ("port", Value::from(3306)),
                    ("host", "localhost".into()),
                    ("dbname", "demo".into()),
                ])?;
                Ok(())
            }),
        )?
        .set_normalizer(
            "database",
            closure(|_, value| {
                // ksort($value)
                let mut entries: Vec<_> = value
                    .as_array()
                    .map(|a| a.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                    .unwrap_or_default();
                entries.sort_by(|(a, _), (b, _)| symfony_options_resolver::php::compare_keys(a, b));
                let mut sorted = PhpArray::new();
                for (k, v) in entries {
                    sorted.insert(k, v);
                }

                Ok(arr(sorted))
            }),
        )?;

    let actual_options =
        resolver.resolve(&php_array!["database" => php_array!["dbname" => "test"]])?;
    let expected_options = php_array![
        "database" => php_array!["dbname" => "test", "host" => "localhost", "port" => 3306],
    ];

    assert_same(expected_options, actual_options);
    Ok(())
}

#[test]
fn testNestedOptionEvaluatedWithLazyDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver.define("bar")?.allowed_types(&["string"])?;
            Ok(())
        }),
    )?;
    // defined by subclass
    resolver.set_default("foo", lazy(|_| Ok(arr(php_array!["bar" => "lazy"]))))?;

    assert_same(
        php_array!["foo" => php_array!["bar" => "lazy"]],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testNestedOptionWithDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver.define("bar")?.allowed_types(&["string"])?;
            Ok(())
        }),
    )?;
    // defined by subclass
    resolver.set_default("foo", php_array!["bar" => "default"])?;

    assert_same(
        php_array!["foo" => php_array!["bar" => "default"]],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testResolveAllNestedOptionDefinitions() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver.set_required("bar")?;
            Ok(())
        }),
    )?;
    // defined by subclass
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver.set_default("bar", "baz")?;
            Ok(())
        }),
    )?;
    // defined by subclass
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver.set_default("ping", "pong")?;
            Ok(())
        }),
    )?;
    assert_same(
        php_array!["foo" => php_array!["ping" => "pong", "bar" => "baz"]],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testSetNestedOptionWithInvalidDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver.define("bar")?.allowed_types(&["int"])?;
            Ok(())
        }),
    )?;
    // defined by subclass
    resolver.set_default("foo", php_array!["bar" => "invalid"])?;

    expect_message(
        resolver.resolve(&empty()),
        Class::InvalidArgument,
        "The option \"foo[bar]\" with value \"invalid\" is expected to be of type \"int\", but is of type \"string\".",
    );
    Ok(())
}

#[test]
fn testSetNestedOptionWithInvalidLazyDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver.define("bar")?.allowed_types(&["int"])?;
            Ok(())
        }),
    )?;
    // defined by subclass
    resolver.set_default("foo", lazy(|_| Ok(arr(php_array!["bar" => "invalid"]))))?;

    expect_message(
        resolver.resolve(&empty()),
        Class::InvalidArgument,
        "The option \"foo[bar]\" with value \"invalid\" is expected to be of type \"int\", but is of type \"string\".",
    );
    Ok(())
}

#[test]
fn testNormalizeNestedValue() -> TestResult {
    let resolver = OptionsResolver::new();
    // defined by superclass
    resolver.set_options(
        "foo",
        nested(|resolver, _| {
            resolver.set_default("bar", Value::Null)?;
            Ok(())
        }),
    )?;
    // defined by subclass
    resolver.set_normalizer(
        "foo",
        closure(|_, resolved_value| {
            let mut resolved_value = resolved_value.as_array().cloned().unwrap_or_default();
            // $resolvedValue['bar'] ??= 'baz';
            let bar = Key::from("bar");
            if resolved_value.get(&bar).is_none_or(Value::is_null) {
                resolved_value.insert(bar, "baz".into());
            }

            Ok(arr(resolved_value))
        }),
    )?;

    assert_same(
        php_array!["foo" => php_array!["bar" => "baz"]],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testFailsIfCyclicDependencyBetweenSameNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_options(
        "database",
        nested(|resolver, parent| {
            resolver.set_default("replicas", parent.offset_get("database")?)?;
            Ok(())
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::OptionDefinition);
    Ok(())
}

#[test]
fn testFailsIfCyclicDependencyBetweenNestedOptionAndParentLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .set_default(
            "version",
            lazy(|options| dig(options, "database", "server_version")),
        )?
        .set_options(
            "database",
            nested(|resolver, parent| {
                resolver.set_default("server_version", parent.offset_get("version")?)?;
                Ok(())
            }),
        )?;

    expect(resolver.resolve(&empty()), Class::OptionDefinition);
    Ok(())
}

#[test]
fn testFailsIfCyclicDependencyBetweenNormalizerAndNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .set_default("name", "default")?
        .set_options(
            "database",
            nested(|resolver, parent| {
                resolver.set_default("host", parent.offset_get("name")?)?;
                Ok(())
            }),
        )?
        .set_normalizer(
            "name",
            closure(|options, _| {
                options.offset_get("database")?;
                Ok(Value::Null)
            }),
        )?;

    expect(resolver.resolve(&empty()), Class::OptionDefinition);
    Ok(())
}

#[test]
fn testFailsIfCyclicDependencyBetweenNestedOptions() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_options(
        "database",
        nested(|resolver, parent| {
            resolver.set_default("host", dig(parent, "replica", "host")?)?;
            Ok(())
        }),
    )?;
    resolver.set_options(
        "replica",
        nested(|resolver, parent| {
            resolver.set_default("host", dig(parent, "database", "host")?)?;
            Ok(())
        }),
    )?;

    expect(resolver.resolve(&empty()), Class::OptionDefinition);
    Ok(())
}

#[test]
fn testGetAccessToParentOptionFromNestedOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("version", 3.15)?.set_options(
        "database",
        nested(|resolver, parent| {
            resolver.set_default("server_version", parent.offset_get("version")?)?;
            Ok(())
        }),
    )?;

    assert_same(
        php_array!["version" => 3.15, "database" => php_array!["server_version" => 3.15]],
        resolver.resolve(&empty())?,
    );
    Ok(())
}

#[test]
fn testResolveLazyOptionWithTransitiveDefaultDependency() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver
        .set_defaults([
            ("ip", DefaultValue::from(Value::Null)),
            (
                "secondary_replica",
                lazy(|options| {
                    let database = options.offset_get("database")?;
                    Ok(database
                        .as_array()
                        .and_then(|d| d.get(&Key::from("primary_replica")))
                        .and_then(Value::as_array)
                        .and_then(|r| r.get(&Key::from("host")).cloned())
                        .unwrap_or_default())
                }),
            ),
        ])?
        .set_options(
            "database",
            nested(|resolver, parent| {
                resolver
                    .set_default("host", parent.offset_get("ip")?)?
                    .set_options(
                        "primary_replica",
                        nested(|resolver, parent| {
                            resolver.set_default("host", parent.offset_get("host")?)?;
                            Ok(())
                        }),
                    )?;
                Ok(())
            }),
        )?;

    let actual_options = resolver.resolve(&php_array!["ip" => "127.0.0.1"])?;
    let expected_options = php_array![
        "ip" => "127.0.0.1",
        "database" => php_array![
            "host" => "127.0.0.1",
            "primary_replica" => php_array!["host" => "127.0.0.1"],
        ],
        "secondary_replica" => "127.0.0.1",
    ];
    assert_same(expected_options, actual_options);
    Ok(())
}

#[test]
fn testAccessToParentOptionFromNestedNormalizerAndLazyOption() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_default("debug", true)?.set_options(
        "database",
        nested(|resolver, parent| {
            let p1 = parent.handle();
            let p2 = parent.handle();
            resolver
                .set_defined("logging")?
                .set_default("profiling", lazy(move |_| p1.offset_get("debug")))?
                .set_normalizer(
                    "logging",
                    closure(move |_, value| {
                        Ok(if Value::Bool(false) == p2.offset_get("debug")? {
                            true.into()
                        } else {
                            value
                        })
                    }),
                )?;
            Ok(())
        }),
    )?;

    let actual_options = resolver.resolve(&php_array![
        "debug" => false,
        "database" => php_array!["logging" => false],
    ])?;
    let expected_options = php_array![
        "debug" => false,
        "database" => php_array!["profiling" => false, "logging" => true],
    ];

    assert_same(expected_options, actual_options);
    Ok(())
}

#[test]
fn testRemoveAlsoRemovesDeprecation() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_deprecated("foo", "vendor/package", "1.0", DEFAULT_DEPRECATION_MESSAGE)?;
    assert!(resolver.is_deprecated("foo"));

    resolver.remove("foo")?;
    assert!(!resolver.is_deprecated("foo"));

    resolver.set_default("foo", "bar")?;
    assert!(!resolver.is_deprecated("foo"));

    let (result, deprecations) =
        symfony_deprecation_contracts::collect(|| resolver.resolve(&php_array!["foo" => "value"]));
    result?;
    assert_eq!(0, deprecations.len());
    Ok(())
}
