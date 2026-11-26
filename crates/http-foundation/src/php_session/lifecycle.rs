//! The session functions that start, write, regenerate and end a session,
//! in the order PHP 8.4.27 calls the save handler (recorded from the
//! built-in web server, `oracle/recorders/http_foundation_native_session.php`):
//!
//! - `session_start()`: `false` without a warning once headers are sent
//!   (with `use_cookies`); `$_SESSION` emptied; the id is `session_id()`'s,
//!   else the request cookie's; `open()`; a new id when there is none, or
//!   when strict mode's `validateId()` says no; the cookie, for an id that
//!   did not come from the request; `read()` (`false` fails, closing and
//!   forgetting the id); decoding (failing destroys the session); garbage
//!   collection with `gc_probability`/`gc_divisor`; the cache limiter's
//!   headers (failing once headers are sent).
//! - `session_write_close()`: `updateTimestamp()` instead of `write()` for
//!   data that did not change and is not empty, with `lazy_write` and a
//!   handler that has it; nothing written for a `$_SESSION` that is not
//!   an array; then `close()`.
//! - `session_regenerate_id()`: the old session destroyed or written,
//!   `close()`, `open()`, a new id (checked with `validateId()` in strict
//!   mode), `read()`, and its cookie.
//!
//! A save handler "has" `create_sid()`, `validateId()` and
//! `updateTimestamp()` when its Rust type says so
//! ([`SessionHandlerInterface::as_session_id`], `as_update_timestamp`): PHP
//! calls those methods whenever an object has them.
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{Key, PhpString, Value};

use super::{
    SessionVar, Status, files, ini_bool, ini_get, ini_int, random_below, random_id,
    send_cache_limiter, send_session_cookie, serializers, session_name, session_status,
    valid_prefix, warn, with_session,
};
use crate::{
    exception::Error,
    session::{Data, storage::handler::SessionHandlerInterface},
    streamed_response::Throwable,
};

/// What a session function gives: PHP's value, or the exception a save
/// handler threw.
pub type SessionResult<T> = Result<T, Throwable>;

fn thrown(error: Error) -> Throwable {
    Arc::new(error)
}

/// The save handler in use.
#[derive(Debug, Clone)]
enum Module {
    Files,
    User(Arc<dyn SessionHandlerInterface>),
}

impl Module {
    fn current() -> Self {
        with_session(|s| s.handler.clone()).map_or(Self::Files, Self::User)
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::User(_) => "user",
        }
    }

    fn open(&self, save_path: &[u8], name: &[u8]) -> SessionResult<bool> {
        match self {
            Self::Files => Ok(files::open(save_path)),
            Self::User(h) => h.open(save_path, name),
        }
    }

    fn close(&self) -> SessionResult<bool> {
        match self {
            Self::Files => Ok(files::close()),
            Self::User(h) => h.close(),
        }
    }

    fn read(&self, function: &str, id: &[u8]) -> SessionResult<Option<PhpString>> {
        match self {
            Self::Files => Ok(files::read(function, id)),
            Self::User(h) => h.read(id),
        }
    }

    fn write(&self, function: &str, id: &[u8], data: &[u8]) -> SessionResult<bool> {
        match self {
            Self::Files => Ok(files::write(function, id, data)),
            Self::User(h) => h.write(id, data),
        }
    }

    fn has_update_timestamp(&self) -> bool {
        match self {
            Self::Files => true,
            Self::User(h) => h.as_update_timestamp().is_some(),
        }
    }

    fn update_timestamp(&self, function: &str, id: &[u8], data: &[u8]) -> SessionResult<bool> {
        match self {
            Self::Files => Ok(files::update_timestamp(function, id)),
            Self::User(h) => match h.as_update_timestamp() {
                Some(t) => t.update_timestamp(id, data),
                None => h.write(id, data),
            },
        }
    }

    /// `validateId($id)`, when the handler has it.
    fn validate(&self, id: &[u8]) -> Option<SessionResult<bool>> {
        match self {
            Self::Files => Some(Ok(files::validate(id))),
            Self::User(h) => h.as_update_timestamp().map(|t| t.validate_id(id)),
        }
    }

    fn destroy(&self, id: &[u8]) -> SessionResult<bool> {
        match self {
            Self::Files => Ok(files::destroy(id)),
            Self::User(h) => h.destroy(id),
        }
    }

    fn gc(&self, max_lifetime: i64) -> SessionResult<Option<i64>> {
        match self {
            Self::Files => Ok(files::gc(max_lifetime)),
            Self::User(h) => h.gc(max_lifetime),
        }
    }

    /// A new id: the handler's `create_sid()`, or PHP's own.
    fn create_sid(&self) -> SessionResult<Option<PhpString>> {
        let Self::User(h) = self else {
            return Ok(random_id());
        };
        let Some(creator) = h.as_session_id() else {
            return Ok(random_id());
        };
        let outer = with_session(|s| std::mem::replace(&mut s.in_create_sid, true));
        let id = creator.create_sid();
        with_session(|s| s.in_create_sid = outer);
        Ok(Some(id?))
    }
}

