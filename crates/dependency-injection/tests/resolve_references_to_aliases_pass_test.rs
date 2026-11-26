//! Tests/Compiler/ResolveReferencesToAliasesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; `expectUserDeprecationMessage()` collects the deprecations
//! the pass triggers.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveReferencesToAliasesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{Value, array::Key, php_array};
use symfony_dependency_injection::compiler::{
    CompilerPassInterface, ResolveReferencesToAliasesPass,
};
use symfony_dependency_injection::{
    Alias, ContainerBuilder, Definition, ErrorKind, Reference, Result,
};
use symfony_deprecation_contracts::collect;

fn process(container: &mut ContainerBuilder) -> Result<()> {
    let mut pass = ResolveReferencesToAliasesPass::new();
    pass.process(container)
}

fn reference_id(value: &Value) -> String {
    Reference::from_value(value).unwrap().id().to_string()
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    container.set_alias("bar", "foo").unwrap();
    let def = container.register("moo", None).unwrap();
    def.set_arguments(php_array![Reference::new("bar")]);

    process(&mut container).unwrap();

    let arguments = def.arguments();
    assert_eq!("foo", reference_id(arguments.get(&Key::Int(0)).unwrap()));
}

#[test]
fn testProcessRecursively() {
    let mut container = ContainerBuilder::new();
    container.set_alias("bar", "foo").unwrap();
    container.set_alias("moo", "bar").unwrap();
    let def = container.register("foobar", None).unwrap();
    def.set_arguments(php_array![Reference::new("moo")]);

    process(&mut container).unwrap();

    let arguments = def.arguments();
    assert_eq!("foo", reference_id(arguments.get(&Key::Int(0)).unwrap()));
}

#[test]
fn testAliasCircularReference() {
    let mut container = ContainerBuilder::new();
    container.set_alias("bar", "foo").unwrap();
    container.set_alias("foo", "bar").unwrap();
    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
}

#[test]
fn testResolveFactory() {
    let mut container = ContainerBuilder::new();
    container.register("factory", Some("Factory")).unwrap();
    container
        .set_alias("factory_alias", Alias::new("factory", false))
        .unwrap();
    let foo = Definition::default();
    foo.set_factory(php_array![Reference::new("factory_alias"), "createFoo"]);
    container.set_definition("foo", foo).unwrap();
    let bar = Definition::default();
    bar.set_factory(php_array!["Factory", "createFoo"]);
    container.set_definition("bar", bar).unwrap();

    process(&mut container).unwrap();

    let resolved_foo_factory = container.get_definition("foo").unwrap().factory();
    let resolved_bar_factory = container.get_definition("bar").unwrap().factory();

    assert_eq!(
        "factory",
        reference_id(
            resolved_foo_factory
                .as_array()
                .unwrap()
                .get(&Key::Int(0))
                .unwrap()
        )
    );
    assert_eq!(
        Some(&Value::from("Factory")),
        resolved_bar_factory.as_array().unwrap().get(&Key::Int(0))
    );
}

fn messages(deprecations: &[symfony_deprecation_contracts::Deprecation]) -> Vec<String> {
    deprecations
        .iter()
        .map(|d| String::from_utf8_lossy(&d.message).into_owned())
        .collect()
}

#[test]
fn testDeprecationNoticeWhenReferencedByAlias() {
    let mut container = ContainerBuilder::new();

    container.register("foo", Some("stdClass")).unwrap();

    let alias_deprecated = Alias::new("foo", false);
    alias_deprecated
        .set_deprecated("foobar", "1.2.3.4", "")
        .unwrap();
    container
        .set_alias("deprecated_foo_alias", alias_deprecated)
        .unwrap();

    let alias = Alias::new("deprecated_foo_alias", false);
    container.set_alias("alias", alias).unwrap();

    let (result, deprecations) = collect(|| process(&mut container));
    result.unwrap();
    assert!(messages(&deprecations).contains(&"Since foobar 1.2.3.4: The \"deprecated_foo_alias\" service alias is deprecated. You should stop using it, as it will be removed in the future. It is being referenced by the \"alias\" alias.".to_string()));
}

