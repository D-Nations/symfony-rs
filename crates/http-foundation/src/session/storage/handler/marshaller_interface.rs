//! `Symfony\Component\Cache\Marshaller\MarshallerInterface` (symfony/cache),
//! which `MarshallingSessionHandler` uses: values made into strings for
//! storage, and back.
//!
//! Only the interface: symfony/cache is not ported. `marshall()`'s
//! `?array &$failed` collects the keys that could not be marshalled.
//!
//! Upstream: none

use std::fmt;

use php_value::{Key, PhpArray, PhpString, Value};

use super::HandlerResult;

/// `MarshallerInterface`.
pub trait MarshallerInterface: fmt::Debug + Send + Sync {
    /// `marshall($values, &$failed)`.
    fn marshall(&self, values: &PhpArray, failed: &mut Vec<Key>) -> HandlerResult<PhpArray>;

    /// `unmarshall($value)`.
    fn unmarshall(&self, value: &PhpString) -> HandlerResult<Value>;
}
