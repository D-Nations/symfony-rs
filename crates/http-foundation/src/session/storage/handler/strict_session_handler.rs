//! Port of Symfony\Component\HttpFoundation\Session\Storage\Handler\StrictSessionHandler.php:
//! a plain `\SessionHandlerInterface` given what `AbstractSessionHandler`
//! adds (prefetching, empty data destroying the session, the session
//! cookie), so PHP can use it in strict mode.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Handler/StrictSessionHandler.php

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use php_value::PhpString;

use super::{
    HandlerResult, SessionHandlerInterface, SessionIdInterface,
    SessionUpdateTimestampHandlerInterface,
    abstract_session_handler::{AbstractSessionHandler, Backend},
};
use crate::exception::{Class, Error, Result};

/// `StrictSessionHandler::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\StrictSessionHandler";

/// `new StrictSessionHandler($handler)`.
#[derive(Debug)]
pub struct StrictSessionHandler {
    handler: Arc<dyn SessionHandlerInterface>,
    parent: AbstractSessionHandler,
    /// `$this->doDestroy`.
    do_destroy: AtomicBool,
}

impl StrictSessionHandler {
    /// Fails with `\LogicException` for a handler that already implements
    /// `\SessionUpdateTimestampHandlerInterface`.
    pub fn new(handler: Arc<dyn SessionHandlerInterface>) -> Result<Self> {
        if handler.as_update_timestamp().is_some() {
            return Err(Error::new(
                Class::PhpLogic,
                format!(
                    "\"{}\" is already an instance of \"SessionUpdateTimestampHandlerInterface\", you cannot wrap it with \"{CLASS}\".",
                    handler.class()
                ),
            ));
        }
        Ok(Self {
            handler,
            parent: AbstractSessionHandler::default(),
            do_destroy: AtomicBool::new(false),
        })
    }

    /// `isWrapper()`: whether it wraps PHP's own `\SessionHandler`.
    pub fn is_wrapper(&self) -> bool {
        self.handler.is_native()
    }
}

impl Backend for StrictSessionHandler {
    fn class(&self) -> String {
        CLASS.to_owned()
    }

    fn do_read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.handler.read(session_id)
    }

    fn do_write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.handler.write(session_id, data)
    }

    fn do_destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.do_destroy.store(false, Ordering::Relaxed);
        self.handler.destroy(session_id)
    }

    /// `destroy()`: the wrapped handler destroys the session even when
    /// the parent finds it new.
    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.do_destroy.store(true, Ordering::Relaxed);
        let destroyed = self.parent.destroy(self, session_id)?;
        if self.do_destroy.load(Ordering::Relaxed) {
            return self.do_destroy(session_id);
        }
        Ok(destroyed)
    }
}

impl SessionHandlerInterface for StrictSessionHandler {
    fn class(&self) -> String {
        CLASS.to_owned()
    }

    fn write_class(&self) -> String {
        super::abstract_session_handler::CLASS.to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        self.parent.open(save_path, session_name);
        self.handler.open(save_path, session_name)
    }

    fn close(&self) -> HandlerResult<bool> {
        self.handler.close()
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

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        self.handler.gc(max_lifetime)
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        Some(self)
    }
}

impl SessionUpdateTimestampHandlerInterface for StrictSessionHandler {
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool> {
        Ok(self.parent.validate_id(self, session_id))
    }

    /// `updateTimestamp()`: a write.
    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        SessionHandlerInterface::write(self, session_id, data)
    }
}

impl SessionIdInterface for StrictSessionHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        super::create_sid()
    }
}
