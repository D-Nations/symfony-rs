//! Port of Symfony\Component\DependencyInjection\ContainerInterface.php:
//! the invalid-reference behaviours. The interface's methods come with
//! the `Container` port.
//!
//! Upstream: symfony/dependency-injection ContainerInterface.php

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
