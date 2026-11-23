//! Helpers shared by the ported PHPUnit tests: `expectException()` /
//! `expectExceptionMessage()`, and the enums of upstream's Tests/Fixtures.
//!
//! Upstream: symfony/config Tests/Fixtures/TestEnum.php Tests/Fixtures/TestEnum2.php Tests/Fixtures/StringBackedTestEnum.php Tests/Fixtures/StringBackedTestEnum2.php Tests/Fixtures/IntegerBackedTestEnum.php
#![allow(dead_code)]

pub mod builder;
pub mod cache;
pub mod configuration;
pub mod fs;
pub mod loaders;

use php_value::{Value, constants::Registry, object::EnumCase};
use symfony_config::{Class, Error};

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

pub const FIXTURES: &str = "Symfony\\Component\\Config\\Tests\\Fixtures";

/// `expectException($class)` and `expectExceptionMessage($message)`: the
/// error is an instance of `class` and its message contains `message`.
#[track_caller]
pub fn expect_exception<T: std::fmt::Debug>(result: Result<T, Error>, class: Class, message: &str) {
    match result {
        Ok(v) => panic!("expected {} ({message:?}), got Ok({v:?})", class.name()),
        Err(e) => {
            assert!(e.class().is_a(&class), "expected {}, got {e}", class.name());
            let actual = String::from_utf8_lossy(e.get_message());
            assert!(
                actual.contains(message),
                "message {actual:?} does not contain {message:?}"
            );
        }
    }
}

/// `expectException($class)` alone.
#[track_caller]
pub fn expect_class<T: std::fmt::Debug>(result: Result<T, Error>, class: Class) {
    expect_exception(result, class, "");
}

/// The fixtures' enums, registered as PHP would load them.
pub fn enums() -> Registry {
    let mut registry = Registry::empty();
    for class in ["TestEnum", "TestEnum2"] {
        let fqcn = format!("{FIXTURES}\\{class}");
        registry.define_enum(
            &fqcn,
            ["Foo", "Bar", "Ccc"]
                .into_iter()
                .map(|name| EnumCase::new(fqcn.clone(), name, None))
                .collect(),
        );
    }
    let backed = |class: &str, cases: Vec<(&str, Value)>| -> (String, Vec<EnumCase>) {
        let fqcn = format!("{FIXTURES}\\{class}");
        let cases = cases
            .into_iter()
            .map(|(name, value)| EnumCase::new(fqcn.clone(), name, Some(value)))
            .collect();
        (fqcn, cases)
    };
    for (fqcn, cases) in [
        backed(
            "StringBackedTestEnum",
            vec![
                ("Foo", Value::from("foo")),
                ("BarBaz", Value::from("bar baz")),
            ],
        ),
        backed(
            "StringBackedTestEnum2",
            vec![("Foo", Value::from("foo")), ("Bar", Value::from("bar"))],
        ),
        backed(
            "IntegerBackedTestEnum",
            vec![("One", Value::Int(1)), ("Two", Value::Int(2))],
        ),
    ] {
        registry.define_enum(&fqcn, cases);
    }
    registry
}

/// `\Symfony\Component\Config\Tests\Fixtures\<Enum>::<Case>`.
pub fn case(registry: &Registry, name: &str) -> Value {
    registry
        .constant(&format!("{FIXTURES}\\{name}"))
        .cloned()
        .unwrap_or_else(|| panic!("no enum case {name}"))
}

/// The fully qualified name of a fixture enum.
pub fn fixture(class: &str) -> String {
    format!("{FIXTURES}\\{class}")
}
