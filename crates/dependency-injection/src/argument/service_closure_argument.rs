//! Port of Symfony\Component\DependencyInjection\Argument\ServiceClosureArgument.php.
//!
//! Upstream: symfony/dependency-injection Argument/ServiceClosureArgument.php

use php_value::{PhpArray, Value, array::Key};

use crate::exception::{Error, Result};
use crate::object::{ObjectData, Shared};

#[derive(Debug, Clone, Default)]
pub struct ServiceClosureArgumentData {
    values: PhpArray,
}

impl ObjectData for ServiceClosureArgumentData {
    fn class(&self) -> &'static str {
        ServiceClosureArgument::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("values", Value::Array(self.values.clone()));
        properties
    }
}

/// `ServiceClosureArgument`: a closure that returns one service.
#[derive(Debug, Clone)]
pub struct ServiceClosureArgument(Shared<ServiceClosureArgumentData>);

impl ServiceClosureArgument {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Argument\\ServiceClosureArgument";

    /// `new ServiceClosureArgument($value)`.
    pub fn new(value: Value) -> Self {
        Self(Shared::new(ServiceClosureArgumentData {
            values: PhpArray::list([value]),
        }))
    }

    /// `getValues()`: `[$value]`.
    pub fn values(&self) -> PhpArray {
        self.0.read(|a| a.values.clone())
    }

    /// The one value.
    pub fn value(&self) -> Value {
        self.0
            .read(|a| a.values.get(&Key::Int(0)).cloned())
            .unwrap_or(Value::Null)
    }

    /// `setValues($values)`: the array's keys must be exactly `[0]`.
    pub fn set_values(&self, values: PhpArray) -> Result<()> {
        if values.len() != 1 || !values.contains_key(&Key::Int(0)) {
            return Err(Error::invalid_argument(
                "A ServiceClosureArgument must hold one and only one value.",
            ));
        }
        self.0.write(|a| a.values = values);
        Ok(())
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }

    pub fn to_value(&self) -> Value {
        self.0.to_value()
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        Shared::from_value(value).map(Self)
    }
}

impl From<ServiceClosureArgument> for Value {
    fn from(argument: ServiceClosureArgument) -> Self {
        argument.to_value()
    }
}
