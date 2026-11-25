//! Port of Symfony\Component\DependencyInjection\Argument\ServiceLocatorArgument.php.
//!
//! Upstream: symfony/dependency-injection Argument/ServiceLocatorArgument.php

use php_value::{PhpArray, Value};

use crate::argument::IteratorArgument;
use crate::object::{ObjectData, Shared};

#[derive(Debug, Clone, Default)]
pub struct ServiceLocatorArgumentData {
    values: PhpArray,
    tagged_iterator_argument: Option<IteratorArgument>,
}

impl ObjectData for ServiceLocatorArgumentData {
    fn class(&self) -> &'static str {
        ServiceLocatorArgument::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("values", Value::Array(self.values.clone()));
        properties.insert(
            "taggedIteratorArgument",
            self.tagged_iterator_argument
                .as_ref()
                .map_or(Value::Null, IteratorArgument::to_value),
        );
        properties
    }
}

/// `ServiceLocatorArgument`: a locator of services, by key.
#[derive(Debug, Clone)]
pub struct ServiceLocatorArgument(Shared<ServiceLocatorArgumentData>);

impl ServiceLocatorArgument {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Argument\\ServiceLocatorArgument";

    /// `new ServiceLocatorArgument($values)`.
    pub fn new(values: PhpArray) -> Self {
        Self(Shared::new(ServiceLocatorArgumentData {
            values,
            tagged_iterator_argument: None,
        }))
    }

    /// `new ServiceLocatorArgument($taggedIteratorArgument)`: no values
    /// until a compiler pass resolves the tag.
    pub fn from_tagged(tagged: IteratorArgument) -> Self {
        Self(Shared::new(ServiceLocatorArgumentData {
            values: PhpArray::new(),
            tagged_iterator_argument: Some(tagged),
        }))
    }

    /// `getTaggedIteratorArgument()`.
    pub fn tagged_iterator_argument(&self) -> Option<IteratorArgument> {
        self.0.read(|a| a.tagged_iterator_argument.clone())
    }

    /// `getValues()`.
    pub fn values(&self) -> PhpArray {
        self.0.read(|a| a.values.clone())
    }

    /// `setValues($values)`.
    pub fn set_values(&self, values: PhpArray) {
        self.0.write(|a| a.values = values);
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

impl From<ServiceLocatorArgument> for Value {
    fn from(argument: ServiceLocatorArgument) -> Self {
        argument.to_value()
    }
}
