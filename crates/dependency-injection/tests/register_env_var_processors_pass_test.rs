//! Tests/Compiler/RegisterEnvVarProcessorsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. `SimpleProcessor` and `BadProcessor` are class metadata
//! whose `getProvidedTypes()` gives its result.
//!
//! testSimpleProcessor drops one assertion: it gets `foo` from the
//! locator, which needs PHP object creation; the locator alias is
//! checked instead.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/RegisterEnvVarProcessorsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{Value, php_array};
use symfony_dependency_injection::class_metadata::{ClassMetadata, ClassRegistry, MethodMetadata};
use symfony_dependency_injection::compiler::{CompilerPassInterface, RegisterEnvVarProcessorsPass};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";
const INTERFACE: &str = "Symfony\\Component\\DependencyInjection\\EnvVarProcessorInterface";

fn container() -> ContainerBuilder {
    let provided = |types: &str| {
        MethodMetadata::public("getProvidedTypes")
            .with_static(true)
            .with_return_type("array")
            .with_returns(Value::Array(php_array!["foo" => types]))
    };
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::interface(INTERFACE));
    registry.add_class(
        ClassMetadata::class(&format!("{NS}SimpleProcessor"))
            .with_interfaces(&[INTERFACE])
            .with_method(provided("string")),
    );
    registry.add_class(
        ClassMetadata::class(&format!("{NS}BadProcessor"))
            .with_parent(&format!("{NS}SimpleProcessor"))
            .with_method(provided("string|foo")),
    );
    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));
    container
}

#[test]
fn testSimpleProcessor() {
    let mut container = container();
    container
        .register("foo", Some(&format!("{NS}SimpleProcessor")))
        .unwrap()
        .add_tag("container.env_var_processor", php_value::PhpArray::new());

    RegisterEnvVarProcessorsPass
        .process(&mut container)
        .unwrap();

    assert!(container.has("container.env_var_processors_locator"));
    assert!(
        container
            .get_alias("container.env_var_processors_locator")
            .unwrap()
            .is_public()
    );

    let all = ["bool", "int", "float", "string", "array"];
    let expected: Vec<(&str, Vec<&str>)> = vec![
        ("foo", vec!["string"]),
        ("base64", vec!["string"]),
        ("bool", vec!["bool"]),
        ("not", vec!["bool"]),
        ("const", all.to_vec()),
        ("csv", vec!["array"]),
        ("file", vec!["string"]),
        ("float", vec!["float"]),
        ("int", vec!["int"]),
        ("json", vec!["array"]),
        ("key", all.to_vec()),
        ("url", vec!["array"]),
        ("query_string", vec!["array"]),
        ("resolve", vec!["string"]),
        ("default", all.to_vec()),
        ("string", vec!["string"]),
        ("trim", vec!["string"]),
        ("require", all.to_vec()),
        ("enum", vec!["BackedEnum"]),
        ("shuffle", vec!["array"]),
        ("defined", vec!["bool"]),
        ("urlencode", vec!["string"]),
    ];
    let actual: Vec<(String, Vec<String>)> = container
        .get_parameter_bag()
        .get_provided_types()
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    let expected: Vec<(String, Vec<String>)> = expected
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.into_iter().map(String::from).collect()))
        .collect();
    assert_eq!(expected, actual);
}

#[test]
fn testNoProcessor() {
    let mut container = container();

    RegisterEnvVarProcessorsPass
        .process(&mut container)
        .unwrap();

    assert!(!container.has("container.env_var_processors_locator"));
}

#[test]
fn testBadProcessor() {
    let mut container = container();
    container
        .register("foo", Some(&format!("{NS}BadProcessor")))
        .unwrap()
        .add_tag("container.env_var_processor", php_value::PhpArray::new());

    let e = RegisterEnvVarProcessorsPass
        .process(&mut container)
        .unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Invalid type \"foo\" returned by \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\BadProcessor::getProvidedTypes()\", expected one of \"array\", \"bool\", \"float\", \"int\", \"string\", \"BackedEnum\".",
        e.message()
    );
}
