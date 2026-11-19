//! Port of Symfony\Component\Config\Definition\IntegerNode.php: a
//! NumericNode for ints.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/IntegerNode.php

use php_value::{Number, Value};

use crate::definition::{
    base_node::{DEFAULT_PATH_SEPARATOR, Node, ScalarKind},
    exception::Error,
    numeric_node::NumericData,
    scalar_node::{invalid_type, scalar},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\IntegerNode";

/// `new IntegerNode($name, $parent, $min, $max, $pathSeparator)`.
pub struct IntegerNode;

impl IntegerNode {
    pub fn new(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        min: Option<Number>,
        max: Option<Number>,
    ) -> Result<Node, Error> {
        Self::with_separator(name, parent, min, max, DEFAULT_PATH_SEPARATOR.as_bytes())
    }

    pub fn with_separator(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        min: Option<Number>,
        max: Option<Number>,
        separator: &[u8],
    ) -> Result<Node, Error> {
        scalar(
            name.as_ref(),
            parent,
            separator,
            ScalarKind::Integer(NumericData { min, max }),
        )
    }
}

/// IntegerNode's `validateType()`.
pub(crate) fn validate_type(node: &Node, value: &Value) -> Result<(), Error> {
    match value {
        Value::Int(_) => Ok(()),
        _ => Err(invalid_type(node, "int", value)),
    }
}
