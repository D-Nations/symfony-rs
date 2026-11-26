//! `Symfony\Component\HttpFoundation\Session\Storage\Handler`, and PHP's
//! session handler interfaces they implement (`\SessionHandlerInterface`,
//! `\SessionUpdateTimestampHandlerInterface`, `\SessionIdInterface`),
//! written from the PHP manual.
//!
//! A handler's methods may throw anything, so they return a
//! [`Throwable`]. `read()`'s `string|false` is `Option`, `gc()`'s
//! `int|false` too. The `instanceof` checks Symfony makes become methods:
//! [`SessionHandlerInterface::as_update_timestamp`] and
//! [`SessionHandlerInterface::is_native`] (`\SessionHandler`, PHP's own
//! handler).
//!
//! Upstream: none

use std::fmt;

use php_value::{PhpString, Value};

use crate::streamed_response::Throwable;

pub mod abstract_session_handler;
pub mod identity_marshaller;
pub mod marshaller_interface;
pub mod marshalling_session_handler;
pub mod migrating_session_handler;
pub mod null_session_handler;
pub mod strict_session_handler;

/// What a handler method returns, or what it threw.
pub type HandlerResult<T> = Result<T, Throwable>;

/// `\SessionHandlerInterface`.
pub trait SessionHandlerInterface: fmt::Debug + Send + Sync {
    /// `get_debug_type($handler)`.
    fn class(&self) -> String;

    /// `open($savePath, $sessionName)`.
    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool>;

    /// `close()`.
    fn close(&self) -> HandlerResult<bool>;

    /// `read($sessionId)`: `None` is `false`.
    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>>;

    /// `write($sessionId, $data)`.
    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool>;

    /// `destroy($sessionId)`.
    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool>;

    /// `gc($maxlifetime)`: `None` is `false`.
    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>>;

    /// The handler as a `\SessionUpdateTimestampHandlerInterface`, if it
    /// is one.
    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        None
    }

    /// The class that declares `write()`, which a `TypeError` for its
    /// arguments names (`AbstractSessionHandler` for its subclasses that
    /// keep its `write()`).
    fn write_class(&self) -> String {
        self.class()
    }

    /// The handler as a `\SessionIdInterface`, if it is one: PHP then
    /// asks it for new ids (`create_sid()`).
    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        None
    }

    /// `$handler instanceof \SessionHandler`.
    fn is_native(&self) -> bool {
        false
    }
}

/// `\SessionUpdateTimestampHandlerInterface`.
pub trait SessionUpdateTimestampHandlerInterface {
    /// `validateId($sessionId)`.
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool>;

    /// `updateTimestamp($sessionId, $data)`.
    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool>;
}

/// `\SessionIdInterface`.
pub trait SessionIdInterface {
    /// `create_sid()`.
    fn create_sid(&self) -> HandlerResult<PhpString>;
}

/// `session_create_id() ?: throw new \RuntimeException(...)`, which every
/// handler's `create_sid()` is.
pub(crate) fn create_sid() -> HandlerResult<PhpString> {
    match crate::php_session::session_create_id(b"")? {
        Some(id) if Value::String(id.clone()).is_truthy() => Ok(id),
        _ => Err(std::sync::Arc::new(crate::exception::Error::new(
            crate::exception::Class::PhpRuntime,
            "Unable to create a session ID.",
        )) as Throwable),
    }
}
