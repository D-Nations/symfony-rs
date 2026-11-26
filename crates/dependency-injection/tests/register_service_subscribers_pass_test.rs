//! Tests/Compiler/RegisterServiceSubscribersPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The fixture subscribers are class metadata whose
//! `getSubscribedServices()` gives its result.
//!
//! `assertEquals()` on the locator's factories is PHPUnit's array
//! comparison: the same keys, in any order, with equal values.
//! testExtraServiceSubscriber's `compile()` is `support::compile()`: the
//! error comes from this pass, before the pending autowire passes would
//! run on the subscriber.
//!
//! Pending, by reason:
//!
//! - they end with `AutowirePass`, not ported yet: testUnionServices,
//!   testServiceSubscriberWithSemanticId (`union_services_locator` and
//!   `semantic_id_locator` check their locators up to that pass);
//! - testSubscribedServiceWithAttributes: it also returns `SubscribedService`
//!   objects with `Autowire`, `AutowireDecorated` and `Target` attributes;
//! - they test service-contracts' `ServiceMethodsSubscriberTrait`, which
//!   runs PHP: testServiceMethodsSubscriberTraitWithSubscribedServiceAttributeOnStaticMethod,
//!   testServiceMethodsSubscriberTraitWithSubscribedServiceAttributeOnMethodWithRequiredParameters,
//!   testServiceMethodsSubscriberTraitWithSubscribedServiceAttributeOnMethodMissingReturnType.
//!   The fixtures that use the trait in the other tests are metadata
//!   whose `getSubscribedServices()` gives what the trait returns.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/RegisterServiceSubscribersPassTest.php
#![allow(
    non_snake_case,
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::disallowed_names
)]

mod support;

use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::InvalidBehavior::{Exception, Ignore};
use symfony_dependency_injection::argument::ServiceClosureArgument;
use symfony_dependency_injection::class_metadata::{
    ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata,
};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, RegisterServiceSubscribersPass, ResolveBindingsPass,
    ResolveServiceSubscribersPass,
};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, ErrorKind, InvalidBehavior, Reference, Result,
};

const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const SUBSCRIBER: &str = "Symfony\\Contracts\\Service\\ServiceSubscriberInterface";
const PSR_CONTAINER: &str = "Psr\\Container\\ContainerInterface";
const SERVICE_PROVIDER: &str = "Symfony\\Contracts\\Service\\ServiceProviderInterface";
const SERVICE_LOCATOR: &str = "Symfony\\Component\\DependencyInjection\\ServiceLocator";

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
    registry.add_class(
        subscriber(&f("TestServiceSubscriber"), services).with_method(
            MethodMetadata::public("setServiceProvider").with_parameters(vec![
                ParameterMetadata::new("container", Some(SERVICE_PROVIDER)),
            ]),
        ),
    );
    let (def1, def2, def3) = (
        f("TestDefinition1"),
        f("TestDefinition2"),
        f("TestDefinition3"),
    );
    let mut services = PhpArray::list([Value::from(format!("string|{def2}|{def1}"))]);
    services.insert("bar", Value::from(format!("{def1}|{def2}")));
    services.insert("baz", Value::from(format!("?{def1}|{def2}")));
    registry.add_class(subscriber(&f("TestServiceSubscriberUnion"), services));
    let mut services = PhpArray::list([Value::from(format!("{def1}&{def2}"))]);
    services.insert("bar", Value::from(format!("{def1}&{def2}")));
    services.insert("baz", Value::from(format!("?{def1}&{def2}")));
    registry.add_class(subscriber(
        &f("TestServiceSubscriberIntersection"),
        services,
    ));
    // ServiceMethodsSubscriberTrait::getSubscribedServices() of the fixtures.
    let (parent, child) = (
        f("TestServiceSubscriberParent"),
        f("TestServiceSubscriberChild"),
    );
    let mut parent_services = PhpArray::new();
    parent_services.insert(
        format!("{parent}::testDefinition1").as_str(),
        Value::from(def1.as_str()),
    );
    registry.add_class(subscriber(&parent, parent_services.clone()));
    let mut services = parent_services;
    services.insert(
        format!("{child}::testDefinition2").as_str(),
        Value::from(format!("?{def2}")),
    );
    services.insert("custom_name", Value::from(def3.as_str()));
    services.insert(
        format!("{child}::invalidDefinition").as_str(),
        Value::from(f("InvalidDefinition")),
    );
    services.insert(
        format!("{child}::testDefinition4").as_str(),
        Value::from(def3.as_str()),
    );
    registry.add_class(
        ClassMetadata::class(&child)
            .with_parent(&parent)
            .with_method(
                MethodMetadata::public("getSubscribedServices")
                    .with_static(true)
                    .with_return_type("array")
                    .with_returns(Value::Array(services)),
            ),
    );
    let union_trait = f("TestServiceSubscriberUnionWithTrait");
    let mut services = PhpArray::new();
    services.insert(
        format!("{union_trait}::method1").as_str(),
        Value::from(format!("?{def1}|{def2}|null")),
    );
    services.insert(
        format!("{union_trait}::method2").as_str(),
        Value::from(format!("{def1}|{def2}")),
    );
    registry.add_class(subscriber(&union_trait, services));
    let intersection_trait = f("TestServiceSubscriberIntersectionWithTrait");
    let mut services = PhpArray::new();
    services.insert(
        format!("{intersection_trait}::method1").as_str(),
        Value::from(format!("{def1}&{def2}")),
    );
    registry.add_class(subscriber(&intersection_trait, services));
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

