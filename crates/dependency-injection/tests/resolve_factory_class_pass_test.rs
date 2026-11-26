//! Tests/Compiler/ResolveFactoryClassPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; the data provider is a loop.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveFactoryClassPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key};
use symfony_dependency_injection::compiler::{CompilerPassInterface, ResolveFactoryClassPass};
use symfony_dependency_injection::{ContainerBuilder, Definition, ErrorKind, Reference};

fn list<const N: usize>(values: [Value; N]) -> Value {
    Value::Array(PhpArray::list(values))
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();

    let factory = container.register("factory", Some("Foo\\Bar")).unwrap();
    factory.set_factory(list([Value::Null, Value::from("create")]));

    let mut pass = ResolveFactoryClassPass::new();
    pass.process(&mut container).unwrap();

    assert_eq!(
        list([Value::from("Foo\\Bar"), Value::from("create")]),
        factory.factory()
    );
}

#[test]
fn testInlinedDefinitionFactoryIsProcessed() {
    let mut container = ContainerBuilder::new();

    let factory = container.register("factory", None).unwrap();
    let inline = Definition::with_class("Baz\\Qux");
    inline.set_factory(list([Value::Null, Value::from("getInstance")]));
    factory.set_factory(list([inline.to_value(), Value::from("create")]));

    let mut pass = ResolveFactoryClassPass::new();
    pass.process(&mut container).unwrap();

    let inner = factory
        .factory()
        .as_array()
        .unwrap()
        .get(&Key::Int(0))
        .cloned()
        .unwrap();
    assert_eq!(
        list([Value::from("Baz\\Qux"), Value::from("getInstance")]),
        Definition::from_value(&inner).unwrap().factory()
    );
}

fn provide_fulfilled_factories() -> Vec<Value> {
    vec![
        list([Value::from("Foo\\Bar"), Value::from("create")]),
        list([Value::from(Reference::new("foo")), Value::from("create")]),
        list([
            Definition::with_class("Baz").to_value(),
            Value::from("create"),
        ]),
    ]
}

#[test]
fn testIgnoresFulfilledFactories() {
    for factory in provide_fulfilled_factories() {
        let mut container = ContainerBuilder::new();
        let definition = Definition::default();
        definition.set_factory(factory.clone());

        container.set_definition("factory", definition).unwrap();

        let mut pass = ResolveFactoryClassPass::new();
        pass.process(&mut container).unwrap();

        assert_eq!(
            factory,
            container.get_definition("factory").unwrap().factory()
        );
    }
}

#[test]
fn testNotAnyClassThrowsException() {
    let mut container = ContainerBuilder::new();

    let factory = container.register("factory", None).unwrap();
    factory.set_factory(list([Value::Null, Value::from("create")]));

    let mut pass = ResolveFactoryClassPass::new();
    let e = pass.process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "The \"factory\" service is defined to be created by a factory, but is missing the factory class. Did you forget to define the factory or service class?",
        e.message()
    );
}
