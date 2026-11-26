//! Tests/Compiler/ResolveTaggedIteratorArgumentPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; `assertEquals()` on two `TaggedIteratorArgument`s compares
//! their tag data and their values (keys, ids, types).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveTaggedIteratorArgumentPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::argument::IteratorArgument;
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, ResolveTaggedIteratorArgumentPass,
};
use symfony_dependency_injection::{ContainerBuilder, InvalidBehavior, Reference};

fn typed(id: &str, class: &str) -> Value {
    Value::from(Reference::typed(
        id,
        class,
        InvalidBehavior::Exception,
        None,
        PhpArray::new(),
    ))
}

#[track_caller]
fn assert_argument_equals(expected: &IteratorArgument, actual: &Value) {
    let actual = IteratorArgument::from_value(actual).unwrap();
    assert_eq!(expected.tagged_data(), actual.tagged_data());
    let (e, a) = (expected.values(), actual.values());
    assert_eq!(
        e.keys().cloned().collect::<Vec<Key>>(),
        a.keys().cloned().collect::<Vec<Key>>()
    );
    for (e, a) in e.values().zip(a.values()) {
        assert_eq!(Reference::from_value(e), Reference::from_value(a));
    }
}

fn foos(container: &ContainerBuilder, id: &str) -> Value {
    container
        .get_definition(id)
        .unwrap()
        .properties()
        .get(&Key::from("foos"))
        .cloned()
        .unwrap()
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    container
        .register("a", Some("stdClass"))
        .unwrap()
        .add_tag("foo", PhpArray::new());
    container
        .register("b", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["priority" => 20]);
    container
        .register("c", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["priority" => 10]);
    container
        .register("d", Some("stdClass"))
        .unwrap()
        .set_property(
            "foos",
            IteratorArgument::tagged("foo", None, false, Vec::new(), true).to_value(),
        );

    ResolveTaggedIteratorArgumentPass::new()
        .process(&mut container)
        .unwrap();

    let expected = IteratorArgument::tagged("foo", None, false, Vec::new(), true);
    expected.set_values(PhpArray::list([
        Value::from(Reference::new("b")),
        Value::from(Reference::new("c")),
        Value::from(Reference::new("a")),
    ]));
    assert_argument_equals(&expected, &foos(&container, "d"));
}

#[test]
fn testProcessWithIndexes() {
    let mut container = ContainerBuilder::new();
    container
        .register("service_a", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["key" => "1"]);
    container
        .register("service_b", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["key" => "2"]);
    container
        .register("service_c", Some("stdClass"))
        .unwrap()
        .set_property(
            "foos",
            IteratorArgument::tagged("foo", Some("key"), false, Vec::new(), true).to_value(),
        );

    ResolveTaggedIteratorArgumentPass::new()
        .process(&mut container)
        .unwrap();

    let expected = IteratorArgument::tagged("foo", Some("key"), false, Vec::new(), true);
    expected.set_values(php_array![
        "1" => typed("service_a", "stdClass"),
        "2" => typed("service_b", "stdClass"),
    ]);
    assert_argument_equals(&expected, &foos(&container, "service_c"));
}

#[test]
fn testProcesWithAutoExcludeReferencingService() {
    let mut container = ContainerBuilder::new();
    container
        .register("service_a", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["key" => "1"]);
    container
        .register("service_b", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["key" => "2"]);
    container
        .register("service_c", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["key" => "3"])
        .set_property(
            "foos",
            IteratorArgument::tagged("foo", Some("key"), false, Vec::new(), true).to_value(),
        );

    ResolveTaggedIteratorArgumentPass::new()
        .process(&mut container)
        .unwrap();

    let expected = IteratorArgument::tagged("foo", Some("key"), false, Vec::new(), true);
    expected.set_values(php_array![
        "1" => typed("service_a", "stdClass"),
        "2" => typed("service_b", "stdClass"),
    ]);
    assert_argument_equals(&expected, &foos(&container, "service_c"));
}

#[test]
fn testProcesWithoutAutoExcludeReferencingService() {
    let mut container = ContainerBuilder::new();
    container
        .register("service_a", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["key" => "1"]);
    container
        .register("service_b", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["key" => "2"]);
    container
        .register("service_c", Some("stdClass"))
        .unwrap()
        .add_tag("foo", php_array!["key" => "3"])
        .set_property(
            "foos",
            IteratorArgument::tagged("foo", Some("key"), false, Vec::new(), false).to_value(),
        );

    ResolveTaggedIteratorArgumentPass::new()
        .process(&mut container)
        .unwrap();

    let expected = IteratorArgument::tagged("foo", Some("key"), false, Vec::new(), false);
    expected.set_values(php_array![
        "1" => typed("service_a", "stdClass"),
        "2" => typed("service_b", "stdClass"),
        "3" => typed("service_c", "stdClass"),
    ]);
    assert_argument_equals(&expected, &foos(&container, "service_c"));
}
