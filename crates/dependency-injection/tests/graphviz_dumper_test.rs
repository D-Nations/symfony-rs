//! Tests/Dumper/GraphvizDumperTest.php at symfony/dependency-injection
//! 9b34cad8. The fixture containers are built in Rust
//! (support/containers.rs); the expected files are what PHP's
//! GraphvizDumper writes for the same containers
//! (tests/data/dumpers.json, `scripts/record.sh di-dumpers`), which are
//! upstream's `Fixtures/graphviz/*.dot` byte for byte.
//!
//! The test after the upstream ones is an extra: the recorder's "edge"
//! container.
//!
//! Upstream: symfony/dependency-injection Tests/Dumper/GraphvizDumperTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::doc_markdown)]

#[path = "support/containers.rs"]
mod containers;

use containers::*;
use php_value::{PhpArray, Value, php_array};
use symfony_dependency_injection::ContainerBuilder;
use symfony_dependency_injection::dumper::{DumperInterface, GraphvizDumper};

fn dump(container: &mut ContainerBuilder, options: &PhpArray) -> String {
    let out = GraphvizDumper::new(container).dump(options).unwrap();
    String::from_utf8(out.as_bytes().to_vec()).unwrap()
}

/// `assertStringEqualsGeneratedFile()`.
fn assert_recorded(case: &str, mut container: ContainerBuilder) {
    let expected = String::from_utf8(recorded(case).unwrap()).unwrap();
    assert_eq!(expected, dump(&mut container, &PhpArray::new()), "{case}");
}

#[test]
fn testDump() {
    // ->dump() dumps an empty container as an empty dot file
    assert_recorded("dot:empty", ContainerBuilder::new());

    // ->dump() dumps services
    assert_recorded("dot:container9", container9());

    // ->dump() dumps services
    assert_recorded("dot:container10", container10());

    let options = php_array![
        "graph" => Value::Array(php_array!["ratio" => "normal"]),
        "node" => Value::Array(php_array![
            "fontsize" => Value::Int(13),
            "fontname" => "Verdana",
            "shape" => "square",
        ]),
        "edge" => Value::Array(php_array![
            "fontsize" => Value::Int(12),
            "fontname" => "Verdana",
            "color" => "white",
            "arrowhead" => "closed",
            "arrowsize" => Value::Int(1),
        ]),
        "node.instance" => Value::Array(php_array!["fillcolor" => "green", "style" => "empty"]),
        "node.definition" => Value::Array(php_array!["fillcolor" => "grey"]),
        "node.missing" => Value::Array(php_array!["fillcolor" => "red", "style" => "empty"]),
    ];
    let mut container = container10();
    // ->dump() dumps services
    assert_eq!(
        dump(&mut container, &options),
        String::from_utf8(recorded("dot:container10_options").unwrap()).unwrap()
    );
}

#[test]
fn testDumpWithFrozenContainer() {
    // ->dump() dumps services
    assert_recorded("dot:container13", container13());
}

#[test]
fn testDumpWithFrozenCustomClassContainer() {
    // ->dump() dumps services
    assert_recorded("dot:container14", container14());
}

#[test]
fn testDumpWithUnresolvedParameter() {
    // ->dump() dumps services
    assert_recorded("dot:container17", container17());
}

#[test]
fn testDumpWithInlineDefinition() {
    // ->dump() dumps nested references
    assert_recorded("dot:inline_definition", inline_definition());
}

#[test]
fn edge_container_dumps_as_php_does() {
    assert_recorded("dot:edge", edge());
}
