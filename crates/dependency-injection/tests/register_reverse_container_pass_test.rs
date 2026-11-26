//! Tests/Compiler/RegisterReverseContainerPassTest.php at
//! symfony/dependency-injection 9b34cad8.
//!
//! `compile()` is `support::compile()` (nothing is autowired).
//!
//! Pending: testPublicServices and testReversibleServices (they get the
//! services, which needs PHP object creation).
//! `fills_the_locator_argument` runs the pass alone on their container.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/RegisterReverseContainerPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

mod support;

use php_value::{PhpArray, Value, array::Key};
use symfony_dependency_injection::argument::ServiceLocatorArgument;
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, Pass, PassType, RegisterReverseContainerPass,
};
use symfony_dependency_injection::{ContainerBuilder, InvalidBehavior, Reference};

#[test]
fn testCompileRemovesUnusedServices() {
    let mut container = ContainerBuilder::new();
    container.register("foo", Some("stdClass")).unwrap();
    container
        .register(
            "reverse_container",
            Some("Symfony\\Component\\DependencyInjection\\ReverseContainer"),
        )
        .unwrap()
        .add_argument(Reference::new("service_container"))
        .add_argument(ServiceLocatorArgument::new(PhpArray::new()).to_value())
        .set_public(true);

    container
        .add_compiler_pass(
            Pass::new(RegisterReverseContainerPass::new(true)),
            PassType::BeforeOptimization,
            0,
        )
        .unwrap();
    support::compile(&mut container, false).unwrap();

    assert!(!container.has("foo"));
}

/// testReversibleServices's container, with the pass run before removing.
#[test]
fn fills_the_locator_argument() {
    let mut container = ContainerBuilder::new();
    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .set_property("foo", Reference::new("foo"))
        .set_public(true);
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_tag("container.reversible", PhpArray::new());
    let locator = ServiceLocatorArgument::new(PhpArray::new());
    container
        .register(
            "reverse_container",
            Some("Symfony\\Component\\DependencyInjection\\ReverseContainer"),
        )
        .unwrap()
        .add_argument(Reference::new("service_container"))
        .add_argument(locator.to_value())
        .set_public(true);

    RegisterReverseContainerPass::new(true)
        .process(&mut container)
        .unwrap();

    let values = locator.values();
    let ids: Vec<Key> = values.keys().cloned().collect();
    assert_eq!(vec![Key::from("foo"), Key::from("reverse_container")], ids);
    let foo = Reference::from_value(values.get(&Key::from("foo")).unwrap()).unwrap();
    assert_eq!(InvalidBehavior::IgnoreUninitialized, foo.invalid_behavior());

    RegisterReverseContainerPass::new(false)
        .process(&mut container)
        .unwrap();
    let values = locator.values();
    assert_eq!(1, values.len());
    let foo: &Value = values.get(&Key::from("foo")).unwrap();
    assert_eq!(
        InvalidBehavior::Exception,
        Reference::from_value(foo).unwrap().invalid_behavior()
    );
}
