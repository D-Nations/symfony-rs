//! Tests/Compiler/AliasDeprecatedPublicServicesPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's; a data provider's cases run in one test, in order.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/AliasDeprecatedPublicServicesPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value};
use symfony_dependency_injection::compiler::{
    AliasDeprecatedPublicServicesPass, CompilerPassInterface,
};
use symfony_dependency_injection::{ContainerBuilder, Deprecation, ErrorKind};

fn attributes(entries: &[(&str, &str)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (k, v) in entries {
        array.insert(*k, Value::from(*v));
    }
    array
}

#[test]
fn testProcess() {
    let mut container = ContainerBuilder::new();
    container
        .register("foo", None)
        .unwrap()
        .set_public(true)
        .add_tag(
            "container.private",
            attributes(&[("package", "foo/bar"), ("version", "1.2")]),
        );

    AliasDeprecatedPublicServicesPass::new()
        .process(&mut container)
        .unwrap();

    assert!(container.has_alias("foo"));

    let alias = container.get_alias("foo").unwrap();

    assert_eq!(".container.private.foo", alias.id());
    assert!(alias.is_public());
    assert!(!alias.is_private());
    assert_eq!(
        Some(Deprecation {
            package: "foo/bar".to_string(),
            version: "1.2".to_string(),
            message: "Accessing the \"foo\" service directly from the container is deprecated, use dependency injection instead.".to_string(),
        }),
        alias.deprecation("foo")
    );
}

#[test]
fn testProcessWithMissingAttribute() {
    let cases = [
        ("package", attributes(&[("version", "1.2")])),
        ("version", attributes(&[("package", "foo/bar")])),
    ];
    for (attribute, attributes) in cases {
        let mut container = ContainerBuilder::new();
        container
            .register("foo", None)
            .unwrap()
            .add_tag("container.private", attributes);

        let e = AliasDeprecatedPublicServicesPass::new()
            .process(&mut container)
            .unwrap_err();
        assert!(e.is(&ErrorKind::InvalidArgument));
        assert_eq!(
            format!(
                "The \"{attribute}\" attribute is mandatory for the \"container.private\" tag on the \"foo\" service."
            ),
            e.message()
        );
    }
}

#[test]
fn testProcessWithNonPublicService() {
    let mut container = ContainerBuilder::new();
    container.register("foo", None).unwrap().add_tag(
        "container.private",
        attributes(&[("package", "foo/bar"), ("version", "1.2")]),
    );

    AliasDeprecatedPublicServicesPass::new()
        .process(&mut container)
        .unwrap();

    assert!(container.has_definition("foo"));
}