/// Both passes, as most tests run them.
fn process(container: &mut ContainerBuilder) -> Result<()> {
    RegisterServiceSubscribersPass::new().process(container)?;
    ResolveServiceSubscribersPass::new().process(container)
}

/// `$container->getDefinition((string) $value)`.
fn definition_of(container: &ContainerBuilder, value: &Value) -> Definition {
    let id = value.to_php_string_or_stringable().unwrap();
    container.get_definition(&id.to_string_lossy()).unwrap()
}

/// `$foo->getMethodCalls()[0][1][0]`.
fn first_call_argument(foo: &Definition) -> Value {
    let calls = foo.method_calls();
    let Some(Value::Array(call)) = calls.get(&Key::Int(0)) else {
        panic!("no method call");
    };
    let Some(Value::Array(arguments)) = call.get(&Key::Int(1)) else {
        panic!("no arguments");
    };
    arguments.get(&Key::Int(0)).unwrap().clone()
}

/// `$container->getDefinition((string) $locator->getFactory()[0])->getArgument(0)`.
fn factories(container: &ContainerBuilder, locator: &Definition) -> PhpArray {
    let Value::Array(factory) = locator.factory() else {
        panic!("no factory");
    };
    match definition_of(container, factory.get(&Key::Int(0)).unwrap())
        .argument(0)
        .unwrap()
    {
        Value::Array(a) => a,
        _ => panic!("not an array"),
    }
}

fn assert_locator(locator: &Definition) {
    assert!(!locator.is_public());
    assert_eq!(Some(SERVICE_LOCATOR.to_string()), locator.class());
}

/// `assertEquals()` of `ServiceClosureArgument`s of typed references.
#[track_caller]
fn assert_factories(expected: &[(&str, Reference)], actual: &PhpArray) {
    let mut keys: Vec<String> = actual
        .keys()
        .map(|k| String::from_utf8_lossy(&k.bytes()).into_owned())
        .collect();
    keys.sort();
    let mut expected_keys: Vec<&str> = expected.iter().map(|(k, _)| *k).collect();
    expected_keys.sort_unstable();
    assert_eq!(expected_keys, keys);
    for (key, reference) in expected {
        let closure = actual.get(&Key::from(*key)).unwrap();
        let closure = ServiceClosureArgument::from_value(closure).unwrap();
        assert_eq!(
            Some(reference),
            Reference::from_value(&closure.value()),
            "{key}"
        );
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

    let e = process(&mut container).unwrap_err();
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

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "The \"container.service_subscriber\" tag accepts only the \"key\" and \"id\" attributes, \"bar\" given for service \"foo\".",
        e.message()
    );
}

