//! FinalizationTest.php at symfony/config v8.1.10. Names and assertions
//! are PHPUnit's; `assertEquals()` is `phpunit_equals()`.
//!
//! Upstream: symfony/config Tests/Definition/FinalizationTest.php
#![allow(non_snake_case)]

mod common;

use common::TestResult;
use php_value::{PhpArray, Value, php_array, vectors::phpunit_equals};
use symfony_config::{
    Error, Node, Processor,
    definition::builder::{TreeBuilder, class},
};

fn array(a: PhpArray) -> Value {
    Value::Array(a)
}

fn process(tree: &Node, configs: PhpArray) -> Result<PhpArray, Error> {
    let processor = Processor::new();

    processor.process(tree, &configs)
}

#[test]
fn testUnsetKeyWithDeepHierarchy() -> TestResult {
    let tb = TreeBuilder::new("config")?;
    tb.get_root_node()
        .children()
        .node("level1", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .can_be_unset(true)
        .children()
        .node("level2", "array")?
        .downcast::<class::Array>()
        .ok_or("not an ArrayNodeDefinition")?
        .can_be_unset(true)
        .children()
        .node("somevalue", "scalar")?
        .end()
        .node("anothervalue", "scalar")?
        .end()
        .end()
        .end()
        .node("level1_scalar", "scalar")?
        .end()
        .end()
        .end()
        .end()
        .end();
    let tree = tb.build_tree()?;

    let a = array(php_array![
        "level1" => array(php_array![
            "level2" => array(php_array!["somevalue" => "foo", "anothervalue" => "bar"]),
            "level1_scalar" => "foo",
        ])
    ]);
    let b = array(php_array!["level1" => array(php_array!["level2" => false])]);

    let expected = array(php_array![
        "level1" => array(php_array!["level1_scalar" => "foo"])
    ]);
    let actual = array(process(&tree, php_array![a, b])?);
    assert!(
        phpunit_equals(&expected, &actual),
        "expected {expected:?}, got {actual:?}"
    );
    Ok(())
}
