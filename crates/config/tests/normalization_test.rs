//! NormalizationTest.php at symfony/config v8.1.10. Names, providers and
//! assertions are PHPUnit's; `assertSame()` is `identical()`.
//!
//! Upstream: symfony/config Tests/Definition/NormalizationTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_exception};
use php_value::{PhpArray, Value, php_array};
use symfony_config::{
    Class, Node,
    definition::builder::{TreeBuilder, class},
};

fn array(a: PhpArray) -> Value {
    Value::Array(a)
}

#[track_caller]
fn assert_normalized(tree: &Node, denormalized: Value, normalized: Value) -> TestResult {
    let actual = tree.normalize(denormalized)?;
    assert!(
        normalized.identical(&actual),
        "expected {normalized:?}, got {actual:?}"
    );
    Ok(())
}

fn encoder_tree() -> Result<Node, Box<dyn std::error::Error>> {
    let tb = TreeBuilder::new("root_name")?;
    let root = tb.get_root_node();
    root.fix_xml_config("encoder", None)
        .children()
        .node("encoders", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .use_attribute_as_key("class", true)
        .prototype("array")?
        .before_normalization()
        .if_string()
        .then(|v| Ok(array(php_array!["algorithm" => v])))
        .end()?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .children()
        .node("algorithm", "scalar")?
        .end()
        .end()
        .end()
        .end()
        .end()
        .end();
    Ok(tb.build_tree()?)
}

fn get_encoder_tests() -> Vec<Value> {
    let encoder = || php_array!["class" => "foo", "algorithm" => "plaintext"];
    vec![
        // XML
        array(php_array!["encoder" => array(PhpArray::list([array(encoder())]))]),
        // XML when only one element of this type
        array(php_array!["encoder" => array(encoder())]),
        // YAML/PHP
        array(php_array!["encoders" => array(PhpArray::list([array(encoder())]))]),
        // YAML/PHP
        array(php_array!["encoders" => array(php_array!["foo" => "plaintext"])]),
        // YAML/PHP
        array(php_array![
            "encoders" => array(php_array!["foo" => array(php_array!["algorithm" => "plaintext"])])
        ]),
    ]
}

#[test]
fn testNormalizeEncoders() -> TestResult {
    let normalized = array(php_array![
        "encoders" => array(php_array!["foo" => array(php_array!["algorithm" => "plaintext"])])
    ]);
    for denormalized in get_encoder_tests() {
        assert_normalized(&encoder_tree()?, denormalized, normalized.clone())?;
    }
    Ok(())
}

fn anonymous_keys_tree() -> Result<Node, Box<dyn std::error::Error>> {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .children()
        .node("logout", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .fix_xml_config("handler", None)
        .children()
        .node("handlers", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .prototype("scalar")?
        .end()
        .end()
        .end()
        .end()
        .end()
        .end();
    Ok(tb.build_tree()?)
}

#[test]
fn testAnonymousKeysArray() -> TestResult {
    let abc = || array(php_array!["a", "b", "c"]);
    let normalized = array(php_array!["logout" => array(php_array!["handlers" => abc()])]);
    let configs = [
        array(php_array!["logout" => array(php_array!["handlers" => abc()])]),
        array(php_array!["logout" => array(php_array!["handler" => abc()])]),
    ];
    for denormalized in configs {
        assert_normalized(&anonymous_keys_tree()?, denormalized, normalized.clone())?;
    }
    Ok(())
}

fn get_numeric_keys_test_tree() -> Result<Node, Box<dyn std::error::Error>> {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .children()
        .node("thing", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .use_attribute_as_key("id", true)
        .prototype("array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .prototype("scalar")?
        .end()
        .end()
        .end()
        .end()
        .end();
    Ok(tb.build_tree()?)
}

#[test]
fn testNumericKeysAsAttributes() -> TestResult {
    let normalized = array(php_array![
        "thing" => array(php_array![
            42 => array(php_array!["foo", "bar"]),
            1337 => array(php_array!["baz", "qux"]),
        ])
    ]);
    let configs = [
        array(php_array![
            "thing" => array(php_array![
                42 => array(php_array!["foo", "bar"]),
                1337 => array(php_array!["baz", "qux"]),
            ])
        ]),
        array(php_array![
            "thing" => array(php_array![
                array(php_array![0 => "foo", 1 => "bar", "id" => 42]),
                array(php_array![0 => "baz", 1 => "qux", "id" => 1337]),
            ])
        ]),
    ];
    for denormalized in configs {
        assert_normalized(
            &get_numeric_keys_test_tree()?,
            denormalized,
            normalized.clone(),
        )?;
    }
    Ok(())
}

#[test]
fn testNonAssociativeArrayThrowsExceptionIfAttributeNotSet() -> TestResult {
    let denormalized = array(php_array![
        "thing" => array(php_array![
            array(php_array!["foo", "bar"]),
            array(php_array!["baz", "qux"]),
        ])
    ]);
    expect_exception(
        get_numeric_keys_test_tree()?.normalize(denormalized),
        Class::InvalidConfiguration,
        "The attribute \"id\" must be set for path \"root.thing\".",
    );
    Ok(())
}

#[test]
fn testAssociativeArrayPreserveKeys() -> TestResult {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .prototype("array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .children()
        .node("foo", "scalar")?
        .end()
        .end()
        .end()
        .end();
    let tree = tb.build_tree()?;

    let data = array(php_array!["first" => array(php_array!["foo" => "bar"])]);
    assert_normalized(&tree, data.clone(), data)
}

#[test]
fn testFloatLikeValueAsMapKeyAttribute() -> TestResult {
    let tb = TreeBuilder::new("root")?;
    tb.get_root_node()
        .use_attribute_as_key("number", true)
        .array_prototype()?
        .children()
        .scalar_node("foo")?
        .end()
        .end()
        .end()
        .end();
    let tree = tb.build_tree()?;

    assert_normalized(
        &tree,
        array(php_array![array(
            php_array!["number" => 3.0, "foo" => "bar"]
        )]),
        array(php_array!["3.0" => array(php_array!["foo" => "bar"])]),
    )
}
