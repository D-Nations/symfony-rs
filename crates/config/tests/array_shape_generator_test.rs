//! ArrayShapeGeneratorTest.php at symfony/config v8.1.10. Names and
//! assertions are PHPUnit's; data providers are lists of cases, and
//! `assertStringMatchesFormat()` is written for the `%s` and `%A` it uses.
//!
//! Upstream: symfony/config Tests/Definition/ArrayShapeGeneratorTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, enums, fixture};
use php_value::{Number, Value};
use symfony_config::{
    Node,
    definition::{
        ArrayNode, ArrayShapeGenerator, BooleanNode, EnumNode, FloatNode, IntegerNode,
        PrototypedArrayNode, ScalarNode, StringNode, VariableNode,
        builder::{ArrayNodeDefinition, ExprType},
    },
};

fn generate(node: &Node) -> Result<String, symfony_config::Error> {
    Ok(String::from_utf8_lossy(&ArrayShapeGenerator::generate(node)?).into_owned())
}

#[track_caller]
fn assert_string_contains_string(needle: &str, haystack: &str) {
    assert!(
        haystack.contains(needle),
        "{haystack:?} does not contain {needle:?}"
    );
}

/// `assertStringMatchesFormat()`: `%s` is one or more characters other
/// than a line end, `%A` zero or more of any character.
#[track_caller]
fn assert_string_matches_format(format: &str, string: &str) {
    fn matches(format: &[u8], s: &[u8]) -> bool {
        match format {
            [] => s.is_empty(),
            [b'%', b's', rest @ ..] => (1..=s.len())
                .take_while(|&n| !matches!(s[n - 1], b'\n' | b'\r'))
                .any(|n| matches(rest, &s[n..])),
            [b'%', b'A', rest @ ..] => (0..=s.len()).any(|n| matches(rest, &s[n..])),
            [c, rest @ ..] => s.first() == Some(c) && matches(rest, &s[1..]),
        }
    }
    assert!(
        matches(format.as_bytes(), string.as_bytes()),
        "{string:?} does not match format {format:?}"
    );
}

fn provide_nodes() -> Result<Vec<(Node, String)>, symfony_config::Error> {
    let registry = enums();
    let nullable_boolean_node = BooleanNode::new("node", None)?;
    nullable_boolean_node.set_default_value(Value::Null)?;

    Ok(vec![
        (ArrayNode::new("node", None)?, "array<mixed>".to_owned()),
        (StringNode::new("node", None)?, "string".to_owned()),
        (BooleanNode::new("node", None)?, "bool".to_owned()),
        (
            nullable_boolean_node,
            "bool|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator|null".to_owned(),
        ),
        (
            EnumNode::new("node", None, vec![Value::from("a"), Value::from("b")])?,
            "\"a\"|\"b\"".to_owned(),
        ),
        (
            EnumNode::of_enum("node", None, &fixture("StringBackedTestEnum"), &registry)?,
            "value-of<\\Symfony\\Component\\Config\\Tests\\Fixtures\\StringBackedTestEnum>|\\Symfony\\Component\\Config\\Tests\\Fixtures\\StringBackedTestEnum".to_owned(),
        ),
        (
            EnumNode::of_enum("node", None, &fixture("IntegerBackedTestEnum"), &registry)?,
            "value-of<\\Symfony\\Component\\Config\\Tests\\Fixtures\\IntegerBackedTestEnum>|\\Symfony\\Component\\Config\\Tests\\Fixtures\\IntegerBackedTestEnum".to_owned(),
        ),
        (
            ScalarNode::new("node", None)?,
            "scalar|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator|null".to_owned(),
        ),
        (VariableNode::new("node", None)?, "mixed".to_owned()),
        (IntegerNode::new("node", None, None, None)?, "int".to_owned()),
        (
            IntegerNode::new("node", None, Some(Number::Int(1)), None)?,
            "int".to_owned(),
        ),
        (
            IntegerNode::new("node", None, None, Some(Number::Int(10)))?,
            "int".to_owned(),
        ),
        (
            IntegerNode::new("node", None, Some(Number::Int(1)), Some(Number::Int(10)))?,
            "int".to_owned(),
        ),
        (FloatNode::new("node", None, None, None)?, "float".to_owned()),
        (
            FloatNode::new("node", None, Some(Number::Float(1.1)), None)?,
            "float".to_owned(),
        ),
        (
            FloatNode::new("node", None, None, Some(Number::Float(10.1)))?,
            "float".to_owned(),
        ),
        (
            FloatNode::new(
                "node",
                None,
                Some(Number::Float(1.1)),
                Some(Number::Float(10.1)),
            )?,
            "float".to_owned(),
        ),
    ])
}

#[test]
fn testPhpDocHandlesNodeTypes() -> TestResult {
    for (node, expected) in provide_nodes()? {
        let array_node = ArrayNode::new("root", None)?;
        array_node.add_child(&node)?;

        let expected = format!("node?: {expected}");

        assert_string_contains_string(&expected, &generate(&array_node)?);
    }
    Ok(())
}

