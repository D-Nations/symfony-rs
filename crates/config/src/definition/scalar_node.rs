//! Port of Symfony\Component\Config\Definition\ScalarNode.php: a
//! VariableNode that only accepts scalars and null. Its `isValueEmpty()`
//! (null and `""`, never a placeholder) and placeholder types are in
//! [`crate::definition::base_node`]'s dispatch.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/ScalarNode.php

use php_value::Value;

use crate::{
    definition::{
        base_node::{DEFAULT_PATH_SEPARATOR, Kind, Node, ScalarKind},
        exception::{Class, Error},
        variable_node::VariableData,
    },
    php::Msg,
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\ScalarNode";

/// `new ScalarNode($name, $parent, $pathSeparator)`.
pub struct ScalarNode;

impl ScalarNode {
    pub fn new(name: impl AsRef<[u8]>, parent: Option<&Node>) -> Result<Node, Error> {
        Self::with_separator(name, parent, DEFAULT_PATH_SEPARATOR.as_bytes())
    }

    pub fn with_separator(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        separator: &[u8],
    ) -> Result<Node, Error> {
        scalar(name.as_ref(), parent, separator, ScalarKind::Scalar)
    }
}

pub(crate) fn scalar(
    name: &[u8],
    parent: Option<&Node>,
    separator: &[u8],
    kind: ScalarKind,
) -> Result<Node, Error> {
    Node::construct(
        name,
        parent,
        separator,
        Kind::Variable {
            data: VariableData::default(),
            scalar: Some(kind),
        },
    )
}

/// "Invalid type for path "..". Expected "<expected>", but got "<type>"."
/// with the info as a hint.
pub(crate) fn invalid_type(node: &Node, expected: &str, value: &Value) -> Error {
    node.config_error(
        Class::InvalidType,
        Msg::new("Invalid type for path \"")
            .b(&node.get_path())
            .s("\". Expected \"")
            .s(expected)
            .s("\", but got \"")
            .s(&value.debug_type())
            .s("\"."),
        true,
    )
}

/// ScalarNode's `validateType()`.
pub(crate) fn validate_type(node: &Node, value: &Value) -> Result<(), Error> {
    match value {
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => Ok(()),
        _ => Err(invalid_type(node, "scalar", value)),
    }
}
