//! Node/NodeTest.php at symfony/expression-language v8.1.9. Names and
//! assertions are PHPUnit's. PHPUnit's mock nodes are function nodes
//! whose compiler and evaluator count their calls.
//!
//! Upstream: symfony/expression-language Tests/Node/NodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use common::c;
use php_value::{PhpArray, PhpString, Value, php_array};
use symfony_expression_language::{
    Compiler, ExpressionFunction, Functions,
    node::{FunctionNode, Node},
};

#[test]
fn testToString() {
    let node = Node::list(vec![c("foo")]);
    assert_eq!(
        node.to_php_string(),
        "Node(\n    ConstantNode(value: 'foo')\n)"
    );
}

/// A function `mock<i>` and a node calling it.
fn mock(
    i: i64,
    compiled: &Arc<AtomicUsize>,
    evaluated: &Arc<AtomicUsize>,
    functions: &mut Functions,
) -> Node {
    let name = format!("mock{i}");
    let (compiled, evaluated) = (Arc::clone(compiled), Arc::clone(evaluated));
    functions.insert(ExpressionFunction::new(
        name.as_str(),
        move |_: &[PhpString]| {
            compiled.fetch_add(1, Ordering::SeqCst);
            Ok(PhpString::default())
        },
        move |_: &PhpArray, _: &[Value]| {
            evaluated.fetch_add(1, Ordering::SeqCst);
            Ok(Value::Int(i))
        },
    ));
    Node::Function(FunctionNode::new(name.as_str(), Node::list(Vec::new())))
}

#[test]
fn testCompileActuallyCompilesAllNodes() {
    let (compiled, evaluated) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let mut functions = Functions::new();
    let nodes = (1..=10)
        .map(|i| mock(i, &compiled, &evaluated, &mut functions))
        .collect();
    let node = Node::list(nodes);
    node.compile(&mut Compiler::new(functions)).unwrap();
    assert_eq!(compiled.load(Ordering::SeqCst), 10);
    assert_eq!(evaluated.load(Ordering::SeqCst), 0);
}

#[test]
fn testEvaluateActuallyEvaluatesAllNodes() {
    let (compiled, evaluated) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let mut functions = Functions::new();
    let nodes = (1..=3)
        .map(|i| mock(i, &compiled, &evaluated, &mut functions))
        .collect();
    let node = Node::list(nodes);
    assert_eq!(
        node.evaluate(&functions, &PhpArray::new()).unwrap(),
        Value::Array(php_array![0 => 1, 1 => 2, 2 => 3])
    );
    assert_eq!(evaluated.load(Ordering::SeqCst), 3);
}
