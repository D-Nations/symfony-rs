//! Port of Symfony\Component\HttpFoundation\Session\Attribute\AttributeBagInterface.php:
//! the attributes a session keeps.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Attribute/AttributeBagInterface.php

use php_value::{PhpArray, Value};

use crate::session::session_bag_interface::SessionBagInterface;

/// `AttributeBagInterface`.
pub trait AttributeBagInterface: SessionBagInterface {
    /// `has($name)`.
    fn has(&self, name: &str) -> bool;

    /// `get($name, $default)`.
    fn get(&self, name: &str, default: Value) -> Value;

    /// `set($name, $value)`.
    fn set(&self, name: &str, value: Value);

    /// `all()`.
    fn all(&self) -> PhpArray;

    /// `replace($attributes)`.
    fn replace(&self, attributes: &PhpArray);

    /// `remove($name)`: the removed value, or null.
    fn remove(&self, name: &str) -> Value;
}
