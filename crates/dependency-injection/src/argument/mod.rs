//! Port of Symfony\Component\DependencyInjection\Argument\ArgumentInterface.php:
//! the argument objects whose values compiler passes read and replace.
//!
//! Not yet ported: `EnvClosure`, `LazyClosure`, `RewindableGenerator` and
//! `Argument\ServiceLocator`, the run-time halves that the compiled
//! container uses.
//!
//! Upstream: symfony/dependency-injection Argument/ArgumentInterface.php

mod abstract_argument;
mod bound_argument;
mod env_closure;
mod env_closure_argument;
mod iterator_argument;
mod service_closure_argument;
mod service_locator_argument;

pub use abstract_argument::AbstractArgument;
pub use bound_argument::{BoundArgument, DEFAULTS_BINDING, INSTANCEOF_BINDING, SERVICE_BINDING};
pub use env_closure::EnvClosure;
pub use env_closure_argument::EnvClosureArgument;
pub use iterator_argument::{IteratorArgument, TaggedData};
pub use service_closure_argument::ServiceClosureArgument;
pub use service_locator_argument::ServiceLocatorArgument;

use php_value::{PhpArray, Value};

use crate::exception::Result;

/// A value that implements `ArgumentInterface`.
#[derive(Debug, Clone)]
pub enum Argument {
    Bound(BoundArgument),
    EnvClosure(EnvClosureArgument),
    /// `IteratorArgument` and `TaggedIteratorArgument`.
    Iterator(IteratorArgument),
    ServiceClosure(ServiceClosureArgument),
    ServiceLocator(ServiceLocatorArgument),
}

impl Argument {
    /// The argument, when `value instanceof ArgumentInterface`.
    pub fn from_value(value: &Value) -> Option<Self> {
        if let Some(a) = IteratorArgument::from_value(value) {
            return Some(Self::Iterator(a));
        }
        if let Some(a) = ServiceClosureArgument::from_value(value) {
            return Some(Self::ServiceClosure(a));
        }
        if let Some(a) = ServiceLocatorArgument::from_value(value) {
            return Some(Self::ServiceLocator(a));
        }
        if let Some(a) = BoundArgument::from_value(value) {
            return Some(Self::Bound(a));
        }
        EnvClosureArgument::from_value(value).map(|a| Self::EnvClosure(a.clone()))
    }

    /// `getValues()`.
    pub fn values(&self) -> PhpArray {
        match self {
            Self::Bound(a) => a.values(),
            Self::EnvClosure(a) => a.values(),
            Self::Iterator(a) => a.values(),
            Self::ServiceClosure(a) => a.values(),
            Self::ServiceLocator(a) => a.values(),
        }
    }

    /// `setValues($values)`.
    pub fn set_values(&self, values: PhpArray) -> Result<()> {
        match self {
            Self::Bound(a) => a.set_values(&values),
            Self::EnvClosure(a) => a.set_values(&values)?,
            Self::Iterator(a) => a.set_values(values),
            Self::ServiceClosure(a) => a.set_values(values)?,
            Self::ServiceLocator(a) => a.set_values(values),
        }
        Ok(())
    }

    pub fn to_value(&self) -> Value {
        match self {
            Self::Bound(a) => a.to_value(),
            Self::EnvClosure(a) => Value::from(a.clone()),
            Self::Iterator(a) => a.to_value(),
            Self::ServiceClosure(a) => a.to_value(),
            Self::ServiceLocator(a) => a.to_value(),
        }
    }
}
