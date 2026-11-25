//! Port of Symfony\Component\Config\Definition\BooleanNode.php: a
//! `ScalarNode` for booleans, optionally nullable. A boolean is never
//! empty.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/BooleanNode.php

use php_value::Value;

use crate::definition::{
    base_node::{DEFAULT_PATH_SEPARATOR, Node, ScalarKind},
    exception::Error,
    scalar_node::{invalid_type, scalar},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\BooleanNode";

/// `new BooleanNode($name, $parent, $pathSeparator, $nullable)`.
pub struct BooleanNode;

impl BooleanNode {
    pub fn new(name: impl AsRef<[u8]>, parent: Option<&Node>) -> Result<Node, Error> {
        Self::with_options(name, parent, DEFAULT_PATH_SEPARATOR.as_bytes(), false)
    }

    pub fn with_options(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        separator: &[u8],
        nullable: bool,
    ) -> Result<Node, Error> {
        scalar(
            name.as_ref(),
            parent,
            separator,
            ScalarKind::Boolean { nullable },
        )
    }
}

/// `BooleanNode`'s `validateType()`.
pub(crate) fn validate_type(node: &Node, value: &Value, nullable: bool) -> Result<(), Error> {
    match value {
        Value::Bool(_) => Ok(()),
        Value::Null if nullable => Ok(()),
        _ => Err(invalid_type(
            node,
            if nullable { "bool\" or \"null" } else { "bool" },
            value,
        )),
    }
}
