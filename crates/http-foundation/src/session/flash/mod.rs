//! The flash bags of `Symfony\Component\HttpFoundation\Session\Flash`, and
//! the `(array) $messages` cast their `set()` makes of `string|array`.
//!
//! Upstream: none

use php_value::{PhpArray, Value};

use crate::exception::{Class, Error, Result};

pub mod auto_expire_flash_bag;
pub mod flash_bag;
pub mod flash_bag_interface;

/// `string|array $messages` in coercive mode, then `(array)`: a scalar is
/// its string, a list of one; null or an object that is not `\Stringable`
/// is PHP's `\TypeError`.
pub(crate) mod messages {
    use super::{Class, Error, PhpArray, Result, Value};

    pub(crate) fn cast(method: &str, messages: Value) -> Result<PhpArray> {
        match messages {
            Value::Array(array) => Ok(array),
            other => match other.to_php_string_or_stringable() {
                Some(text) if !other.is_null() => Ok(PhpArray::list([Value::String(text)])),
                _ => Err(Error::new(
                    Class::PhpTypeError,
                    format!(
                        "{method}(): Argument #2 ($messages) must be of type array|string, {} given",
                        other.value_name()
                    ),
                )),
            },
        }
    }
}
