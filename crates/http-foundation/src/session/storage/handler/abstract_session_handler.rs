//! Port of Symfony\Component\HttpFoundation\Session\Storage\Handler\AbstractSessionHandler.php:
//! what Symfony's handlers share for PHP's session extension.
//!
//! - **Prefetch:** `validateId()` reads the session, and the next `read()`
//!   of the same id is answered from that read, or the read's failure
//!   thrown. A failed read keeps the id valid.
//! - **Empty data:** writing it destroys the session.
//! - **Cookie:** destroying the session read on this request also takes
//!   its cookie out (`SessionUtils`) and sends an expired one when no
//!   cookie was sent or the request had one.
//! - **Cache headers:** `open()` sends `Cache-Control` when
//!   `session.cache_limiter` is empty.
//!
//! PHP's abstract class becomes this state and the parent's methods,
//! which a concrete handler calls with itself as the [`Backend`]: its
//! `doRead()`, `doWrite()` and `doDestroy()`, and the `destroy()` that
//! `write()` and `updateTimestamp()` call (a subclass may override it).
//! `igbinary_serialize()` does not exist here, so the empty data is `''`
//! alone. `__serialize()` and `__unserialize()`, which refuse, have no
//! counterpart.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Session/Storage/Handler/AbstractSessionHandler.php

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::{
    Key, PhpString, Value,
    filter::{FILTER_VALIDATE_BOOLEAN, filter_var},
};

use super::HandlerResult;
use crate::{
    exception::{Class, Error},
    php::with_globals,
    php_session::{header, headers_sent, ini_get, session_get_cookie_params, setcookie},
    sapi::SetCookie,
    session::session_utils::pop_session_cookie,
    streamed_response::Throwable,
};

/// `AbstractSessionHandler::class`.
pub const CLASS: &str =
    "Symfony\\Component\\HttpFoundation\\Session\\Storage\\Handler\\AbstractSessionHandler";

/// What a subclass gives `AbstractSessionHandler`: the abstract methods,
/// its class, and its `destroy()`.
pub trait Backend {
    /// `static::class`.
    fn class(&self) -> String;
    /// `doRead($sessionId)`.
    fn do_read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>>;
    /// `doWrite($sessionId, $data)`.
    fn do_write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool>;
    /// `doDestroy($sessionId)`.
    fn do_destroy(&self, session_id: &[u8]) -> HandlerResult<bool>;
    /// `$this->destroy($sessionId)`.
    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool>;
}

#[derive(Debug, Default)]
struct State {
    session_name: Option<PhpString>,
    /// `$this->prefetchId`, `$this->prefetchData`, `$this->prefetchError`.
    prefetch: Option<(PhpString, Option<PhpString>, Option<Throwable>)>,
    new_session_id: Option<PhpString>,
    read_session_id: Option<PhpString>,
}

/// The state `AbstractSessionHandler` keeps.
#[derive(Debug, Default)]
pub struct AbstractSessionHandler {
    state: Mutex<State>,
}

fn is_empty_data(data: &[u8]) -> bool {
    data.is_empty()
}

impl AbstractSessionHandler {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `open($savePath, $sessionName)`.
    pub fn open(&self, _save_path: &[u8], session_name: &[u8]) -> bool {
        self.state().session_name = Some(PhpString::from(session_name));
        let limiter = ini_get("cache_limiter").unwrap_or_default();
        if !headers_sent()
            && !Value::String(limiter.clone()).is_truthy()
            && limiter.as_bytes() != b"0"
        {
            let expire = php_value::numeric::int_argument(
                ini_get("cache_expire").unwrap_or_default().as_bytes(),
            )
            .unwrap_or(0);
            header(
                format!(
                    "Cache-Control: max-age={}, private, must-revalidate",
                    expire.saturating_mul(60)
                ),
                true,
            );
        }
        true
    }

    /// `validateId($sessionId)`: the session read now, kept for `read()`.
    pub fn validate_id(&self, backend: &dyn Backend, session_id: &[u8]) -> bool {
        let (data, error) = match self.read(backend, session_id) {
            Ok(data) => (data, None),
            Err(error) => (None, Some(error)),
        };
        let valid = data.as_ref().is_none_or(|d| !d.as_bytes().is_empty());
        self.state().prefetch = Some((PhpString::from(session_id), data, error));
        valid
    }

