//! Tests/Compiler/ServiceLocatorTagPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Pending, by reason:
//!
//! - they read the locator through `$container->get()`, which needs PHP
//!   object creation (the Rust emitter replaces it): testScalarServices,
//!   testProcessValue, testServiceWithKeyOverwritesPreviousInheritedKey,
//!   testInheritedKeyOverwritesPreviousServiceWithKey, testTaggedServices,
//!   testTaggedServicesKeysAreKept, testNestedServiceLocatorArgument;
//! - they also call `compile()`, which stops at the first pass that is
//!   not ported yet: testIndexedByServiceIdWithDecoration,
//!   testIndexedByAsTaggedItemWithDecoration,
//!   testExcludeSelfFromTaggedServiceLocator.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ServiceLocatorTagPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::{
    BoundArgument, DEFAULTS_BINDING, ServiceLocatorArgument,
};
use symfony_dependency_injection::compiler::{CompilerPassInterface, ServiceLocatorTagPass};
use symfony_dependency_injection::service_locator::SERVICE_LOCATOR_CLASS;
use symfony_dependency_injection::{ContainerBuilder, ErrorKind, Reference};

fn keys(array: &PhpArray) -> Vec<String> {
    array
        .keys()
        .map(|k| String::from_utf8_lossy(&k.bytes()).into_owned())
        .collect()
}

fn factory_service(container: &ContainerBuilder, id: &str) -> String {
    let factory = container.get_definition(id).unwrap().factory();
    let Value::Array(factory) = factory else {
        panic!("not a factory array");
    };
    Reference::from_value(factory.get(&Key::Int(0)).unwrap())
        .unwrap()
        .id()
        .to_string()
}

#[test]
fn testNoServices() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", Some(SERVICE_LOCATOR_CLASS))
        .unwrap()
        .add_tag("container.service_locator", PhpArray::new());

    let e = ServiceLocatorTagPass::new()
        .process(&mut container)
        .unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Invalid definition for service \"foo\": an array of references is expected as first argument when the \"container.service_locator\" tag is set.",
        e.message()
    );
}

#[test]
fn testServiceListIsOrdered() {
    let mut container = ContainerBuilder::new();

    container.register("bar", Some("CustomDefinition")).unwrap();
    container.register("baz", Some("CustomDefinition")).unwrap();

    container
        .register("foo", Some(SERVICE_LOCATOR_CLASS))
        .unwrap()
        .set_arguments(PhpArray::list([Value::Array(PhpArray::list([
            Value::from(Reference::new("baz")),
            Value::from(Reference::new("bar")),
        ]))]))
        .add_tag("container.service_locator", PhpArray::new());

    ServiceLocatorTagPass::new()
        .process(&mut container)
        .unwrap();

    let Value::Array(argument) = container
        .get_definition("foo")
        .unwrap()
        .argument(0)
        .unwrap()
    else {
        panic!("not an array");
    };
    assert_eq!(vec!["bar", "baz"], keys(&argument));
}

#[test]
fn testBindingsAreCopied() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", None)
        .unwrap()
        .set_bindings(&php_array!["foo" => "foo"]);

    let locator = ServiceLocatorTagPass::register(
        &mut container,
        php_array!["foo" => Value::from(Reference::new("foo"))],
        Some("foo"),
    )
    .unwrap();
    let locator = factory_service(&container, locator.id());
    let locator = container.get_definition(&locator).unwrap();

    assert_eq!(vec!["foo"], keys(&locator.bindings()));
    assert!(
        BoundArgument::from_value(locator.bindings().get(&Key::from("foo")).unwrap()).is_some()
    );
}

#[test]
fn testLocatorIdDoesNotDependOnTheFileDeclaringBindings() {
    assert_eq!(
        register_locator_with_binding(BoundArgument::new(
            Value::from("bar"),
            true,
            DEFAULTS_BINDING,
            Some("/project-a/config/services.yaml".to_string())
        )),
        register_locator_with_binding(BoundArgument::new(
            Value::from("bar"),
            true,
            DEFAULTS_BINDING,
            Some("/project-b/config/services.yaml".to_string())
        ))
    );
}

