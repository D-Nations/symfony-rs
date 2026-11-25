//! Node/BinaryNodeTest.php at symfony/expression-language v8.1.9. Names
//! and assertions are PHPUnit's. `eval()` of compiled code is not
//! available: testCompileMatchesWithInvalidRegexpAsExpression checks the
//! compiled source and runs the regex check the compiled closure runs.
//!
//! Upstream: symfony/expression-language Tests/Node/BinaryNodeTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{assert_compile, assert_dump, assert_evaluate, c, message, no_functions, no_values};
use php_value::{Value, php_array};
use symfony_expression_language::{
    Class, Compiler, Functions,
    node::{ArrayNode, BinaryNode, NameNode, Node, binary_node::MATCHES_FUNCTION},
};

fn b(operator: &str, left: impl Into<Value>, right: impl Into<Value>) -> Node {
    Node::Binary(BinaryNode::new(operator, c(left), c(right)))
}

fn array_ab() -> Node {
    let mut array = ArrayNode::new();
    array.add_element(c("a"), None);
    array.add_element(c("b"), None);
    Node::Array(array)
}

fn bn(operator: &str, left: impl Into<Value>, right: Node) -> Node {
    Node::Binary(BinaryNode::new(operator, c(left), right))
}

#[test]
fn testEvaluate() {
    let cases: Vec<(Value, Node)> = vec![
        (true.into(), b("or", true, false)),
        (true.into(), b("||", true, false)),
        (false.into(), b("xor", true, true)),
        (false.into(), b("and", true, false)),
        (false.into(), b("&&", true, false)),
        (0.into(), b("&", 2, 4)),
        (6.into(), b("|", 2, 4)),
        (6.into(), b("^", 2, 4)),
        (32.into(), b("<<", 2, 4)),
        (2.into(), b(">>", 32, 4)),
        (true.into(), b("<", 1, 2)),
        (true.into(), b("<=", 1, 2)),
        (true.into(), b("<=", 1, 1)),
        (false.into(), b(">", 1, 2)),
        (false.into(), b(">=", 1, 2)),
        (true.into(), b(">=", 1, 1)),
        (true.into(), b("===", true, true)),
        (false.into(), b("!==", true, true)),
        (false.into(), b("==", 2, 1)),
        (true.into(), b("!=", 2, 1)),
        ((-1).into(), b("-", 1, 2)),
        (3.into(), b("+", 1, 2)),
        (4.into(), b("*", 2, 2)),
        (1.into(), b("/", 2, 2)),
        (1.into(), b("%", 5, 2)),
        (25.into(), b("**", 5, 2)),
        ("ab".into(), b("~", "a", "b")),
        (true.into(), bn("in", "a", array_ab())),
        (false.into(), bn("in", "c", array_ab())),
        (true.into(), bn("not in", "c", array_ab())),
        (false.into(), bn("not in", "a", array_ab())),
        (php_array![0 => 1, 1 => 2, 2 => 3].into(), b("..", 1, 3)),
        (true.into(), b("starts with", "abc", "a")),
        (false.into(), b("starts with", "abc", "b")),
        (true.into(), b("ends with", "abc", "c")),
        (false.into(), b("ends with", "abc", "b")),
        (1.into(), b("matches", "abc", "/^[a-z]+$/")),
        (0.into(), b("matches", "", "/^[a-z]+$/")),
        (0.into(), b("matches", Value::Null, "/^[a-z]+$/")),
    ];
    for (expected, node) in cases {
        assert_evaluate(expected, &node, &no_values(), &no_functions());
    }
}

