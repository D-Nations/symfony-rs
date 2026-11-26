//! Tests/Compiler/ResolveClassPassTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's; data providers are
//! loops. Fixture classes are class metadata: `CaseSensitiveClass`, the
//! test class itself, and for the autoloader test a class whose
//! interface is not registered (it fails to load, as in PHP).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveClassPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::Arc;

use symfony_dependency_injection::class_metadata::{ClassMetadata, ClassRegistry};
use symfony_dependency_injection::compiler::{CompilerPassInterface, ResolveClassPass};
use symfony_dependency_injection::{ContainerBuilder, Definition, DefinitionError, ErrorKind};

const CASE_SENSITIVE_CLASS: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Fixtures\\CaseSensitiveClass";
const SELF_CLASS: &str =
    "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\ResolveClassPassTest";

fn container() -> ContainerBuilder {
    let mut registry = ClassRegistry::new();
    registry.add_class(ClassMetadata::class(CASE_SENSITIVE_CLASS));
    registry.add_class(ClassMetadata::class(SELF_CLASS));
    registry.add_class(ClassMetadata::class("stdClass"));
    registry.add_class(ClassMetadata::class("DateTimeImmutable"));
    registry.add_class(
        ClassMetadata::class("Acme\\ClassImplementsUnavailableInterface").with_interfaces(&[
            "Symfony\\Component\\DependencyInjection\\Tests\\Compiler\\UnavailableInterface",
        ]),
    );
    let mut container = ContainerBuilder::new();
    container.set_class_registry(Arc::new(registry));
    container
}

fn process(container: &mut ContainerBuilder) -> symfony_dependency_injection::Result<()> {
    ResolveClassPass::new().process(container)
}

#[test]
fn testResolveClassFromId() {
    // provideValidClassId
    let service_id = CASE_SENSITIVE_CLASS;
    let mut container = container();
    let def = container.register(service_id, None).unwrap();

    process(&mut container).unwrap();

    assert_eq!(Some(service_id.to_string()), def.class());
}

#[test]
fn testWontResolveClassFromId() {
    for service_id in ["stdClass", "bar", "DateTimeImmutable"] {
        let mut container = container();
        let def = container.register(service_id, None).unwrap();

        process(&mut container).unwrap();

        assert_eq!(None, def.class());
    }
}

#[test]
fn testNonFqcnChildDefinition() {
    let mut container = container();
    let parent = container
        .register("App\\Foo.parent", Some("App\\Foo"))
        .unwrap();
    let child = container
        .set_definition("App\\Foo.child", Definition::child("App\\Foo"))
        .unwrap();

    process(&mut container).unwrap();

    assert_eq!(Some("App\\Foo".to_string()), parent.class());
    assert_eq!(None, child.class());
}

#[test]
fn testClassFoundChildDefinition() {
    let mut container = container();
    let parent = container.register("foo.parent", Some("App\\Foo")).unwrap();
    let child = container
        .set_definition(SELF_CLASS, Definition::child("App\\Foo"))
        .unwrap();

    process(&mut container).unwrap();

    assert_eq!(Some("App\\Foo".to_string()), parent.class());
    assert_eq!(Some(SELF_CLASS.to_string()), child.class());
}

#[test]
fn testAmbiguousChildDefinition() {
    let mut container = container();
    container.register("app.foo", Some("App\\Foo")).unwrap();
    container
        .set_definition("App\\Foo\\Child", Definition::child("App\\Foo"))
        .unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Service definition \"App\\Foo\\Child\" has a parent but no class, and its name looks like a FQCN. Either the class is missing or you want to inherit it from the parent service. To resolve this ambiguity, please rename this service to a non-FQCN (e.g. using dots), or create the missing class.",
        e.message()
    );
}

#[test]
fn testCircularParentDefinitions() {
    let mut container = container();
    container
        .set_definition("a", Definition::child("b"))
        .unwrap();
    let b = Definition::child("a");
    b.set_class(Some(CASE_SENSITIVE_CLASS));
    container.set_definition("b", b).unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
    assert_eq!(
        "Circular reference detected for service \"b\", path: \"b -> a -> b\".",
        e.message()
    );
}

#[test]
fn testSelfParentDefinition() {
    let mut container = container();
    container
        .set_definition("a", Definition::child("a"))
        .unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::ServiceCircularReference));
    assert_eq!(
        "Circular reference detected for service \"a\", path: \"a -> a\".",
        e.message()
    );
}

#[test]
fn testMissingParentDefinitionIsLeftToResolveChildDefinitions() {
    let mut container = container();
    container
        .set_definition("a", Definition::child("b"))
        .unwrap();
    container
        .set_definition("b", Definition::child("missing"))
        .unwrap();

    process(&mut container).unwrap();

    assert_eq!(None, container.get_definition("a").unwrap().class());
}

#[test]
fn testSkipsDefinitionsWithErrors() {
    let mut container = container();
    let def = container.register("App\\SomeClass", None).unwrap();
    def.add_error(DefinitionError::Text("Some error message".to_string()));

    process(&mut container).unwrap();

    // The class should not be set because the definition has errors
    assert_eq!(None, def.class());
}

#[test]
fn testInvalidClassNameDefinition() {
    let mut container = container();
    container.register("Acme\\UnknownClass", None).unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Service id \"Acme\\UnknownClass\" looks like a FQCN but no corresponding class or interface exists. To resolve this ambiguity, please rename this service to a non-FQCN (e.g. using dots), or create the missing class or interface.",
        e.message()
    );
}

#[test]
fn testInvalidClassWhoseImplementedInterfaceIsMissingDefinition() {
    let mut container = container();
    container
        .register("Acme\\ClassImplementsUnavailableInterface", None)
        .unwrap();

    let e = process(&mut container).unwrap_err();
    assert!(e.is(&ErrorKind::InvalidArgument));
    assert_eq!(
        "Service id \"Acme\\ClassImplementsUnavailableInterface\" looks like a FQCN but no corresponding class or interface exists. To resolve this ambiguity, please rename this service to a non-FQCN (e.g. using dots), or create the missing class or interface.",
        e.message()
    );
}
