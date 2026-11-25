//! ParserTest.php at symfony/expression-language v8.1.9. Names and
//! assertions are PHPUnit's; `assertEquals` on node trees is `==`.
//!
//! Upstream: symfony/expression-language Tests/ParserTest.php
#![allow(non_snake_case, clippy::unwrap_used, clippy::expect_used)]

use php_value::{PhpArray, Value, php_array};
use symfony_expression_language::{
    Class, Error, Functions, IGNORE_UNKNOWN_FUNCTIONS, IGNORE_UNKNOWN_VARIABLES, Lexer, Parser,
    node::{
        ArrayNode, BinaryNode, ConditionalNode, ConstantNode, GetAttrNode, GetAttrType, NameNode,
        Node, NullCoalesceNode, UnaryNode,
    },
};

fn names(list: &[&str]) -> PhpArray {
    PhpArray::list(list.iter().map(|&n| Value::from(n)))
}

fn parse(expression: &str, names: &PhpArray) -> Result<Node, Error> {
    Parser::new(Functions::new()).parse(Lexer::new().tokenize(expression)?, names, 0)
}

fn lint(expression: &str, names: &PhpArray, flags: u32) -> Result<(), Error> {
    Parser::new(Functions::new()).lint(Lexer::new().tokenize(expression)?, names, flags)
}

fn assert_syntax_error(result: Result<impl std::fmt::Debug, Error>, message: &str) {
    let error = result.expect_err(message);
    assert_eq!(error.class(), &Class::SyntaxError, "{error}");
    assert!(
        error.message().to_string_lossy().contains(message),
        "{error}"
    );
}

#[test]
fn testParseWithInvalidName() {
    assert_syntax_error(
        parse("foo", &PhpArray::new()),
        "Variable \"foo\" is not valid around position 1 for expression `foo`.",
    );
}

#[test]
fn testParseWithZeroInNames() {
    assert_syntax_error(
        parse("foo", &PhpArray::list([Value::Int(0)])),
        "Variable \"foo\" is not valid around position 1 for expression `foo`.",
    );
}

#[test]
fn testParsePrimaryExpressionWithUnknownFunctionThrows() {
    assert_syntax_error(
        parse("foo()", &PhpArray::new()),
        "The function \"foo\" does not exist around position 1 for expression `foo()`.",
    );
}

#[test]
fn testParseUnknownFunction() {
    assert_syntax_error(
        parse("foo()", &PhpArray::new()),
        "The function \"foo\" does not exist around position 1 for expression `foo()`.",
    );
}

fn c(value: impl Into<Value>) -> Node {
    Node::Constant(ConstantNode::new(value))
}

fn id(value: &str) -> Node {
    Node::Constant(ConstantNode::with(value, true, false))
}

fn null_safe_id(value: &str) -> Node {
    Node::Constant(ConstantNode::with(value, true, true))
}

fn name(n: &str) -> Node {
    Node::Name(NameNode::new(n))
}

fn attr(node: Node, attribute: Node, arguments: ArrayNode, kind: GetAttrType) -> Node {
    Node::GetAttr(GetAttrNode::new(node, attribute, arguments, kind))
}

fn args() -> ArrayNode {
    ArrayNode::arguments()
}

fn create_get_attr_node(node: Node, item: &str, kind: GetAttrType) -> Node {
    let attribute = Node::Constant(ConstantNode::with(item, kind != GetAttrType::Array, false));
    attr(node, attribute, args(), kind)
}

