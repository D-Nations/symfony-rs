//! Tests/Compiler/CheckAliasValidityPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; the fixture classes of `Tests/Fixtures/CheckAliasValidityPass/`
//! are class metadata.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckAliasValidityPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use symfony_dependency_injection::class_metadata::{ClassMetadata, ClassRegistry};
use symfony_dependency_injection::compiler::{CheckAliasValidityPass, CompilerPassInterface};
use symfony_dependency_injection::{ContainerBuilder, Reference, Result};

const NS: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\CheckAliasValidityPass\\";

fn foo_interface() -> String {
    format!("{NS}FooInterface")
}

fn foo_implementing() -> String {
    format!("{NS}FooImplementing")
}

fn foo_not_implementing() -> String {
    format!("{NS}FooNotImplementing")
}

fn container() -> ContainerBuilder {
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::interface(&foo_interface()));
    registry
        .add_class(ClassMetadata::class(&foo_implementing()).with_interfaces(&[&foo_interface()]));
    registry.add_class(ClassMetadata::class(&foo_not_implementing()));
    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));
    container
}

fn process(container: &mut ContainerBuilder) -> Result<()> {
    CheckAliasValidityPass.process(container)
}

#[test]
fn testProcessDetectsClassNotImplementingAliasedInterface() {
    let mut container = container();
    container
        .register("a", None)
        .unwrap()
        .set_class(Some(&foo_not_implementing()));
    container.set_alias(&foo_interface(), "a").unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.instance_of("Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException"));
    assert_eq!(
        format!(
            "Invalid alias definition: alias \"{i}\" is referencing class \"{c}\" but this class does not implement \"{i}\". Because this alias is an interface, \"{c}\" must implement \"{i}\".",
            i = foo_interface(),
            c = foo_not_implementing()
        ),
        e.message()
    );
}

#[test]
fn testProcessAcceptsClassImplementingAliasedInterface() {
    let mut container = container();
    container
        .register("a", None)
        .unwrap()
        .set_class(Some(&foo_implementing()));
    container.set_alias(&foo_interface(), "a").unwrap();

    process(&mut container).unwrap();
}

#[test]
fn testProcessIgnoresArbitraryAlias() {
    let mut container = container();
    container
        .register("a", None)
        .unwrap()
        .set_class(Some(&foo_implementing()));
    container.set_alias("not_an_interface", "a").unwrap();

    process(&mut container).unwrap();
}

#[test]
fn testProcessIgnoresTargetWithFactory() {
    let mut container = container();
    container
        .register("a", None)
        .unwrap()
        .set_factory(Reference::new("foo"));
    container.set_alias(&foo_interface(), "a").unwrap();

    process(&mut container).unwrap();
}

#[test]
fn testProcessIgnoresTargetWithoutClass() {
    let mut container = container();
    container.register("a", None).unwrap();
    container.set_alias(&foo_interface(), "a").unwrap();

    process(&mut container).unwrap();
}

#[test]
fn testProcessIgnoresDeprecatedAlias() {
    let mut container = container();
    container
        .register("a", None)
        .unwrap()
        .set_class(Some(&foo_not_implementing()));
    container
        .set_alias(&foo_interface(), "a")
        .unwrap()
        .set_deprecated(
            "vendor/package",
            "1.2",
            "The \"%alias_id%\" alias is deprecated.",
        )
        .unwrap();

    process(&mut container).unwrap();
}
