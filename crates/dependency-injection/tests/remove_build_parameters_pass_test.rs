//! Tests/Compiler/RemoveBuildParametersPassTest.php at
//! symfony/dependency-injection 9b34cad8. Names and assertions are
//! PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/Compiler/RemoveBuildParametersPassTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use php_value::{PhpArray, Value};
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::compiler::{CompilerPassInterface, RemoveBuildParametersPass};

fn array(entries: &[(&str, Value)]) -> PhpArray {
    let mut array = PhpArray::new();
    for (k, v) in entries {
        array.insert(*k, v.clone());
    }
    array
}

#[test]
fn testBuildParametersShouldBeRemoved() {
    let mut builder = ContainerBuilder::new();
    builder.set_parameter("foo", "Foo").unwrap();
    builder.set_parameter(".bar", "Bar").unwrap();

    let mut pass = RemoveBuildParametersPass::default();
    pass.process(&mut builder).unwrap();

    assert!(
        Value::from("Foo").identical(&builder.get_parameter("foo").unwrap()),
        "\"foo\" parameter must be defined."
    );
    assert!(
        !builder.has_parameter(".bar"),
        "\".bar\" parameter must be removed."
    );
    assert!(
        Value::Array(array(&[(".bar", Value::from("Bar"))]))
            .identical(&Value::Array(pass.get_removed_parameters().clone())),
        "\".bar\" parameter must be returned with its value."
    );
}

#[test]
fn testArrayBuildParametersArePreservedWhenConfigured() {
    let mut builder = ContainerBuilder::new();
    builder.set_parameter("foo", "Foo").unwrap();
    builder.set_parameter(".scalar", "Bar").unwrap();
    let baz = Value::Array(array(&[("baz", Value::from("qux"))]));
    builder.set_parameter(".array", baz.clone()).unwrap();

    let mut pass = RemoveBuildParametersPass::new(true);
    pass.process(&mut builder).unwrap();

    assert!(
        Value::from("Foo").identical(&builder.get_parameter("foo").unwrap()),
        "\"foo\" parameter must be defined."
    );
    assert!(
        !builder.has_parameter(".scalar"),
        "\".scalar\" parameter must be removed."
    );
    assert!(
        builder.has_parameter(".array"),
        "\".array\" parameter must be preserved."
    );
    assert!(
        baz.identical(&builder.get_parameter(".array").unwrap()),
        "\".array\" parameter must retain its value."
    );
    assert!(
        Value::Array(array(&[(".scalar", Value::from("Bar"))]))
            .identical(&Value::Array(pass.get_removed_parameters().clone())),
        "Only \".scalar\" parameter must be returned as removed."
    );

    let log = builder.get_compiler().get_log();
    let class = RemoveBuildParametersPass::CLASS;
    assert!(log.contains(&format!("{class}: Removing build parameter \".scalar\".")));
    assert!(log.contains(&format!(
        "{class}: Keeping array build parameter \".array\" for placeholder resolution."
    )));
}

#[test]
fn testNonArrayBuildParametersAreAlwaysRemoved() {
    let mut builder = ContainerBuilder::new();
    builder.set_parameter(".scalar", "Bar").unwrap();
    let baz = Value::Array(array(&[("baz", Value::from("qux"))]));
    builder.set_parameter(".array", baz.clone()).unwrap();

    let mut pass = RemoveBuildParametersPass::default();
    pass.process(&mut builder).unwrap();

    assert!(
        !builder.has_parameter(".scalar"),
        "\".scalar\" parameter must be removed."
    );
    assert!(
        !builder.has_parameter(".array"),
        "\".array\" parameter must be removed."
    );
    assert!(
        Value::Array(array(&[(".scalar", Value::from("Bar")), (".array", baz)]))
            .identical(&Value::Array(pass.get_removed_parameters().clone())),
        "Both parameters must be returned as removed."
    );
}
