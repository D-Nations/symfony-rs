//! Tests/Compiler/AutowireRequiredPropertiesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! The fixture classes of `autowiring_classes.php` are given as class
//! metadata. Upstream first runs `ResolveClassPass`, which another port
//! owns; it changes nothing here since every definition names its class.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AutowireRequiredPropertiesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key};
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry, PropertyMetadata, Visibility,
};
use symfony_dependency_injection::compiler::{
    AutowireRequiredPropertiesPass, CompilerPassInterface,
};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind, Reference};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";
const REQUIRED: &str = "Symfony\\Contracts\\Service\\Attribute\\Required";
const TARGET: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Target";

fn c(short: &str) -> String {
    format!("{NS}{short}")
}

fn attribute(class: &str, arguments: PhpArray) -> AttributeMetadata {
    AttributeMetadata {
        class: class.to_string(),
        arguments,
    }
}

fn foo_property(visibility: Visibility, is_readonly: bool) -> PropertyMetadata {
    PropertyMetadata {
        name: "foo".to_string(),
        visibility,
        is_readonly,
        type_: Some(c("Foo")),
        attributes: vec![attribute(REQUIRED, PhpArray::new())],
        ..PropertyMetadata::default()
    }
}

fn with_property(mut class: ClassMetadata, property: PropertyMetadata) -> ClassMetadata {
    class.properties.push(property);
    class
}

fn fixtures() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    r.add_class(ClassMetadata::class(&c("Foo")));
    r.add_class(with_property(
        ClassMetadata::class(&c("AutowireProperty")),
        foo_property(Visibility::Public, false),
    ));
    let mut target = foo_property(Visibility::Public, false);
    target.attributes.push(attribute(
        TARGET,
        PhpArray::list([Value::from("foo.target")]),
    ));
    r.add_class(with_property(
        ClassMetadata::class(&c("AutowirePropertyWithTarget")),
        target,
    ));
    r.add_class(with_property(
        ClassMetadata::class(&c("AutowireReadonlyProperty")),
        foo_property(Visibility::Public, true),
    ));
    r.add_class(with_property(
        ClassMetadata::class(&c("AutowirePrivateProperty")),
        foo_property(Visibility::Private, false),
    ));
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

#[test]
fn testAttribute() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();

    container
        .register("property_injection", Some(&c("AutowireProperty")))
        .unwrap()
        .set_autowired(true);

    AutowireRequiredPropertiesPass::new()
        .process(&mut container)
        .unwrap();

    let properties = container
        .get_definition("property_injection")
        .unwrap()
        .properties();

    assert!(properties.contains_key(&Key::from("foo")));
    let foo = Reference::from_value(properties.get(&Key::from("foo")).unwrap()).unwrap();
    assert_eq!(c("Foo"), foo.to_string());
}

#[test]
fn testTargetAttributeIsPropagatedOnRequiredProperty() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();

    container
        .register(
            "property_injection_with_target",
            Some(&c("AutowirePropertyWithTarget")),
        )
        .unwrap()
        .set_autowired(true);

    AutowireRequiredPropertiesPass::new()
        .process(&mut container)
        .unwrap();

    let properties = container
        .get_definition("property_injection_with_target")
        .unwrap()
        .properties();

    assert!(properties.contains_key(&Key::from("foo")));
    let foo = Reference::from_value(properties.get(&Key::from("foo")).unwrap()).unwrap();
    assert!(foo.type_().is_some(), "a TypedReference");
    let attributes: Vec<&Value> = foo
        .attributes()
        .unwrap()
        .values()
        .filter(|a| a.as_object().is_some_and(|o| o.class() == TARGET))
        .collect();
    assert_eq!(1, attributes.len());
    let name = attributes[0]
        .as_object()
        .unwrap()
        .properties()
        .get(&Key::from("name"))
        .cloned()
        .unwrap();
    assert!(Value::from("foo.target").identical(&name));
}

#[test]
fn testAttributeWithReadonlyProperty() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();
    container
        .register("property_injection", Some(&c("AutowireReadonlyProperty")))
        .unwrap()
        .set_autowired(true);

    let error = AutowireRequiredPropertiesPass::new()
        .process(&mut container)
        .unwrap_err();
    assert_eq!(&ErrorKind::InvalidArgument, error.kind());
    assert_eq!(
        "Cannot autowire non-public(set) property \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\AutowireReadonlyProperty::$foo\" with #[Symfony\\Contracts\\Service\\Attribute\\Required].",
        error.message()
    );
}

#[test]
fn testAttributeWithPrivateProperty() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();
    container
        .register("property_injection", Some(&c("AutowirePrivateProperty")))
        .unwrap()
        .set_autowired(true);

    let error = AutowireRequiredPropertiesPass::new()
        .process(&mut container)
        .unwrap_err();
    assert_eq!(&ErrorKind::InvalidArgument, error.kind());
    assert_eq!(
        "Cannot autowire non-public(set) property \"Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\AutowirePrivateProperty::$foo\" with #[Symfony\\Contracts\\Service\\Attribute\\Required].",
        error.message()
    );
}
