//! Tests/Compiler/CheckCircularReferencesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckCircularReferencesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value};
use symfony_dependency_injection::argument::{IteratorArgument, ServiceLocatorArgument};
use symfony_dependency_injection::compiler::{
    AnalyzeServiceReferencesPass, CheckCircularReferencesPass, Compiler, CompilerPassInterface,
    Pass, ServiceLocatorTagPass,
};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind, Reference, Result};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    let mut compiler = Compiler::new();
    let pass_config = compiler.get_pass_config_mut();
    pass_config.set_optimization_passes(vec![
        Pass::new(AnalyzeServiceReferencesPass::new(true, true)),
        Pass::new(CheckCircularReferencesPass::new()),
    ]);
    pass_config.set_removing_passes(Vec::new());
    pass_config.set_after_removing_passes(Vec::new());

    compiler.compile(container)
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
fn testProcessDefersLazyServices() {
    let mut container = ContainerBuilder::new();

    container
        .register("a", None)
        .unwrap()
        .add_argument(ServiceLocatorArgument::from_tagged(IteratorArgument::tagged(
            "tag",
            None,
            true,
            Vec::new(),
            true,
        )));
    container
        .register("b", None)
        .unwrap()
        .add_argument(Reference::new("c"))
        .add_tag("tag", PhpArray::new());
    container
        .register("c", None)
        .unwrap()
        .add_argument(Reference::new("b"));

    ServiceLocatorTagPass::new().process(&mut container).unwrap();

    assert_circular(&mut container);
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