#[test]
fn testPrototypedArrayNodePhpDoc() -> TestResult {
    let prototype = PrototypedArrayNode::new("proto", None)?;
    prototype.set_prototype(&StringNode::new("child", None)?)?;

    let root = ArrayNode::new("root", None)?;
    root.add_child(&prototype)?;

    let expected = "array{\n *     proto?: list<string|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator>,\n * }";

    assert_string_contains_string(expected, &generate(&root)?);
    Ok(())
}

#[test]
fn testPrototypedArrayNodePhpDocWithKeyAttribute() -> TestResult {
    let prototype = PrototypedArrayNode::new("proto", None)?;
    prototype.set_prototype(&StringNode::new("child", None)?)?;
    prototype.set_key_attribute(b"name", true)?;

    let root = ArrayNode::new("root", None)?;
    root.add_child(&prototype)?;

    let expected = "array{\n *     proto?: array<string, string|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator>,\n * }";

    assert_string_contains_string(expected, &generate(&root)?);
    Ok(())
}

#[test]
fn testPrototypedArrayNodePhpDocWithAcceptAndWrap() -> TestResult {
    let proto = ArrayNodeDefinition::new("proto");
    proto
        .use_attribute_as_key("name", true)
        .accept_and_wrap(&[ExprType::String, ExprType::BackedEnum], None)?
        .prototype("scalar")?
        .end();

    let root = ArrayNodeDefinition::new("root");
    root.append(&proto);

    let expected = "array{\n *     proto?: \\BackedEnum|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator|string|array<string, scalar|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator|null>,\n * }";

    assert_string_contains_string(expected, &generate(&root.get_node(false)?)?);
    Ok(())
}

#[test]
fn testPrototypedArrayNodePhpDocWithStringAcceptAndWrap() -> TestResult {
    let proto = ArrayNodeDefinition::new("proto");
    proto
        .use_attribute_as_key("name", true)
        .accept_and_wrap(&[ExprType::String], None)?
        .prototype("scalar")?
        .end();

    let root = ArrayNodeDefinition::new("root");
    root.append(&proto);

    let expected = "proto?: \\Symfony\\Component\\Config\\Loader\\ParamConfigurator|string|array<string, scalar|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator|null>,";

    assert_string_contains_string(expected, &generate(&root.get_node(false)?)?);
    Ok(())
}

#[test]
fn testPhpDocWithAcceptAndWrapOnPlainArrayNode() -> TestResult {
    let child = ArrayNodeDefinition::new("child");
    child
        .accept_and_wrap(&[ExprType::String], Some(b"value"))?
        .children()
        .scalar_node("value")?
        .end()
        .end();

    let root = ArrayNodeDefinition::new("root");
    root.append(&child);

    let expected = "child?: \\Symfony\\Component\\Config\\Loader\\ParamConfigurator|string|array{";

    assert_string_contains_string(expected, &generate(&root.get_node(false)?)?);
    Ok(())
}

#[test]
fn testArrayPrototypePhpDoc() -> TestResult {
    let proto = ArrayNodeDefinition::new("proto");
    proto
        .array_prototype()?
        .children()
        .boolean_node("nested")?
        .end()
        .end()
        .end();

    let root = ArrayNodeDefinition::new("root");
    root.append(&proto);

    let expected = "array{\n *     proto?: list<array{ // Default: []\n *         nested?: bool|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator,\n *     }>,\n * }";

    assert_string_contains_string(expected, &generate(&root.get_node(false)?)?);
    Ok(())
}

#[test]
fn testPhpDocHandlesRequiredNode() -> TestResult {
    let child = BooleanNode::new("node", None)?;
    child.set_required(true);

    let root = ArrayNode::new("root", None)?;
    root.add_child(&child)?;

    assert_string_contains_string("node?: bool", &generate(&root)?);
    Ok(())
}

#[test]
fn testPhpDocHandlesRequiredNodeWithNoDeepMerging() -> TestResult {
    let child = BooleanNode::new("node", None)?;
    child.set_required(true);

    let root = ArrayNode::new("root", None)?;
    root.set_perform_deep_merging(false)?;
    root.add_child(&child)?;

    let expected = "node: bool";

    assert_string_contains_string(expected, &generate(&root)?);
    Ok(())
}

#[test]
fn testPhpDocHandleAdditionalDocumentation() -> TestResult {
    let child = BooleanNode::new("node", None)?;
    child.set_deprecated(
        b"vendor/package",
        b"1.0",
        b"The \"%path%\" option is deprecated.",
    );
    child.set_default_value(Value::Bool(true))?;
    child.set_info(b"This is a boolean node.");

    let root = ArrayNode::new("root", None)?;
    root.add_child(&child)?;

    assert_string_contains_string(
        "node?: bool|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator, // Deprecated: The \"node\" option is deprecated. // This is a boolean node. // Default: true",
        &generate(&root)?,
    );
    Ok(())
}

