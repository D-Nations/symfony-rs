//! Port of Symfony\Component\HttpFoundation\Session\Storage\Handler\MarshallingSessionHandler.php:
//! session data passed through a marshaller (to encrypt it, for one)
//! on its way to and from another handler.
//!
//! Data the marshaller cannot read (`\DomainException`) is a missing
//! session, as PHP treats data it cannot decode; empty data reaches the
//! handler as it is, so the handler destroys the session. `read()`
//! returns `string|false`: the marshaller's other answers convert as
//! PHP's coercive mode converts them, or fail with its `\TypeError`.
//!
//! The handler must be an `AbstractSessionHandler`; here, a handler that
//! implements `\SessionUpdateTimestampHandlerInterface`, which every
//! `AbstractSessionHandler` does.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Handler/MarshallingSessionHandler.php

use std::sync::Arc;

use php_value::{Key, PhpArray, PhpString, Value};

use super::{
    HandlerResult, SessionHandlerInterface, SessionIdInterface,
    SessionUpdateTimestampHandlerInterface, marshaller_interface::MarshallerInterface,
};
use crate::{
    exception::{Class, Error, Result},
    streamed_response::Throwable,
};

/// `MarshallingSessionHandler::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\MarshallingSessionHandler";

/// `new MarshallingSessionHandler($handler, $marshaller)`.
#[derive(Debug)]
pub struct MarshallingSessionHandler {
    handler: Arc<dyn SessionHandlerInterface>,
    marshaller: Arc<dyn MarshallerInterface>,
}

/// Whether a throwable is a `\DomainException` (or a subclass).
fn is_domain_exception(error: &Throwable) -> bool {
    if let Some(error) = error.downcast_ref::<Error>() {
        return error.is_a("DomainException");
    }
    error
        .downcast_ref::<php_value::Thrown>()
        .is_some_and(|t| t.class == "DomainException")
}

impl MarshallingSessionHandler {
    /// Fails with `\TypeError` for a handler that is not an
    /// `AbstractSessionHandler`.
    pub fn new(
        handler: Arc<dyn SessionHandlerInterface>,
        marshaller: Arc<dyn MarshallerInterface>,
    ) -> Result<Self> {
        if handler.as_update_timestamp().is_none() {
            return Err(Error::new(
                Class::PhpTypeError,
                format!(
                    "{CLASS}::__construct(): Argument #1 ($handler) must be of type Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\AbstractSessionHandler, {} given",
                    handler.class()
                ),
            ));
        }
        Ok(Self {
            handler,
            marshaller,
        })
    }

    fn timestamp(&self) -> HandlerResult<&dyn SessionUpdateTimestampHandlerInterface> {
        self.handler.as_update_timestamp().ok_or_else(|| {
            Arc::new(Error::new(
                Class::NotPorted,
                "the handler lost its interface",
            )) as Throwable
        })
    }
}

/// `string $data` given a value, in coercive mode.
fn string_argument(method: &str, value: &Value) -> HandlerResult<PhpString> {
    match value {
        Value::String(s) => Ok(s.clone()),
        Value::Int(_) | Value::Float(_) | Value::Bool(_) => {
            Ok(value.to_php_string().unwrap_or_default())
        }
        other => Err(Arc::new(Error::new(
            Class::PhpTypeError,
            format!(
                "{method}(): Argument #2 ($data) must be of type string, {} given",
                other.value_name()
            ),
        ))),
    }
}

impl SessionHandlerInterface for MarshallingSessionHandler {
    fn class(&self) -> String {
        CLASS.to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        self.handler.open(save_path, session_name)
    }

    fn close(&self) -> HandlerResult<bool> {
        self.handler.close()
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        let Some(data) = self.handler.read(session_id)? else {
            return Ok(None);
        };
        match self.marshaller.unmarshall(&data) {
            Ok(Value::String(s)) => Ok(Some(s)),
            Ok(Value::Bool(false)) => Ok(None),
            Ok(value @ (Value::Int(_) | Value::Float(_) | Value::Bool(true))) => {
                Ok(value.to_php_string())
            }
            Ok(other) => Err(Arc::new(Error::new(
                Class::PhpTypeError,
                format!(
                    "{CLASS}::read(): Return value must be of type string|false, {} returned",
                    other.value_name()
                ),
            ))),
            // data that cannot be unmarshalled is treated as a missing session, as PHP does with data it cannot decode
            Err(error) if is_domain_exception(&error) => Ok(Some(PhpString::default())),
            Err(error) => Err(error),
        }
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        // empty data must reach the inner handler unchanged so that it destroys the session
        if data.is_empty() {
            return self.handler.write(session_id, data);
        }
        let mut values = PhpArray::new();
        values.insert("data", Value::String(PhpString::from(data)));
        let mut failed = PhpArray::new();
        let marshalled = self.marshaller.marshall(&values, &mut failed)?;
        let key = Key::from_bytes(b"data");
        // isset($failed['data']): a key, though symfony/cache's marshallers list the failed keys as values
        if failed.get(&key).is_some_and(|v| !v.is_null()) {
            return Ok(false);
        }
        let value = marshalled.get(&key).cloned().unwrap_or(Value::Null);
        let data = string_argument(&format!("{}::write", self.handler.write_class()), &value)?;
        self.handler.write(session_id, data.as_bytes())
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.handler.destroy(session_id)
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        self.handler.gc(max_lifetime)
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        Some(self)
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        Some(self)
    }
}

impl SessionUpdateTimestampHandlerInterface for MarshallingSessionHandler {
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.timestamp()?.validate_id(session_id)
    }

    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.timestamp()?.update_timestamp(session_id, data)
    }
}

impl SessionIdInterface for MarshallingSessionHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        super::create_sid()
    }
}
