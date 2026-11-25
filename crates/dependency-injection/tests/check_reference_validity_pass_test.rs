//! Tests/Compiler/CheckReferenceValidityPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckReferenceValidityPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use symfony_dependency_injection::compiler::{CheckReferenceValidityPass, CompilerPassInterface};
use symfony_dependency_injection::{ContainerBuilder, Reference, Result};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    CheckReferenceValidityPass::new().process(container)
}

#[test]
fn testProcessDetectsReferenceToAbstractDefinition() {
    let mut container = ContainerBuilder::new();

    container.register("a", None).unwrap().set_abstract(true);
    container
        .register("b", None)
        .unwrap()
        .add_argument(Reference::new("a"));

    let e = process(&mut container).unwrap_err();
    assert!(e.instance_of("RuntimeException"));
    assert_eq!(
        "The definition \"b\" has a reference to an abstract definition \"a\". Abstract definitions cannot be the target of references.",
        e.message()
    );
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", None)
        .unwrap()
        .add_argument(Reference::new("b"));
    container.register("b", None).unwrap();

    process(&mut container).unwrap();
}
