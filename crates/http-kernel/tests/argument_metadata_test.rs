//! ArgumentMetadataTest.php at symfony/http-kernel v8.1.11. Names and
//! assertions are PHPUnit's; `new Foo('bar')` is a `Foo` object with its
//! `bar` property.
//!
//! Upstream: symfony/http-kernel Tests/ControllerMetadata/ArgumentMetadataTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{Value, object::GenericObject, php_array};
use symfony_http_kernel::{controller_metadata::ArgumentMetadata, exception::instance_of};

const FOO: &str = "Symfony\\Component\\HttpKernel\\Tests\\Fixtures\\Attribute\\Foo";

fn foo(value: &str) -> Value {
    Value::Object(Arc::new(GenericObject::new(
        FOO,
        php_array!["bar" => value],
    )))
}

fn same_objects(expected: &[Value], actual: &[Value]) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(e, a)| match (e, a) {
            (Value::Object(e), Value::Object(a)) => {
                e.class() == a.class() && e.properties() == a.properties()
            }
            _ => false,
        })
}

fn with_attributes(attributes: Vec<Value>) -> ArgumentMetadata {
    ArgumentMetadata::new(
        "foo",
        Some("string"),
        false,
        true,
        Value::string("default value"),
        true,
        attributes,
        "n/a",
    )
}

#[test]
fn testWithBcLayerWithDefault() {
    let argument = ArgumentMetadata::simple(
        "foo",
        Some("string"),
        false,
        true,
        Value::string("default value"),
    );

    assert!(!argument.is_nullable());
}

#[test]
fn testDefaultValueAvailable() {
    let argument = with_attributes(Vec::new());

    assert!(argument.is_nullable());
    assert!(argument.has_default_value());
    assert_eq!(
        Value::string("default value"),
        argument.get_default_value().unwrap()
    );
}

#[test]
fn testDefaultValueUnavailable() {
    let argument = ArgumentMetadata::new(
        "foo",
        Some("string"),
        false,
        false,
        Value::Null,
        false,
        Vec::new(),
        "n/a",
    );

    assert!(!argument.is_nullable());
    assert!(!argument.has_default_value());
    let e = argument.get_default_value().unwrap_err();
    assert!(instance_of(e.as_ref(), "LogicException"));
}

#[test]
fn testGetAttributes() {
    let argument = with_attributes(vec![foo("bar")]);
    assert!(same_objects(
        &[foo("bar")],
        &argument.get_attributes(None, 0)
    ));
}

#[test]
fn testGetAttributesOfType() {
    let argument = with_attributes(vec![foo("bar")]);
    assert!(same_objects(
        &[foo("bar")],
        &argument.get_attributes_of_type(FOO, 0)
    ));
}
