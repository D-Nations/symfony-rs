//! Node/ArrayNodeTest.php at symfony/expression-language v8.1.9 (with
//! `AbstractNodeTestCase`'s tests). Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/expression-language Tests/Node/ArrayNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, c, no_functions, no_values};
use php_value::php_array;
use symfony_expression_language::node::{ArrayNode, Node};

fn get_array_node() -> Node {
    let mut array = ArrayNode::new();
    array.add_element(c("a"), Some(c("b")));
    array.add_element(c("b"), None);
    Node::Array(array)
}

#[test]
fn testEvaluate() {
    assert_evaluate(
        php_array!["b" => "a", 0 => "b"],
        &get_array_node(),
        &no_values(),
        &no_functions(),
    );
}

#[test]
fn testCompile() {
    assert_compile(
        "[\"b\" => \"a\", 0 => \"b\"]",
        &get_array_node(),
        &no_functions(),
    );
}

#[test]
fn testDump() {
    assert_dump("{\"b\": \"a\", 0: \"b\"}", &get_array_node());

    let mut array = ArrayNode::new();
    array.add_element(c("c"), Some(c("a\"b")));
    array.add_element(c("d"), Some(c("a\\b")));
    assert_dump(
        "{\"a\\\"b\": \"c\", \"a\\\\b\": \"d\"}",
        &Node::Array(array),
    );

    let mut array = ArrayNode::new();
    array.add_element(c("c"), None);
    array.add_element(c("d"), None);
    assert_dump("[\"c\", \"d\"]", &Node::Array(array));
}
