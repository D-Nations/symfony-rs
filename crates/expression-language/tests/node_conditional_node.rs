//! Node/ConditionalNodeTest.php at symfony/expression-language v8.1.9.
//! Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/expression-language Tests/Node/ConditionalNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, c, no_functions, no_values};
use symfony_expression_language::node::{ConditionalNode, Node};

fn node(condition: bool) -> Node {
    Node::Conditional(ConditionalNode::new(c(condition), c(1), c(2)))
}

#[test]
fn testEvaluate() {
    assert_evaluate(1, &node(true), &no_values(), &no_functions());
    assert_evaluate(2, &node(false), &no_values(), &no_functions());
}

#[test]
fn testCompile() {
    assert_compile("((true) ? (1) : (2))", &node(true), &no_functions());
    assert_compile("((false) ? (1) : (2))", &node(false), &no_functions());
}

#[test]
fn testDump() {
    assert_dump("(true ? 1 : 2)", &node(true));
    assert_dump("(false ? 1 : 2)", &node(false));
}