/// The factories of testNoAttributes and testBinding.
fn test_service_subscriber_factories(bar: &str) -> Vec<(String, Reference)> {
    let (subscriber, custom, def1) = (
        f("TestServiceSubscriber"),
        f("CustomDefinition"),
        f("TestDefinition1"),
    );
    vec![
        (
            subscriber.clone(),
            typed(&subscriber, &subscriber, Exception, None),
        ),
        (custom.clone(), typed(&custom, &custom, Ignore, None)),
        ("bar".into(), typed(bar, &custom, Exception, Some("bar"))),
        ("baz".into(), typed(&custom, &custom, Ignore, Some("baz"))),
        (
            "late_alias".into(),
            typed(&def1, &def1, Exception, Some("late_alias")),
        ),
    ]
}

fn borrowed(expected: &[(String, Reference)]) -> Vec<(&str, Reference)> {
    expected
        .iter()
        .map(|(k, r)| (k.as_str(), r.clone()))
        .collect()
}

#[test]
fn testNoAttributes() {
    let mut container = container();

    container
        .register("foo", Some(&f("TestServiceSubscriber")))
        .unwrap()
        .add_argument(Reference::new(PSR_CONTAINER))
        .add_tag("container.service_subscriber", PhpArray::new());

    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    let locator = definition_of(&container, &foo.argument(0).unwrap());

    assert_locator(&locator);

    let expected = test_service_subscriber_factories(&f("CustomDefinition"));
    assert_factories(&borrowed(&expected), &factories(&container, &locator));
}

#[test]
fn testWithAttributes() {
    let mut container = container();

    container
        .register("foo", Some(&f("TestServiceSubscriber")))
        .unwrap()
        .set_autowired(true)
        .add_argument(Reference::new(PSR_CONTAINER))
        .add_tag(
            "container.service_subscriber",
            php_array!["key" => "bar", "id" => "bar"],
        )
        // should be ignored: the first wins
        .add_tag(
            "container.service_subscriber",
            php_array!["key" => "bar", "id" => "baz"],
        );

    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    let locator = definition_of(&container, &foo.argument(0).unwrap());

    assert_locator(&locator);

    let expected = test_service_subscriber_factories("bar");
    assert_factories(&borrowed(&expected), &factories(&container, &locator));
}

/// testUnionServices up to its `AutowirePass` run.
#[test]
fn union_services_locator() {
    let mut container = container();

    container.register("bar", Some("stdClass")).unwrap();
    container.set_alias(&f("TestDefinition1"), "bar").unwrap();
    container.set_alias(&f("TestDefinition2"), "bar").unwrap();
    container
        .register("foo", Some(&f("TestServiceSubscriberUnion")))
        .unwrap()
        .add_argument(Reference::new(PSR_CONTAINER))
        .add_tag("container.service_subscriber", PhpArray::new());

    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    let locator = definition_of(&container, &foo.argument(0).unwrap());

    assert_locator(&locator);

    let (def1, def2) = (f("TestDefinition1"), f("TestDefinition2"));
    let first = format!("string|{def2}|{def1}");
    let union = format!("{def1}|{def2}");
    assert_factories(
        &[
            (&first, typed(&first, &first, Exception, None)),
            ("bar", typed(&union, &union, Exception, Some("bar"))),
            ("baz", typed(&union, &union, Ignore, Some("baz"))),
        ],
        &factories(&container, &locator),
    );
}

#[test]
fn testIntersectionServices() {
    let mut container = container();

    container
        .register("foo", Some(&f("TestServiceSubscriberIntersection")))
        .unwrap()
        .add_argument(Reference::new(PSR_CONTAINER))
        .add_tag("container.service_subscriber", PhpArray::new());

    process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    let locator = definition_of(&container, &foo.argument(0).unwrap());

    assert_locator(&locator);

    let intersection = format!("{}&{}", f("TestDefinition1"), f("TestDefinition2"));
    assert_factories(
        &[
            (
                &intersection,
                typed(&intersection, &intersection, Exception, None),
            ),
            (
                "bar",
                typed(&intersection, &intersection, Exception, Some("bar")),
            ),
            (
                "baz",
                typed(&intersection, &intersection, Ignore, Some("baz")),
            ),
        ],
        &factories(&container, &locator),
    );
}

