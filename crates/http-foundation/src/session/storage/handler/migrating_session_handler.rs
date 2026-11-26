//! Port of Symfony\Component\HttpFoundation\Session\Storage\Handler\MigratingSessionHandler.php:
//! moves sessions from one handler to another. Sessions are read from the
//! current handler, and written to both, until the write-only handler
//! takes over.
//!
//! Each handler that does not implement
//! `\SessionUpdateTimestampHandlerInterface` is wrapped in a
//! `StrictSessionHandler`. The write-only handler's answers are ignored.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Handler/MigratingSessionHandler.php

use std::sync::Arc;

use php_value::PhpString;

use super::{
    HandlerResult, SessionHandlerInterface, SessionIdInterface,
    SessionUpdateTimestampHandlerInterface, strict_session_handler::StrictSessionHandler,
};
use crate::{
    exception::{Class, Error, Result},
    streamed_response::Throwable,
};

/// `MigratingSessionHandler::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\MigratingSessionHandler";

/// `new MigratingSessionHandler($currentHandler, $writeOnlyHandler)`.
#[derive(Debug)]
pub struct MigratingSessionHandler {
    current_handler: Arc<dyn SessionHandlerInterface>,
    write_only_handler: Arc<dyn SessionHandlerInterface>,
}

fn with_timestamps(
    handler: Arc<dyn SessionHandlerInterface>,
) -> Result<Arc<dyn SessionHandlerInterface>> {
    if handler.as_update_timestamp().is_some() {
        return Ok(handler);
    }
    Ok(Arc::new(StrictSessionHandler::new(handler)?))
}

fn timestamp(
    handler: &dyn SessionHandlerInterface,
) -> HandlerResult<&dyn SessionUpdateTimestampHandlerInterface> {
    handler.as_update_timestamp().ok_or_else(|| {
        Arc::new(Error::new(
            Class::NotPorted,
            "the handler lost its interface",
        )) as Throwable
    })
}

impl MigratingSessionHandler {
    pub fn new(
        current_handler: Arc<dyn SessionHandlerInterface>,
        write_only_handler: Arc<dyn SessionHandlerInterface>,
    ) -> Result<Self> {
        Ok(Self {
            current_handler: with_timestamps(current_handler)?,
            write_only_handler: with_timestamps(write_only_handler)?,
        })
    }
}

impl SessionHandlerInterface for MigratingSessionHandler {
    fn class(&self) -> String {
        CLASS.to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        let result = self.current_handler.open(save_path, session_name)?;
        self.write_only_handler.open(save_path, session_name)?;
        Ok(result)
    }

    fn close(&self) -> HandlerResult<bool> {
        let result = self.current_handler.close()?;
        self.write_only_handler.close()?;
        Ok(result)
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        // No reading from new handler until switch-over
        self.current_handler.read(session_id)
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        let result = self.current_handler.write(session_id, data)?;
        self.write_only_handler.write(session_id, data)?;
        Ok(result)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        let result = self.current_handler.destroy(session_id)?;
        self.write_only_handler.destroy(session_id)?;
        Ok(result)
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        let result = self.current_handler.gc(max_lifetime)?;
        self.write_only_handler.gc(max_lifetime)?;
        Ok(result)
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        Some(self)
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        Some(self)
    }
}

impl SessionUpdateTimestampHandlerInterface for MigratingSessionHandler {
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool> {
        // No reading from new handler until switch-over
        timestamp(self.current_handler.as_ref())?.validate_id(session_id)
    }

    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        let result =
            timestamp(self.current_handler.as_ref())?.update_timestamp(session_id, data)?;
        timestamp(self.write_only_handler.as_ref())?.update_timestamp(session_id, data)?;
        Ok(result)
    }
}

impl SessionIdInterface for MigratingSessionHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        super::create_sid()
    }
}
