//! Tests/Compiler/CheckArgumentsValidityPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; a data provider's cases run in one test, in order.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/CheckArgumentsValidityPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::compare::loose_equals;
use php_value::{PhpArray, Value};
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::compiler::{CheckArgumentsValidityPass, CompilerPassInterface};

fn list(values: impl IntoIterator<Item = Value>) -> PhpArray {
    PhpArray::list(values)
}

fn call(method: &str, arguments: PhpArray) -> Value {
    Value::Array(list([Value::from(method), Value::Array(arguments)]))
}

fn s(text: &str) -> Value {
    Value::from(text)
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    let definition = container.register("foo", None).unwrap();
    definition.set_arguments(list([Value::Null, Value::Int(1), s("a")]));
    definition
        .set_method_calls(&list([
            call("bar", list([s("a"), s("b")])),
            call("baz", list([s("c"), s("d")])),
        ]))
        .unwrap();

    let mut pass = CheckArgumentsValidityPass::default();
    pass.process(&mut container).unwrap();

    let foo = container.get_definition("foo").unwrap();
    assert!(loose_equals(
        &Value::Array(list([Value::Null, Value::Int(1), s("a")])),
        &Value::Array(foo.arguments())
    ));
    assert!(loose_equals(
        &Value::Array(list([
            call("bar", list([s("a"), s("b")])),
            call("baz", list([s("c"), s("d")])),
        ])),
        &Value::Array(foo.method_calls())
    ));
}

/// `['a' => 'a', null]`.
fn named_then_positional() -> PhpArray {
    let mut array = PhpArray::new();
    array.insert("a", s("a"));
    array.push(Value::Null).unwrap();
    array
}

/// `[1 => 1]`.
fn gap() -> PhpArray {
    let mut array = PhpArray::new();
    array.insert(1, Value::Int(1));
    array
}

#[test]
fn testException() {
    let cases = [
        (named_then_positional(), PhpArray::new()),
        (gap(), PhpArray::new()),
        (
            PhpArray::new(),
            list([call("baz", named_then_positional())]),
        ),
        (PhpArray::new(), list([call("baz", gap())])),
    ];
    for (arguments, method_calls) in cases {
        let mut container = ContainerBuilder::new();
        let definition = container.register("foo", None).unwrap();
        definition.set_arguments(arguments);
        definition.set_method_calls(&method_calls).unwrap();

        let mut pass = CheckArgumentsValidityPass::default();
        let e = pass.process(&mut container).unwrap_err();
        assert!(
            e.instance_of("Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException")
        );
    }
}

#[test]
fn testNoException() {
    let mut container = ContainerBuilder::new();
    let definition = container.register("foo", None).unwrap();
    definition.set_arguments(named_then_positional());

    let mut pass = CheckArgumentsValidityPass::new(false);
    pass.process(&mut container).unwrap();
    assert_eq!(1, definition.errors().len());
}

/// The messages of each check.
#[test]
fn messages() {
    let message = |arguments: PhpArray, method_calls: PhpArray| {
        let mut container = ContainerBuilder::new();
        let definition = container.register("foo", None).unwrap();
        definition.set_arguments(arguments);
        definition.set_method_calls(&method_calls).unwrap();
        CheckArgumentsValidityPass::default()
            .process(&mut container)
            .unwrap_err()
            .message()
    };
    assert_eq!(
        "Invalid constructor argument for service \"foo\": cannot use positional argument after named argument. Check your service definition.",
        message(named_then_positional(), PhpArray::new())
    );
    assert_eq!(
        "Invalid constructor argument 2 for service \"foo\": argument 1 must be defined before. Check your service definition.",
        message(gap(), PhpArray::new())
    );
    let mut dollar = PhpArray::new();
    dollar.insert("$a", s("a"));
    assert_eq!(
        "Invalid argument for method call \"baz\" of service \"foo\": integer expected but found string \"$a\". Check your service definition.",
        message(PhpArray::new(), list([call("baz", dollar)]))
    );
}
