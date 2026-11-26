//! Tests/Compiler/ResolveEnvPlaceholdersPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's. The pass's `$format` is an `EnvFormat` (`null` is
//! `Placeholder`, `true` is `Resolve`).
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/ResolveEnvPlaceholdersPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::Value;
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::argument::EnvClosureArgument;
use symfony_dependency_injection::compiler::{CompilerPassInterface, ResolveEnvPlaceholdersPass};
use symfony_dependency_injection::container_builder::EnvFormat;

fn env_placeholder(container: &ContainerBuilder, name: &str) -> String {
    container
        .get_parameter_bag()
        .resolve_value(&Value::from(name), &[])
        .unwrap()
        .to_php_string()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn testEnvClosureArgumentIsFormatted() {
    let mut container = ContainerBuilder::new();
    let placeholder = env_placeholder(&container, "%env(FOO)%");
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_argument(EnvClosureArgument::new(placeholder, Value::from("bar"), true).unwrap());

    ResolveEnvPlaceholdersPass::new(EnvFormat::Placeholder)
        .process(&mut container)
        .unwrap();

    let argument = container
        .get_definition("foo")
        .unwrap()
        .argument(0)
        .unwrap();
    let argument = EnvClosureArgument::from_value(&argument)
        .expect("assertInstanceOf(EnvClosureArgument::class)");
    assert_eq!("%env(FOO)%", argument.value());
    assert_eq!(&Value::from("bar"), argument.default_value());
    assert!(argument.is_stringable());
}

#[test]
fn testEnvClosureArgumentIsNotResolvedToTheValueOfTheEnvVar() {
    let mut container = ContainerBuilder::new();
    let placeholder = env_placeholder(&container, "%env(FOO)%");
    let argument = Value::from(EnvClosureArgument::new(placeholder, Value::Null, false).unwrap());
    container
        .register("foo", Some("stdClass"))
        .unwrap()
        .add_argument(argument.clone());

    ResolveEnvPlaceholdersPass::default()
        .process(&mut container)
        .unwrap();

    assert_eq!(
        argument,
        container
            .get_definition("foo")
            .unwrap()
            .argument(0)
            .unwrap()
    );
}
