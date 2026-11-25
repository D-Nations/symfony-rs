//! Tests/Compiler/ReplaceAliasByActualDefinitionPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ReplaceAliasByActualDefinitionPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value, array::Key};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, ReplaceAliasByActualDefinitionPass,
};
use symfony_dependency_injection::{ContainerBuilder, Definition, Reference, Result};

fn process(container: &mut ContainerBuilder) -> Result<()> {
    ReplaceAliasByActualDefinitionPass::new().process(container)
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();

    let a_definition = container.register("a", Some("\\stdClass")).unwrap();
    a_definition.set_public(true);
    a_definition.set_factory(Value::Array(PhpArray::list([
        Reference::new("b").into(),
        Value::from("createA"),
    ])));

    let b_definition = Definition::with_class("\\stdClass");
    container.set_definition("b", b_definition).unwrap();

    container
        .set_alias("a_alias", "a")
        .unwrap()
        .set_public(true)
        .set_deprecated("foo/bar", "1.2", "%alias_id%")
        .unwrap();
    container
        .set_alias("b_alias", "b")
        .unwrap()
        .set_public(true)
        .set_deprecated("foo/bar", "1.2", "%alias_id%")
        .unwrap();

    container
        .set_alias("container", "service_container")
        .unwrap();

    process(&mut container).unwrap();

    assert!(
        container.has("a"),
        "->process() does nothing to public definitions."
    );
    assert!(container.has_alias("a_alias"));
    assert!(container.get_alias("a_alias").unwrap().is_deprecated());
    assert!(
        !container.has("b"),
        "->process() removes non-public definitions."
    );
    assert!(
        container.has("b_alias") && !container.has_alias("b_alias"),
        "->process() replaces alias to actual."
    );
    assert!(
        container
            .get_definition("b_alias")
            .unwrap()
            .has_tag("container.private")
    );

    assert!(container.has("container"));

    let resolved_factory = a_definition.factory();
    let first = resolved_factory
        .as_array()
        .unwrap()
        .get(&Key::Int(0))
        .unwrap();
    assert_eq!("b_alias", Reference::from_value(first).unwrap().id());
}

#[test]
fn testProcessWithInvalidAlias() {
    let mut container = ContainerBuilder::new();
    container.set_alias("a_alias", "a").unwrap();
    let e = process(&mut container).unwrap_err();
    assert!(e.instance_of("InvalidArgumentException"));
}

#[test]
fn testProcessDoesNotInvertAliasChainWithDeprecatedAlias() {
    let mut container = ContainerBuilder::new();

    // Original service (will be decorated)
    container.register("my_service", Some("stdClass")).unwrap();

    // Private decorator that decorates the service
    container
        .register("my_decorator", Some("stdClass"))
        .unwrap()
        .set_decorated_service(
            Some("my_service"),
            None,
            0,
            symfony_dependency_injection::InvalidBehavior::Exception,
        )
        .unwrap()
        .set_public(false);

    // Simulate the state after DecoratorServicePass:
    container.remove_definition("my_service");
    container
        .set_alias("my_service", "my_decorator")
        .unwrap()
        .set_public(true);

    // Deprecated alias pointing to the original service ID
    container
        .set_alias("MyServiceInterface", "my_decorator")
        .unwrap()
        .set_public(true)
        .set_deprecated(
            "my/package",
            "1.0",
            "The \"%alias_id%\" alias is deprecated.",
        )
        .unwrap();

    process(&mut container).unwrap();

    // The non-deprecated alias (my_service) should be used for renaming,
    // NOT the deprecated one (MyServiceInterface)
    assert!(
        container.has_definition("my_service"),
        "my_service should be the definition, not an alias"
    );
    assert!(
        !container.has_alias("my_service"),
        "my_service should not be an alias"
    );

    // The deprecated alias should remain an alias pointing to my_service
    assert!(
        container.has_alias("MyServiceInterface"),
        "MyServiceInterface should be an alias"
    );
    assert_eq!(
        "my_service",
        container.get_alias("MyServiceInterface").unwrap().id()
    );
    assert!(
        container
            .get_alias("MyServiceInterface")
            .unwrap()
            .is_deprecated()
    );
}
