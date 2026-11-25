//! Port of Symfony\Component\Config\Definition\StringNode.php: a
//! `ScalarNode` for strings.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/StringNode.php

use php_value::Value;

use crate::definition::{
    base_node::{DEFAULT_PATH_SEPARATOR, Node, ScalarKind},
    exception::Error,
    scalar_node::{invalid_type, scalar},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\StringNode";

/// `new StringNode($name, $parent, $pathSeparator)`.
pub struct StringNode;

impl StringNode {
    pub fn new(name: impl AsRef<[u8]>, parent: Option<&Node>) -> Result<Node, Error> {
        Self::with_separator(name, parent, DEFAULT_PATH_SEPARATOR.as_bytes())
    }

    pub fn with_separator(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        separator: &[u8],
    ) -> Result<Node, Error> {
        scalar(name.as_ref(), parent, separator, ScalarKind::String)
    }
}

/// `StringNode`'s `validateType()`.
pub(crate) fn validate_type(node: &Node, value: &Value) -> Result<(), Error> {
    match value {
        Value::String(_) => Ok(()),
        _ => Err(invalid_type(node, "string", value)),
    }
}
