//! `Symfony\Component\Cache\Marshaller\MarshallerInterface` (symfony/cache),
//! which `MarshallingSessionHandler` uses: values made into strings for
//! storage, and back.
//!
//! Only the interface: symfony/cache is not ported. `marshall()`'s
//! `?array &$failed` is the PHP array the marshaller fills (symfony/cache's
//! own marshallers list the keys that failed: `$failed[] = $id`).
//!
//! Upstream: none

use std::fmt;

use php_value::{PhpArray, PhpString, Value};

use super::HandlerResult;

/// `MarshallerInterface`.
pub trait MarshallerInterface: fmt::Debug + Send + Sync {
    /// `marshall($values, &$failed)`.
    fn marshall(&self, values: &PhpArray, failed: &mut PhpArray) -> HandlerResult<PhpArray>;

    /// `unmarshall($value)`.
    fn unmarshall(&self, value: &PhpString) -> HandlerResult<Value>;
}
