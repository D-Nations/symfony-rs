//! Tests/Loader/IniFileLoaderTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's; a data provider's rows
//! run in a loop. `PHP_VERSION` is a constant the test defines for the
//! loader (`set_constants()`); the port has no PHP process to read it
//! from. Tests skip, with a note, when `upstream/` is not fetched.
//!
//! Not ported: testTypeConversionsWithNativePhp, which compares the
//! loader with PHP's own `INI_SCANNER_TYPED` parsing (no PHP here).
//!
//! Upstream: symfony/dependency-injection Tests/Loader/IniFileLoaderTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

mod loader_support;

use std::sync::Arc;

use loader_support::{assert_error, di};
use php_value::constants::Registry;
use php_value::{Key, PhpArray, Value, php_array};
use symfony_config::FileLocator;
use symfony_config::loader::LoaderInterface;
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::loader::{IniFileLoader, SharedContainer, shared};

const PHP_VERSION: &str = "8.4.27";

fn set_up() -> Option<(SharedContainer, Arc<IniFileLoader>)> {
    let fixtures = loader_support::fixtures()?;
    let container = shared(ContainerBuilder::new());
    let loader = IniFileLoader::new(
        container.clone(),
        Arc::new(FileLocator::new([format!("{fixtures}/ini")])),
        None,
        false,
    );
    let mut constants = Registry::with_core();
    constants.define("PHP_VERSION", Value::from(PHP_VERSION));
    loader.set_constants(Arc::new(constants));
    Some((container, loader))
}

fn all(container: &SharedContainer) -> PhpArray {
    container.lock().unwrap().get_parameter_bag().all().clone()
}

#[test]
fn testIniFileCanBeLoaded() {
    let Some((container, loader)) = set_up() else {
        return;
    };
    loader.load(&Value::from("parameters.ini"), None).unwrap();
    assert_eq!(
        php_array!["foo" => "bar", "bar" => "%foo%"],
        all(&container),
        "->load() takes a single file name as its first argument"
    );
}

fn get_type_conversions() -> Vec<(&'static str, Value)> {
    vec![
        ("true_comment", Value::Bool(true)),
        ("true", Value::Bool(true)),
        ("false", Value::Bool(false)),
        ("on", Value::Bool(true)),
        ("off", Value::Bool(false)),
        ("yes", Value::Bool(true)),
        ("no", Value::Bool(false)),
        ("none", Value::Bool(false)),
        ("null", Value::Null),
        ("constant", Value::from(PHP_VERSION)),
        ("12_int", Value::Int(12)),
        ("12_string", Value::from("12")),
        ("12_quoted_number", Value::Int(12)),
        ("12_comment", Value::Int(12)),
        ("12_string_comment", Value::from("12")),
        ("12_quoted_number_comment", Value::Int(12)),
        ("-12_negative", Value::Int(-12)),
        ("one", Value::Int(1)),
        ("zero", Value::Int(0)),
        ("0b0110_byte_string", Value::Int(0b0110)),
        (
            "11112222333344445555_great_number",
            Value::from("1111,2222,3333,4444,5555"),
        ),
        ("0777_number_starting_with_0", Value::Int(0o777)),
        ("255_hexadecimal", Value::Int(0xFF)),
        ("100.0_exponential", Value::Float(1e2)),
        ("-120.0_exponential", Value::Float(-1.2E2)),
        ("-10100.1_negative_float", Value::Float(-10100.1)),
        ("-10,100.1_negative_float", Value::from("-10,100.1")),
        ("list", Value::Array(php_array![1, 2])),
        ("map", Value::Array(php_array!["one" => 1, "two" => 2])),
    ]
}

#[test]
fn testTypeConversions() {
    for (key, value) in get_type_conversions() {
        let Some((container, loader)) = set_up() else {
            return;
        };
        loader.load(&Value::from("types.ini"), None).unwrap();
        let parameters = all(&container);
        assert!(
            value.identical(parameters.get(&Key::from(key)).unwrap()),
            "->load() converts values to PHP types: {key}: {:?}",
            parameters.get(&Key::from(key))
        );
    }
}

#[test]
fn testExceptionIsRaisedWhenIniFileDoesNotExist() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = loader.load(&Value::from("foo.ini"), None).unwrap_err();
    assert_error(
        &e,
        "InvalidArgumentException",
        "The file \"foo.ini\" does not exist (in:",
    );
}

#[test]
fn testExceptionIsRaisedWhenIniFileCannotBeParsed() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = loader.load(&Value::from("nonvalid.ini"), None).unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "The \"nonvalid.ini\" file is not valid.",
    );
}

#[test]
fn testExceptionIsRaisedWhenIniFileIsAlmostValid() {
    let Some((_, loader)) = set_up() else {
        return;
    };
    let e = loader
        .load(&Value::from("almostvalid.ini"), None)
        .unwrap_err();
    assert_error(
        &e,
        &di("InvalidArgumentException"),
        "The \"almostvalid.ini\" file is not valid.",
    );
}

#[test]
fn testSupports() {
    let loader = IniFileLoader::new(
        shared(ContainerBuilder::new()),
        Arc::new(FileLocator::default()),
        None,
        false,
    );

    assert!(
        loader.supports(&Value::from("foo.ini"), None),
        "->supports() returns true if the resource is loadable"
    );
    assert!(
        !loader.supports(&Value::from("foo.foo"), None),
        "->supports() returns false if the resource is not loadable"
    );
    assert!(
        loader.supports(&Value::from("with_wrong_ext.yml"), Some(b"ini")),
        "->supports() returns true if the resource with forced type is loadable"
    );
}

#[test]
fn testWhenEnv() {
    let Some(fixtures) = loader_support::fixtures() else {
        return;
    };
    let container = shared(ContainerBuilder::new());
    let loader = IniFileLoader::new(
        container.clone(),
        Arc::new(FileLocator::new([format!("{fixtures}/ini")])),
        Some("some-env"),
        false,
    );
    loader.load(&Value::from("when-env.ini"), None).unwrap();

    assert!(
        Value::Array(php_array!["foo" => 234, "bar" => 345])
            .identical(&Value::Array(all(&container)))
    );
}
