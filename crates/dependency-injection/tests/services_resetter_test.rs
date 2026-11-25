//! Port of Symfony\Component\DependencyInjection\Tests\ServicesResetterTest
//! from Tests/ServicesResetterTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's.
//!
//! The fixtures (`ResettableService`, `ClearableService`,
//! `MultiResettableService`, `LazyResettableService`) are
//! [`ServiceObject`]s; their static counters are per-test counters, since
//! Rust tests run in parallel. The `ProxyHelper` lazy proxy of
//! `testResetLazyServices` is [`ServiceObject::lazy_proxy`]. An
//! `\ArrayIterator` is [`RewindableGenerator::from_pairs`], a `\WeakMap`
//! a [`ResetMap`].
//!
//! Upstream: symfony/dependency-injection Tests/ServicesResetterTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use php_value::Value;
use symfony_dependency_injection::argument::RewindableGenerator;
use symfony_dependency_injection::{Error, ErrorKind, ResetMap, ServiceObject, ServicesResetter};

const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";

type Counter = Arc<AtomicI64>;

fn counter() -> Counter {
    Arc::new(AtomicI64::new(0))
}

fn count(c: &Counter) -> i64 {
    c.load(Ordering::SeqCst)
}

fn bump(
    c: &Counter,
) -> impl Fn(&ServiceObject, &[Value]) -> symfony_dependency_injection::Result<Value>
+ Send
+ Sync
+ 'static {
    let c = c.clone();
    move |_, _| {
        c.fetch_add(1, Ordering::SeqCst);
        Ok(Value::Null)
    }
}

fn resettable_service(counter: &Counter) -> ServiceObject {
    ServiceObject::new(format!("{FIXTURES}ResettableService")).method("reset", bump(counter))
}

fn clearable_service(counter: &Counter) -> ServiceObject {
    ServiceObject::new(format!("{FIXTURES}ClearableService")).method("clear", bump(counter))
}

fn lazy_resettable_service(counter: &Counter) -> ServiceObject {
    ServiceObject::new(format!("{FIXTURES}LazyResettableService"))
        .implements("stdClass")
        .method("foo", |_, _| Ok(Value::Bool(true)))
        .method("reset", bump(counter))
}

fn services(pairs: Vec<(&str, Value)>) -> RewindableGenerator {
    RewindableGenerator::from_pairs(
        pairs
            .into_iter()
            .map(|(id, service)| (Value::from(id), service))
            .collect(),
    )
}

fn methods(pairs: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    pairs
        .iter()
        .map(|(id, methods)| {
            (
                (*id).to_owned(),
                methods.iter().map(|&m| m.to_owned()).collect(),
            )
        })
        .collect()
}

fn runtime_exception(message: &str) -> Error {
    Error::new(ErrorKind::Other("RuntimeException".to_owned()), message)
}

#[test]
fn testResetServices() {
    let resettable = counter();
    let clearable = counter();
    let reset_first = counter();
    let reset_second = counter();
    let resetter = ServicesResetter::new(
        services(vec![
            ("id1", resettable_service(&resettable).into_value()),
            ("id2", clearable_service(&clearable).into_value()),
            (
                "id3",
                ServiceObject::new(format!("{FIXTURES}MultiResettableService"))
                    .method("resetFirst", bump(&reset_first))
                    .method("resetSecond", bump(&reset_second))
                    .into_value(),
            ),
        ]),
        methods(&[
            ("id1", &["reset"]),
            ("id2", &["clear"]),
            ("id3", &["resetFirst", "resetSecond"]),
        ]),
        None,
    );

    resetter.reset().unwrap();

    assert_eq!(1, count(&resettable));
    assert_eq!(1, count(&clearable));
    assert_eq!(1, count(&reset_first));
    assert_eq!(1, count(&reset_second));
}

