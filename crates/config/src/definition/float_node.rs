//! Port of Symfony\Component\Config\Definition\FloatNode.php: a
//! NumericNode for floats; ints are accepted too (and kept as ints, as
//! upstream only casts its local copy).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/FloatNode.php

use php_value::{Number, Value};

use crate::definition::{
    base_node::{DEFAULT_PATH_SEPARATOR, Node, ScalarKind},
    exception::Error,
    numeric_node::NumericData,
    scalar_node::{invalid_type, scalar},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\FloatNode";

/// `new FloatNode($name, $parent, $min, $max, $pathSeparator)`.
pub struct FloatNode;

impl FloatNode {
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
            ScalarKind::Float(NumericData { min, max }),
        )
    }
}

/// FloatNode's `validateType()`.
pub(crate) fn validate_type(node: &Node, value: &Value) -> Result<(), Error> {
    match value {
        Value::Int(_) | Value::Float(_) => Ok(()),
        _ => Err(invalid_type(node, "float", value)),
    }
}