#[test]
fn testPhpDocHandleMultilineDoc() -> TestResult {
    let child = BooleanNode::new("node", None)?;
    child.set_deprecated(
        b"vendor/package",
        b"1.0",
        b"The \"%path%\" option is deprecated.",
    );
    child.set_default_value(Value::Bool(true))?;
    child.set_info(
        b"This is a boolean node.\nSet to true to enable it.\r\nSet to false to disable it.",
    );

    let root = ArrayNode::new("root", None)?;
    root.add_child(&child)?;

    assert_string_contains_string(
        "node?: bool|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator, // Deprecated: The \"node\" option is deprecated. // This is a boolean node. Set to true to enable it. Set to false to disable it. // Default: true",
        &generate(&root)?,
    );
    Ok(())
}

#[test]
fn testPhpDocDoesNotCloseCommentBlock() -> TestResult {
    let child = ScalarNode::new("schedule", None)?;
    child.set_info(b"Cron expression for when normal/incremental syncs should run");
    child.set_default_value(Value::from("*/30 * * * *"))?;

    let root = ArrayNode::new("root", None)?;
    root.add_child(&child)?;
    root.add_child(&EnumNode::new(
        "preset",
        None,
        vec![Value::from("*/5 * * * *"), Value::from("*/30 * * * *")],
    )?)?;

    let generated = generate(&root)?;

    // A "*/" in the info, default value or an enum value would prematurely close the surrounding doc block and generate invalid PHP.
    assert!(!generated.contains("*/"), "{generated:?} contains \"*/\"");
    // The slash is escaped so the values stay recognizable instead of being dropped.
    assert_string_contains_string("*\\/30 * * * *", &generated);
    assert_string_contains_string("\"*\\/5 * * * *\"|\"*\\/30 * * * *\"", &generated);
    Ok(())
}

#[test]
fn testPhpDocShapeSingleLevel() -> TestResult {
    let root = ArrayNode::new("root", None)?;

    assert_string_matches_format("array<%s>", &generate(&root)?);
    Ok(())
}

#[test]
fn testPhpDocShapeMultiLevel() -> TestResult {
    let root = ArrayNode::new("root", None)?;
    let child = ArrayNode::new("child", None)?;
    root.add_child(&child)?;

    assert_string_matches_format("array{%Achild?: array<%s>,%A}", &generate(&root)?);
    Ok(())
}

#[test]
fn testCanBeEnabled() -> TestResult {
    let root = ArrayNodeDefinition::new("root");
    root.can_be_enabled(None)?;

    assert_eq!(
        "bool|array{
 *     enabled?: bool|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator, // Default: false
 * }",
        generate(&root.get_node(false)?)?
    );
    Ok(())
}

#[test]
fn testCanBeDisabled() -> TestResult {
    let root = ArrayNodeDefinition::new("root");
    root.can_be_disabled(None)?;

    assert_eq!(
        "bool|array{
 *     enabled?: bool|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator, // Default: true
 * }",
        generate(&root.get_node(false)?)?
    );
    Ok(())
}

#[test]
fn testBeforeNormalizationIfTrueMakesArrayShapeUnsealed() -> TestResult {
    let root = ArrayNodeDefinition::new("root");
    root.children()
        .scalar_node("child")?
        .end()
        .end()
        .before_normalization()
        .if_true_with(|v| matches!(v, Value::Array(a) if a.get(&php_value::Key::from("extra")).is_some_and(|x| !x.is_null())))
        .then(Ok)
        .end()?;

    let shape = generate(&root.get_node(false)?)?;

    assert_string_contains_string(
        "array{
 *     child?: scalar|\\Symfony\\Component\\Config\\Loader\\ParamConfigurator|null,
 *     ...<string, mixed>
 * }",
        &shape,
    );
    Ok(())
}

fn provide_quoted_nodes() -> Result<Vec<(Node, &'static str)>, symfony_config::Error> {
    Ok(vec![
        (StringNode::new("int", None)?, "'int'"),
        (StringNode::new("float", None)?, "'float'"),
        (StringNode::new("null", None)?, "'null'"),
        (StringNode::new("bool", None)?, "'bool'"),
        (StringNode::new("scalar", None)?, "'scalar'"),
        (StringNode::new("hell\"o", None)?, "'hell\\\"o'"),
        (StringNode::new("hell'o", None)?, "'hell\\'o'"),
        (StringNode::new("@key", None)?, "'@key'"),
    ])
}

#[test]
fn testPhpdocQuoteNodeName() -> TestResult {
    for (node, expected) in provide_quoted_nodes()? {
        let array_node = ArrayNode::new("root", None)?;
        array_node.add_child(&node)?;

        assert_string_contains_string(expected, &generate(&array_node)?);
    }
    Ok(())
}