#[test]
fn testCompile() {
    let matches = format!("{MATCHES_FUNCTION}\"/^[a-z]+\\$/\", \"abc\")");
    let cases: Vec<(&str, Node)> = vec![
        ("(true || false)", b("or", true, false)),
        ("(true || false)", b("||", true, false)),
        ("(true xor true)", b("xor", true, true)),
        ("(true && false)", b("and", true, false)),
        ("(true && false)", b("&&", true, false)),
        ("(2 & 4)", b("&", 2, 4)),
        ("(2 | 4)", b("|", 2, 4)),
        ("(2 ^ 4)", b("^", 2, 4)),
        ("(2 << 4)", b("<<", 2, 4)),
        ("(32 >> 4)", b(">>", 32, 4)),
        ("(1 < 2)", b("<", 1, 2)),
        ("(1 <= 2)", b("<=", 1, 2)),
        ("(1 <= 1)", b("<=", 1, 1)),
        ("(1 > 2)", b(">", 1, 2)),
        ("(1 >= 2)", b(">=", 1, 2)),
        ("(1 >= 1)", b(">=", 1, 1)),
        ("(true === true)", b("===", true, true)),
        ("(true !== true)", b("!==", true, true)),
        ("(2 == 1)", b("==", 2, 1)),
        ("(2 != 1)", b("!=", 2, 1)),
        ("(1 - 2)", b("-", 1, 2)),
        ("(1 + 2)", b("+", 1, 2)),
        ("(2 * 2)", b("*", 2, 2)),
        ("(2 / 2)", b("/", 2, 2)),
        ("(5 % 2)", b("%", 5, 2)),
        ("pow(5, 2)", b("**", 5, 2)),
        ("(\"a\" . \"b\")", b("~", "a", "b")),
        (
            "\\in_array(\"a\", [0 => \"a\", 1 => \"b\"], true)",
            bn("in", "a", array_ab()),
        ),
        (
            "\\in_array(\"c\", [0 => \"a\", 1 => \"b\"], true)",
            bn("in", "c", array_ab()),
        ),
        (
            "!\\in_array(\"c\", [0 => \"a\", 1 => \"b\"], true)",
            bn("not in", "c", array_ab()),
        ),
        (
            "!\\in_array(\"a\", [0 => \"a\", 1 => \"b\"], true)",
            bn("not in", "a", array_ab()),
        ),
        ("range(1, 3)", b("..", 1, 3)),
        (&matches, b("matches", "abc", "/^[a-z]+$/")),
        (
            "str_starts_with(\"abc\", \"a\")",
            b("starts with", "abc", "a"),
        ),
        ("str_ends_with(\"abc\", \"a\")", b("ends with", "abc", "a")),
    ];
    for (expected, node) in cases {
        assert_compile(expected, &node, &no_functions());
    }
}

/// The compiled `matches` closure, as upstream's test source spells it.
#[test]
fn compiled_matches_closure_is_upstreams() {
    assert_eq!(
        MATCHES_FUNCTION,
        r#"(static function ($regexp, $str) { set_error_handler(static fn ($t, $m) => throw new \Symfony\Component\ExpressionLanguage\SyntaxError(sprintf('Regexp "%s" passed to "matches" is not valid', $regexp).substr($m, 12))); try { return preg_match($regexp, (string) $str); } finally { restore_error_handler(); } })("#
    );
}

#[test]
fn testDump() {
    let cases: Vec<(&str, Node)> = vec![
        ("(true or false)", b("or", true, false)),
        ("(true || false)", b("||", true, false)),
        ("(true xor true)", b("xor", true, true)),
        ("(true and false)", b("and", true, false)),
        ("(true && false)", b("&&", true, false)),
        ("(2 & 4)", b("&", 2, 4)),
        ("(2 | 4)", b("|", 2, 4)),
        ("(2 ^ 4)", b("^", 2, 4)),
        ("(2 << 4)", b("<<", 2, 4)),
        ("(32 >> 4)", b(">>", 32, 4)),
        ("(1 < 2)", b("<", 1, 2)),
        ("(1 <= 2)", b("<=", 1, 2)),
        ("(1 <= 1)", b("<=", 1, 1)),
        ("(1 > 2)", b(">", 1, 2)),
        ("(1 >= 2)", b(">=", 1, 2)),
        ("(1 >= 1)", b(">=", 1, 1)),
        ("(true === true)", b("===", true, true)),
        ("(true !== true)", b("!==", true, true)),
        ("(2 == 1)", b("==", 2, 1)),
        ("(2 != 1)", b("!=", 2, 1)),
        ("(1 - 2)", b("-", 1, 2)),
        ("(1 + 2)", b("+", 1, 2)),
        ("(2 * 2)", b("*", 2, 2)),
        ("(2 / 2)", b("/", 2, 2)),
        ("(5 % 2)", b("%", 5, 2)),
        ("(5 ** 2)", b("**", 5, 2)),
        ("(\"a\" ~ \"b\")", b("~", "a", "b")),
        ("(\"a\" in [\"a\", \"b\"])", bn("in", "a", array_ab())),
        ("(\"c\" in [\"a\", \"b\"])", bn("in", "c", array_ab())),
        (
            "(\"c\" not in [\"a\", \"b\"])",
            bn("not in", "c", array_ab()),
        ),
        (
            "(\"a\" not in [\"a\", \"b\"])",
            bn("not in", "a", array_ab()),
        ),
        ("(1 .. 3)", b("..", 1, 3)),
        (
            "(\"abc\" matches \"/^[a-z]+$/\")",
            b("matches", "abc", "/^[a-z]+$/"),
        ),
    ];
    for (expected, node) in cases {
        assert_dump(expected, &node);
    }
}