#[expect(clippy::too_many_lines, reason = "upstream's data provider")]
fn get_parse_data() -> Vec<(Node, &'static str, PhpArray)> {
    let mut arguments = ArrayNode::arguments();
    arguments.add_element(c("arg1"), None);
    arguments.add_element(c(2), None);
    arguments.add_element(c(true), None);

    let mut array_node = ArrayNode::new();
    array_node.add_element(name("bar"), None);

    let none = PhpArray::new;
    vec![
        (name("a"), "a", names(&["a"])),
        (c("a"), "\"a\"", none()),
        (c(3), "3", none()),
        (c(false), "false", none()),
        (c(true), "true", none()),
        (c(Value::Null), "null", none()),
        (Node::Unary(UnaryNode::new("-", c(3))), "-3", none()),
        (Node::Binary(BinaryNode::new("-", c(3), c(3))), "3 - 3", none()),
        (
            Node::Binary(BinaryNode::new(
                "*",
                Node::Binary(BinaryNode::new("-", c(3), c(3))),
                c(2),
            )),
            "(3 - 3) * 2",
            none(),
        ),
        (attr(name("foo"), id("bar"), args(), GetAttrType::Property), "foo.bar", names(&["foo"])),
        (attr(name("foo"), id("bar"), args(), GetAttrType::Method), "foo.bar()", names(&["foo"])),
        (attr(name("foo"), id("not"), args(), GetAttrType::Method), "foo.not()", names(&["foo"])),
        (
            attr(name("foo"), id("bar"), arguments, GetAttrType::Method),
            "foo.bar(\"arg1\", 2, true)",
            names(&["foo"]),
        ),
        (attr(name("foo"), c(3), args(), GetAttrType::Array), "foo[3]", names(&["foo"])),
        (
            Node::Conditional(ConditionalNode::new(c(true), c(true), c(false))),
            "true ? true : false",
            none(),
        ),
        (Node::Binary(BinaryNode::new("matches", c("foo"), c("/foo/"))), "\"foo\" matches \"/foo/\"", none()),
        (Node::Binary(BinaryNode::new("starts with", c("foo"), c("f"))), "\"foo\" starts with \"f\"", none()),
        (Node::Binary(BinaryNode::new("ends with", c("foo"), c("f"))), "\"foo\" ends with \"f\"", none()),
        (Node::Binary(BinaryNode::new("contains", c("foo"), c("f"))), "\"foo\" contains \"f\"", none()),
        (attr(name("foo"), null_safe_id("bar"), args(), GetAttrType::Property), "foo?.bar", names(&["foo"])),
        (attr(name("foo"), null_safe_id("bar"), args(), GetAttrType::Method), "foo?.bar()", names(&["foo"])),
        (attr(name("foo"), null_safe_id("not"), args(), GetAttrType::Method), "foo?.not()", names(&["foo"])),
        (
            Node::GetAttr(GetAttrNode::with_null_safe(name("foo"), c(0), args(), GetAttrType::Array, true)),
            "foo?.[0]",
            names(&["foo"]),
        ),
        (
            Node::NullCoalesce(NullCoalesceNode::new(
                attr(name("foo"), id("bar"), args(), GetAttrType::Property),
                c("default"),
            )),
            "foo.bar ?? \"default\"",
            names(&["foo"]),
        ),
        (
            Node::NullCoalesce(NullCoalesceNode::new(
                attr(name("foo"), c("bar"), args(), GetAttrType::Array),
                c("default"),
            )),
            "foo[\"bar\"] ?? \"default\"",
            names(&["foo"]),
        ),
        // chained calls
        (
            create_get_attr_node(
                create_get_attr_node(
                    create_get_attr_node(
                        create_get_attr_node(name("foo"), "bar", GetAttrType::Method),
                        "foo",
                        GetAttrType::Method,
                    ),
                    "baz",
                    GetAttrType::Property,
                ),
                "3",
                GetAttrType::Array,
            ),
            "foo.bar().foo().baz[3]",
            names(&["foo"]),
        ),
        (name("foo"), "bar", php_array!["foo" => "bar"]),
        // Operators collisions
        (
            Node::Binary(BinaryNode::new(
                "in",
                attr(name("foo"), id("not"), args(), GetAttrType::Property),
                Node::Array(array_node),
            )),
            "foo.not in [bar]",
            names(&["foo", "bar"]),
        ),
        (
            Node::Binary(BinaryNode::new(
                "or",
                Node::Unary(UnaryNode::new("not", name("foo"))),
                attr(name("foo"), id("not"), args(), GetAttrType::Property),
            )),
            "not foo or foo.not",
            names(&["foo"]),
        ),
        (
            Node::Binary(BinaryNode::new("xor", name("foo"), name("bar"))),
            "foo xor bar",
            names(&["foo", "bar"]),
        ),
        (Node::Binary(BinaryNode::new("..", c(0), c(3))), "0..3", none()),
        (Node::Binary(BinaryNode::new("+", c(0), c(0.1))), "0+.1", none()),
    ]
}

