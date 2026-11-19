//! BaseNodeTest.php at symfony/config v8.1.10. The anonymous BaseNode
//! subclass is a [`CustomNode`]; the stubbed parent NodeInterface is a
//! custom node whose `getPath()` returns the stubbed path.
//!
//! Upstream: symfony/config Tests/Definition/BaseNodeTest.php
#![allow(non_snake_case)]

mod common;

use std::sync::Arc;

use common::TestResult;
use php_value::{PhpString, Value};
use symfony_config::{
    Error, Node,
    definition::{CustomNode, DEFAULT_PATH_SEPARATOR},
};

/// The test's anonymous class: every method returns null or true.
struct Anonymous;

impl CustomNode for Anonymous {
    fn class(&self) -> &str {
        "BaseNode@anonymous"
    }
    fn validate_type(&self, _: &Node, _: &Value) -> Result<(), Error> {
        Ok(())
    }
    fn normalize_value(&self, _: &Node, _: Value) -> Result<Value, Error> {
        Ok(Value::Null)
    }
    fn merge_values(&self, _: &Node, _: Value, _: Value) -> Result<Value, Error> {
        Ok(Value::Null)
    }
    fn finalize_value(&self, _: &Node, _: Value) -> Result<Value, Error> {
        Ok(Value::Null)
    }
    fn has_default_value(&self, _: &Node) -> bool {
        true
    }
    fn get_default_value(&self, _: &Node) -> Result<Value, Error> {
        Ok(Value::Null)
    }
}

/// `$this->createStub(NodeInterface::class)` with `getPath()` stubbed.
struct StubParent(&'static str);

impl CustomNode for StubParent {
    fn class(&self) -> &str {
        "NodeInterface@stub"
    }
    fn validate_type(&self, _: &Node, _: &Value) -> Result<(), Error> {
        Ok(())
    }
    fn normalize_value(&self, _: &Node, v: Value) -> Result<Value, Error> {
        Ok(v)
    }
    fn merge_values(&self, _: &Node, _: Value, r: Value) -> Result<Value, Error> {
        Ok(r)
    }
    fn finalize_value(&self, _: &Node, v: Value) -> Result<Value, Error> {
        Ok(v)
    }
    fn has_default_value(&self, _: &Node) -> bool {
        false
    }
    fn get_default_value(&self, _: &Node) -> Result<Value, Error> {
        Ok(Value::Null)
    }
    fn get_path(&self, _: &Node) -> Option<PhpString> {
        Some(PhpString::from(self.0))
    }
}

/// `[$expected, [$name, $parentPath?, $separator?]]`.
type PathRow = (
    &'static str,
    (&'static str, Option<&'static str>, Option<&'static str>),
);

fn provide_path() -> Vec<PathRow> {
    vec![
        ("root", ("root", None, None)),
        ("foo.bar.baz.bim", ("bim", Some("foo.bar.baz"), None)),
        ("foo", ("foo", None, Some("/"))),
        ("foo.bar/baz/bim", ("bim", Some("foo.bar/baz"), Some("/"))),
    ]
}

#[test]
fn testGetPathForChildNode() -> TestResult {
    for (expected, (name, parent_path, separator)) in provide_path() {
        // Upstream passes the separator only with a parent.
        let (parent, separator) = match parent_path {
            Some(path) => (
                Some(Node::custom(b"", None, b".", Arc::new(StubParent(path)))?),
                separator.unwrap_or(DEFAULT_PATH_SEPARATOR),
            ),
            None => (None, DEFAULT_PATH_SEPARATOR),
        };
        let node = Node::custom(
            name.as_bytes(),
            parent.as_ref(),
            separator.as_bytes(),
            Arc::new(Anonymous),
        )?;
        assert_eq!(node.get_path(), PhpString::from(expected));
    }
    Ok(())
}