#[test]
fn testDeprecationNoticeWhenReferencedByDefinition() {
    let mut container = ContainerBuilder::new();

    container.register("foo", Some("stdClass")).unwrap();

    let alias_deprecated = Alias::new("foo", false);
    alias_deprecated
        .set_deprecated("foobar", "1.2.3.4", "")
        .unwrap();
    container
        .set_alias("foo_aliased", alias_deprecated)
        .unwrap();

    container
        .register("definition", None)
        .unwrap()
        .set_arguments(php_array![Reference::new("foo_aliased")]);

    let (result, deprecations) = collect(|| process(&mut container));
    result.unwrap();
    assert!(messages(&deprecations).contains(&"Since foobar 1.2.3.4: The \"foo_aliased\" service alias is deprecated. You should stop using it, as it will be removed in the future. It is being referenced by the \"definition\" service.".to_string()));
}

#[test]
fn testNoDeprecationNoticeWhenReferencedByDeprecatedAlias() {
    let mut container = ContainerBuilder::new();

    container.register("foo", Some("stdClass")).unwrap();

    let alias_deprecated = Alias::new("foo", false);
    alias_deprecated
        .set_deprecated("foobar", "1.2.3.4", "")
        .unwrap();
    container
        .set_alias("deprecated_foo_alias", alias_deprecated)
        .unwrap();

    let alias = Alias::new("deprecated_foo_alias", false);
    alias.set_deprecated("foobar", "1.2.3.4", "").unwrap();
    container.set_alias("alias", alias).unwrap();

    let (result, deprecations) = collect(|| process(&mut container));
    result.unwrap();
    assert_eq!(
        deprecations,
        [] as [symfony_deprecation_contracts::Deprecation; 0]
    );
}

#[test]
fn testNoDeprecationNoticeWhenReferencedByDeprecatedDefinition() {
    let mut container = ContainerBuilder::new();

    container.register("foo", Some("stdClass")).unwrap();

    let alias_deprecated = Alias::new("foo", false);
    alias_deprecated
        .set_deprecated("foobar", "1.2.3.4", "")
        .unwrap();
    container
        .set_alias("foo_aliased", alias_deprecated)
        .unwrap();

    container
        .register("definition", None)
        .unwrap()
        .set_deprecated("foobar", "1.2.3.4", "")
        .unwrap()
        .set_arguments(php_array![Reference::new("foo_aliased")]);

    let (result, deprecations) = collect(|| process(&mut container));
    result.unwrap();
    assert_eq!(
        deprecations,
        [] as [symfony_deprecation_contracts::Deprecation; 0]
    );
}

#[test]
fn testDeprecationIsPreservedWhenResolvingAliasChain() {
    let mut container = ContainerBuilder::new();

    // Create a chain: deprecated_alias -> intermediate_alias -> service
    container.register("service", Some("stdClass")).unwrap();

    container
        .set_alias("intermediate_alias", "service")
        .unwrap();

    let deprecated_alias = Alias::new("intermediate_alias", false);
    deprecated_alias.set_public(true);
    deprecated_alias
        .set_deprecated(
            "my/package",
            "1.0",
            "The \"%alias_id%\" alias is deprecated.",
        )
        .unwrap();
    container
        .set_alias("deprecated_alias", deprecated_alias)
        .unwrap();

    process(&mut container).unwrap();

    // After resolving, deprecated_alias should point directly to service
    // but should still be deprecated
    let resolved_alias = container.get_alias("deprecated_alias").unwrap();
    assert_eq!("service", resolved_alias.id());
    assert!(
        resolved_alias.is_deprecated(),
        "Deprecation should be preserved when resolving alias chain"
    );
    assert_eq!(
        "my/package",
        resolved_alias
            .deprecation("deprecated_alias")
            .unwrap()
            .package
    );
}
