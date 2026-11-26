//! Tests/Compiler/TagDecoratorPassTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's.
//!
//! The test's `process()` also runs `DecoratorServicePass`, which is not
//! ported yet. testDecorateByTagIgnoreOnInvalid,
//! testDecorateByTagNullOnInvalid and testDecorateByTagExceptionOnInvalid
//! run `TagDecoratorPass` alone: no decorator is left for
//! `DecoratorServicePass` to process (or the first pass throws).
//!
//! Pending: testDecorateByTag and testDecorateByTagPriority (they check
//! the aliases `DecoratorServicePass` creates).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/TagDecoratorPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{Value, php_array};
use symfony_dependency_injection::compiler::{CompilerPassInterface, TagDecoratorPass};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind, InvalidBehavior};

#[test]
fn testDecorateByTagIgnoreOnInvalid() {
    let mut container = ContainerBuilder::new();
    container
        .register("decorator", Some("stdClass"))
        .unwrap()
        .add_resource_tag(
            "container.tag_decorator",
            php_array![
                "decorates_tag" => "non_existent_tag",
                "on_invalid" => Value::Int(InvalidBehavior::Ignore.as_int()),
            ],
        );

    TagDecoratorPass.process(&mut container).unwrap();
    assert!(!container.has("decorator"));
}

#[test]
fn testDecorateByTagNullOnInvalid() {
    let mut container = ContainerBuilder::new();
    container
        .register("decorator", Some("stdClass"))
        .unwrap()
        .add_resource_tag(
            "container.tag_decorator",
            php_array![
                "decorates_tag" => "non_existent_tag",
                "on_invalid" => Value::Int(InvalidBehavior::Null.as_int()),
            ],
        );

    TagDecoratorPass.process(&mut container).unwrap();
    assert!(!container.has("decorator"));
}

#[test]
fn testDecorateByTagExceptionOnInvalid() {
    let mut container = ContainerBuilder::new();
    container
        .register("decorator", Some("stdClass"))
        .unwrap()
        .add_resource_tag(
            "container.tag_decorator",
            php_array!["decorates_tag" => "non_existent_tag"],
        );

    let e = TagDecoratorPass.process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
}

/// testDecorateByTag's decorators, before `DecoratorServicePass`.
#[test]
fn decorators_are_cloned_per_tagged_service() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_tag("tag", php_value::PhpArray::new());
    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .add_tag("tag", php_value::PhpArray::new());
    container
        .register("decorator", Some("stdClass"))
        .unwrap()
        .add_resource_tag(
            "container.tag_decorator",
            php_array!["decorates_tag" => "tag", "priority" => 2],
        );

    TagDecoratorPass.process(&mut container).unwrap();

    assert!(!container.has("decorator"));
    let foo = container
        .get_definition(".decorator.foo.decorator")
        .unwrap();
    let bar = container
        .get_definition(".decorator.bar.decorator")
        .unwrap();
    assert!(!foo.ptr_eq(&bar));
    assert!(!foo.has_tag("container.tag_decorator"));
    assert!(!foo.has_tag("container.excluded"));
    let decorated = foo.decorated_service().unwrap();
    assert_eq!("foo", decorated.id);
    assert_eq!(2, decorated.priority);
    assert_eq!("bar", bar.decorated_service().unwrap().id);
}
