//! Helpers shared by the node tests: `AbstractNodeTestCase`'s three tests.
//!
//! Upstream: symfony/expression-language Tests/Node/AbstractNodeTestCase.php
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use php_value::{PhpArray, Value};
use symfony_expression_language::{Compiler, Functions, Node};

/// `testEvaluate($expected, $node, $variables, $functions)`:
/// `assertSame`.
pub fn assert_evaluate(
    expected: impl Into<Value>,
    node: &Node,
    variables: &PhpArray,
    functions: &Functions,
) {
    let expected = expected.into();
    let actual = node
        .evaluate(functions, variables)
        .unwrap_or_else(|e| panic!("{node}: {e}"));
    assert_eq!(expected, actual, "{node}");
}

/// `testCompile($expected, $node, $functions)`.
pub fn assert_compile(expected: &str, node: &Node, functions: &Functions) {
    let mut compiler = Compiler::new(functions.clone());
    node.compile(&mut compiler)
        .unwrap_or_else(|e| panic!("{node}: {e}"));
    assert_eq!(compiler.source(), expected, "{node}");
}

/// `testDump($expected, $node)`.
pub fn assert_dump(expected: &str, node: &Node) {
    assert_eq!(
        node.dump().unwrap_or_else(|e| panic!("{node}: {e}")),
        expected,
        "{node}"
    );
}

pub fn no_values() -> PhpArray {
    PhpArray::new()
}

pub fn no_functions() -> Functions {
    Functions::new()
}

/// A `ConstantNode` as a `Node`.
pub fn c(value: impl Into<Value>) -> Node {
    Node::Constant(symfony_expression_language::node::ConstantNode::new(value))
}

/// The error message's text.
pub fn message(error: &symfony_expression_language::Error) -> String {
    error.message().to_string_lossy().into_owned()
}
