//! Port of Symfony\Component\Config\Definition\NumericNode.php: a
//! ScalarNode with an optional minimum and maximum, checked when the
//! value is finalized (with PHP's loose `<` and `>`, and the value printed
//! as `sprintf('%s')` prints it). A numeric value is never empty.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/NumericNode.php

use php_value::{Number, Value};

use crate::{
    definition::{
        base_node::{DEFAULT_PATH_SEPARATOR, Kind, Node, ScalarKind},
        exception::{Class, Error},
        scalar_node::scalar,
        variable_node::{self, VariableData},
    },
    php::{self, Msg},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\NumericNode";

/// NumericNode's properties.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct NumericData {
    pub(crate) min: Option<Number>,
    pub(crate) max: Option<Number>,
}

/// `new NumericNode($name, $parent, $min, $max, $pathSeparator)`.
pub struct NumericNode;

impl NumericNode {
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
            ScalarKind::Numeric(NumericData { min, max }),
        )
    }
}

pub(crate) fn number_value(n: Number) -> Value {
    match n {
        Number::Int(i) => Value::Int(i),
        Number::Float(f) => Value::Float(f),
    }
}

impl Node {
    fn numeric(&self, method: &str) -> Result<NumericData, Error> {
        match self.kind() {
            Kind::Variable {
                scalar: Some(ScalarKind::Numeric(n) | ScalarKind::Integer(n) | ScalarKind::Float(n)),
                ..
            } => Ok(n),
            _ => Err(crate::definition::base_node::undefined_method(
                &self.class(),
                method,
            )),
        }
    }

    /// `getMin()`.
    pub fn get_min(&self) -> Result<Option<Number>, Error> {
        Ok(self.numeric("getMin")?.min)
    }

    /// `getMax()`.
    pub fn get_max(&self) -> Result<Option<Number>, Error> {
        Ok(self.numeric("getMax")?.max)
    }
}

/// NumericNode's `finalizeValue()`.
pub(crate) fn finalize_value(
    node: &Node,
    data: &VariableData,
    numeric: &NumericData,
    value: Value,
) -> Result<Value, Error> {
    let value = variable_node::finalize_value(node, data, value)?;
    if node.is_handling_placeholder() {
        return Ok(value);
    }
    let mut message = None;
    if let Some(min) = numeric.min.map(number_value)
        && php::less(&value, &min)
    {
        message = Some(
            Msg::new("The value ")
                .b(&php::to_bytes(&value))
                .s(" is too small for path \"")
                .b(&node.get_path())
                .s("\". Should be greater than or equal to ")
                .b(&php::to_bytes(&min)),
        );
    }
    if let Some(max) = numeric.max.map(number_value)
        && php::greater(&value, &max)
    {
        message = Some(
            Msg::new("The value ")
                .b(&php::to_bytes(&value))
                .s(" is too big for path \"")
                .b(&node.get_path())
                .s("\". Should be less than or equal to ")
                .b(&php::to_bytes(&max)),
        );
    }
    if let Some(message) = message {
        return Err(node.config_error(Class::InvalidConfiguration, message, false));
    }
    Ok(value)
}
