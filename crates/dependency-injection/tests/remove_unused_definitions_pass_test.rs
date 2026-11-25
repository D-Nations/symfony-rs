//! Tests/Compiler/RemoveUnusedDefinitionsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Pending: testProcessConsiderEnvVariablesAsUsedEvenInPrivateServices
//! (runs `ResolveParameterPlaceHoldersPass`, ported in another slice).
//! `env_variables_of_removed_services_are_counted` checks the same count
//! with the placeholder resolved by the parameter bag.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/RemoveUnusedDefinitionsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{PhpArray, Value, object::StdClass};
use symfony_dependency_injection::compiler::{CompilerPassInterface, RemoveUnusedDefinitionsPass};
use symfony_dependency_injection::{ContainerBuilder, Definition, DefinitionError, Reference};

fn process(container: &mut ContainerBuilder) {
    RemoveUnusedDefinitionsPass::new()
        .process(container)
        .unwrap();
}

fn list(values: impl IntoIterator<Item = Value>) -> PhpArray {
    PhpArray::list(values)
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap();
    container.register("bar", None).unwrap();
    container
        .register("moo", None)
        .unwrap()
        .set_public(true)
        .set_arguments(list([Reference::new("bar").into()]));

    process(&mut container);

    assert!(!container.has_definition("foo"));
    assert!(container.has_definition("bar"));
    assert!(container.has_definition("moo"));
}

#[test]
fn testProcessRemovesUnusedDefinitionsRecursively() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap();
    container
        .register("bar", None)
        .unwrap()
        .set_arguments(list([Reference::new("foo").into()]));

    process(&mut container);

    assert!(!container.has_definition("foo"));
    assert!(!container.has_definition("bar"));
}

#[test]
fn testProcessWorksWithInlinedDefinitions() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap();
    container
        .register("bar", None)
        .unwrap()
        .set_public(true)
        .set_arguments(list([Definition::new(
            None,
            list([Reference::new("foo").into()]),
        )
        .into()]));

    process(&mut container);

    assert!(container.has_definition("foo"));
    assert!(container.has_definition("bar"));
}

#[test]
fn testProcessWontRemovePrivateFactory() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .set_factory(Value::Array(list([
            Value::from("stdClass"),
            Value::from("getInstance"),
        ])))
        .set_public(true);

    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .set_factory(Value::Array(list([
            Reference::new("foo").into(),
            Value::from("getInstance"),
        ])));

    container
        .register("foobar", None)
        .unwrap()
        .set_public(true)
        .add_argument(Reference::new("bar"));

    process(&mut container);

    assert!(container.has_definition("foo"));
    assert!(container.has_definition("bar"));
    assert!(container.has_definition("foobar"));
}

/// testProcessConsiderEnvVariablesAsUsedEvenInPrivateServices, with the
/// parameter bag resolving `%env(FOOBAR)%` in place of
/// `ResolveParameterPlaceHoldersPass`.
#[test]
fn env_variables_of_removed_services_are_counted() {
    let mut container = ContainerBuilder::new();
    container.set_parameter("env(FOOBAR)", "test").unwrap();
    let placeholder = container
        .get_parameter_bag()
        .resolve_value(&Value::from("%env(FOOBAR)%"), &[])
        .unwrap();
    container
        .register("foo", None)
        .unwrap()
        .set_arguments(list([placeholder]));

    process(&mut container);

    assert!(!container.has_definition("foo"));

    let env_counters = container.get_env_counters();
    assert_eq!(Some(&1), env_counters.get("FOOBAR"));
}

#[test]
fn testProcessDoesNotErrorOnServicesThatDoNotHaveDefinitions() {
    let mut container = ContainerBuilder::new();
    container
        .register("defined", None)
        .unwrap()
        .add_argument(Reference::new("not.defined"))
        .set_public(true);

    container
        .set("not.defined", Some(StdClass::object(PhpArray::new())))
        .unwrap();

    process(&mut container);

    assert!(!container.has_definition("not.defined"));
}

#[test]
fn testProcessWorksWithClosureErrorsInDefinitions() {
    let definition = Definition::default();
    definition.add_error(DefinitionError::Lazy(Arc::new(|| "foo bar".to_string())));

    let mut container = ContainerBuilder::new();
    container.set_definition("foo", definition).unwrap();

    process(&mut container);

    assert!(!container.has_definition("foo"));
}
