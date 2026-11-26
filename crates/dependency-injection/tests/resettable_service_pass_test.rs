//! Tests/Compiler/ResettableServicePassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! `$container->compile()` is `support::compile()` (nothing is
//! autowired). The fixture classes are class metadata.
//!
//! testIgnoreInvalidMethod drops its last part: it gets and resets the
//! services, which needs PHP object creation.
//!
//! Pending: testNonSharedServiceIsResetThroughWeakMap (it creates
//! services).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResettableServicePassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

mod support;

use std::sync::Arc;

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::IteratorArgument;
use symfony_dependency_injection::class_metadata::{ClassMetadata, ClassRegistry};
use symfony_dependency_injection::compiler::{Pass, PassType, ResettableServicePass};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, ErrorKind, InvalidBehavior, Reference, Result,
};

const FIXTURES: &str = "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\";
const SERVICES_RESETTER: &str =
    "Symfony\\Component\\HttpKernel\\DependencyInjection\\ServicesResetter";

fn f(short: &str) -> String {
    format!("{FIXTURES}{short}")
}

/// A container that knows the fixture classes.
fn container() -> ContainerBuilder {
    let mut registry = ClassRegistry::new();
    for class in [
        f("ResettableService"),
        f("ClearableService"),
        f("MultiResettableService"),
    ] {
        registry.add_class(ClassMetadata::class(&class));
    }
    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));
    container
}

fn register_resetter(container: &mut ContainerBuilder) {
    container
        .register("services_resetter", Some(SERVICES_RESETTER))
        .unwrap()
        .set_public(true)
        .set_arguments(PhpArray::list([Value::Null, Value::Array(PhpArray::new())]));
}

/// `addCompilerPass(new ResettableServicePass())`, then `compile()`.
fn compile(container: &mut ContainerBuilder) -> Result<()> {
    container
        .add_compiler_pass(
            Pass::new(ResettableServicePass),
            PassType::BeforeOptimization,
            0,
        )
        .unwrap();
    support::compile(container, false)
}

fn ignore_uninitialized(id: &str) -> Value {
    Value::from(Reference::with_behavior(
        id,
        InvalidBehavior::IgnoreUninitialized,
    ))
}

#[track_caller]
fn assert_iterator(expected: &[(&str, Value)], actual: &Value) {
    let actual = IteratorArgument::from_value(actual).unwrap();
    assert!(!actual.is_tagged());
    let values = actual.values();
    let keys: Vec<String> = values
        .keys()
        .map(|k| String::from_utf8_lossy(&k.bytes()).into_owned())
        .collect();
    assert_eq!(expected.iter().map(|(k, _)| *k).collect::<Vec<_>>(), keys);
    for ((_, e), a) in expected.iter().zip(values.values()) {
        assert_eq!(Reference::from_value(e), Reference::from_value(a));
    }
}

fn list(items: &[&str]) -> Value {
    Value::Array(PhpArray::list(items.iter().map(|s| Value::from(*s))))
}

#[test]
fn testCompilerPass() {
    let mut container = container();
    container
        .register("one", Some(&f("ResettableService")))
        .unwrap()
        .set_public(true)
        .add_tag("kernel.reset", php_array!["method" => "reset"]);
    container
        .register("two", Some(&f("ClearableService")))
        .unwrap()
        .set_public(true)
        .add_tag("kernel.reset", php_array!["method" => "clear"]);
    container
        .register("three", Some(&f("MultiResettableService")))
        .unwrap()
        .set_public(true)
        .add_tag("kernel.reset", php_array!["method" => "resetFirst"])
        .add_tag("kernel.reset", php_array!["method" => "resetSecond"]);

    register_resetter(&mut container);
    compile(&mut container).unwrap();

    let definition = container.get_definition("services_resetter").unwrap();

    let arguments = definition.arguments();
    assert_eq!(2, arguments.len());
    assert_iterator(
        &[
            ("one", ignore_uninitialized("one")),
            ("two", ignore_uninitialized("two")),
            ("three", ignore_uninitialized("three")),
        ],
        arguments.get(&Key::Int(0)).unwrap(),
    );
    assert!(
        Value::Array(php_array![
            "one" => list(&["reset"]),
            "two" => list(&["clear"]),
            "three" => list(&["resetFirst", "resetSecond"]),
        ])
        .identical(arguments.get(&Key::Int(1)).unwrap())
    );
}

#[test]
fn testMissingMethod() {
    let mut container = container();
    container
        .register(&f("ResettableService"), None)
        .unwrap()
        .add_tag("kernel.reset", PhpArray::new());
    container
        .register("services_resetter", Some(SERVICES_RESETTER))
        .unwrap()
        .set_arguments(PhpArray::list([Value::Null, Value::Array(PhpArray::new())]));

    let e = compile(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::Runtime));
    assert_eq!(
        "Tag \"kernel.reset\" requires the \"method\" attribute to be set on service \"Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\ResettableService\".",
        e.message()
    );
}

