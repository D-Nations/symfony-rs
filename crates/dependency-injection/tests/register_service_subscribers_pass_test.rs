//! Tests/Compiler/RegisterServiceSubscribersPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The fixture subscribers are class metadata whose
//! `getSubscribedServices()` gives its result.
//!
//! Pending, by reason:
//!
//! - they read the locator after `ResolveServiceSubscribersPass` (or
//!   `ResolveBindingsPass`, `AutowirePass`), not ported yet:
//!   testNoAttributes, testWithAttributes, testUnionServices,
//!   testIntersectionServices, testServiceMethodsSubscriberTraitWithSubscribedServiceAttribute,
//!   testServiceMethodsSubscriberTraitWithUnionReturnType,
//!   testServiceMethodsSubscriberTraitWithIntersectionReturnType,
//!   testServiceSubscriberWithSemanticId, testSubscribedServiceWithAttributes,
//!   testBinding;
//! - `compile()` stops at the first pass that is not ported yet:
//!   testExtraServiceSubscriber;
//! - they test service-contracts' `ServiceMethodsSubscriberTrait`, which
//!   runs PHP: testServiceMethodsSubscriberTraitWithSubscribedServiceAttributeOnStaticMethod,
//!   testServiceMethodsSubscriberTraitWithSubscribedServiceAttributeOnMethodWithRequiredParameters,
//!   testServiceMethodsSubscriberTraitWithSubscribedServiceAttributeOnMethodMissingReturnType.
//!
//! `no_attributes_locator` and `semantic_id_locator` check the locators
//! of testNoAttributes and testServiceSubscriberWithSemanticId as this
//! pass leaves them (through the `container.service_subscriber.locator`
//! tag).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/RegisterServiceSubscribersPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::InvalidBehavior::{Exception, Ignore};
use symfony_dependency_injection::argument::ServiceClosureArgument;
use symfony_dependency_injection::class_metadata::{ClassMetadata, ClassRegistry, MethodMetadata};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, RegisterServiceSubscribersPass,
};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind, InvalidBehavior, Reference};

const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const SUBSCRIBER: &str = "Symfony\\Contracts\\Service\\ServiceSubscriberInterface";

fn f(short: &str) -> String {
    format!("{FIXTURES}{short}")
}

fn subscriber(class: &str, services: PhpArray) -> ClassMetadata {
    ClassMetadata::class(class)
        .with_interfaces(&[SUBSCRIBER])
        .with_method(
            MethodMetadata::public("getSubscribedServices")
                .with_static(true)
                .with_return_type("array")
                .with_returns(Value::Array(services)),
        )
}

fn container() -> ContainerBuilder {
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::interface(SUBSCRIBER));
    registry.add_class(ClassMetadata::class(&f("CustomDefinition")));
    registry.add_class(ClassMetadata::class("stdClass"));
    let custom = f("CustomDefinition");
    let mut services = PhpArray::list([
        Value::from(f("TestServiceSubscriber")),
        Value::from(format!("?{custom}")),
    ]);
    services.insert("bar", Value::from(custom.as_str()));
    services.insert("baz", Value::from(format!("?{custom}")));
    services.insert("late_alias", Value::from(f("TestDefinition1")));
    registry.add_class(subscriber(&f("TestServiceSubscriber"), services));
    registry.add_class(subscriber(
        "SemanticIdSubscriber",
        php_array![
            "some.service" => "stdClass",
            "some_service" => "stdClass",
            "another_service" => "stdClass",
        ],
    ));
    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));
    container
}

/// The locator's factories, through the subscriber's locator tag and the
/// context locator's factory.
fn locator_factories(container: &ContainerBuilder, id: &str) -> PhpArray {
    let tag = container
        .get_definition(id)
        .unwrap()
        .tag("container.service_subscriber.locator");
    let Some(Value::Array(tag)) = tag.get(&Key::Int(0)) else {
        panic!("no locator tag");
    };
    let locator = tag.get(&Key::from("id")).unwrap().to_php_string().unwrap();
    let locator = container
        .get_definition(&locator.to_string_lossy())
        .unwrap();
    let Value::Array(factory) = locator.factory() else {
        panic!("no factory");
    };
    let prototype = Reference::from_value(factory.get(&Key::Int(0)).unwrap()).unwrap();
    match container
        .get_definition(prototype.id())
        .unwrap()
        .argument(0)
        .unwrap()
    {
        Value::Array(a) => a,
        _ => panic!("not an array"),
    }
}