#[test]
fn testExtraServiceSubscriber() {
    let mut container = container();
    container
        .register("foo_service", Some(&f("TestServiceSubscriber")))
        .unwrap()
        .set_autowired(true)
        .add_argument(Reference::new(PSR_CONTAINER))
        .add_tag(
            "container.service_subscriber",
            php_array!["key" => "test", "id" => f("TestServiceSubscriber")],
        );
    container
        .register(
            &f("TestServiceSubscriber"),
            Some(&f("TestServiceSubscriber")),
        )
        .unwrap();

    let e = support::compile(&mut container, false).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert!(
        e.message().contains("Service key \"test\" does not exist in the map returned by \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\TestServiceSubscriber::getSubscribedServices()\" for service \"foo_service\"."),
        "{}",
        e.message()
    );
}

/// A subscriber whose `setContainer()` gets the container.
fn method_subscriber(container: &mut ContainerBuilder, class: &str) -> Definition {
    container
        .register("foo", Some(class))
        .unwrap()
        .add_method_call(
            "setContainer",
            PhpArray::list([Reference::new(PSR_CONTAINER).into()]),
            false,
        )
        .unwrap()
        .add_tag("container.service_subscriber", PhpArray::new());

    process(container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    definition_of(container, &first_call_argument(&foo))
}

#[test]
fn testServiceMethodsSubscriberTraitWithSubscribedServiceAttribute() {
    let mut container = container();
    let (child, parent) = (
        f("TestServiceSubscriberChild"),
        f("TestServiceSubscriberParent"),
    );
    let locator = method_subscriber(&mut container, &child);

    let (def1, def2, def3, invalid) = (
        f("TestDefinition1"),
        f("TestDefinition2"),
        f("TestDefinition3"),
        f("InvalidDefinition"),
    );
    assert_factories(
        &[
            (
                &format!("{child}::invalidDefinition"),
                typed(&invalid, &invalid, Exception, None),
            ),
            (
                &format!("{child}::testDefinition2"),
                typed(&def2, &def2, Ignore, None),
            ),
            (
                &format!("{child}::testDefinition4"),
                typed(&def3, &def3, Exception, None),
            ),
            (
                &format!("{parent}::testDefinition1"),
                typed(&def1, &def1, Exception, None),
            ),
            (
                "custom_name",
                typed(&def3, &def3, Exception, Some("custom_name")),
            ),
        ],
        &factories(&container, &locator),
    );
}

#[test]
fn testServiceMethodsSubscriberTraitWithUnionReturnType() {
    let mut container = container();
    let class = f("TestServiceSubscriberUnionWithTrait");
    let locator = method_subscriber(&mut container, &class);

    let (def1, def2) = (f("TestDefinition1"), f("TestDefinition2"));
    let nullable = format!("{def1}|{def2}|null");
    let union = format!("{def1}|{def2}");
    assert_factories(
        &[
            (
                &format!("{class}::method1"),
                typed(&nullable, &nullable, Ignore, None),
            ),
            (
                &format!("{class}::method2"),
                typed(&union, &union, Exception, None),
            ),
        ],
        &factories(&container, &locator),
    );
}

#[test]
fn testServiceMethodsSubscriberTraitWithIntersectionReturnType() {
    let mut container = container();
    let class = f("TestServiceSubscriberIntersectionWithTrait");
    let locator = method_subscriber(&mut container, &class);

    let intersection = format!("{}&{}", f("TestDefinition1"), f("TestDefinition2"));
    assert_factories(
        &[(
            &format!("{class}::method1"),
            typed(&intersection, &intersection, Exception, None),
        )],
        &factories(&container, &locator),
    );
}

/// testServiceSubscriberWithSemanticId up to its `AutowirePass` run.
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
    let locator = method_subscriber(&mut container, "SemanticIdSubscriber");

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
        &factories(&container, &locator),
    );
}

#[test]
fn testBinding() {
    let mut container = container();

    container
        .register("foo", Some(&f("TestServiceSubscriber")))
        .unwrap()
        .add_method_call("setServiceProvider", PhpArray::new(), false)
        .unwrap()
        .add_tag("container.service_subscriber", PhpArray::new());

    RegisterServiceSubscribersPass::new()
        .process(&mut container)
        .unwrap();
    ResolveBindingsPass::new().process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    let locator = definition_of(&container, &first_call_argument(&foo));

    assert_locator(&locator);

    let expected = test_service_subscriber_factories(&f("CustomDefinition"));
    assert_factories(&borrowed(&expected), &factories(&container, &locator));
}