#[test]
fn testIgnoreInvalidMethod() {
    let mut container = container();
    container
        .register(&f("ResettableService"), None)
        .unwrap()
        .set_public(true)
        .add_tag(
            "kernel.reset",
            php_array!["method" => "missingMethod", "on_invalid" => "ignore"],
        );
    register_resetter(&mut container);

    compile(&mut container).unwrap();

    let mut expected = PhpArray::new();
    expected.insert(f("ResettableService").as_str(), list(&["?missingMethod"]));
    assert!(
        Value::Array(expected).identical(
            &container
                .get_definition("services_resetter")
                .unwrap()
                .argument(1)
                .unwrap()
        )
    );
}

#[test]
fn testCompilerPassWithNonSharedServices() {
    let mut container = container();
    container
        .register("shared", Some(&f("ResettableService")))
        .unwrap()
        .set_public(true)
        .add_tag("kernel.reset", php_array!["method" => "reset"]);
    container
        .register("non_shared", Some(&f("ClearableService")))
        .unwrap()
        .set_public(true)
        .set_shared(false)
        .add_tag("kernel.reset", php_array!["method" => "clear"]);

    register_resetter(&mut container);
    compile(&mut container).unwrap();

    let definition = container.get_definition("services_resetter").unwrap();

    // Non-shared services should NOT be in the IteratorArgument
    assert_iterator(
        &[("shared", ignore_uninitialized("shared"))],
        &definition.argument(0).unwrap(),
    );

    // Methods should only contain shared services
    assert!(
        Value::Array(php_array!["shared" => list(&["reset"])])
            .identical(&definition.argument(1).unwrap())
    );

    // A Definition with factory [service_container, getResetMap] should be passed as 3rd argument
    let arg2 = Definition::from_value(&definition.argument(2).unwrap()).unwrap();
    assert_eq!(Some("WeakMap".to_string()), arg2.class());
    assert_factory(&arg2);

    // Non-shared service should have container.tracked_for_reset tag
    assert!(
        Value::Array(PhpArray::list([Value::Array(
            php_array!["method" => "clear"]
        )]))
        .identical(&Value::Array(
            container
                .get_definition("non_shared")
                .unwrap()
                .tag("container.tracked_for_reset")
        ))
    );
}

#[track_caller]
fn assert_factory(definition: &Definition) {
    let Value::Array(factory) = definition.factory() else {
        panic!("no factory");
    };
    assert_eq!(2, factory.len());
    assert_eq!(
        Some(&Reference::new("service_container")),
        Reference::from_value(factory.get(&Key::Int(0)).unwrap())
    );
    assert!(Value::from("getResetMap").identical(factory.get(&Key::Int(1)).unwrap()));
}

#[test]
fn testCompilerPassWithOnlyNonSharedServices() {
    let mut container = container();
    container
        .register("non_shared", Some(&f("ResettableService")))
        .unwrap()
        .set_public(true)
        .set_shared(false)
        .add_tag("kernel.reset", php_array!["method" => "reset"]);

    register_resetter(&mut container);
    compile(&mut container).unwrap();

    // services_resetter should still exist even with only non-shared services
    assert!(container.has("services_resetter"));

    let definition = container.get_definition("services_resetter").unwrap();

    // IteratorArgument should be empty
    assert_iterator(&[], &definition.argument(0).unwrap());

    // Methods should be empty (non-shared methods are in the container.tracked_for_reset tag)
    assert!(Value::Array(PhpArray::new()).identical(&definition.argument(1).unwrap()));

    // A Definition with factory should be passed
    let arg2 = Definition::from_value(&definition.argument(2).unwrap()).unwrap();
    assert_factory(&arg2);
}

#[test]
fn testCompilerPassWithNonSharedMultipleResetMethods() {
    let mut container = container();
    container
        .register("non_shared", Some(&f("ResettableService")))
        .unwrap()
        .set_public(true)
        .set_shared(false)
        .add_tag("kernel.reset", php_array!["method" => "reset"])
        .add_tag("kernel.reset", php_array!["method" => "clear"]);

    register_resetter(&mut container);
    compile(&mut container).unwrap();

    assert!(
        Value::Array(PhpArray::list([
            Value::Array(php_array!["method" => "reset"]),
            Value::Array(php_array!["method" => "clear"]),
        ]))
        .identical(&Value::Array(
            container
                .get_definition("non_shared")
                .unwrap()
                .tag("container.tracked_for_reset")
        ))
    );
}

#[test]
fn testCompilerPassWithNonSharedOnInvalidIgnore() {
    let mut container = container();
    container
        .register("non_shared", Some(&f("ResettableService")))
        .unwrap()
        .set_public(true)
        .set_shared(false)
        .add_tag(
            "kernel.reset",
            php_array!["method" => "reset", "on_invalid" => "ignore"],
        );

    register_resetter(&mut container);
    compile(&mut container).unwrap();

    assert!(
        Value::Array(PhpArray::list([Value::Array(
            php_array!["method" => "?reset"]
        )]))
        .identical(&Value::Array(
            container
                .get_definition("non_shared")
                .unwrap()
                .tag("container.tracked_for_reset")
        ))
    );
}

#[test]
fn testCompilerPassWithoutResetters() {
    let mut container = container();
    container
        .register("services_resetter", Some(SERVICES_RESETTER))
        .unwrap()
        .set_arguments(PhpArray::list([Value::Null, Value::Array(PhpArray::new())]));

    compile(&mut container).unwrap();

    assert!(!container.has("services_resetter"));
}
