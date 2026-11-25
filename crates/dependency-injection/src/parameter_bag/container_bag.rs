//! Port of Symfony\Component\DependencyInjection\ParameterBag\ContainerBag.php
//! and ContainerBagInterface.php.
//!
//! A `FrozenParameterBag` that reads the container's parameters. Its own
//! bag is never constructed upstream (no `parent::__construct()`), so it
//! is unresolved: `resolveValue()` resolves through the container's
//! parameters as an unresolved bag does, and the mutators fail as a
//! frozen bag's do.
//!
//! Upstream: symfony/dependency-injection ParameterBag/ContainerBag.php ParameterBag/ContainerBagInterface.php

use std::sync::Arc;

use php_value::{PhpArray, Value};

use crate::container::Container;
use crate::exception::Result;
use crate::parameter_bag::{ParameterBag, escape_value, unescape_value};

/// `ContainerBagInterface` (a PSR-11 container of parameters).
pub trait ContainerBagInterface: psr_container::ContainerInterface {
    /// `all()`.
    fn all(&self) -> PhpArray;

    /// `resolveValue($value)`.
    fn resolve_value(&self, value: &Value) -> Result<Value>;

    /// `escapeValue($value)`.
    fn escape_value(&self, value: &Value) -> Value;

    /// `unescapeValue($value)`.
    fn unescape_value(&self, value: &Value) -> Value;
}

/// `Symfony\Component\DependencyInjection\ParameterBag\ContainerBag`.
pub const CONTAINER_BAG_CLASS: &str =
    "Symfony\\Component\\DependencyInjection\\ParameterBag\\ContainerBag";

/// `ContainerBag`.
#[derive(Debug, Clone)]
pub struct ContainerBag {
    container: Arc<Container>,
}

impl ContainerBag {
    pub fn new(container: Arc<Container>) -> Self {
        Self { container }
    }

    /// `$bag instanceof $class`.
    pub fn instance_of(class: &str) -> bool {
        matches!(
            class.trim_start_matches('\\'),
            CONTAINER_BAG_CLASS
                | "Symfony\\Component\\DependencyInjection\\ParameterBag\\ContainerBagInterface"
                | "Symfony\\Component\\DependencyInjection\\ParameterBag\\FrozenParameterBag"
                | "Symfony\\Component\\DependencyInjection\\ParameterBag\\ParameterBag"
                | "Symfony\\Component\\DependencyInjection\\ParameterBag\\ParameterBagInterface"
                | psr_container::CONTAINER_INTERFACE
        )
    }

    /// `all()`.
    pub fn all(&self) -> PhpArray {
        self.container.with_parameter_bag(|bag| bag.all().clone())
    }

    /// `get($name)`.
    pub fn get(&self, name: &str) -> Result<Value> {
        self.container.get_parameter(name)
    }

    /// `has($name)`.
    pub fn has(&self, name: &str) -> bool {
        self.container.has_parameter(name)
    }

    /// `resolveValue($value)`.
    pub fn resolve_value(&self, value: &Value) -> Result<Value> {
        let mut bag = self.container.get_parameter_bag();
        bag.set_resolved(false);
        bag.resolve_value(value, &[])
    }

    /// `escapeValue($value)`.
    pub fn escape_value(&self, value: &Value) -> Value {
        escape_value(value)
    }

    /// `unescapeValue($value)`.
    pub fn unescape_value(&self, value: &Value) -> Value {
        unescape_value(value)
    }

    fn frozen() -> ParameterBag {
        ParameterBag::frozen(PhpArray::new(), Vec::new(), Vec::new())
    }

    /// `clear()`: refused, the bag is frozen.
    pub fn clear(&self) -> Result<()> {
        Self::frozen().clear()
    }

    /// `add($parameters)`: refused, the bag is frozen.
    pub fn add(&self, parameters: &PhpArray) -> Result<()> {
        Self::frozen().add(parameters)
    }

    /// `set($name, $value)`: refused, the bag is frozen.
    pub fn set(&self, name: &str, value: Value) -> Result<()> {
        Self::frozen().set(name, value)
    }

    /// `remove($name)`: refused, the bag is frozen.
    pub fn remove(&self, name: &str) -> Result<()> {
        Self::frozen().remove(name)
    }
}

impl ContainerBagInterface for ContainerBag {
    fn all(&self) -> PhpArray {
        ContainerBag::all(self)
    }

    fn resolve_value(&self, value: &Value) -> Result<Value> {
        ContainerBag::resolve_value(self, value)
    }

    fn escape_value(&self, value: &Value) -> Value {
        escape_value(value)
    }

    fn unescape_value(&self, value: &Value) -> Value {
        unescape_value(value)
    }
}

/// PSR-11's `get($name)`/`has($name)`.
impl psr_container::ContainerInterface for ContainerBag {
    fn get(&self, id: &[u8]) -> std::result::Result<Value, psr_container::Throwable> {
        let id = String::from_utf8_lossy(id);
        ContainerBag::get(self, &id).map_err(|e| Arc::new(e) as _)
    }

    fn has(&self, id: &[u8]) -> bool {
        std::str::from_utf8(id).is_ok_and(|id| ContainerBag::has(self, id))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use php_value::php_array;

    use super::*;
    use crate::exception::ErrorKind;

    #[test]
    fn resolves_through_the_container_and_stays_frozen() {
        let bag = ParameterBag::new(&php_array!["foo" => "value"]).unwrap();
        let container_bag = ContainerBag::new(Container::new(Some(bag)));
        let resolved = container_bag
            .resolve_value(&Value::from("%foo%-x %%"))
            .unwrap();
        assert!(resolved.identical(&Value::from("value-x %%")));
        let e = container_bag.set("bar", Value::Null).unwrap_err();
        assert!(e.is(&ErrorKind::Logic));
    }
}
