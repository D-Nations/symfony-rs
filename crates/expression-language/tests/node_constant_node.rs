//! Node/ConstantNodeTest.php at symfony/expression-language v8.1.9.
//! Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/expression-language Tests/Node/ConstantNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, c, no_functions, no_values};
use php_value::{Value, php_array};
use symfony_expression_language::node::{ConstantNode, Node};

#[test]
fn testEvaluate() {
    let cases: Vec<(Value, Node)> = vec![
        (false.into(), c(false)),
        (true.into(), c(true)),
        (Value::Null, c(Value::Null)),
        (3.into(), c(3)),
        (3.3.into(), c(3.3)),
        ("foo".into(), c("foo")),
        (
            php_array![0 => 1, "b" => "a"].into(),
            c(php_array![0 => 1, "b" => "a"]),
        ),
    ];
    for (expected, node) in cases {
        assert_evaluate(expected, &node, &no_values(), &no_functions());
    }
}

#[test]
#[expect(clippy::excessive_precision, reason = "upstream's literal")]
fn testCompile() {
    let cases: Vec<(&str, Node)> = vec![
        // Booleans
        ("false", c(false)),
        ("true", c(true)),
        // Null
        ("null", c(Value::Null)),
        // Integers
        ("3", c(3)),
        ("-10", c(-10)),
        ("0", c(0)),
        // Floats
        ("3.3", c(3.3)),
        ("42.0", c(42.0)),
        ("-1.23", c(-1.23)),
        ("0.1", c(0.1)),
        ("1.0", c(1.0)),
        ("1.0E-6", c(1.0e-6)),
        ("1.23456789E+20", c(1.234_567_89e20)),
        ("3.3", c(3.299_999_999_999_999_8)),
        ("0.30000000000000004", c(0.1 + 0.2)),
        ("INF", c(f64::INFINITY)),
        ("-INF", c(f64::NEG_INFINITY)),
        ("NAN", c(f64::NAN)),
        // Strings
        ("\"foo\"", c("foo")),
        ("\"\"", c("")),
        ("\"a\\\"b\"", c("a\"b")),
        // Arrays
        (
            "[0 => 1, \"b\" => \"a\"]",
            c(php_array![0 => 1, "b" => "a"]),
        ),
        ("[]", c(php_array![])),
    ];
    for (expected, node) in cases {
        assert_compile(expected, &node, &no_functions());
    }
}

#[test]
fn testDump() {
    let cases: Vec<(&str, Node)> = vec![
        ("false", c(false)),
        ("true", c(true)),
        ("null", c(Value::Null)),
        ("3", c(3)),
        ("3.3", c(3.3)),
        ("\"foo\"", c("foo")),
        (
            "foo",
            Node::Constant(ConstantNode::with("foo", true, false)),
        ),
        (
            "{0: 1, \"b\": \"a\", 1: true}",
            c(php_array![0 => 1, "b" => "a", 1 => true]),
        ),
        (
            "{\"a\\\"b\": \"c\", \"a\\\\b\": \"d\"}",
            c(php_array!["a\"b" => "c", "a\\b" => "d"]),
        ),
        ("[\"c\", \"d\"]", c(php_array![0 => "c", 1 => "d"])),
        (
            "{\"a\": [\"b\"]}",
            c(php_array!["a" => php_array![0 => "b"]]),
        ),
    ];
    for (expected, node) in cases {
        assert_dump(expected, &node);
    }
}
