//! Node/GetAttrNodeTest.php at symfony/expression-language v8.1.9. Names
//! and assertions are PHPUnit's. The test's `Obj` class (property `foo`,
//! method `foo()`) is a `MethodObject`.
//!
//! Upstream: symfony/expression-language Tests/Node/GetAttrNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use std::sync::Arc;

use common::{assert_compile, assert_dump, assert_evaluate, c, message, no_functions};
use php_value::{PhpArray, Value, object::ArrayObject, php_array};
use symfony_expression_language::{
    Class, Functions, MethodObject,
    node::{ArrayNode, ConstantNode, GetAttrNode, GetAttrType, NameNode, Node},
};

fn obj() -> Value {
    MethodObject::new("Obj")
        .property("foo", "bar")
        .method("foo", |_| Ok(Value::from("baz")))
        .into_value()
}

fn get_array_node() -> ArrayNode {
    let mut array = ArrayNode::new();
    array.add_element(c("a"), Some(c("b")));
    array.add_element(c("b"), None);
    array
}

fn name(n: &str) -> Node {
    Node::Name(NameNode::new(n))
}

fn attr(node: Node, attribute: Node, arguments: ArrayNode, kind: GetAttrType) -> Node {
    Node::GetAttr(GetAttrNode::new(node, attribute, arguments, kind))
}

fn attr_null_safe(node: Node, attribute: Node, kind: GetAttrType) -> Node {
    Node::GetAttr(GetAttrNode::with_null_safe(
        node,
        attribute,
        ArrayNode::arguments(),
        kind,
        true,
    ))
}

#[test]
fn testEvaluate() {
    let foo_array = || php_array!["foo" => php_array!["b" => "a", 0 => "b"]];
    let cases: Vec<(Value, Node, PhpArray)> = vec![
        (
            "b".into(),
            attr(name("foo"), c(0), get_array_node(), GetAttrType::Array),
            foo_array(),
        ),
        (
            "a".into(),
            attr(name("foo"), c("b"), get_array_node(), GetAttrType::Array),
            foo_array(),
        ),
        (
            "bar".into(),
            attr(
                name("foo"),
                c("foo"),
                get_array_node(),
                GetAttrType::Property,
            ),
            php_array!["foo" => obj()],
        ),
        (
            "baz".into(),
            attr(name("foo"), c("foo"), get_array_node(), GetAttrType::Method),
            php_array!["foo" => obj()],
        ),
        (
            "a".into(),
            attr(
                name("foo"),
                name("index"),
                get_array_node(),
                GetAttrType::Array,
            ),
            php_array!["foo" => php_array!["b" => "a", 0 => "b"], "index" => "b"],
        ),
        (
            Value::Null,
            attr_null_safe(name("foo"), c(0), GetAttrType::Array),
            php_array!["foo" => Value::Null],
        ),
        // null-safe array access against an \ArrayAccess receiver: helper returns it as-is.
        (
            "v".into(),
            attr_null_safe(name("foo"), c("k"), GetAttrType::Array),
            php_array!["foo" => Value::Object(Arc::new(ArrayObject::new(php_array!["k" => "v"])))],
        ),
        // nested null-safe array access on a present 2D structure.
        (
            "x".into(),
            attr_null_safe(
                attr_null_safe(name("foo"), c(0), GetAttrType::Array),
                c(1),
                GetAttrType::Array,
            ),
            php_array!["foo" => php_array![0 => php_array![0 => "a", 1 => "x"]]],
        ),
    ];
    for (expected, node, variables) in cases {
        assert_evaluate(expected, &node, &variables, &no_functions());
    }
}

#[test]
fn testEvaluateNullSafeArrayOnNonArrayThrows() {
    let node = attr_null_safe(name("foo"), c(0), GetAttrType::Array);
    let error = node
        .evaluate(&Functions::new(), &php_array!["foo" => 42])
        .unwrap_err();
    assert_eq!(error.class(), &Class::RuntimeException);
    assert!(message(&error).contains("Unable to get an item of non-array \"foo\"."));
}