fn path() -> PhpString {
    ini_get("save_path").unwrap_or_default()
}

fn shown(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes)
}

/// The session ends: no longer active, its id forgotten.
fn forget() {
    with_session(|s| {
        s.status = Status::None;
        s.id = PhpString::default();
    });
}

fn serializer() -> SessionResult<serializers::Serializer> {
    serializers::Serializer::from_name(ini_get("serialize_handler").unwrap_or_default().as_bytes())
        .ok_or_else(|| {
            thrown(Error::new(
                crate::exception::Class::NotPorted,
                "an unknown session.serialize_handler",
            ))
        })
}

/// `$_SESSION`'s entries, for an array.
fn entries() -> Option<Vec<(Key, Value)>> {
    with_session(|s| match &s.data {
        SessionVar::Array(data) => {
            Some(data.entries().map(|(k, r)| (k.clone(), r.get())).collect())
        }
        _ => None,
    })
}

/// Encodes `$_SESSION`, raising the warnings as `function`'s; `None` is
/// PHP's `false`.
fn encode(function: &str, entries: &[(Key, Value)]) -> SessionResult<Option<PhpString>> {
    let encoded = serializers::encode(serializer()?, entries.iter().map(|(k, v)| (k, v.clone())))
        .map_err(thrown)?;
    for warning in encoded.warnings {
        warn(function, warning);
    }
    Ok(encoded.text)
}

/// Decodes `data` into `$_SESSION`; `false` when it cannot be decoded.
fn decode(function: &str, data: &[u8]) -> SessionResult<bool> {
    let serializer = serializer()?;
    if serializer == serializers::Serializer::Php {
        // what PHP warns of the session before decoding into it
        for (key, _) in entries().unwrap_or_default() {
            if let Key::Int(n) = key {
                warn(function, format!("Skipping numeric key {n}"));
            }
        }
    }
    match serializers::decode(serializer, data).map_err(thrown)? {
        None => Ok(false),
        Some(serializers::Decoded::Replace(value)) => {
            with_session(|s| s.data = SessionVar::from_value(value));
            Ok(true)
        }
        Some(serializers::Decoded::Entries(decoded)) => {
            with_session(|s| {
                if !matches!(s.data, SessionVar::Array(_)) {
                    s.data = SessionVar::Array(Data::default());
                }
                if let SessionVar::Array(data) = &mut s.data {
                    for (key, value) in decoded {
                        data.assign(key, value);
                    }
                }
            });
            Ok(true)
        }
    }
}

/// Decoding failed: the session destroyed, `$_SESSION` emptied.
fn decode_failed(function: &str, module: &Module, id: &[u8]) -> SessionResult<()> {
    module.destroy(id)?;
    module.close()?;
    warn(
        function,
        "Failed to decode session object. Session has been destroyed",
    );
    with_session(|s| s.data = SessionVar::Array(Data::default()));
    forget();
    Ok(())
}

/// `session_create_id($prefix)`: `None` is PHP's `false`.
pub fn session_create_id(prefix: &[u8]) -> SessionResult<Option<PhpString>> {
    if !valid_prefix(prefix) {
        warn(
            "session_create_id",
            "Prefix cannot contain special characters. Only the A-Z, a-z, 0-9, \"-\", and \",\" characters are allowed",
        );
        return Ok(None);
    }
    let with_prefix = |id: PhpString| PhpString::from([prefix, id.as_bytes()].concat());
    if with_session(|s| s.in_create_sid) || session_status() != Status::Active {
        return Ok(random_id().map(with_prefix));
    }
    let module = Module::current();
    for _ in 0..3 {
        let Some(id) = module.create_sid()? else {
            return Ok(None);
        };
        if !module.validate(id.as_bytes()).transpose()?.unwrap_or(false) {
            return Ok(Some(with_prefix(id)));
        }
    }
    warn("session_create_id", "Failed to create new ID");
    Ok(None)
}

/// A new id for the session; `None` (with PHP's warning) when the handler
/// gives none.
fn new_id(function: &str, module: &Module, check: bool) -> SessionResult<Option<PhpString>> {
    for _ in 0..3 {
        let Some(id) = module.create_sid()? else {
            break;
        };
        if !check {
            return Ok(Some(id));
        }
        if !module.validate(id.as_bytes()).transpose()?.unwrap_or(false) {
            return Ok(Some(id));
        }
    }
    warn(
        function,
        format!(
            "Failed to create session ID: {} (path: {})",
            module.name(),
            shown(path().as_bytes())
        ),
    );
    Ok(None)
}

