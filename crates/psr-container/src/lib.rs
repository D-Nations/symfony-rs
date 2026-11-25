//! Port of psr/container (PSR-11): ContainerInterface.php,
//! ContainerExceptionInterface.php and NotFoundExceptionInterface.php.
//!
//! An identifier is a PHP string (bytes); an entry is any PHP value
//! (`mixed`), a php-value [`Value`]. The two exception interfaces have no
//! methods beyond `\Throwable`'s: an error a container returns is any
//! shared Rust error, and an implementation reports which of the two
//! interfaces it implements through [`CONTAINER_EXCEPTION_INTERFACE`] and
//! [`NOT_FOUND_EXCEPTION_INTERFACE`] in its own `instanceof` answer.
//!
//! psr/container copyright (c) 2013-2016 container-interop, (c) 2016 PHP
//! Framework Interoperability Group; MIT.
//!
//! Upstream: psr/container src/ContainerInterface.php src/ContainerExceptionInterface.php src/NotFoundExceptionInterface.php

#![forbid(unsafe_code)]

use std::sync::Arc;

use php_value::Value;

/// `Psr\Container\ContainerInterface`.
pub const CONTAINER_INTERFACE: &str = "Psr\\Container\\ContainerInterface";

/// `Psr\Container\ContainerExceptionInterface`, a `\Throwable`: any error
/// while retrieving an entry.
pub const CONTAINER_EXCEPTION_INTERFACE: &str = "Psr\\Container\\ContainerExceptionInterface";

/// `Psr\Container\NotFoundExceptionInterface`, a
/// `ContainerExceptionInterface`: no entry was found.
pub const NOT_FOUND_EXCEPTION_INTERFACE: &str = "Psr\\Container\\NotFoundExceptionInterface";

/// A thrown `\Throwable`, shared as a PHP exception object is.
pub type Throwable = Arc<dyn std::error::Error + Send + Sync + 'static>;

/// `ContainerInterface`: a container that exposes its entries.
pub trait ContainerInterface: Send + Sync {
    /// `get($id)`: the entry for the identifier. Errs with a
    /// `NotFoundExceptionInterface` when there is none, or a
    /// `ContainerExceptionInterface` when retrieving it fails.
    fn get(&self, id: &[u8]) -> Result<Value, Throwable>;

    /// `has($id)`: whether `get($id)` would find an entry. `true` does not
    /// promise that `get($id)` succeeds.
    fn has(&self, id: &[u8]) -> bool;
}