    /// `read($sessionId)`.
    pub fn read(
        &self,
        backend: &dyn Backend,
        session_id: &[u8],
    ) -> HandlerResult<Option<PhpString>> {
        let prefetch = {
            let mut state = self.state();
            state.read_session_id = Some(PhpString::from(session_id));
            state.prefetch.take()
        };
        if let Some((prefetch_id, prefetch_data, prefetch_error)) = prefetch {
            let empty = prefetch_data
                .as_ref()
                .is_some_and(|d| d.as_bytes().is_empty());
            if prefetch_id.as_bytes() == session_id || empty {
                if let Some(error) = prefetch_error {
                    return Err(error);
                }
                self.state().new_session_id = empty.then(|| PhpString::from(session_id));
                return Ok(prefetch_data);
            }
        }
        let data = backend.do_read(session_id)?;
        let empty = data.as_ref().is_some_and(|d| d.as_bytes().is_empty());
        self.state().new_session_id = empty.then(|| PhpString::from(session_id));
        Ok(data)
    }

    /// `updateTimestamp($sessionId, $data)`: empty data destroys the
    /// session.
    pub fn update_timestamp(
        &self,
        backend: &dyn Backend,
        session_id: &[u8],
        data: &[u8],
    ) -> HandlerResult<bool> {
        if is_empty_data(data) {
            return backend.destroy(session_id);
        }
        Ok(true)
    }

    /// `write($sessionId, $data)`: empty data destroys the session.
    pub fn write(
        &self,
        backend: &dyn Backend,
        session_id: &[u8],
        data: &[u8],
    ) -> HandlerResult<bool> {
        if is_empty_data(data) {
            return backend.destroy(session_id);
        }
        self.state().new_session_id = None;
        backend.do_write(session_id, data)
    }

    /// `destroy($sessionId)`: the session's cookie too, for the session
    /// read on this request; a session that was new is not destroyed
    /// again.
    pub fn destroy(&self, backend: &dyn Backend, session_id: &[u8]) -> HandlerResult<bool> {
        let (read_here, session_name) = {
            let state = self.state();
            (
                state
                    .read_session_id
                    .as_ref()
                    .is_some_and(|r| r.as_bytes() == session_id),
                state.session_name.clone(),
            )
        };
        let use_cookies = filter_var(
            &Value::String(ini_get("use_cookies").unwrap_or_default()),
            FILTER_VALIDATE_BOOLEAN,
            &Value::Null,
        )
        .is_ok_and(|v| v.is_truthy());
        if read_here && !headers_sent() && use_cookies {
            let Some(session_name) = session_name else {
                return Err(Arc::new(Error::new(
                    Class::PhpLogic,
                    format!(
                        "Session name cannot be empty, did you forget to call \"parent::open()\" in \"{}\"?.",
                        backend.class()
                    ),
                )));
            };
            let cookie = pop_session_cookie(session_name.as_bytes(), session_id);
            let in_request = with_globals(|g| {
                g.cookie
                    .get(&Key::from_bytes(session_name.as_bytes()))
                    .is_some_and(|v| !v.is_null())
            });
            if cookie.is_none() || in_request {
                let params = session_get_cookie_params();
                let text = |name: &str| match params.get(&name.into()) {
                    Some(Value::String(s)) => s.clone(),
                    _ => PhpString::default(),
                };
                let flag = |name: &str| params.get(&name.into()).is_some_and(Value::is_truthy);
                setcookie(SetCookie {
                    name: session_name,
                    value: PhpString::default(),
                    expires: 0,
                    path: text("path"),
                    domain: text("domain"),
                    secure: flag("secure"),
                    httponly: flag("httponly"),
                });
            }
        }
        let was_new = self
            .state()
            .new_session_id
            .as_ref()
            .is_some_and(|n| n.as_bytes() == session_id);
        if was_new {
            return Ok(true);
        }
        backend.do_destroy(session_id)
    }
}
