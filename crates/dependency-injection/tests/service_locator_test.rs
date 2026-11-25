//! Port of Symfony\Component\DependencyInjection\Tests\ServiceLocatorTest
//! from Tests/ServiceLocatorTest.php at symfony/dependency-injection
//! 9b34cad8, with the tests it inherits from symfony/service-contracts'
//! `ServiceLocatorTestCase` (`testHas`, `testGet`,
//! `testGetDoesNotMemoize`, `testThrowsOnUndefinedInternalService`).
//! Names and assertions are PHPUnit's; `expectExceptionMessage()` checks
//! a substring.
//!
//! The calling object PHP finds with `debug_backtrace()` is passed as a
//! [`Caller`]: the test case, or `SomeServiceSubscriber` in
//! `testThrowsInServiceSubscriber`. A factory gets the locator, which
//! stands for the `&$locator` the PHP closures capture. The closures'
//! return types are declared with [`LocatorEntry::returns`].
//!
//! Upstream: symfony/dependency-injection Tests/ServiceLocatorTest.php
//! Upstream: symfony/service-contracts Test/ServiceLocatorTestCase.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use php_value::Value;
use symfony_dependency_injection::{
    Caller, Container, ErrorKind, LocatorEntry, ServiceLocator, ServiceObject,
};

const TEST_CLASS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\ServiceLocatorTest";

fn this() -> Caller {
    Caller::new(TEST_CLASS, false)
}

fn returns(value: &'static str) -> LocatorEntry {
    LocatorEntry::new(move |_| Ok(Value::from(value)))
}

fn get_service_locator(factories: Vec<(&str, LocatorEntry)>) -> ServiceLocator {
    ServiceLocator::new(
        factories
            .into_iter()
            .map(|(id, f)| (id.to_owned(), f))
            .collect(),
    )
}

fn same(a: &Value, b: &Value) -> bool {
    a.identical(b)
}

#[test]
fn testHas() {
    let locator = get_service_locator(vec![
        ("foo", returns("bar")),
        ("bar", returns("baz")),
        ("0", returns("dummy")),
    ]);

    assert!(locator.has("foo"));
    assert!(locator.has("bar"));
    assert!(!locator.has("dummy"));
}

#[test]
fn testGet() {
    let locator = get_service_locator(vec![("foo", returns("bar")), ("bar", returns("baz"))]);

    assert!(same(
        &Value::from("bar"),
        &locator.get_from("foo", Some(&this())).unwrap()
    ));
    assert!(same(
        &Value::from("baz"),
        &locator.get_from("bar", Some(&this())).unwrap()
    ));
}

#[test]
fn testGetDoesNotMemoize() {
    let i = Arc::new(AtomicI64::new(0));
    let locator = get_service_locator(vec![(
        "foo",
        LocatorEntry::new({
            let i = i.clone();
            move |_| {
                i.fetch_add(1, Ordering::SeqCst);
                Ok(Value::from("bar"))
            }
        }),
    )]);

    assert!(same(
        &Value::from("bar"),
        &locator.get_from("foo", Some(&this())).unwrap()
    ));
    assert!(same(
        &Value::from("bar"),
        &locator.get_from("foo", Some(&this())).unwrap()
    ));
    assert_eq!(2, i.load(Ordering::SeqCst));
}

#[test]
fn testThrowsOnUndefinedInternalService() {
    let locator = get_service_locator(vec![(
        "foo",
        LocatorEntry::new(|locator| locator.get("bar")),
    )]);

    let e = locator.get_from("foo", Some(&this())).unwrap_err();
    assert!(e.instance_of("Psr\\Container\\NotFoundExceptionInterface"));
    assert!(e.message().contains(
        "The service \"foo\" has a dependency on a non-existent service \"bar\". This locator only knows about the \"foo\" service."
    ));
}

#[test]
fn testGetThrowsOnUndefinedService() {
    let locator = get_service_locator(vec![("foo", returns("bar")), ("bar", returns("baz"))]);

    let e = locator.get_from("dummy", Some(&this())).unwrap_err();
    assert!(e.instance_of("Psr\\Container\\NotFoundExceptionInterface"));
    assert!(e.message().contains(
        "Service \"dummy\" not found: the container inside \"Symfony\\Component\\DependencyInjection\\Tests\\ServiceLocatorTest\" is a smaller service locator that only knows about the \"foo\" and \"bar\" services."
    ));
}

