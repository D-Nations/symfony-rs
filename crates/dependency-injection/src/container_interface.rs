//! Port of Symfony\Component\DependencyInjection\ContainerInterface.php:
//! the invalid-reference behaviours and the interface.
//!
//! A service is a PHP object value. The methods take `&self`: a container
//! creates its services on demand behind shared references, as PHP's
//! containers are shared objects, so implementations keep their state
//! behind interior mutability. PSR-11's `get()`/`has()` are this trait's
//! [`ContainerInterface::get`] with the default behaviour and
//! [`ContainerInterface::has`].
//!
//! Upstream: symfony/dependency-injection ContainerInterface.php

use php_value::Value;

use crate::exception::Result;

/// `Symfony\Component\DependencyInjection\ContainerInterface`.
pub const CONTAINER_INTERFACE: &str = "Symfony\\Component\\DependencyInjection\\ContainerInterface";

/// `ContainerInterface::*_ON_INVALID_REFERENCE` and
/// `IGNORE_ON_UNINITIALIZED_REFERENCE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum InvalidBehavior {
    /// `RUNTIME_EXCEPTION_ON_INVALID_REFERENCE` (0).
    RuntimeException,
    /// `EXCEPTION_ON_INVALID_REFERENCE` (1), the default.
    #[default]
    Exception,
    /// `NULL_ON_INVALID_REFERENCE` (2).
    Null,
    /// `IGNORE_ON_INVALID_REFERENCE` (3).
    Ignore,
    /// `IGNORE_ON_UNINITIALIZED_REFERENCE` (4).
    IgnoreUninitialized,
}

impl InvalidBehavior {
    /// The constant's value.
    pub fn as_int(self) -> i64 {
        match self {
            Self::RuntimeException => 0,
            Self::Exception => 1,
            Self::Null => 2,
            Self::Ignore => 3,
            Self::IgnoreUninitialized => 4,
        }
    }

    /// The behaviour for a constant's value.
    pub fn from_int(value: i64) -> Option<Self> {
        Some(match value {
            0 => Self::RuntimeException,
            1 => Self::Exception,
            2 => Self::Null,
            3 => Self::Ignore,
            4 => Self::IgnoreUninitialized,
            _ => return None,
        })
    }
}

/// `ContainerInterface`: a container of services and parameters.
pub trait ContainerInterface: Send + Sync {
    /// `set($id, $service)`.
    fn set(&self, id: &str, service: Option<Value>) -> Result<()>;

    /// `get($id, $invalidBehavior)`: the service, or `None` where the
    /// behaviour allows a missing service.
    fn get(&self, id: &str, invalid_behavior: InvalidBehavior) -> Result<Option<Value>>;

    /// `has($id)`.
    fn has(&self, id: &str) -> bool;

    /// `initialized($id)`.
    fn initialized(&self, id: &str) -> bool;

    /// `getParameter($name)`.
    fn get_parameter(&self, name: &str) -> Result<Value>;

    /// `hasParameter($name)`.
    fn has_parameter(&self, name: &str) -> bool;

    /// `setParameter($name, $value)`.
    fn set_parameter(&self, name: &str, value: Value) -> Result<()>;
}
