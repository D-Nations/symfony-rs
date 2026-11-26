//! Port of Symfony\Component\HttpFoundation\Session\Storage\Handler\NullSessionHandler.php:
//! a handler that keeps nothing; every id is valid.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Handler/NullSessionHandler.php

use php_value::PhpString;

use super::{
    HandlerResult, SessionHandlerInterface, SessionIdInterface,
    SessionUpdateTimestampHandlerInterface,
    abstract_session_handler::{AbstractSessionHandler, Backend},
};

/// `NullSessionHandler::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\NullSessionHandler";

/// `new NullSessionHandler()`.
#[derive(Debug, Default)]
pub struct NullSessionHandler {
    parent: AbstractSessionHandler,
}

impl Backend for NullSessionHandler {
    fn class(&self) -> String {
        CLASS.to_owned()
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.parent.read(self, session_id)
    }

    fn do_read(&self, _session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        Ok(Some(PhpString::default()))
    }

    fn do_write(&self, _session_id: &[u8], _data: &[u8]) -> HandlerResult<bool> {
        Ok(true)
    }

    fn do_destroy(&self, _session_id: &[u8]) -> HandlerResult<bool> {
        Ok(true)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.parent.destroy(self, session_id)
    }
}

impl SessionHandlerInterface for NullSessionHandler {
    fn class(&self) -> String {
        CLASS.to_owned()
    }

    fn write_class(&self) -> String {
        super::abstract_session_handler::CLASS.to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        Ok(self.parent.open(save_path, session_name))
    }

    fn close(&self) -> HandlerResult<bool> {
        Ok(true)
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.parent.read(self, session_id)
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.parent.write(self, session_id, data)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        Backend::destroy(self, session_id)
    }

    fn gc(&self, _max_lifetime: i64) -> HandlerResult<Option<i64>> {
        Ok(Some(0))
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        Some(self)
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        Some(self)
    }
}

impl SessionUpdateTimestampHandlerInterface for NullSessionHandler {
    /// `validateId()`: every id is valid.
    fn validate_id(&self, _session_id: &[u8]) -> HandlerResult<bool> {
        Ok(true)
    }

    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.parent.update_timestamp(self, session_id, data)
    }
}

impl SessionIdInterface for NullSessionHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        super::create_sid()
    }
}
