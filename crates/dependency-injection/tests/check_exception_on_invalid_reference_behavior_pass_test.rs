//! Tests/Compiler/CheckExceptionOnInvalidReferenceBehaviorPassTest.php
//! at symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Pending: testWithErroredServiceLocator and testWithErroredHiddenService
//! (both cases of each; they build the locator with
//! `ServiceLocatorTagPass::register()`, ported in another slice).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckExceptionOnInvalidReferenceBehaviorPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::PhpArray;
use symfony_dependency_injection::argument::BoundArgument;
use symfony_dependency_injection::compiler::{
    CheckExceptionOnInvalidReferenceBehaviorPass, CompilerPassInterface,
};
use symfony_dependency_injection::{ContainerBuilder, Definition, ErrorKind, Reference, Result};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    CheckExceptionOnInvalidReferenceBehaviorPass::new().process(container)
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();

    container
        .register("a", Some("\\stdClass"))
        .unwrap()
        .add_argument(Reference::new("b"));
    container.register("b", Some("\\stdClass")).unwrap();

    process(&mut container).unwrap();
}

#[test]
fn testProcessThrowsExceptionOnInvalidReference() {
    let mut container = ContainerBuilder::new();

    container
        .register("a", Some("\\stdClass"))
        .unwrap()
        .add_argument(Reference::new("b"));

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
}

#[test]
fn testProcessThrowsExceptionOnInvalidReferenceFromInlinedDefinition() {
    let mut container = ContainerBuilder::new();

    let def = Definition::default();
    def.add_argument(Reference::new("b"));

    container
        .register("a", Some("\\stdClass"))
        .unwrap()
        .add_argument(def);

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
}

#[test]
fn testProcessDefinitionWithBindings() {
    let mut container = ContainerBuilder::new();

    container
        .register("b", None)
        .unwrap()
        .set_bindings(&PhpArray::list([BoundArgument::new(
            Reference::new("a").into(),
            true,
            0,
            None,
        )
        .to_value()]));

    process(&mut container).unwrap();
}

#[test]
fn testProcessThrowsExceptionOnInvalidReferenceWithAlternatives() {
    let mut container = ContainerBuilder::new();

    container
        .register("a", Some("\\stdClass"))
        .unwrap()
        .add_argument(Reference::new("@ccc"));

    container.register("ccc", Some("\\stdClass")).unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    assert_eq!(
        "The service \"a\" has a dependency on a non-existent service \"@ccc\". Did you mean this: \"ccc\"?",
        e.message()
    );
}

#[test]
fn testCurrentIdIsExcludedFromAlternatives() {
    let mut container = ContainerBuilder::new();
    container
        .register("app.my_service", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("app.my_service2"));

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
    assert_eq!(
        "The service \"app.my_service\" has a dependency on a non-existent service \"app.my_service2\".",
        e.message()
    );
}
