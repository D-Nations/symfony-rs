//! Tests/Compiler/ResolveInvalidReferencesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Pending: testProcessSetDecoratedAsNullOnInvalid and
//! testProcessSetOnlyDecoratedAsNullOnInvalid run `DecoratorServicePass`
//! first, which is not ported in this slice.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveInvalidReferencesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::ServiceClosureArgument;
use symfony_dependency_injection::compiler::{CompilerPassInterface, ResolveInvalidReferencesPass};
use symfony_dependency_injection::{ContainerBuilder, InvalidBehavior, Reference};

fn process(container: &mut ContainerBuilder) {
    let mut pass = ResolveInvalidReferencesPass::new();
    pass.process(container).unwrap();
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    let def = container.register("foo", None).unwrap();
    def.set_arguments(php_array![
        Reference::with_behavior("bar", InvalidBehavior::Null),
        Reference::with_behavior("baz", InvalidBehavior::Ignore),
    ])
    .add_method_call(
        "foo",
        php_array![Reference::with_behavior("moo", InvalidBehavior::Ignore)],
        false,
    )
    .unwrap();

    process(&mut container);

    let arguments = def.arguments();
    assert_eq!(php_array![Value::Null, Value::Null], arguments);
    assert_eq!(0, def.method_calls().len());
}

#[test]
fn testProcessIgnoreInvalidArgumentInCollectionArgument() {
    let mut container = ContainerBuilder::new();
    container.register("baz", None).unwrap();
    let baz = Value::from(Reference::with_behavior("baz", InvalidBehavior::Ignore));
    let def = container.register("foo", None).unwrap();
    def.set_arguments(php_array![php_array![
        Value::from(Reference::with_behavior("bar", InvalidBehavior::Ignore)),
        baz.clone(),
        Value::from(Reference::with_behavior("moo", InvalidBehavior::Null)),
    ]]);

    process(&mut container);

    let arguments = def.arguments();
    assert_eq!(
        &Value::from(php_array![baz, Value::Null]),
        arguments.get(&Key::Int(0)).unwrap()
    );
}

#[test]
fn testProcessKeepMethodCallOnInvalidArgumentInCollectionArgument() {
    let mut container = ContainerBuilder::new();
    container.register("baz", None).unwrap();
    let baz = Value::from(Reference::with_behavior("baz", InvalidBehavior::Ignore));
    let def = container.register("foo", None).unwrap();
    def.add_method_call(
        "foo",
        php_array![php_array![
            Value::from(Reference::with_behavior("bar", InvalidBehavior::Ignore)),
            baz.clone(),
            Value::from(Reference::with_behavior("moo", InvalidBehavior::Null)),
        ]],
        false,
    )
    .unwrap();

    process(&mut container);

    let calls = def.method_calls();
    assert_eq!(1, def.method_calls().len());
    let call = calls.get(&Key::Int(0)).unwrap().as_array().unwrap();
    let args = call.get(&Key::Int(1)).unwrap().as_array().unwrap();
    assert_eq!(
        &Value::from(php_array![baz, Value::Null]),
        args.get(&Key::Int(0)).unwrap()
    );
}

#[test]
fn testProcessIgnoreNonExistentServices() {
    let mut container = ContainerBuilder::new();
    let def = container.register("foo", None).unwrap();
    def.set_arguments(php_array![Reference::new("bar")]);

    process(&mut container);

    let arguments = def.arguments();
    assert_eq!(
        "bar",
        Reference::from_value(arguments.get(&Key::Int(0)).unwrap())
            .unwrap()
            .id()
    );
}

#[test]
fn testProcessRemovesPropertiesOnInvalid() {
    let mut container = ContainerBuilder::new();
    let def = container.register("foo", None).unwrap();
    def.set_property(
        "foo",
        Reference::with_behavior("bar", InvalidBehavior::Ignore),
    );

    process(&mut container);

    assert_eq!(PhpArray::new(), def.properties());
}

#[test]
fn testProcessRemovesArgumentsOnInvalid() {
    let mut container = ContainerBuilder::new();
    let def = container.register("foo", None).unwrap();
    def.add_argument(php_array![php_array![
        Value::from(Reference::with_behavior("bar", InvalidBehavior::Ignore)),
        ServiceClosureArgument::new(
            Reference::with_behavior("baz", InvalidBehavior::Ignore).into()
        )
        .to_value(),
    ]]);

    process(&mut container);

    assert_eq!(php_array![php_array![PhpArray::new()]], def.arguments());
}

#[test]
fn testProcessExcludedServiceAndNullOnInvalid() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_tag("container.excluded", PhpArray::new());
    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .add_argument(Reference::with_behavior("foo", InvalidBehavior::Null));

    process(&mut container);

    assert_eq!(
        php_array![Value::Null],
        container.get_definition("bar").unwrap().arguments()
    );
}