#[test]
fn testResetLazyServices() {
    let lazy_counter = counter();
    let lazy_service = {
        let lazy_counter = lazy_counter.clone();
        Arc::new(ServiceObject::lazy_proxy(
            "DI_LazyResettableServiceProxy",
            &[&format!("{FIXTURES}LazyResettableService"), "stdClass"],
            &["foo", "reset"],
            move || Ok(lazy_resettable_service(&lazy_counter)),
        ))
    };
    let lazy_value = Value::Object(lazy_service.clone());

    let resetter = ServicesResetter::new(
        services(vec![("lazy", lazy_value)]),
        methods(&[("lazy", &["reset"])]),
        None,
    );

    resetter.reset().unwrap();
    assert_eq!(0, count(&lazy_counter));

    resetter.reset().unwrap();
    assert_eq!(0, count(&lazy_counter));

    assert!(
        lazy_service
            .call("foo", &[])
            .unwrap()
            .identical(&Value::Bool(true))
    );

    resetter.reset().unwrap();
    assert_eq!(1, count(&lazy_counter));
}

#[test]
fn testResetNonSharedServices() {
    let resettable = counter();
    let reset_map = ResetMap::new();
    let non_shared_instance = resettable_service(&resettable).into_value();
    reset_map.insert(
        non_shared_instance.as_object().unwrap(),
        vec!["reset".to_owned()],
    );

    let resetter = ServicesResetter::new(services(Vec::new()), Vec::new(), Some(reset_map));
    resetter.reset().unwrap();

    assert_eq!(1, count(&resettable));
}

#[test]
fn testResetMixedSharedAndNonSharedServices() {
    let resettable = counter();
    let clearable = counter();
    let reset_map = ResetMap::new();
    let non_shared_instance = clearable_service(&clearable).into_value();
    reset_map.insert(
        non_shared_instance.as_object().unwrap(),
        vec!["clear".to_owned()],
    );

    let resetter = ServicesResetter::new(
        services(vec![("id1", resettable_service(&resettable).into_value())]),
        methods(&[("id1", &["reset"])]),
        Some(reset_map),
    );
    resetter.reset().unwrap();

    assert_eq!(1, count(&resettable));
    assert_eq!(1, count(&clearable));
}

#[test]
fn testResetNonSharedWithOptionalMethod() {
    let reset_map = ResetMap::new();
    let instance = ServiceObject::new("stdClass").into_value();
    reset_map.insert(
        instance.as_object().unwrap(),
        vec!["?nonExistentMethod".to_owned()],
    );

    let resetter = ServicesResetter::new(services(Vec::new()), Vec::new(), Some(reset_map));

    // Should not throw - '?' prefix means the method is optional
    resetter.reset().unwrap();
}

#[test]
fn testNonSharedInstancesGarbageCollected() {
    let resettable = counter();
    let reset_map = ResetMap::new();
    let instance = resettable_service(&resettable).into_value();
    reset_map.insert(instance.as_object().unwrap(), vec!["reset".to_owned()]);

    assert_eq!(1, reset_map.count());

    drop(instance);

    assert_eq!(0, reset_map.count());
}

#[test]
fn testResetThrowingServiceDoesNotPreventLaterServicesReset() {
    let resettable = counter();
    let throwing_service = ServiceObject::new("class@anonymous")
        .method("reset", |_, _| Err(runtime_exception("boom")))
        .into_value();

    let resetter = ServicesResetter::new(
        services(vec![
            ("id1", throwing_service),
            ("id2", resettable_service(&resettable).into_value()),
        ]),
        methods(&[("id1", &["reset"]), ("id2", &["reset"])]),
        None,
    );

    let thrown = resetter.reset().unwrap_err();

    assert!(thrown.instance_of("RuntimeException"));
    assert_eq!("boom", thrown.message());
    assert_eq!(1, count(&resettable));
}

#[test]
fn testFirstThrowableIsRethrownAfterAllMethodsRun() {
    let second_counter = counter();
    let service = {
        let second_counter = second_counter.clone();
        ServiceObject::new("class@anonymous")
            .method("resetFirst", |_, _| Err(runtime_exception("first")))
            .method("resetSecond", move |_, _| {
                second_counter.fetch_add(1, Ordering::SeqCst);
                Err(runtime_exception("second"))
            })
            .into_value()
    };

    let resetter = ServicesResetter::new(
        services(vec![("id1", service)]),
        methods(&[("id1", &["resetFirst", "resetSecond"])]),
        None,
    );

    let thrown = resetter.reset().unwrap_err();

    assert!(thrown.instance_of("RuntimeException"));
    assert_eq!("first", thrown.message());
    assert_eq!(1, count(&second_counter));
}