const INVALID: &str = "Regexp \"this is not a regexp\" passed to \"matches\" is not valid: Delimiter must not be alphanumeric";

fn assert_syntax_error(error: &symfony_expression_language::Error, expected: &str) {
    assert_eq!(error.class(), &Class::SyntaxError, "{error}");
    assert!(message(error).contains(expected), "{error}");
}

#[test]
fn testEvaluateMatchesWithInvalidRegexp() {
    let node = b("matches", "abc", "this is not a regexp");
    let error = node.evaluate(&Functions::new(), &no_values()).unwrap_err();
    assert_syntax_error(&error, INVALID);
}

#[test]
fn testEvaluateMatchesWithInvalidRegexpAsExpression() {
    let node = Node::Binary(BinaryNode::new(
        "matches",
        c("abc"),
        Node::Name(NameNode::new("regexp")),
    ));
    let error = node
        .evaluate(
            &Functions::new(),
            &php_array!["regexp" => "this is not a regexp"],
        )
        .unwrap_err();
    assert_syntax_error(&error, INVALID);
}

#[test]
fn testCompileMatchesWithInvalidRegexp() {
    let node = b("matches", "abc", "this is not a regexp");
    let mut compiler = Compiler::new(Functions::new());
    let error = node.compile(&mut compiler).unwrap_err();
    assert_syntax_error(&error, INVALID);
}

/// The compiled code checks the regex when it runs: the source calls the
/// closure on `$regexp`, and the closure's check is `evaluateMatches()`'s.
#[test]
fn testCompileMatchesWithInvalidRegexpAsExpression() {
    let node = Node::Binary(BinaryNode::new(
        "matches",
        c("abc"),
        Node::Name(NameNode::new("regexp")),
    ));
    let mut compiler = Compiler::new(Functions::new());
    node.compile(&mut compiler).unwrap();
    assert_eq!(
        compiler.source(),
        format!("{MATCHES_FUNCTION}$regexp, \"abc\")").as_str()
    );
    let error = node
        .evaluate(
            &Functions::new(),
            &php_array!["regexp" => "this is not a regexp"],
        )
        .unwrap_err();
    assert_syntax_error(&error, INVALID);
}

#[test]
fn testCompileMatchesWithBooleanBinaryNode() {
    let node = bn("matches", "abc", b("||", true, false));
    let mut compiler = Compiler::new(Functions::new());
    let error = node.compile(&mut compiler).unwrap_err();
    assert_syntax_error(&error, "The regex passed to \"matches\" must be a string");
}

#[test]
fn testCompileMatchesWithStringBinaryNode() {
    let node = bn("matches", "abc", b("~", "a", "b"));
    let mut compiler = Compiler::new(Functions::new());
    node.compile(&mut compiler).unwrap();
}

#[test]
fn testDivisionByZero() {
    let error = b("/", 1, 0)
        .evaluate(&Functions::new(), &no_values())
        .unwrap_err();
    assert_eq!(error.class(), &Class::DivisionByZeroError);
    assert!(message(&error).contains("Division by zero."));
}

#[test]
fn testModuloByZero() {
    let error = b("%", 1, 0)
        .evaluate(&Functions::new(), &no_values())
        .unwrap_err();
    assert_eq!(error.class(), &Class::DivisionByZeroError);
    assert!(message(&error).contains("Modulo by zero."));
}

#[test]
fn testInOperatorStrictness() {
    for value in [Value::Int(1), Value::from("true")] {
        let mut array = ArrayNode::new();
        array.add_element(c("1"), None);
        array.add_element(c(true), None);
        let node = Node::Binary(BinaryNode::new("in", c(value), Node::Array(array)));
        assert_eq!(
            node.evaluate(&Functions::new(), &no_values()).unwrap(),
            Value::Bool(false)
        );
    }
}

#[test]
fn testEvaluateUnsupportedOperator() {
    let error = b("unsupported", 1, 2)
        .evaluate(&Functions::new(), &no_values())
        .unwrap_err();
    assert_eq!(error.class(), &Class::LogicException);
    assert!(message(&error).contains(
        "\"Symfony\\Component\\ExpressionLanguage\\Node\\BinaryNode\" does not support the \"unsupported\" operator."
    ));
}
