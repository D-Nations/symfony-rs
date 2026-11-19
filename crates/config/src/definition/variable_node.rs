//! Port of Symfony\Component\Config\Definition\VariableNode.php: a node
//! that accepts any value, with an optional default (a value or a closure
//! giving it) and the "empty value" check.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/VariableNode.php

use std::sync::Arc;

use php_value::{Value, json::JsonFlags};

use crate::{
    definition::{
        base_node::{DEFAULT_PATH_SEPARATOR, Kind, Node, undefined_method},
        exception::{Class, Error},
    },
    php::{self, Msg},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\VariableNode";

/// A default value given as a closure (`$v instanceof \Closure ? $v() : $v`).
pub type DefaultClosure = Arc<dyn Fn() -> Result<Value, Error> + Send + Sync>;

/// A default value.
#[derive(Clone)]
pub enum DefaultValue {
    Value(Value),
    Closure(DefaultClosure),
}

/// VariableNode's properties.
#[derive(Clone)]
pub(crate) struct VariableData {
    pub(crate) default_value_set: bool,
    pub(crate) default_value: DefaultValue,
    pub(crate) allow_empty_value: bool,
}

impl Default for VariableData {
    fn default() -> Self {
        Self {
            default_value_set: false,
            default_value: DefaultValue::Value(Value::Null),
            allow_empty_value: true,
        }
    }
}

/// `new VariableNode($name, $parent, $pathSeparator)`.
pub struct VariableNode;

impl VariableNode {
    pub fn new(name: impl AsRef<[u8]>, parent: Option<&Node>) -> Result<Node, Error> {
        Self::with_separator(name, parent, DEFAULT_PATH_SEPARATOR.as_bytes())
    }

    pub fn with_separator(
        name: impl AsRef<[u8]>,
        parent: Option<&Node>,
        separator: &[u8],
    ) -> Result<Node, Error> {
        Node::construct(
            name.as_ref(),
            parent,
            separator,
            Kind::Variable {
                data: VariableData::default(),
                scalar: None,
            },
        )
    }
}

pub(crate) fn get_default_value(data: &VariableData) -> Result<Value, Error> {
    match &data.default_value {
        DefaultValue::Value(v) => Ok(v.clone()),
        DefaultValue::Closure(f) => f(),
    }
}

impl Node {
    /// VariableNode's `setDefaultValue($value)`; PrototypedArrayNode's
    /// takes an array.
    pub fn set_default_value(&self, value: impl Into<DefaultValue>) -> Result<(), Error> {
        let value = value.into();
        match self.kind() {
            Kind::Variable { .. } => {
                self.write(|n| {
                    if let Kind::Variable { data, .. } = &mut n.kind {
                        data.default_value_set = true;
                        data.default_value = value;
                    }
                });
                Ok(())
            }
            Kind::Prototyped(..) => match value {
                DefaultValue::Value(Value::Array(array)) => {
                    self.set_prototyped_default_value(array);
                    Ok(())
                }
                DefaultValue::Value(other) => Err(Error::new(
                    Class::TypeError,
                    format!(
                        "Symfony\\Component\\Config\\Definition\\PrototypedArrayNode::setDefaultValue(): Argument #1 ($value) must be of type array, {} given",
                        other.debug_type()
                    ),
                )),
                DefaultValue::Closure(_) => Err(Error::new(
                    Class::TypeError,
                    "Symfony\\Component\\Config\\Definition\\PrototypedArrayNode::setDefaultValue(): Argument #1 ($value) must be of type array, Closure given",
                )),
            },
            _ => Err(undefined_method(&self.class(), "setDefaultValue")),
        }
    }

    /// `setAllowEmptyValue()`, which VariableNode and its subclasses have.
    pub fn set_allow_empty_value(&self, allow: bool) -> Result<(), Error> {
        let done = self.write(|n| match &mut n.kind {
            Kind::Variable { data, .. } => {
                data.allow_empty_value = allow;
                true
            }
            _ => false,
        });
        if done {
            Ok(())
        } else {
            Err(undefined_method(&self.class(), "setAllowEmptyValue"))
        }
    }
}

impl From<Value> for DefaultValue {
    fn from(v: Value) -> Self {
        Self::Value(v)
    }
}

impl From<DefaultClosure> for DefaultValue {
    fn from(f: DefaultClosure) -> Self {
        Self::Closure(f)
    }
}

/// `json_encode()` of a value `isValueEmpty()` found empty.
fn json_encode_empty(value: &Value) -> Vec<u8> {
    let flags = JsonFlags::default();
    php::json_encode_scalar(value, flags, false).unwrap_or_else(|| b"false".to_vec())
}

/// VariableNode's `finalizeValue()`.
pub(crate) fn finalize_value(
    node: &Node,
    data: &VariableData,
    value: Value,
) -> Result<Value, Error> {
    // Placeholders are refused only when custom validators run, so that
    // an empty value never reaches them.
    if !data.allow_empty_value
        && node.is_handling_placeholder()
        && node.has_final_validation_closures()
    {
        return Err(node.config_error(
            Class::InvalidConfiguration,
            Msg::new("The path \"")
                .b(&node.get_path())
                .s("\" cannot contain an environment variable when empty values are not allowed by definition and are validated."),
            true,
        ));
    }
    if !data.allow_empty_value && node.is_value_empty(&value) {
        return Err(node.config_error(
            Class::InvalidConfiguration,
            Msg::new("The path \"")
                .b(&node.get_path())
                .s("\" cannot contain an empty value, but got ")
                .b(&json_encode_empty(&value))
                .s("."),
            true,
        ));
    }
    Ok(value)
}