/// `session_start()`.
pub fn session_start() -> SessionResult<bool> {
    const F: &str = "session_start";
    if session_status() == Status::Active {
        return Ok(true);
    }
    let use_cookies = ini_bool("use_cookies");
    if use_cookies && super::headers_sent() {
        return Ok(false);
    }
    with_session(|s| s.data = SessionVar::Array(Data::default()));
    let name = session_name();
    let module = Module::current();
    let mut id = super::session_id();
    let mut from_cookie = false;
    if id.as_bytes().is_empty()
        && use_cookies
        && let Some(cookie) = super::request_cookie(name.as_bytes())
        && !cookie.as_bytes().is_empty()
    {
        id = cookie;
        from_cookie = true;
    }
    if !module.open(path().as_bytes(), name.as_bytes())? {
        warn(
            F,
            format!(
                "Failed to initialize storage module: {} (path: {})",
                module.name(),
                shown(path().as_bytes())
            ),
        );
        return Ok(false);
    }
    let renew = if id.as_bytes().is_empty() {
        true
    } else if ini_bool("use_strict_mode") {
        match module.validate(id.as_bytes()) {
            Some(valid) => !valid?,
            None => false,
        }
    } else {
        false
    };
    if renew {
        let Some(new) = new_id(F, &module, false)? else {
            return Ok(false);
        };
        id = new;
        from_cookie = false;
    }
    with_session(|s| {
        s.id = id.clone();
        s.status = Status::Active;
    });
    if use_cookies && !from_cookie {
        send_session_cookie(id.as_bytes());
    }
    let data = match module.read(F, id.as_bytes()) {
        Ok(Some(data)) => data,
        Ok(None) => {
            module.close()?;
            warn(
                F,
                format!(
                    "Failed to read session data: {} (path: {})",
                    module.name(),
                    shown(path().as_bytes())
                ),
            );
            forget();
            return Ok(false);
        }
        Err(error) => {
            with_session(|s| s.status = Status::None);
            return Err(error);
        }
    };
    if !decode(F, data.as_bytes())? {
        decode_failed(F, &module, id.as_bytes())?;
        return Ok(false);
    }
    with_session(|s| s.original = Some(data));
    let probability = ini_int("gc_probability");
    let divisor = ini_int("gc_divisor");
    if probability > 0
        && divisor > 0
        && (probability >= divisor || random_below(divisor) < probability)
    {
        module.gc(ini_int("gc_maxlifetime"))?;
    }
    if !send_cache_limiter() {
        module.close()?;
        forget();
        return Ok(false);
    }
    Ok(true)
}

/// Writes and closes the active session.
fn flush(function: &str) -> SessionResult<()> {
    let module = Module::current();
    let id = super::session_id();
    if let Some(entries) = entries() {
        let text = encode(function, &entries)?.unwrap_or_default();
        let original = with_session(|s| s.original.clone());
        let unchanged = ini_bool("lazy_write")
            && !text.as_bytes().is_empty()
            && original.as_ref() == Some(&text)
            && module.has_update_timestamp();
        let written = if unchanged {
            module.update_timestamp(function, id.as_bytes(), text.as_bytes())
        } else {
            module.write(function, id.as_bytes(), text.as_bytes())
        };
        let written = match written {
            Ok(written) => written,
            Err(error) => {
                with_session(|s| s.status = Status::None);
                return Err(error);
            }
        };
        if !written {
            let message = match module {
                Module::Files => format!(
                    "Failed to write session data (files). Please verify that the current setting of session.save_path is correct ({})",
                    shown(path().as_bytes())
                ),
                Module::User(_) => format!(
                    "Failed to write session data using user defined save handler. (session.save_path: {}, handler: write)",
                    shown(path().as_bytes())
                ),
            };
            warn(function, message);
        }
    }
    with_session(|s| s.status = Status::None);
    module.close()?;
    Ok(())
}

/// `session_write_close()`.
pub fn session_write_close() -> SessionResult<bool> {
    if session_status() != Status::Active {
        return Ok(false);
    }
    flush("session_write_close")?;
    Ok(true)
}

/// The end of the request: an active session is written and closed, as
/// PHP does after the script.
pub fn request_shutdown() -> SessionResult<()> {
    if session_status() == Status::Active {
        flush("Unknown")?;
    }
    Ok(())
}

