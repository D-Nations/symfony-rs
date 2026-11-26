//! Tests/Compiler/AttributeAutoconfigurationPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! A configurator's third parameter, which upstream reflects, is given at
//! registration (`register_attribute_for_autoconfiguration_with_reflector`).
//! `TestServiceWithAttributes` and `stdClass` are given as class metadata.
//! The extras after the upstream tests check the method, property and
//! parameter dispatch, which upstream tests elsewhere.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AttributeAutoconfigurationPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use php_value::vectors::phpunit_equals;
use php_value::{PhpArray, Value};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata,
    PropertyMetadata, Reflector,
};
use symfony_dependency_injection::compiler::{
    AttributeAutoconfigurationPass, CompilerPassInterface,
};
use symfony_dependency_injection::container_builder::{
    AttributeConfigurator, AttributeConfiguratorReflector,
};
use symfony_dependency_injection::{ContainerBuilder, Definition, ErrorKind};

const AS_TAGGED_ITEM: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\AsTaggedItem";
const TEST_SERVICE: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\TestServiceWithAttributes";

fn as_tagged_item() -> AttributeMetadata {
    AttributeMetadata {
        class: AS_TAGGED_ITEM.to_string(),
        arguments: PhpArray::new(),
    }
}

fn fixtures() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    r.add_class(ClassMetadata::class("stdClass"));
    let mut service = ClassMetadata::class(TEST_SERVICE);
    service.attributes = vec![as_tagged_item()];
    r.add_class(service);
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

fn typed(types: &[&str]) -> AttributeConfiguratorReflector {
    AttributeConfiguratorReflector::Typed(types.iter().map(|t| (*t).to_string()).collect())
}

#[test]
fn testProcessAddsNoEmptyInstanceofConditionals() {
    let mut container = container();
    container.register_attribute_for_autoconfiguration(
        AS_TAGGED_ITEM,
        Arc::new(|_: &Definition, _: &AttributeMetadata, _: &Reflector| Ok(())),
    );
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .set_autoconfigured(true);

    AttributeAutoconfigurationPass::new()
        .process(&mut container)
        .unwrap();

    assert!(
        container
            .get_definition("foo")
            .unwrap()
            .instanceof_conditionals()
            .is_empty()
    );
}

#[test]
fn testAttributeConfiguratorCallableMissingType() {
    let mut container = container();
    container.register_attribute_for_autoconfiguration_with_reflector(
        AS_TAGGED_ITEM,
        Arc::new(|_: &Definition, _: &AttributeMetadata, _: &Reflector| Ok(())),
        AttributeConfiguratorReflector::Untyped {
            name: "reflector".to_string(),
            file: "AttributeAutoconfigurationPassTest.php".to_string(),
            line: 41,
        },
    );
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .set_autoconfigured(true);

    let error = AttributeAutoconfigurationPass::new()
        .process(&mut container)
        .unwrap_err();
    assert_eq!(&ErrorKind::Logic, error.kind());
    assert!(error.message().contains("Argument \"$reflector\" of attribute autoconfigurator should have a type, use one or more of \"\\ReflectionClass|\\ReflectionMethod|\\ReflectionProperty|\\ReflectionParameter|\\Reflector\" in "));
}

#[test]
fn testProcessesAbstractServicesWithContainerExcludedTag() {
    let mut container = container();
    container.register_attribute_for_autoconfiguration_with_reflector(
        AS_TAGGED_ITEM,
        Arc::new(
            |definition: &Definition, _: &AttributeMetadata, _: &Reflector| {
                definition.add_tag("processed.tag", PhpArray::new());
                Ok(())
            },
        ),
        typed(&["ReflectionClass"]),
    );

    // Create an abstract service with container.excluded tag and attributes
    let abstract_service = container
        .register("abstract_service", Some(TEST_SERVICE))
        .unwrap()
        .clone();
    let mut source = PhpArray::new();
    source.insert("source", Value::from("test"));
    abstract_service
        .set_autoconfigured(true)
        .set_abstract(true)
        .add_tag("container.excluded", source);

    AttributeAutoconfigurationPass::new()
        .process(&mut container)
        .unwrap();

    // Abstract service with container.excluded tag should be processed
    let expected_conditional = Definition::child("");
    expected_conditional.add_tag("processed.tag", PhpArray::new());
    let mut expected = PhpArray::new();
    expected.insert(TEST_SERVICE, expected_conditional.to_value());
    assert!(phpunit_equals(
        &Value::Array(expected),
        &Value::Array(abstract_service.instanceof_conditionals())
    ));
}

fn recording(calls: &Arc<Mutex<Vec<String>>>) -> AttributeConfigurator {
    let calls = Arc::clone(calls);
    Arc::new(
        move |_: &Definition, _: &AttributeMetadata, reflector: &Reflector| {
            let name = match reflector {
                Reflector::Class(class) => format!("class {}", class.name),
                Reflector::Method { method, .. } => format!("method {}", method.name),
                Reflector::Property { property, .. } => format!("property {}", property.name),
                Reflector::Parameter { parameter, .. } => format!("parameter ${}", parameter.name),
            };
            calls.lock().unwrap().push(name);
            Ok(())
        },
    )
}

#[test]
fn configurators_are_called_for_the_reflectors_their_type_names() {
    let mut registry = ClassRegistry::new();
    let mut class = ClassMetadata::class("Service");
    class.attributes = vec![as_tagged_item()];
    let mut parameter = ParameterMetadata::new("a", Some("int"));
    parameter.attributes = vec![as_tagged_item()];
    let mut method = MethodMetadata::public("run").with_parameters(vec![parameter.clone()]);
    method.attributes = vec![as_tagged_item()];
    let mut private = MethodMetadata::public("hidden")
        .with_visibility(symfony_dependency_injection::class_metadata::Visibility::Private);
    private.attributes = vec![as_tagged_item()];
    let constructor = MethodMetadata::public("__construct").with_parameters(vec![parameter]);
    class = class
        .with_method(constructor)
        .with_method(method)
        .with_method(private);
    class.properties.push(PropertyMetadata {
        name: "p".to_string(),
        attributes: vec![as_tagged_item()],
        ..PropertyMetadata::default()
    });
    registry.add_class(class);
    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));

    let calls = Arc::new(Mutex::new(Vec::new()));
    container.register_attribute_for_autoconfiguration_with_reflector(
        AS_TAGGED_ITEM,
        recording(&calls),
        typed(&["ReflectionMethod", "ReflectionProperty"]),
    );
    container.register_attribute_for_autoconfiguration_with_reflector(
        AS_TAGGED_ITEM,
        recording(&calls),
        typed(&["\\Reflector"]),
    );
    container
        .register("s", Some("Service"))
        .unwrap()
        .set_autoconfigured(true);

    AttributeAutoconfigurationPass::new()
        .process(&mut container)
        .unwrap();

    assert_eq!(
        vec![
            "class Service",
            "parameter $a",
            "method run",
            "method run",
            "parameter $a",
            "property p",
            "property p",
        ],
        *calls.lock().unwrap()
    );
}
