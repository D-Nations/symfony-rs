//! Tests/Compiler/CheckDefinitionValidityPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; a data provider's cases run in one test, in order. The
//! anonymous `new class {}` attribute values are `stdClass` objects.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckDefinitionValidityPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, object::StdClass};
use symfony_dependency_injection::compiler::{CheckDefinitionValidityPass, CompilerPassInterface};
use symfony_dependency_injection::{ContainerBuilder, DefinitionError, ErrorKind, Result};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    CheckDefinitionValidityPass.process(container)
}

fn array(entries: Vec<(&str, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (k, v) in entries {
        array.insert(k, v);
    }
    array
}

fn object() -> Value {
    StdClass::object(PhpArray::new())
}

#[test]
fn testProcessDetectsSyntheticNonPublicDefinitions() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .set_synthetic(true)
        .set_public(false);

    let e = process(&mut container).unwrap_err();
    assert!(e.instance_of("Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException"));
}

#[test]
fn testProcessDetectsNonSyntheticNonAbstractDefinitionWithoutClass() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .set_synthetic(false)
        .set_abstract(false);

    let e = process(&mut container).unwrap_err();
    assert!(e.instance_of("Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException"));
}

#[test]
fn testProcessAcceptsServiceLocatorWithoutClass() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_tag("container.service_locator", PhpArray::new());

    process(&mut container).unwrap();
}

#[test]
fn testProcessDetectsFactoryWithoutClass() {
    let mut container = ContainerBuilder::new();

    container
        .register(".123_anonymous_service_id_should_not_throw_~1234567", None)
        .unwrap()
        .set_factory("factory");
    process(&mut container).unwrap();

    container
        .register(".any_non_anonymous_id_throws", None)
        .unwrap()
        .set_factory("factory");

    let e = process(&mut container).unwrap_err();
    assert!(e.instance_of("Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException"));
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    container.register("a", Some("class")).unwrap();
    container
        .register("b", Some("class"))
        .unwrap()
        .set_synthetic(true);
    container
        .register("c", Some("class"))
        .unwrap()
        .set_abstract(true);

    process(&mut container).unwrap();
}

#[test]
fn testValidTags() {
    let mut container = ContainerBuilder::new();
    let tag = |container: &mut ContainerBuilder, id: &str, attributes: PhpArray| {
        container
            .register(id, Some("class"))
            .unwrap()
            .add_tag("foo", attributes);
    };
    tag(
        &mut container,
        "a",
        array(vec![("bar", Value::from("baz"))]),
    );
    tag(&mut container, "b", array(vec![("bar", Value::Null)]));
    tag(&mut container, "c", array(vec![("bar", Value::Int(1))]));
    tag(&mut container, "d", array(vec![("bar", Value::Float(1.1))]));
    tag(
        &mut container,
        "d",
        array(vec![(
            "bar",
            Value::Array(array(vec![("baz", Value::from("baz"))])),
        )]),
    );
    tag(
        &mut container,
        "e",
        array(vec![(
            "deep",
            Value::Array(array(vec![(
                "foo",
                Value::Array(array(vec![("bar", Value::from("baz"))])),
            )])),
        )]),
    );

    process(&mut container).unwrap();
}

#[test]
fn testInvalidTags() {
    let message = |tag: &str, attribute: &str| {
        format!(
            "A \"tags\" attribute must be of a scalar-type for service \"a\", tag \"{tag}\", attribute \"{attribute}\"."
        )
    };
    let cases = [
        // object attribute value
        ("foo", array(vec![("bar", object())]), message("foo", "bar")),
        // nested object attribute value
        (
            "foo",
            array(vec![("bar", Value::Array(array(vec![("baz", object())])))]),
            message("foo", "bar.baz"),
        ),
        // deeply nested object attribute value
        (
            "foo",
            array(vec![(
                "bar",
                Value::Array(array(vec![(
                    "baz",
                    Value::Array(array(vec![("qux", object())])),
                )])),
            )]),
            message("foo", "bar.baz.qux"),
        ),
    ];
    for (name, attributes, message) in cases {
        let mut container = ContainerBuilder::new();
        container
            .register("a", Some("class"))
            .unwrap()
            .add_tag(name, attributes);

        let e = process(&mut container).unwrap_err();
        assert!(
            e.instance_of("Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException")
        );
        assert_eq!(message, e.message());
    }
}

fn env(container: &ContainerBuilder) -> String {
    container
        .get_parameter_bag()
        .get("env(BAR)")
        .unwrap()
        .to_php_string()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn testDynamicPublicServiceName() {
    let mut container = ContainerBuilder::new();
    let env = env(&container);
    container
        .register(&format!("foo.{env}"), Some("class"))
        .unwrap()
        .set_public(true);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::EnvParameter));
}

#[test]
fn testDynamicPublicAliasName() {
    let mut container = ContainerBuilder::new();
    let env = env(&container);
    container
        .set_alias(&format!("foo.{env}"), "class")
        .unwrap()
        .set_public(true);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::EnvParameter));
}

#[test]
fn testDynamicPrivateName() {
    let mut container = ContainerBuilder::new();
    let env = env(&container);
    container
        .register(&format!("foo.{env}"), Some("class"))
        .unwrap();
    container.set_alias(&format!("bar.{env}"), "class").unwrap();

    process(&mut container).unwrap();
}

#[test]
fn testProcessSkipsDefinitionsWithErrors() {
    let mut container = ContainerBuilder::new();
    let definition = container.register("a", None).unwrap();
    definition.set_synthetic(true).set_public(false);
    definition.add_error(DefinitionError::Text("Some error message".to_string()));

    // This should not throw an exception because the definition has errors
    process(&mut container).unwrap();
}

/// The messages of the class checks.
#[test]
fn class_messages() {
    let mut container = ContainerBuilder::new();
    container.register("a", None).unwrap();
    assert_eq!(
        "The definition for \"a\" has no class. If you intend to inject this service dynamically at runtime, please mark it as synthetic=true. If this is an abstract definition solely used by child definitions, please add abstract=true, otherwise specify a class to get rid of this error.",
        process(&mut container).unwrap_err().message()
    );

    let mut container = ContainerBuilder::new();
    container
        .register("b", None)
        .unwrap()
        .set_factory("factory");
    assert_eq!(
        "Please add the class to service \"b\" even if it is constructed by a factory since we might need to add method calls based on compile-time checks.",
        process(&mut container).unwrap_err().message()
    );
}
