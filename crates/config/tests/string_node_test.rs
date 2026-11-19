//! StringNodeTest.php at symfony/config v8.1.10. Names and assertions are
//! PHPUnit's; the `#[TestWith]` rows are upstream's.
//!
//! Upstream: symfony/config Tests/Definition/StringNodeTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, expect_class};
use php_value::{PhpArray, Value, php_array};
use symfony_config::{Class, definition::StringNode};

#[test]
fn testNormalize() -> TestResult {
    for value in ["", "valid string"] {
        let node = StringNode::new("test", None)?;
        assert_eq!(node.normalize(Value::from(value))?, Value::from(value));
    }
    Ok(())
}

#[test]
fn testNormalizeThrowsExceptionOnInvalidValues() -> TestResult {
    for value in [
        Value::Null,
        Value::Bool(false),
        Value::Bool(true),
        Value::Int(0),
        Value::Int(1),
        Value::Float(0.0),
        Value::Float(0.1),
        Value::Array(PhpArray::new()),
        Value::Array(php_array!["foo" => "bar"]),
    ] {
        let node = StringNode::new("test", None)?;
        expect_class(node.normalize(value), Class::InvalidType);
    }
    Ok(())
}
