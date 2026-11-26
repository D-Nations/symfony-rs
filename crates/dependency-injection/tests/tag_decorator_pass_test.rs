//! Tests/Compiler/TagDecoratorPassTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/TagDecoratorPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, DecoratorServicePass, TagDecoratorPass,
};
use symfony_dependency_injection::{ContainerBuilder, ErrorKind, InvalidBehavior, Result};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    TagDecoratorPass.process(container)?;
    DecoratorServicePass::new().process(container)
}

fn alias(container: &ContainerBuilder, id: &str) -> String {
    container.get_alias(id).unwrap().id()
}

#[test]
fn testDecorateByTag() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_tag("tag", PhpArray::new());
    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .add_tag("tag", PhpArray::new());
    container
        .register("decorator", Some("stdClass"))
        .unwrap()
        .add_resource_tag(
            "container.tag_decorator",
            php_array!["decorates_tag" => "tag"],
        );

    process(&mut container).unwrap();

    assert!(!container.has("decorator"));

    assert!(container.has(".decorator.foo.decorator"));
    assert!(container.has(".decorator.bar.decorator"));

    assert_eq!(".decorator.foo.decorator", alias(&container, "foo"));
    assert_eq!(".decorator.bar.decorator", alias(&container, "bar"));

    assert!(container.has(".decorator.foo.decorator.inner"));
    assert!(container.has(".decorator.bar.decorator.inner"));
}

#[test]
fn testDecorateByTagPriority() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_tag("tag", PhpArray::new());
    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .add_tag("tag", PhpArray::new());
    container
        .register("decorator1", Some("stdClass"))
        .unwrap()
        .add_resource_tag(
            "container.tag_decorator",
            php_array!["decorates_tag" => "tag", "priority" => 2],
        );
    container
        .register("decorator2", Some("stdClass"))
        .unwrap()
        .add_resource_tag(
            "container.tag_decorator",
            php_array!["decorates_tag" => "tag", "priority" => 1],
        );

    process(&mut container).unwrap();

    assert!(!container.has("decorator1"));
    assert!(!container.has("decorator2"));

    assert!(container.has(".decorator.foo.decorator1"));
    assert!(container.has(".decorator.foo.decorator2"));
    assert!(container.has(".decorator.bar.decorator1"));
    assert!(container.has(".decorator.bar.decorator2"));

    assert_eq!(".decorator.foo.decorator2", alias(&container, "foo"));
    assert_eq!(".decorator.bar.decorator2", alias(&container, "bar"));

    assert_eq!(
        ".decorator.foo.decorator1",
        alias(&container, ".decorator.foo.decorator2.inner")
    );
    assert_eq!(
        ".decorator.bar.decorator1",
        alias(&container, ".decorator.bar.decorator2.inner")
    );
}

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

    process(&mut container).unwrap();
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

    process(&mut container).unwrap();
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

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceNotFound));
}

/// testDecorateByTag's decorators, before `DecoratorServicePass`.
#[test]
fn decorators_are_cloned_per_tagged_service() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_tag("tag", PhpArray::new());
    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .add_tag("tag", PhpArray::new());
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