/// The chained-call row compares the int key `3` (a number token) with
/// the string `'3'` upstream builds: `assertEquals` treats them as equal.
fn loosely_equal(expected: &Node, actual: &Node) -> bool {
    match (expected, actual) {
        (Node::Constant(e), Node::Constant(a)) => {
            e.is_identifier == a.is_identifier
                && e.is_null_safe == a.is_null_safe
                && php_value::vectors::phpunit_equals(&e.value, &a.value)
        }
        (e, a) if e.short_class() == a.short_class() && e.attributes() == a.attributes() => {
            let (ec, ac) = (e.children(), a.children());
            ec.len() == ac.len() && ec.iter().zip(ac).all(|(x, y)| loosely_equal(x, y))
        }
        _ => false,
    }
}

#[test]
fn testParse() {
    for (node, expression, names) in get_parse_data() {
        let actual = parse(expression, &names).expect(expression);
        assert!(loosely_equal(&node, &actual), "{expression}: {node} != {actual}");
    }
}

#[test]
fn testParseWithInvalidPostfixData() {
    for expression in [
        "foo.\"#\"",
        "foo.\"bar\"",
        "foo.**",
        "foo.123",
        "foo.[0]",
        "foo?.[]",
    ] {
        let error = parse(expression, &names(&["foo"])).expect_err(expression);
        assert_eq!(error.class(), &Class::SyntaxError, "{error}");
    }
}

#[test]
fn testNameProposal() {
    assert_syntax_error(parse("foo > bar", &names(&["foo", "baz"])), "Did you mean \"baz\"?");
}

