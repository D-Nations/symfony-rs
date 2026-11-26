//! Tests/Compiler/DefinitionErrorExceptionPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Pending: testSkipNestedErrors (ends with `$container->get('foo')`,
//! which creates a service). `nested_errors_reached_at_run_time_are_skipped`
//! keeps its first half: the pass does not fail.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/DefinitionErrorExceptionPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::PhpArray;
use symfony_dependency_injection::compiler::{CompilerPassInterface, DefinitionErrorExceptionPass};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, DefinitionError, InvalidBehavior, Reference,
};

fn error(text: &str) -> DefinitionError {
    DefinitionError::Text(text.to_string())
}

#[test]
fn testThrowsException() {
    let mut container = ContainerBuilder::new();
    let def = Definition::default();
    def.add_error(error("Things went wrong!"));
    def.add_error(error("Now something else!"));
    container
        .register("foo_service_id", None)
        .unwrap()
        .set_arguments(PhpArray::list([def.into()]));

    let mut pass = DefinitionErrorExceptionPass::new();
    let e = pass.process(&mut container).unwrap_err();
    assert!(e.instance_of("Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException"));
    assert_eq!("Things went wrong!", e.message());
}

#[test]
fn testNoExceptionThrown() {
    let mut container = ContainerBuilder::new();
    let def = Definition::default();
    container
        .register("foo_service_id", None)
        .unwrap()
        .set_arguments(PhpArray::list([def.clone().into()]));

    let mut pass = DefinitionErrorExceptionPass::new();
    pass.process(&mut container).unwrap();
    let argument = container
        .get_definition("foo_service_id")
        .unwrap()
        .argument(0)
        .unwrap();
    assert!(def.ptr_eq(&Definition::from_value(&argument).unwrap()));
}

/// testSkipNestedErrors up to `$container->get('foo')`.
#[test]
fn nested_errors_reached_at_run_time_are_skipped() {
    let mut container = ContainerBuilder::new();

    container
        .register("nested_error", Some("stdClass"))
        .unwrap()
        .add_error(error("Things went wrong!"));

    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("nested_error"));

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::with_behavior(
            "bar",
            InvalidBehavior::RuntimeException,
        ));

    container
        .register("baz", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::with_behavior(
            "bar",
            InvalidBehavior::IgnoreUninitialized,
        ));

    let mut pass = DefinitionErrorExceptionPass::new();
    pass.process(&mut container).unwrap();
}

/// A definition with errors that a service needs at build time fails.
#[test]
fn errors_reached_at_build_time_fail() {
    let mut container = ContainerBuilder::new();
    container
        .register("nested_error", Some("stdClass"))
        .unwrap()
        .add_error(error("Things went wrong!"));
    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("nested_error"));
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::new("bar"));

    let e = DefinitionErrorExceptionPass::new()
        .process(&mut container)
        .unwrap_err();
    assert_eq!("Things went wrong!", e.message());
}

#[test]
fn testSkipErrorFromTag() {
    let mut container = ContainerBuilder::new();
    let def = Definition::default();
    def.add_error(error("Things went wrong!"));
    def.add_tag("container.error", PhpArray::new());
    container
        .register("foo_service_id", None)
        .unwrap()
        .set_arguments(PhpArray::list([def.clone().into()]));

    let mut pass = DefinitionErrorExceptionPass::new();
    pass.process(&mut container).unwrap();
    let argument = container
        .get_definition("foo_service_id")
        .unwrap()
        .argument(0)
        .unwrap();
    assert!(def.ptr_eq(&Definition::from_value(&argument).unwrap()));
}
