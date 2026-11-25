//! Debug/OptionsResolverIntrospectorTest.php at symfony/options-resolver
//! 88f9c561f678a02d54b897014049fa839e33ff82. Names and assertions are
//! PHPUnit's; `assertSame` on closures compares handles.
//!
//! Upstream: symfony/options-resolver Tests/Debug/OptionsResolverIntrospectorTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::needless_pass_by_value,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

use php_value::Value;
use symfony_options_resolver::{
    AllowedValue, Class, DefaultValue, DeprecationMessage, Error, NestedClosure, OptionClosure,
    OptionsResolver, OptionsResolverIntrospector,
};

type TestResult = Result<(), Error>;

fn expect_error<T: std::fmt::Debug>(result: Result<T, Error>, class: Class, message: &str) {
    match result {
        Err(e) => {
            assert!(e.is_a(&class), "{e} is not a {}", class.name());
            assert_eq!(e.message(), message);
        }
        Ok(v) => panic!("expected {}, got {v:?}", class.name()),
    }
}

fn noop() -> OptionClosure {
    OptionClosure::new(|_, _| Ok(Value::Null))
}

#[test]
fn testGetDefault() -> TestResult {
    let resolver = OptionsResolver::new();
    let option = "foo";
    resolver.set_default(option, "bar")?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    assert_eq!(Value::from("bar"), debug.get_default(option)?);
    Ok(())
}

#[test]
fn testGetDefaultNull() -> TestResult {
    let resolver = OptionsResolver::new();
    let option = "foo";
    resolver.set_default(option, Value::Null)?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    assert!(debug.get_default(option)?.is_null());
    Ok(())
}

#[test]
fn testGetDefaultThrowsOnNoConfiguredValue() -> TestResult {
    let resolver = OptionsResolver::new();
    let option = "foo";
    resolver.set_defined(option)?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_default(option),
        Class::NoConfiguration,
        "No default value was set for the \"foo\" option.",
    );
    Ok(())
}

#[test]
fn testGetDefaultThrowsOnNotDefinedOption() {
    let resolver = OptionsResolver::new();

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_default("foo"),
        Class::UndefinedOptions,
        "The option \"foo\" does not exist.",
    );
}

#[test]
fn testGetLazyClosures() -> TestResult {
    let resolver = OptionsResolver::new();
    let mut closures = Vec::new();
    let closure = noop();
    closures.push(closure.clone());
    resolver.set_default(
        "foo",
        DefaultValue::Lazy {
            closure,
            previous: false,
        },
    )?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    let actual = debug.get_lazy_closures("foo")?;
    assert_eq!(closures.len(), actual.len());
    assert!(closures.iter().zip(&actual).all(|(a, b)| a.same(b)));
    Ok(())
}

#[test]
fn testGetLazyClosuresThrowsOnNoConfiguredValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_lazy_closures("foo"),
        Class::NoConfiguration,
        "No lazy closures were set for the \"foo\" option.",
    );
    Ok(())
}

#[test]
fn testGetLazyClosuresThrowsOnNotDefinedOption() {
    let resolver = OptionsResolver::new();

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_lazy_closures("foo"),
        Class::UndefinedOptions,
        "The option \"foo\" does not exist.",
    );
}

#[test]
fn testGetAllowedTypes() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    let allowed_types = ["string", "bool"];
    resolver.set_allowed_types("foo", allowed_types)?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    assert_eq!(allowed_types.to_vec(), debug.get_allowed_types("foo")?);
    Ok(())
}

#[test]
fn testGetAllowedTypesThrowsOnNoConfiguredValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_allowed_types("foo"),
        Class::NoConfiguration,
        "No allowed types were set for the \"foo\" option.",
    );
    Ok(())
}

#[test]
fn testGetAllowedTypesThrowsOnNotDefinedOption() {
    let resolver = OptionsResolver::new();

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_allowed_types("foo"),
        Class::UndefinedOptions,
        "The option \"foo\" does not exist.",
    );
}

#[test]
fn testGetAllowedValues() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    let allowed_values = php_value::php_array!["bar", "baz"];
    resolver.set_allowed_values("foo", allowed_values.clone())?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    let actual = debug.get_allowed_values("foo")?;
    let expected: Vec<_> = allowed_values
        .iter()
        .map(|(k, v)| (k.clone(), AllowedValue::Value(v.clone())))
        .collect();
    assert_eq!(expected.len(), actual.0.len());
    assert!(
        expected
            .iter()
            .zip(&actual.0)
            .all(|((k1, v1), (k2, v2))| k1 == k2 && v1.same(v2))
    );
    Ok(())
}