/// A node from before 8.1 has no `is_null_safe` attribute: here a node
/// built without it (`GetAttrNode::new`), which is the same state.
#[test]
fn testUnserializeMissingIsNullSafeDefaultsToFalse() {
    let restored = attr(
        name("foo"),
        c(0),
        ArrayNode::arguments(),
        GetAttrType::Array,
    );
    let Node::GetAttr(get_attr) = &restored else {
        unreachable!()
    };
    assert!(!get_attr.is_null_safe);
    assert_eq!(restored.dump().unwrap(), "foo[0]");
    assert_eq!(
        restored
            .evaluate(
                &Functions::new(),
                &php_array!["foo" => php_array!["b" => "a", 0 => "b"]]
            )
            .unwrap(),
        Value::from("b")
    );
}

#[test]
fn testCompile() {
    let class = "\\Symfony\\Component\\ExpressionLanguage\\Node\\GetAttrNode";
    let nested = format!(
        "{class}::convertToArrayAccess({class}::convertToArrayAccess($foo, \"foo\")?->offsetGet(0), \"foo?.[0]\")?->offsetGet(1)"
    );
    let single = format!("{class}::convertToArrayAccess($foo, \"foo\")?->offsetGet(0)");
    let cases: Vec<(&str, Node)> = vec![
        (
            "$foo[0]",
            attr(name("foo"), c(0), get_array_node(), GetAttrType::Array),
        ),
        (
            "$foo[\"b\"]",
            attr(name("foo"), c("b"), get_array_node(), GetAttrType::Array),
        ),
        (
            "$foo->foo",
            attr(
                name("foo"),
                c("foo"),
                get_array_node(),
                GetAttrType::Property,
            ),
        ),
        (
            "$foo->foo([\"b\" => \"a\", 0 => \"b\"])",
            attr(name("foo"), c("foo"), get_array_node(), GetAttrType::Method),
        ),
        (
            "$foo[$index]",
            attr(
                name("foo"),
                name("index"),
                get_array_node(),
                GetAttrType::Array,
            ),
        ),
        (
            &single,
            attr_null_safe(name("foo"), c(0), GetAttrType::Array),
        ),
        (
            &nested,
            attr_null_safe(
                attr_null_safe(name("foo"), c(0), GetAttrType::Array),
                c(1),
                GetAttrType::Array,
            ),
        ),
    ];
    for (expected, node) in cases {
        assert_compile(expected, &node, &no_functions());
    }
}

#[test]
fn testDump() {
    let cases: Vec<(&str, Node)> = vec![
        (
            "foo[0]",
            attr(name("foo"), c(0), get_array_node(), GetAttrType::Array),
        ),
        (
            "foo[\"b\"]",
            attr(name("foo"), c("b"), get_array_node(), GetAttrType::Array),
        ),
        (
            "foo.foo",
            attr(
                name("foo"),
                name("foo"),
                get_array_node(),
                GetAttrType::Property,
            ),
        ),
        (
            "foo.foo({\"b\": \"a\", 0: \"b\"})",
            attr(
                name("foo"),
                name("foo"),
                get_array_node(),
                GetAttrType::Method,
            ),
        ),
        (
            "foo[index]",
            attr(
                name("foo"),
                name("index"),
                get_array_node(),
                GetAttrType::Array,
            ),
        ),
        (
            "foo?.foo()",
            attr(
                name("foo"),
                Node::Constant(ConstantNode::with("foo", true, true)),
                ArrayNode::arguments(),
                GetAttrType::Method,
            ),
        ),
        (
            "foo?.[0]",
            attr_null_safe(name("foo"), c(0), GetAttrType::Array),
        ),
    ];
    for (expected, node) in cases {
        assert_dump(expected, &node);
    }
}
