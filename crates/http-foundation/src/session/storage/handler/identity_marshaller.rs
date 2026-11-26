//! Port of Symfony\Component\HttpFoundation\Session\Storage\Handler\IdentityMarshaller.php:
//! session data stored as it is.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Handler/IdentityMarshaller.php

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value};

use super::{HandlerResult, marshaller_interface::MarshallerInterface};
use crate::exception::{Class, Error};

/// `IdentityMarshaller::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\IdentityMarshaller";

/// `new IdentityMarshaller()`.
#[derive(Debug, Clone, Default)]
pub struct IdentityMarshaller;

impl MarshallerInterface for IdentityMarshaller {
    /// `marshall()`: the values, which must be strings
    /// (`\LogicException` otherwise).
    fn marshall(&self, values: &PhpArray, _failed: &mut PhpArray) -> HandlerResult<PhpArray> {
        if values.values().any(|v| !matches!(v, Value::String(_))) {
            return Err(Arc::new(Error::new(
                Class::PhpLogic,
                format!("{CLASS}::marshall accepts only string as data."),
            )));
        }
        Ok(values.clone())
    }

    fn unmarshall(&self, value: &PhpString) -> HandlerResult<Value> {
        Ok(Value::String(value.clone()))
    }
}
