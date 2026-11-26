//! Tests/Compiler/AutowireRequiredMethodsPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! The fixture classes of `autowiring_classes.php` and
//! `WitherStaticReturnType.php` are given as class metadata.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AutowireRequiredMethodsPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key};
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::class_metadata::{
    AttributeMetadata, ClassMetadata, ClassRegistry, MethodMetadata, ParameterMetadata,
};
use symfony_dependency_injection::compiler::{
    AutowireRequiredMethodsPass, CompilerPassInterface, ResolveClassPass,
};

const NS: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\";
const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const REQUIRED: &str = "Symfony\\Contracts\\Service\\Attribute\\Required";

fn c(short: &str) -> String {
    format!("{NS}{short}")
}

fn required(mut method: MethodMetadata) -> MethodMetadata {
    method.attributes.push(AttributeMetadata {
        class: REQUIRED.to_string(),
        arguments: PhpArray::new(),
    });
    method
}

fn method(name: &str, params: &[(&str, &str)]) -> MethodMetadata {
    MethodMetadata::public(name).with_parameters(
        params
            .iter()
            .map(|(n, t)| ParameterMetadata::new(n, Some(&c(t))))
            .collect(),
    )
}

fn fixtures() -> Arc<ClassRegistry> {
    let mut r = ClassRegistry::new();
    for short in ["Foo", "A", "CollisionA", "CollisionB"] {
        r.add_class(ClassMetadata::class(&c(short)));
    }
    r.add_class(
        ClassMetadata::class(&c("AutowireSetter")).with_method(required(
            method("setFoo", &[("foo", "Foo")]).with_return_type("void"),
        )),
    );
    r.add_class(
        ClassMetadata::class(&c("AutowireWither")).with_method(required(
            method("withFoo", &[("foo", "Foo")]).with_return_type("static"),
        )),
    );
    r.add_class(
        ClassMetadata::class(&c("SetterInjectionParent"))
            .with_method(required(method(
                "setDependencies",
                &[("foo", "Foo"), ("a", "A")],
            )))
            .with_method(method("notASetter", &[("a", "A")]))
            .with_method(required(method("setWithCallsConfigured", &[("a", "A")])))
            .with_method(required(method(
                "setChildMethodWithoutDocBlock",
                &[("a", "A")],
            ))),
    );
    r.add_class(
        ClassMetadata::class(&c("SetterInjection"))
            .with_parent(&c("SetterInjectionParent"))
            .with_method(required(method("setFoo", &[("foo", "Foo")])))
            .with_method(required(method(
                "setDependencies",
                &[("foo", "Foo"), ("a", "A")],
            )))
            .with_method(method("setWithCallsConfigured", &[("a", "A")]))
            .with_method(method("notASetter", &[("a", "A")])),
    );
    let mut set_foo = required(method("setFoo", &[("foo", "Foo")]).with_return_type("static"));
    set_foo.doc_comment = Some("/**\n     * @return $this\n     */".to_string());
    r.add_class(
        ClassMetadata::class(&format!("{FIXTURES}WitherStaticReturnType"))
            .with_method(required(
                method("withFoo", &[("foo", "Foo")]).with_return_type("static"),
            ))
            .with_method(set_foo),
    );
    Arc::new(r)
}

fn container() -> ContainerBuilder {
    let mut container = ContainerBuilder::new();
    container.set_class_registry(fixtures());
    container
}

fn call(method: &str, returns_clone: bool) -> Value {
    let mut call = PhpArray::list([Value::from(method), Value::Array(PhpArray::new())]);
    if returns_clone {
        call.push(Value::Bool(true)).unwrap();
    }
    Value::Array(call)
}

#[track_caller]
fn assert_same(expected: Vec<Value>, actual: &PhpArray) {
    let expected = Value::Array(PhpArray::list(expected));
    let actual = Value::Array(actual.clone());
    assert!(expected.identical(&actual), "{expected:?} !== {actual:?}");
}

#[test]
fn testSetterInjectionWithAttribute() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();

    container
        .register("setter_injection", Some(&c("AutowireSetter")))
        .unwrap()
        .set_autowired(true);

    ResolveClassPass::new().process(&mut container).unwrap();
    AutowireRequiredMethodsPass::new()
        .process(&mut container)
        .unwrap();

    let method_calls = container
        .get_definition("setter_injection")
        .unwrap()
        .method_calls();
    assert_same(vec![call("setFoo", false)], &method_calls);
}

#[test]
fn testExplicitMethodInjectionAttribute() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();
    container.register(&c("A"), None).unwrap();
    container.register(&c("CollisionA"), None).unwrap();
    container.register(&c("CollisionB"), None).unwrap();

    container
        .register("setter_injection", Some(&c("SetterInjection")))
        .unwrap()
        .set_autowired(true)
        .add_method_call("notASetter", PhpArray::new(), false)
        .unwrap();

    ResolveClassPass::new().process(&mut container).unwrap();
    AutowireRequiredMethodsPass::new()
        .process(&mut container)
        .unwrap();

    let method_calls = container
        .get_definition("setter_injection")
        .unwrap()
        .method_calls();

    let names: Vec<String> = method_calls
        .values()
        .map(|call| {
            call.as_array()
                .unwrap()
                .get(&Key::Int(0))
                .unwrap()
                .to_php_string()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        vec![
            "notASetter",
            "setFoo",
            "setDependencies",
            "setWithCallsConfigured",
            "setChildMethodWithoutDocBlock"
        ],
        names
    );
    let first = method_calls.values().next().unwrap().as_array().unwrap();
    assert!(
        php_value::compare::loose_equals(
            &Value::Array(PhpArray::new()),
            first.get(&Key::Int(1)).unwrap()
        ),
        "{first:?}"
    );
}

#[test]
fn testWitherWithStaticReturnTypeInjection() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();

    container
        .register("wither", Some(&format!("{FIXTURES}WitherStaticReturnType")))
        .unwrap()
        .set_autowired(true);

    ResolveClassPass::new().process(&mut container).unwrap();
    AutowireRequiredMethodsPass::new()
        .process(&mut container)
        .unwrap();

    let method_calls = container.get_definition("wither").unwrap().method_calls();

    let expected = vec![call("withFoo", true), call("setFoo", false)];
    assert_same(expected, &method_calls);
}

#[test]
fn testWitherInjectionWithAttribute() {
    let mut container = container();
    container.register(&c("Foo"), None).unwrap();

    container
        .register("wither", Some(&c("AutowireWither")))
        .unwrap()
        .set_autowired(true);

    ResolveClassPass::new().process(&mut container).unwrap();
    AutowireRequiredMethodsPass::new()
        .process(&mut container)
        .unwrap();

    assert_same(
        vec![call("withFoo", true)],
        &container.get_definition("wither").unwrap().method_calls(),
    );
}