#[test]
fn testGetAllowedValuesThrowsOnNoConfiguredValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_allowed_values("foo"),
        Class::NoConfiguration,
        "No allowed values were set for the \"foo\" option.",
    );
    Ok(())
}

#[test]
fn testGetAllowedValuesThrowsOnNotDefinedOption() {
    let resolver = OptionsResolver::new();

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_allowed_values("foo"),
        Class::UndefinedOptions,
        "The option \"foo\" does not exist.",
    );
}

#[test]
fn testGetNormalizer() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    let normalizer = noop();
    resolver.set_normalizer("foo", normalizer.clone())?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    assert!(
        debug
            .get_normalizer("foo")?
            .is_some_and(|n| n.same(&normalizer))
    );
    Ok(())
}

#[test]
fn testGetNormalizerThrowsOnNoConfiguredValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_normalizer("foo"),
        Class::NoConfiguration,
        "No normalizer was set for the \"foo\" option.",
    );
    Ok(())
}

#[test]
fn testGetNormalizerThrowsOnNotDefinedOption() {
    let resolver = OptionsResolver::new();

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_normalizer("foo"),
        Class::UndefinedOptions,
        "The option \"foo\" does not exist.",
    );
}

#[test]
fn testGetNormalizers() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    let normalizer1 = noop();
    let normalizer2 = noop();
    resolver.add_normalizer("foo", normalizer1.clone(), false)?;
    resolver.add_normalizer("foo", normalizer2.clone(), false)?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    let actual = debug.get_normalizers("foo")?;
    assert_eq!(2, actual.len());
    assert!(actual[0].same(&normalizer1) && actual[1].same(&normalizer2));
    Ok(())
}

#[test]
fn testGetNormalizersThrowsOnNoConfiguredValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_normalizers("foo"),
        Class::NoConfiguration,
        "No normalizer was set for the \"foo\" option.",
    );
    Ok(())
}

#[test]
fn testGetNormalizersThrowsOnNotDefinedOption() {
    let resolver = OptionsResolver::new();

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_normalizers("foo"),
        Class::UndefinedOptions,
        "The option \"foo\" does not exist.",
    );
}

#[test]
fn testGetDeprecation() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    resolver.set_deprecated(
        "foo",
        "vendor/package",
        "1.1",
        "The option \"foo\" is deprecated.",
    )?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    let deprecation = debug.get_deprecation("foo")?;
    assert_eq!("vendor/package", deprecation.package);
    assert_eq!("1.1", deprecation.version);
    assert!(matches!(
        deprecation.message,
        DeprecationMessage::Text(t) if t == "The option \"foo\" is deprecated."
    ));
    Ok(())
}

#[test]
fn testGetClosureDeprecation() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;
    let closure = noop();
    resolver.set_deprecated("foo", "vendor/package", "1.1", closure.clone())?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    let deprecation = debug.get_deprecation("foo")?;
    assert_eq!("vendor/package", deprecation.package);
    assert_eq!("1.1", deprecation.version);
    assert!(matches!(
        deprecation.message,
        DeprecationMessage::Closure(c) if c.same(&closure)
    ));
    Ok(())
}

#[test]
fn testGetDeprecationMessageThrowsOnNoConfiguredValue() -> TestResult {
    let resolver = OptionsResolver::new();
    resolver.set_defined("foo")?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_deprecation("foo"),
        Class::NoConfiguration,
        "No deprecation was set for the \"foo\" option.",
    );
    Ok(())
}

#[test]
fn testGetDeprecationMessageThrowsOnNotDefinedOption() {
    let resolver = OptionsResolver::new();

    let debug = OptionsResolverIntrospector::new(&resolver);
    expect_error(
        debug.get_deprecation("foo"),
        Class::UndefinedOptions,
        "The option \"foo\" does not exist.",
    );
}

#[test]
fn testGetClosureNested() -> TestResult {
    let resolver = OptionsResolver::new();
    let closure = NestedClosure::new(|_, _| Ok(()));
    resolver.set_options("foo", closure.clone())?;

    let debug = OptionsResolverIntrospector::new(&resolver);
    let nested = debug.get_nested_options("foo")?;
    assert_eq!(1, nested.len());
    assert!(nested[0].same(&closure));
    Ok(())
}
