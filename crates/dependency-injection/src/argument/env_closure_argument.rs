//! Port of Symfony\Component\DependencyInjection\Argument\EnvClosureArgument.php.
//!
//! Upstream: symfony/dependency-injection Argument/EnvClosureArgument.php

use php_value::{PhpArray, Value, value::PhpObject};

use crate::exception::{Error, Result};
use crate::object::{downcast, object_value};

/// `EnvClosureArgument`: a closure that reads an env var at run time.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvClosureArgument {
    value: String,
    default: Value,
    stringable: bool,
}

impl EnvClosureArgument {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Argument\\EnvClosureArgument";

    /// `new EnvClosureArgument($value, $default, $stringable)`.
    pub fn new(value: impl Into<String>, default: Value, stringable: bool) -> Result<Self> {
        if stringable && !matches!(default, Value::Null | Value::String(_)) {
            return Err(Error::invalid_argument(
                "The default value of a stringable EnvClosureArgument must be a string or null.",
            ));
        }
        Ok(Self {
            value: value.into(),
            default,
            stringable,
        })
    }

    /// `getValue()`.
    pub fn value(&self) -> &str {
        &self.value
    }

    /// `getDefault()`.
    pub fn default_value(&self) -> &Value {
        &self.default
    }

    /// `isStringable()`.
    pub fn is_stringable(&self) -> bool {
        self.stringable
    }

    /// `getValues()`: always empty.
    pub fn values(&self) -> PhpArray {
        PhpArray::new()
    }

    /// `setValues($values)`: refuses any value.
    pub fn set_values(&self, values: &PhpArray) -> Result<()> {
        if values.is_empty() {
            Ok(())
        } else {
            Err(Error::invalid_argument(
                "An EnvClosureArgument cannot hold values.",
            ))
        }
    }

    pub fn from_value(value: &Value) -> Option<&Self> {
        downcast::<Self>(value)
    }
}

impl PhpObject for EnvClosureArgument {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("default", self.default.clone());
        properties.insert("value", Value::from(self.value.as_str()));
        properties.insert("stringable", Value::Bool(self.stringable));
        properties
    }
}

impl From<EnvClosureArgument> for Value {
    fn from(argument: EnvClosureArgument) -> Self {
        object_value(argument)
    }
}
