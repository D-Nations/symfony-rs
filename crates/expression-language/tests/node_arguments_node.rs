//! Node/ArgumentsNodeTest.php at symfony/expression-language v8.1.9 (it
//! extends `ArrayNodeTest`, whose evaluate data it keeps). Names and
//! assertions are PHPUnit's.
//!
//! Upstream: symfony/expression-language Tests/Node/ArgumentsNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, c, no_functions, no_values};
use php_value::php_array;
use symfony_expression_language::node::{ArrayNode, Node};

fn get_array_node() -> Node {
    let mut array = ArrayNode::arguments();
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
    assert_compile("\"a\", \"b\"", &get_array_node(), &no_functions());
}

#[test]
fn testDump() {
    assert_dump("\"a\", \"b\"", &get_array_node());
}