#[test]
fn testLint() {
    let ok: &[(&str, &[&str], u32)] = &[
        ("foo[\"some_key\"].callFunction(a ? b)", &["foo", "a", "b"], 0),
        ("foo[\"some_key\"]?.callFunction(a ? b)", &["foo", "a", "b"], 0),
        ("foo.bar", &[], IGNORE_UNKNOWN_VARIABLES),
        ("foo()", &[], IGNORE_UNKNOWN_FUNCTIONS),
        ("foo(bar)", &[], IGNORE_UNKNOWN_FUNCTIONS | IGNORE_UNKNOWN_VARIABLES),
        ("[value1, value2, value3,]", &["value1", "value2", "value3"], 0),
        ("{val1: value1, val2: value2, val3: value3,}", &["value1", "value2", "value3"], 0),
        ("foo.not in [bar]", &["foo", "bar"], 0),
    ];
    for &(expression, list, flags) in ok {
        lint(expression, &names(list), flags).unwrap_or_else(|e| panic!("{expression}: {e}"));
    }

    let failing: &[(&str, &[&str], &str)] = &[
        ("foo.bar", &[], "Variable \"foo\" is not valid around position 1 for expression `foo.bar"),
        ("foo()", &[], "The function \"foo\" does not exist around position 1 for expression `foo()"),
        (
            "foo[\"a\"] foo[\"b\"]",
            &["foo"],
            "Unexpected token \"name\" of value \"foo\" around position 10 for expression `foo[\"a\"] foo[\"b\"]`.",
        ),
        (
            "foo[\"some_key\"] // 2",
            &["foo"],
            "Unexpected token \"operator\" of value \"/\" around position 18 for expression `foo[\"some_key\"] // 2`.",
        ),
        (
            "[value1, value2 value3]",
            &["value1", "value2", "value3"],
            "An array element must be followed by a comma. Unexpected token \"name\" of value \"value3\" (\"punctuation\" expected with value \",\") around position 17 for expression `[value1, value2 value3]`.",
        ),
        (
            "foo[\"some_key\")",
            &["foo"],
            "Unclosed \"[\" around position 3 for expression `foo[\"some_key\")`.",
        ),
        (
            "{+: value1}",
            &["value1"],
            "A hash key must be a quoted string, a number, a name, or an expression enclosed in parentheses (unexpected token \"operator\" of value \"+\" around position 2 for expression `{+: value1}`.",
        ),
        (
            "foo[]",
            &["foo"],
            "Unexpected token \"punctuation\" of value \"]\" around position 5 for expression `foo[]`.",
        ),
        (
            "foo[(bar ? bar : \"default\"]",
            &["foo", "bar"],
            "Unclosed \"(\" around position 4 for expression `foo[(bar ? bar : \"default\"]`.",
        ),
        (
            "{key: foo key2: bar}",
            &["foo", "bar"],
            "A hash value must be followed by a comma. Unexpected token \"name\" of value \"key2\" (\"punctuation\" expected with value \",\") around position 11 for expression `{key: foo key2: bar}`.",
        ),
        (
            "{key => foo}",
            &["foo"],
            "Unexpected character \"=\" around position 5 for expression `{key => foo}`.",
        ),
        (
            "[foo: foo]",
            &["foo"],
            "An array element must be followed by a comma. Unexpected token \"punctuation\" of value \":\" (\"punctuation\" expected with value \",\") around position 5 for expression `[foo: foo]`.",
        ),
    ];
    for &(expression, list, message) in failing {
        assert_syntax_error(lint(expression, &names(list), 0), message);
    }
}

fn get_deeply_nested_expressions() -> Vec<(&'static str, String)> {
    vec![
        ("unary operators", format!("{}1", "!".repeat(300))),
        ("signs", format!("{}1", "-".repeat(300))),
        ("parentheses", format!("{}1{}", "(".repeat(300), ")".repeat(300))),
        ("arrays", format!("{}1{}", "[".repeat(300), "]".repeat(300))),
        ("hashes", format!("{}1{}", "{a:".repeat(300), "}".repeat(300))),
        ("binary operators", format!("{}1", "1+".repeat(300))),
        ("properties", format!("a{}", ".b".repeat(300))),
        ("array accesses", format!("a{}", "[0]".repeat(300))),
        ("ternaries", format!("{}1{}", "1?".repeat(300), ":1".repeat(300))),
        (
            "binary operators in nested parentheses",
            format!("{}1{}", "(".repeat(20), format!("{})", " + 1".repeat(20)).repeat(20)),
        ),
        (
            "properties in nested parentheses",
            format!("{}a{}", "(".repeat(20), format!("{})", ".b".repeat(20)).repeat(20)),
        ),
    ]
}

#[test]
fn testDeeplyNestedExpressionIsRejected() {
    for (label, expression) in get_deeply_nested_expressions() {
        let error = lint(&expression, &PhpArray::new(), IGNORE_UNKNOWN_VARIABLES).expect_err(label);
        assert_eq!(error.class(), &Class::SyntaxError, "{label}");
        assert!(
            error
                .message()
                .to_string_lossy()
                .contains("Expression is nested too deeply, the maximum nesting level is 256"),
            "{label}: {error}"
        );
    }
}

#[test]
fn testDeepButReasonableExpressionIsAccepted() {
    let expression = format!("{}a{}", "(".repeat(100), " + 1)".repeat(100));
    lint(&expression, &names(&["a"]), 0).unwrap();
}

#[test]
fn testChainedTernariesWithBinaryConditionsAreAccepted() {
    let expression = format!("{}0", "a and a and a ? 1 : ".repeat(64));
    lint(&expression, &names(&["a"]), 0).unwrap();
}