/// `session_abort()`: closed without writing.
pub fn session_abort() -> SessionResult<bool> {
    if session_status() != Status::Active {
        return Ok(false);
    }
    with_session(|s| s.status = Status::None);
    Module::current().close()?;
    Ok(true)
}

/// `session_reset()`: `$_SESSION` read again from the save handler.
pub fn session_reset() -> SessionResult<bool> {
    if session_status() != Status::Active {
        return Ok(false);
    }
    let module = Module::current();
    let id = super::session_id();
    module.open(path().as_bytes(), session_name().as_bytes())?;
    if let Some(data) = module.read("session_reset", id.as_bytes())? {
        with_session(|s| s.data = SessionVar::Array(Data::default()));
        if !decode("session_reset", data.as_bytes())? {
            decode_failed("session_reset", &module, id.as_bytes())?;
            return Ok(false);
        }
        with_session(|s| s.original = Some(data));
    }
    Ok(true)
}

/// `session_destroy()`: `$_SESSION` is left as it is.
pub fn session_destroy() -> SessionResult<bool> {
    const F: &str = "session_destroy";
    if session_status() != Status::Active {
        warn(F, "Trying to destroy uninitialized session");
        return Ok(false);
    }
    let module = Module::current();
    let destroyed = module.destroy(super::session_id().as_bytes())?;
    if !destroyed {
        warn(F, "Session object destruction failed");
    }
    module.close()?;
    forget();
    Ok(destroyed)
}

/// `session_regenerate_id($deleteOldSession)`.
pub fn session_regenerate_id(delete_old_session: bool) -> SessionResult<bool> {
    const F: &str = "session_regenerate_id";
    if session_status() != Status::Active {
        warn(
            F,
            "Session ID cannot be regenerated when there is no active session",
        );
        return Ok(false);
    }
    if super::headers_sent() {
        warn(
            F,
            "Session ID cannot be regenerated after headers have already been sent",
        );
        return Ok(false);
    }
    let module = Module::current();
    let old = super::session_id();
    if delete_old_session {
        if !module.destroy(old.as_bytes())? {
            warn(
                F,
                format!(
                    "Session object destruction failed. ID: {} (path: {})",
                    module.name(),
                    shown(path().as_bytes())
                ),
            );
            return Ok(false);
        }
    } else if let Some(entries) = entries() {
        let text = encode(F, &entries)?.unwrap_or_default();
        module.write(F, old.as_bytes(), text.as_bytes())?;
    }
    module.close()?;
    if !module.open(path().as_bytes(), session_name().as_bytes())? {
        warn(
            F,
            format!(
                "Failed to open session: {} (path: {})",
                module.name(),
                shown(path().as_bytes())
            ),
        );
        forget();
        return Ok(false);
    }
    let Some(id) = new_id(F, &module, ini_bool("use_strict_mode"))? else {
        forget();
        return Ok(false);
    };
    with_session(|s| s.id = id.clone());
    let Some(data) = module.read(F, id.as_bytes())? else {
        warn(
            F,
            format!(
                "Failed to create(read) session ID: {} (path: {})",
                module.name(),
                shown(path().as_bytes())
            ),
        );
        forget();
        return Ok(false);
    };
    with_session(|s| s.original = Some(data));
    if ini_bool("use_cookies") {
        send_session_cookie(id.as_bytes());
    }
    Ok(true)
}

/// `session_gc()`: the number of sessions removed; `None` is PHP's
/// `false`.
pub fn session_gc() -> SessionResult<Option<i64>> {
    if session_status() != Status::Active {
        warn(
            "session_gc",
            "Session cannot be garbage collected when there is no active session",
        );
        return Ok(None);
    }
    Module::current().gc(ini_int("gc_maxlifetime"))
}

/// `session_encode()`: `None` is PHP's `false`.
pub fn session_encode() -> SessionResult<Option<PhpString>> {
    if session_status() != Status::Active {
        warn("session_encode", "Cannot encode non-existent session");
        return Ok(None);
    }
    match entries() {
        Some(entries) => encode("session_encode", &entries),
        None => Ok(None),
    }
}

/// `session_decode($data)`: entries merged into `$_SESSION` (`php`), or
/// `$_SESSION` replaced (`php_serialize`); data that cannot be decoded
/// destroys the session.
pub fn session_decode(data: &[u8]) -> SessionResult<bool> {
    const F: &str = "session_decode";
    if session_status() != Status::Active {
        warn(
            F,
            "Session data cannot be decoded when there is no active session",
        );
        return Ok(false);
    }
    if decode(F, data)? {
        return Ok(true);
    }
    decode_failed(F, &Module::current(), super::session_id().as_bytes())?;
    Ok(false)
}
