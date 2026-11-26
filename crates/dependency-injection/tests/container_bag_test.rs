//! Port of Symfony\Component\DependencyInjection\Tests\ParameterBag\ContainerBagTest
//! from Tests/ParameterBag/ContainerBagTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; `setUp()` is [`set_up`]. `assertInstanceOf()` reads
//! [`ContainerBag::instance_of`] and checks the Rust traits.
//!
//! Upstream: symfony/dependency-injection Tests/ParameterBag/ContainerBagTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{Value, php_array};
use symfony_dependency_injection::parameter_bag::{ContainerBag, ContainerBagInterface};
use symfony_dependency_injection::{Container, ParameterBag};

fn set_up() -> ContainerBag {
    let parameter_bag = ParameterBag::new(&php_array!["foo" => "value"]).unwrap();
    ContainerBag::new(Container::new(Some(parameter_bag)))
}

#[test]
fn testGetAllParameters() {
    let container_bag = set_up();
    assert_eq!(php_array!["foo" => "value"], container_bag.all());
}

#[test]
fn testHasAParameter() {
    let container_bag = set_up();
    assert!(container_bag.has("foo"));
    assert!(!container_bag.has("bar"));
}

#[test]
fn testGetParameter() {
    let container_bag = set_up();
    assert!(Value::from("value").identical(&container_bag.get("foo").unwrap()));
}

#[test]
fn testGetParameterNotFound() {
    let container_bag = set_up();
    let e = container_bag.get("bar").unwrap_err();
    assert!(e.instance_of(
        "Symfony\\Component\\DependencyInjection\\Exception\\InvalidArgumentException"
    ));
}

#[test]
fn testInstanceOf() {
    fn container_bag_interface<T: ContainerBagInterface>(_: &T) {}
    fn psr_container<T: psr_container::ContainerInterface>(_: &T) {}

    let container_bag = set_up();
    assert!(ContainerBag::instance_of(
        "Symfony\\Component\\DependencyInjection\\ParameterBag\\FrozenParameterBag"
    ));
    assert!(ContainerBag::instance_of(
        "Symfony\\Component\\DependencyInjection\\ParameterBag\\ContainerBagInterface"
    ));
    assert!(ContainerBag::instance_of(
        "Psr\\Container\\ContainerInterface"
    ));
    container_bag_interface(&container_bag);
    psr_container(&container_bag);
}
