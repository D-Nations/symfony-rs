//! Node/FunctionNodeTest.php at symfony/expression-language v8.1.9.
//! Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/expression-language Tests/Node/FunctionNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, c, no_values};
use php_value::{PhpString, Value};
use symfony_expression_language::{
    ExpressionFunction, Functions,
    node::{FunctionNode, Node},
};

fn get_callables() -> Functions {
    let mut functions = Functions::new();
    functions.insert(ExpressionFunction::new(
        "foo",
        |args: &[PhpString]| {
            Ok(PhpString::from(format!(
                "foo({})",
                args[0].to_string_lossy()
            )))
        },
        |_variables, args: &[Value]| Ok(args[0].clone()),
    ));
    functions
}

fn node() -> Node {
    Node::Function(FunctionNode::new("foo", Node::list(vec![c("bar")])))
}

#[test]
fn testEvaluate() {
    assert_evaluate("bar", &node(), &no_values(), &get_callables());
}

#[test]
fn testCompile() {
    assert_compile("foo(\"bar\")", &node(), &get_callables());
}

#[test]
fn testDump() {
    assert_dump("foo(\"bar\")", &node());
}
