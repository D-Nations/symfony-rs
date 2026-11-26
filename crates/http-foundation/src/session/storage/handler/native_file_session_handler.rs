//! Port of Symfony\Component\HttpFoundation\Session\Storage\Handler\NativeFileSessionHandler.php:
//! PHP's files handler (`\SessionHandler`) on a save path of its own,
//! whose directory is created when missing.
//!
//! The save path is `[N;[MODE;]]DIR`; more than two `;` is refused.
//! Making the handler sets `session.save_path` and `session.save_handler`
//! (`files`), as PHP's constructor does.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Handler/NativeFileSessionHandler.php

use php_value::{PhpString, Value, path::to_path};

use super::{HandlerResult, SessionHandlerInterface, SessionIdInterface};
use crate::{
    exception::{Class, Error, Result},
    php_session::{SessionHandler, ini_get, ini_set},
};

/// `NativeFileSessionHandler::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\NativeFileSessionHandler";

/// `new NativeFileSessionHandler($savePath)`.
#[derive(Debug, Default)]
pub struct NativeFileSessionHandler {
    handler: SessionHandler,
}

fn is_dir(path: &[u8]) -> bool {
    to_path(path).is_some_and(|p| p.is_dir())
}

impl NativeFileSessionHandler {
    /// `$savePath` (`session.save_path` by default).
    pub fn new(save_path: Option<&[u8]>) -> Result<Self> {
        let save_path = save_path.map_or_else(
            || ini_get("save_path").unwrap_or_default().as_bytes().to_vec(),
            <[u8]>::to_vec,
        );
        let mut base_dir = save_path.clone();
        let count = save_path.split(|b| *b == b';').count() - 1;
        if count > 0 {
            if count > 2 {
                return Err(Error::new(
                    Class::PhpInvalidArgument,
                    format!(
                        "Invalid argument $savePath '{}'.",
                        String::from_utf8_lossy(&save_path)
                    ),
                ));
            }
            // characters after last ';' are the path
            let last = save_path.iter().rposition(|b| *b == b';').unwrap_or(0);
            base_dir = save_path[last..]
                .iter()
                .skip_while(|b| **b == b';')
                .copied()
                .collect();
        }
        if Value::String(PhpString::from(base_dir.clone())).is_truthy()
            && !is_dir(&base_dir)
            && to_path(&base_dir).is_none_or(|p| std::fs::create_dir_all(p).is_err())
            && !is_dir(&base_dir)
        {
            return Err(Error::new(
                Class::PhpRuntime,
                format!(
                    "Session Storage was not able to create directory \"{}\".",
                    String::from_utf8_lossy(&base_dir)
                ),
            ));
        }
        if ini_get("save_path").is_none_or(|p| p.as_bytes() != save_path.as_slice()) {
            ini_set("save_path", save_path);
        }
        if ini_get("save_handler").is_none_or(|h| h.as_bytes() != b"files") {
            ini_set("save_handler", "files");
        }
        Ok(Self::default())
    }
}

impl SessionHandlerInterface for NativeFileSessionHandler {
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
        self.handler.read(session_id)
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.handler.write(session_id, data)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.handler.destroy(session_id)
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        self.handler.gc(max_lifetime)
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        Some(self)
    }

    fn is_native(&self) -> bool {
        true
    }
}

impl SessionIdInterface for NativeFileSessionHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        self.handler.create_sid()
    }
}
