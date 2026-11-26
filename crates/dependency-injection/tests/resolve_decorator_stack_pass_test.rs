//! Tests/Compiler/ResolveDecoratorStackPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveDecoratorStackPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key, php_array};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, DecoratorServicePass, ResolveDecoratorStackPass,
};
use symfony_dependency_injection::{
    ContainerBuilder, Definition, ErrorKind, InvalidBehavior, Reference,
};

fn std_class() -> Definition {
    Definition::with_class("stdClass")
}

/// `new Definition(\stdClass::class)` labelled `label` with an `.inner`
/// property.
fn labelled(label: &str) -> Definition {
    let definition = std_class();
    definition
        .set_property("label", label)
        .set_property("inner", Reference::new(".inner"));
    definition
}

/// `str_ends_with($id, '.inner')` is case-sensitive.
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn has_inner_service(container: &ContainerBuilder) -> bool {
    container
        .get_definitions()
        .keys()
        .any(|id| id.ends_with(".inner"))
}

#[test]
fn testStackDecoratesExistingService() {
    let mut container = ContainerBuilder::new();

    // The existing service that should be decorated
    container
        .register("original_service", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .set_property("label", "original");

    // Define a stack that decorates the existing service
    let stack = Definition::child("");
    stack
        .add_tag("container.stack", PhpArray::new())
        .set_arguments(php_array![labelled("A"), labelled("B")])
        .set_decorated_service(
            Some("original_service"),
            None,
            0,
            InvalidBehavior::Exception,
        )
        .unwrap();

    container.set_definition("my_stack", stack).unwrap();

    ResolveDecoratorStackPass::new()
        .process(&mut container)
        .unwrap();
    DecoratorServicePass::new().process(&mut container).unwrap();

    // The original service should now be decorated
    assert!(container.has_alias("original_service"));

    // The innermost definition should wrap the original service
    // and the outermost should be the entry point
    let alias = container.get_alias("my_stack").unwrap();
    let outermost_id = alias.id();

    // The outermost definition should have label 'A'
    let outermost_def = container.get_definition(&outermost_id).unwrap();
    assert!(
        Value::from("A").identical(outermost_def.properties().get(&Key::from("label")).unwrap())
    );

    // Follow the chain: the original service should still be accessible
    assert!(container.has_definition("original_service.inner") || has_inner_service(&container));
}

#[test]
fn testStackDecoratesWithPriority() {
    let mut container = ContainerBuilder::new();

    container
        .register("original_service", Some("stdClass"))
        .unwrap()
        .set_public(true);

    let stack = Definition::child("");
    stack
        .add_tag("container.stack", PhpArray::new())
        .set_arguments(php_array![std_class(), std_class()])
        .set_decorated_service(
            Some("original_service"),
            None,
            5,
            InvalidBehavior::Exception,
        )
        .unwrap();

    container.set_definition("my_stack", stack).unwrap();

    ResolveDecoratorStackPass::new()
        .process(&mut container)
        .unwrap();

    // After resolving the stack, the innermost definition should have
    // the decoratedService set to 'original_service' with priority 5
    let mut innermost_found = false;

    for (id, def) in container.get_definitions().iter() {
        if id.starts_with(".my_stack.")
            && let Some(decorated) = def.decorated_service()
            && decorated.id == "original_service"
        {
            innermost_found = true;
            assert_eq!(5, decorated.priority);
        }
    }

    assert!(
        innermost_found,
        "The innermost stack definition should decorate \"original_service\""
    );
}

#[test]
fn testStackDecoratesWithInvalidBehavior() {
    let mut container = ContainerBuilder::new();

    container
        .register("original_service", Some("stdClass"))
        .unwrap()
        .set_public(true);

    let stack = Definition::child("");
    stack
        .add_tag("container.stack", PhpArray::new())
        .set_arguments(php_array![std_class(), std_class()])
        .set_decorated_service(Some("original_service"), None, 0, InvalidBehavior::Ignore)
        .unwrap();

    container.set_definition("my_stack", stack).unwrap();

    ResolveDecoratorStackPass::new()
        .process(&mut container)
        .unwrap();

    let mut innermost_found = false;

    for (id, def) in container.get_definitions().iter() {
        if id.starts_with(".my_stack.")
            && let Some(decorated) = def.decorated_service()
            && decorated.id == "original_service"
        {
            innermost_found = true;
            assert_eq!(Some(InvalidBehavior::Ignore), decorated.invalid_behavior);
        }
    }

    assert!(innermost_found);
}

#[test]
fn testStackWithoutDecoratesStillWorks() {
    let mut container = ContainerBuilder::new();

    let a = std_class();
    a.set_property("label", "A")
        .set_property("inner", Reference::new(".inner"));
    let b = std_class();
    b.set_property("label", "B");
    let stack = Definition::child("");
    stack
        .add_tag("container.stack", PhpArray::new())
        .set_arguments(php_array![a, b]);
    stack.set_public(true);
    stack.set_changes(vec!["public".to_string()]);

    container.set_definition("my_stack", stack).unwrap();

    ResolveDecoratorStackPass::new()
        .process(&mut container)
        .unwrap();

    // The innermost definition should not decorate any external service
    let stack_defs: Vec<Definition> = container
        .get_definitions()
        .iter()
        .filter(|(id, _)| id.starts_with(".my_stack."))
        .map(|(_, d)| d.clone())
        .collect();
    assert_eq!(2, stack_defs.len());

    let innermost = &stack_defs[0];
    assert_eq!(None, innermost.decorated_service());
}

#[test]
fn testStackDecoratesTag() {
    let mut container = ContainerBuilder::new();

    // Two services tagged with 'my_tag'
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_tag("my_tag", PhpArray::new())
        .set_property("label", "foo");

    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .set_public(true)
        .add_tag("my_tag", PhpArray::new())
        .set_property("label", "bar");

    // A stack that decorates all services tagged 'my_tag'
    let stack = Definition::child("");
    stack
        .add_tag("container.stack", PhpArray::new())
        .add_resource_tag(
            "container.tag_decorator",
            php_array!["decorates_tag" => "my_tag"],
        )
        .set_arguments(php_array![labelled("A"), labelled("B")]);

    container.set_definition("my_stack", stack).unwrap();

    ResolveDecoratorStackPass::new()
        .process(&mut container)
        .unwrap();
    DecoratorServicePass::new().process(&mut container).unwrap();

    // The original stack definition should have been removed
    assert!(!container.has_definition("my_stack"));

    // Both foo and bar should be decorated
    assert!(container.has_alias("foo"));
    assert!(container.has_alias("bar"));
}

#[test]
fn testStackCannotHaveBothDecoratesAndDecoratesTag() {
    let mut container = ContainerBuilder::new();

    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_tag("my_tag", PhpArray::new());

    let stack = Definition::child("");
    stack
        .add_tag("container.stack", PhpArray::new())
        .add_resource_tag(
            "container.tag_decorator",
            php_array!["decorates_tag" => "my_tag"],
        )
        .set_decorated_service(Some("foo"), None, 0, InvalidBehavior::Exception)
        .unwrap()
        .set_arguments(php_array![std_class()]);

    container.set_definition("my_stack", stack).unwrap();

    let e = ResolveDecoratorStackPass::new()
        .process(&mut container)
        .unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert!(
        e.message()
            .contains("cannot have both \"decorates\" and \"decorates_tag\""),
        "{}",
        e.message()
    );
}

/// Not upstream: the shape of a resolved stack (the test of
/// testStackDecoratesTag before `DecoratorServicePass` runs).
#[test]
fn tag_decorator_clones_the_stack_per_tagged_service() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_tag("my_tag", PhpArray::new());
    container
        .register("bar", Some("stdClass"))
        .unwrap()
        .add_tag("my_tag", PhpArray::new());

    let a = std_class();
    a.set_property("label", "A");
    let stack = Definition::child("");
    stack
        .add_tag("container.stack", PhpArray::new())
        .add_resource_tag(
            "container.tag_decorator",
            php_array!["decorates_tag" => "my_tag"],
        )
        .set_arguments(php_array![a.clone(), std_class()]);
    container.set_definition("my_stack", stack).unwrap();

    ResolveDecoratorStackPass::new()
        .process(&mut container)
        .unwrap();

    assert!(!container.has_definition("my_stack"));
    let ids: Vec<&str> = container.get_definitions().keys().collect();
    assert_eq!(
        vec![
            "service_container",
            "foo",
            "bar",
            "..stack.foo.my_stack.1",
            "..stack.foo.my_stack.0",
            "..stack.bar.my_stack.1",
            "..stack.bar.my_stack.0",
        ],
        ids
    );
    assert_eq!(
        "..stack.foo.my_stack.0",
        container.get_alias(".stack.foo.my_stack").unwrap().id()
    );
    let innermost = container
        .get_definition("..stack.foo.my_stack.1")
        .unwrap()
        .decorated_service()
        .unwrap();
    assert_eq!("foo", innermost.id);
    let outer = container.get_definition("..stack.foo.my_stack.0").unwrap();
    assert_eq!(
        "..stack.foo.my_stack.1",
        outer.decorated_service().unwrap().id
    );
    // Each clone has its own definitions.
    assert!(!outer.ptr_eq(&a));
    assert!(!outer.ptr_eq(&container.get_definition("..stack.bar.my_stack.0").unwrap()));
}
