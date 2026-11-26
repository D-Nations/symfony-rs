//! Node/UnaryNodeTest.php at symfony/expression-language v8.1.9. Names
//! and assertions are PHPUnit's.
//!
//! Upstream: symfony/expression-language Tests/Node/UnaryNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, c, no_functions, no_values};
use php_value::Value;
use symfony_expression_language::node::{Node, UnaryNode};

fn u(operator: &str, value: impl Into<Value>) -> Node {
    Node::Unary(UnaryNode::new(operator, c(value)))
}

#[test]
fn testEvaluate() {
    let cases: Vec<(Value, Node)> = vec![
        ((-1).into(), u("-", 1)),
        (3.into(), u("+", 3)),
        (3.into(), u("+", "3")),
        (1.5.into(), u("+", "1.5")),
        (false.into(), u("!", true)),
        (false.into(), u("not", true)),
        ((-6).into(), u("~", 5)),
    ];
    for (expected, node) in cases {
        assert_evaluate(expected, &node, &no_values(), &no_functions());
    }
}

#[test]
fn testCompile() {
    assert_compile("(-1)", &u("-", 1), &no_functions());
    assert_compile("(+3)", &u("+", 3), &no_functions());
    assert_compile("(!true)", &u("!", true), &no_functions());
    assert_compile("(!true)", &u("not", true), &no_functions());
    assert_compile("(~5)", &u("~", 5), &no_functions());
}

#[test]
fn testDump() {
    assert_dump("(- 1)", &u("-", 1));
    assert_dump("(+ 3)", &u("+", 3));
    assert_dump("(! true)", &u("!", true));
    assert_dump("(not true)", &u("not", true));
    assert_dump("(~ 5)", &u("~", 5));
}