#[test]
fn testThrowsOnCircularReference() {
    let locator = get_service_locator(vec![
        ("foo", LocatorEntry::new(|locator| locator.get("bar"))),
        ("bar", LocatorEntry::new(|locator| locator.get("baz"))),
        ("baz", LocatorEntry::new(|locator| locator.get("bar"))),
    ]);

    let e = locator.get_from("foo", Some(&this())).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
    assert!(e.instance_of("Psr\\Container\\ContainerExceptionInterface"));
    assert!(
        e.message().contains(
            "Circular reference detected for service \"bar\", path: \"bar -> baz -> bar\"."
        )
    );
}

/// `SomeServiceSubscriber`, a `ServiceSubscriberInterface`.
struct SomeServiceSubscriber {
    container: ServiceLocator,
}

impl SomeServiceSubscriber {
    const CLASS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\SomeServiceSubscriber";

    fn get_foo(&self) -> symfony_dependency_injection::Result<Value> {
        self.container
            .get_from("foo", Some(&Caller::new(Self::CLASS, true)))
    }
}

#[test]
fn testThrowsInServiceSubscriber() {
    let container = Container::new(None);
    container
        .set("foo", Some(ServiceObject::new("stdClass").into_value()))
        .unwrap();
    let locator = get_service_locator(vec![("bar", LocatorEntry::new(|_| Ok(Value::Null)))]);
    let subscriber = SomeServiceSubscriber {
        container: locator.with_context("caller", container),
    };

    let e = subscriber.get_foo().unwrap_err();
    assert!(e.instance_of("Psr\\Container\\NotFoundExceptionInterface"));
    assert!(e.message().contains(
        "Service \"foo\" not found: even though it exists in the app's container, the container inside \"caller\" is a smaller service locator that only knows about the \"bar\" service. Unless you need extra laziness, try using dependency injection instead. Otherwise, you need to declare it using \"SomeServiceSubscriber::getSubscribedServices()\"."
    ));
}

#[test]
fn testGetThrowsServiceNotFoundException() {
    let container = Container::new(None);
    container
        .set("foo", Some(ServiceObject::new("stdClass").into_value()))
        .unwrap();

    let locator = ServiceLocator::new(Vec::new());
    let locator = locator.with_context("foo", container);

    let e = locator.get_from("foo", Some(&this())).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    assert!(e.message().contains(
        "Service \"foo\" not found: even though it exists in the app's container, the container inside \"foo\" is a smaller service locator that is empty... Try using dependency injection instead."
    ));
}

#[test]
fn testInvoke() {
    let locator = get_service_locator(vec![("foo", returns("bar")), ("bar", returns("baz"))]);

    assert!(same(
        &Value::from("bar"),
        &locator.invoke("foo").unwrap().unwrap()
    ));
    assert!(same(
        &Value::from("baz"),
        &locator.invoke("bar").unwrap().unwrap()
    ));
    assert!(
        locator.invoke("dummy").unwrap().is_none(),
        "->__invoke() should return null on invalid service"
    );
}

#[test]
fn testProvidesServicesInformation() {
    let locator = ServiceLocator::new(vec![
        ("foo".to_owned(), returns("bar")),
        ("bar".to_owned(), returns("baz").returns("string", false)),
        ("baz".to_owned(), returns("zaz").returns("string", true)),
    ]);

    assert_eq!(
        locator.get_provided_services(),
        vec![
            ("foo".to_owned(), "?".to_owned()),
            ("bar".to_owned(), "string".to_owned()),
            ("baz".to_owned(), "?string".to_owned()),
        ]
    );
}

#[test]
fn testIsCountableAndIterable() {
    let locator = get_service_locator(vec![("foo", returns("bar")), ("bar", returns("baz"))]);

    assert_eq!(2, locator.count());
    let services: Vec<(String, Value)> = locator.iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(2, services.len());
    assert_eq!("foo", services[0].0);
    assert!(same(&Value::from("bar"), &services[0].1));
    assert_eq!("bar", services[1].0);
    assert!(same(&Value::from("baz"), &services[1].1));
}
