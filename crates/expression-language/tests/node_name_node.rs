//! Node/NameNodeTest.php at symfony/expression-language v8.1.9. Names and
//! assertions are PHPUnit's.
//!
//! Upstream: symfony/expression-language Tests/Node/NameNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, no_functions};
use php_value::php_array;
use symfony_expression_language::node::{NameNode, Node};

#[test]
fn testEvaluate() {
    assert_evaluate(
        "bar",
        &Node::Name(NameNode::new("foo")),
        &php_array!["foo" => "bar"],
        &no_functions(),
    );
}

#[test]
fn testCompile() {
    assert_compile("$foo", &Node::Name(NameNode::new("foo")), &no_functions());
}

#[test]
fn testDump() {
    assert_dump("foo", &Node::Name(NameNode::new("foo")));
}