#[test]
fn testLocatorIdDoesNotDependOnPreviouslyCreatedBindings() {
    let id = register_locator_with_binding(BoundArgument::new(
        Value::from("bar"),
        true,
        DEFAULTS_BINDING,
        None,
    ));
    BoundArgument::new(Value::from("baz"), true, 0, None);

    assert_eq!(
        id,
        register_locator_with_binding(BoundArgument::new(
            Value::from("bar"),
            true,
            DEFAULTS_BINDING,
            None
        ))
    );
}

#[test]
fn testLocatorsAreSharedOnlyByServicesSharingTheSameBindings() {
    let mut container = ContainerBuilder::new();
    let bindings = php_array![
        "$bar" => BoundArgument::new(Value::from("bar"), true, DEFAULTS_BINDING, None).to_value()
    ];
    container
        .register("foo1", None)
        .unwrap()
        .set_bindings(&bindings);
    container
        .register("foo2", None)
        .unwrap()
        .set_bindings(&bindings);
    container.register("foo3", None).unwrap().set_bindings(&php_array![
        "$bar" => BoundArgument::new(Value::from("bar"), true, DEFAULTS_BINDING, None).to_value()
    ]);

    let mut locators = Vec::new();
    for id in ["foo1", "foo2", "foo3"] {
        let locator = ServiceLocatorTagPass::register(
            &mut container,
            php_array!["baz" => Value::from(Reference::new("baz"))],
            Some(id),
        )
        .unwrap();
        locators.push(factory_service(&container, locator.id()));
    }

    assert_eq!(locators[0], locators[1]);
    assert_ne!(locators[0], locators[2]);
    assert!(
        Value::Array(container.get_definition("foo3").unwrap().bindings()).identical(
            &Value::Array(container.get_definition(&locators[2]).unwrap().bindings())
        )
    );
}

#[test]
fn testServicesKeysAreKept() {
    let mut container = ContainerBuilder::new();
    container.register("service-1", None).unwrap();
    container.register("service-2", None).unwrap();
    container.register("service-3", None).unwrap();

    let mut map = PhpArray::new();
    map.insert(0, Value::from(Reference::new("service-1")));
    map.insert("service-2", Value::from(Reference::new("service-2")));
    map.insert("foo", Value::from(Reference::new("service-3")));
    let locator = ServiceLocatorTagPass::register(&mut container, map, None).unwrap();
    let locator = container.get_definition(locator.id()).unwrap();
    let Some(Value::Array(factories)) = locator.arguments().get(&Key::Int(0)).cloned() else {
        panic!("not an array");
    };

    assert_eq!(
        vec![Key::Int(0), Key::from("service-2"), Key::from("foo")],
        factories.keys().cloned().collect::<Vec<Key>>()
    );
}

#[test]
fn testDefinitionOrderIsTheSame() {
    let mut container = ContainerBuilder::new();
    container.register("service-1", None).unwrap();
    container.register("service-2", None).unwrap();

    let locator = ServiceLocatorTagPass::register(
        &mut container,
        php_array![
            "service-2" => Value::from(Reference::new("service-2")),
            "service-1" => Value::from(Reference::new("service-1")),
        ],
        None,
    )
    .unwrap();
    let locator = container.get_definition(locator.id()).unwrap();
    let Some(Value::Array(factories)) = locator.arguments().get(&Key::Int(0)).cloned() else {
        panic!("not an array");
    };

    assert_eq!(vec!["service-2", "service-1"], keys(&factories));
}

#[test]
fn testBindingsAreProcessed() {
    let mut container = ContainerBuilder::new();

    let definition = container.register("foo", None).unwrap();
    definition.set_bindings(
        &php_array!["foo" => ServiceLocatorArgument::new(PhpArray::new()).to_value()],
    );

    ServiceLocatorTagPass::new()
        .process(&mut container)
        .unwrap();

    let binding =
        BoundArgument::from_value(definition.bindings().get(&Key::from("foo")).unwrap()).unwrap();
    assert!(Reference::from_value(binding.values().get(&Key::Int(0)).unwrap()).is_some());
}

fn register_locator_with_binding(binding: BoundArgument) -> String {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", None)
        .unwrap()
        .set_bindings(&php_array!["$bar" => binding.to_value()]);

    ServiceLocatorTagPass::register(
        &mut container,
        php_array!["baz" => Value::from(Reference::new("baz"))],
        Some("foo"),
    )
    .unwrap()
    .id()
    .to_string()
}