#[track_caller]
fn assert_factories(expected: &[(&str, Reference)], actual: &PhpArray) {
    let keys: Vec<String> = actual
        .keys()
        .map(|k| String::from_utf8_lossy(&k.bytes()).into_owned())
        .collect();
    assert_eq!(expected.iter().map(|(k, _)| *k).collect::<Vec<_>>(), keys);
    for ((_, reference), closure) in expected.iter().zip(actual.values()) {
        let closure = ServiceClosureArgument::from_value(closure).unwrap();
        assert_eq!(Some(reference), Reference::from_value(&closure.value()));
    }
}

fn typed(id: &str, type_: &str, behavior: InvalidBehavior, name: Option<&str>) -> Reference {
    Reference::typed(id, type_, behavior, name.map(String::from), PhpArray::new())
}

#[test]
fn testInvalidClass() {
    let mut container = container();

    container
        .register("foo", Some(&f("CustomDefinition")))
        .unwrap()
        .add_tag("container.service_subscriber", PhpArray::new());

    let e = RegisterServiceSubscribersPass::new()
        .process(&mut container)
        .unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Service \"foo\" must implement interface \"Symfony\\Contracts\\Service\\ServiceSubscriberInterface\".",
        e.message()
    );
}

#[test]
fn testInvalidAttributes() {
    let mut container = container();

    container
        .register("foo", Some(&f("TestServiceSubscriber")))
        .unwrap()
        .add_tag("container.service_subscriber", php_array!["bar" => "123"]);

    let e = RegisterServiceSubscribersPass::new()
        .process(&mut container)
        .unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "The \"container.service_subscriber\" tag accepts only the \"key\" and \"id\" attributes, \"bar\" given for service \"foo\".",
        e.message()
    );
}

#[test]
fn no_attributes_locator() {
    let mut container = container();

    container
        .register("foo", Some(&f("TestServiceSubscriber")))
        .unwrap()
        .add_argument(Reference::new("Psr\\Container\\ContainerInterface"))
        .add_tag("container.service_subscriber", PhpArray::new());

    RegisterServiceSubscribersPass::new()
        .process(&mut container)
        .unwrap();

    let (subscriber, custom, def1) = (
        f("TestServiceSubscriber"),
        f("CustomDefinition"),
        f("TestDefinition1"),
    );
    assert_factories(
        &[
            (
                &subscriber,
                typed(&subscriber, &subscriber, Exception, None),
            ),
            (&custom, typed(&custom, &custom, Ignore, None)),
            ("bar", typed(&custom, &custom, Exception, Some("bar"))),
            ("baz", typed(&custom, &custom, Ignore, Some("baz"))),
            (
                "late_alias",
                typed(&def1, &def1, Exception, Some("late_alias")),
            ),
        ],
        &locator_factories(&container, "foo"),
    );
    let bindings = container.get_definition("foo").unwrap().bindings();
    assert!(bindings.contains_key(&Key::from("Psr\\Container\\ContainerInterface")));
}

#[test]
fn semantic_id_locator() {
    let mut container = container();

    container
        .register("some.service", Some("stdClass"))
        .unwrap();
    container
        .set_alias("stdClass $someService", "some.service")
        .unwrap();
    container
        .set_alias("stdClass $some_service", "some.service")
        .unwrap();
    container
        .set_alias("stdClass $anotherService", "some.service")
        .unwrap();
    container
        .register("foo", Some("SemanticIdSubscriber"))
        .unwrap()
        .add_tag("container.service_subscriber", PhpArray::new());

    RegisterServiceSubscribersPass::new()
        .process(&mut container)
        .unwrap();

    assert_factories(
        &[
            (
                "some.service",
                typed("stdClass", "stdClass", Exception, Some("some.service")),
            ),
            (
                "some_service",
                typed("stdClass", "stdClass", Exception, Some("some_service")),
            ),
            (
                "another_service",
                typed("stdClass", "stdClass", Exception, Some("anotherService")),
            ),
        ],
        &locator_factories(&container, "foo"),
    );
}
