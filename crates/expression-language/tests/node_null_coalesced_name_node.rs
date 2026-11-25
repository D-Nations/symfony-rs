//! Node/NullCoalescedNameNodeTest.php at symfony/expression-language
//! v8.1.9. Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/expression-language Tests/Node/NullCoalescedNameNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, no_functions, no_values};
use php_value::Value;
use symfony_expression_language::node::{Node, NullCoalescedNameNode};

fn node() -> Node {
    Node::NullCoalescedName(NullCoalescedNameNode::new("foo"))
}

#[test]
fn testEvaluate() {
    assert_evaluate(Value::Null, &node(), &no_values(), &no_functions());
}

#[test]
fn testCompile() {
    assert_compile("$foo ?? null", &node(), &no_functions());
}

#[test]
fn testDump() {
    assert_dump("foo ?? null", &node());
}
