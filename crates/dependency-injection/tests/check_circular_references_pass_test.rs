//! Tests/Compiler/CheckCircularReferencesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. Upstream's `process()` compiles with a `Compiler` whose
//! optimization passes are `AnalyzeServiceReferencesPass(true)` and this
//! pass; here those two passes run directly, since the merge and
//! before-optimization passes of that compiler belong to other slices
//! (they change nothing in these containers).
//!
//! Pending: testProcessDefersLazyServices (runs `ServiceLocatorTagPass`,
//! ported in another slice).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckCircularReferencesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value};
use symfony_dependency_injection::argument::IteratorArgument;
use symfony_dependency_injection::compiler::{
    AnalyzeServiceReferencesPass, CheckCircularReferencesPass, CompilerPassInterface,
};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind, Reference, Result};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    AnalyzeServiceReferencesPass::new(true, true).process(container)?;
    CheckCircularReferencesPass::new().process(container)
}

fn assert_circular(container: &mut ContainerBuilder) {
    let e = process(container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
}

fn factory(service: &str) -> Value {
    Value::Array(PhpArray::list([
        Reference::new(service).into(),
        Value::from("getInstance"),
    ]))
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));
    container
        .register("b", None)
        .unwrap()
        .add_argument(Reference::new("a"));

    assert_circular(&mut container);
}

#[test]
fn testProcessWithAliases() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));
    container.set_alias("b", "c").unwrap();
    container.set_alias("c", "a").unwrap();

    assert_circular(&mut container);
}

#[test]
fn testProcessWithFactory() {
    let mut container = ContainerBuilder::new();

    container
        .register("a", Some("stdClass"))
        .unwrap()
        .set_factory(factory("b"));

    container
        .register("b", Some("stdClass"))
        .unwrap()
        .set_factory(factory("a"));

    assert_circular(&mut container);
}

#[test]
fn testProcessDetectsIndirectCircularReference() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));
    container
        .register("b", None)
        .unwrap()
        .add_argument(Reference::new("c"));
    container
        .register("c", None)
        .unwrap()
        .add_argument(Reference::new("a"));

    assert_circular(&mut container);
}

#[test]
fn testProcessDetectsIndirectCircularReferenceWithFactory() {
    let mut container = ContainerBuilder::new();

    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));

    container
        .register("b", Some("stdClass"))
        .unwrap()
        .set_factory(factory("c"));

    container
        .register("c", None)
        .unwrap()
        .add_argument(Reference::new("a"));

    assert_circular(&mut container);
}

#[test]
fn testDeepCircularReference() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));
    container
        .register("b", None)
        .unwrap()
        .add_argument(Reference::new("c"));
    container
        .register("c", None)
        .unwrap()
        .add_argument(Reference::new("b"));

    assert_circular(&mut container);
}

#[test]
fn testProcessIgnoresMethodCalls() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));
    container
        .register("b", None)
        .unwrap()
        .add_method_call("setA", PhpArray::list([Reference::new("a").into()]), false)
        .unwrap();

    process(&mut container).unwrap();
}

#[test]
fn testProcessIgnoresLazyServices() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .set_lazy(true)
        .add_argument(Reference::new("b"));
    container
        .register("b", None)
        .unwrap()
        .add_argument(Reference::new("a"));

    // just make sure that a lazily loaded service does not trigger a CircularReferenceException
    process(&mut container).unwrap();
}

#[test]
fn testProcessIgnoresIteratorArguments() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));
    container.register("b", None).unwrap().add_argument(
        IteratorArgument::new(PhpArray::list([Reference::new("a").into()])).to_value(),
    );

    // just make sure that an IteratorArgument does not trigger a CircularReferenceException
    process(&mut container).unwrap();
}

/// The message names the cycle.
#[test]
fn circular_reference_message() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));
    container
        .register("b", None)
        .unwrap()
        .add_argument(Reference::new("a"));

    assert_eq!(
        "Circular reference detected for service \"a\", path: \"a -> b -> a\".",
        process(&mut container).unwrap_err().message()
    );
}
