//! PHP's session extension, as far as Symfony's session storages and
//! handlers use it, written from the PHP manual and PHP 8.4.27's answers
//! in the oracle (no php.ini); no PHP source code (PLAN.md decision 8).
//!
//! The state is this thread's (a request runs on one thread), in
//! [`crate::php::Globals`]:
//!
//! - the session's status, id and name (`PHPSESSID`);
//! - its `session.*` ini settings, at PHP's defaults (`cache_limiter`
//!   `nocache`, `cache_expire` 180, `use_cookies` 1, `save_handler`
//!   `files`, `sid_length` 32 with 4 bits per character, the cookie
//!   parameters `0`, `/`, `''`, off, off, `''`), changed only as
//!   `ini_set()` allows (not while a session is active or once headers
//!   are sent, and not to an invalid name or an unknown serializer);
//! - `$_SESSION` ([`SessionVar`]), the save handler
//!   (`session_set_save_handler()`, or PHP's files module, [`files`]) and
//!   the data it read, which lazy writing compares with;
//! - the warnings PHP raised, without `html_errors`' escaping
//!   ([`take_warnings`]).
//!
//! `session_start()`, `session_write_close()`, `session_regenerate_id()`
//! and the other session functions are in [`lifecycle`]: the order of the
//! save handler's calls, the session cookie and the cache limiter's
//! headers are PHP's, recorded from the built-in web server.
//!
//! `session_create_id()` gives 32 lowercase hex characters from the OS
//! (`getrandom`), after a prefix of `[a-zA-Z0-9,-]`, or asks the save
//! handler (`create_sid()`) while a session is active. `headers_list()`
//! replays the calls made so far through a `Sapi` ([`crate::sapi`]);
//! `header()`, `header_remove()`, `setcookie()` and the session cookie are
//! recorded as calls, as `Response` records its own.
//!
//! Times (the cookie's `expires`, `public`'s `Expires`, garbage
//! collection) are the system's, as PHP's extension reads them.
//!
//! Upstream: none

pub mod files;
pub mod lifecycle;
pub mod serializers;

use std::sync::Arc;

use php_value::{Key, PhpArray, PhpString, Value, datetime::gmdate, url::urlencode};

pub use self::{files::SessionHandler, lifecycle::*};
use crate::{
    php::{OutputCall, with_globals, with_output},
    sapi::{Ini, Sapi, SetCookie},
    session::{Data, storage::handler::SessionHandlerInterface},
};

/// `session_status()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    /// `PHP_SESSION_DISABLED`.
    Disabled,
    /// `PHP_SESSION_NONE`.
    #[default]
    None,
    /// `PHP_SESSION_ACTIVE`.
    Active,
}

impl Status {
    /// The `PHP_SESSION_*` constant's value.
    pub fn value(self) -> i64 {
        match self {
            Self::Disabled => 0,
            Self::None => 1,
            Self::Active => 2,
        }
    }
}

/// `E_WARNING`.
pub const E_WARNING: i64 = 2;

/// A warning PHP raised, as an error handler gets it when `html_errors`
/// is off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// `E_WARNING`, ...
    pub level: i64,
    pub message: String,
    /// PHP's own text, which `html_errors` escapes; a message an error
    /// handler rewrote is passed on as it is.
    pub html: bool,
}

/// `$_SESSION`.
#[derive(Debug, Clone, Default)]
pub enum SessionVar {
    /// Not set (before a session starts).
    #[default]
    Unset,
    /// An array: keys and the references that hold their values.
    Array(Data),
    /// Any other value (`$_SESSION = null`, or what `php_serialize` read).
    Other(Value),
}

impl SessionVar {
    /// `$_SESSION = $value`.
    pub fn from_value(value: Value) -> Self {
        match value {
            Value::Array(array) => Self::Array(Data::from_array(&array)),
            other => Self::Other(other),
        }
    }

    /// The value, or `None` when it is not set.
    pub fn to_value(&self) -> Option<Value> {
        match self {
            Self::Unset => None,
            Self::Array(data) => Some(Value::Array(data.to_array())),
            Self::Other(value) => Some(value.clone()),
        }
    }

    /// `(bool) $_SESSION`.
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Unset => false,
            Self::Array(data) => !data.is_empty(),
            Self::Other(value) => value.is_truthy(),
        }
    }
}

/// The `session.*` ini settings, by name without the `session.` prefix.
const DEFAULTS: [(&str, &str); 21] = [
    ("save_path", ""),
    ("name", "PHPSESSID"),
    ("save_handler", "files"),
    ("gc_probability", "1"),
    ("gc_divisor", "100"),
    ("gc_maxlifetime", "1440"),
    ("serialize_handler", "php"),
    ("cookie_lifetime", "0"),
    ("cookie_path", "/"),
    ("cookie_domain", ""),
    ("cookie_secure", "0"),
    ("cookie_httponly", "0"),
    ("cookie_samesite", ""),
    ("use_strict_mode", "0"),
    ("use_cookies", "1"),
    ("use_only_cookies", "1"),
    ("cache_limiter", "nocache"),
    ("cache_expire", "180"),
    ("use_trans_sid", "0"),
    ("sid_length", "32"),
    ("lazy_write", "1"),
];

/// The session extension's state for this request.
#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent flags of PHP's session state, each read on its own"
)]
pub struct SessionState {
    pub status: Status,
    pub id: PhpString,
    /// An id was given (`session_id($id)`, even `''`) or made: the next
    /// `session_start()` does not read the cookie.
    id_given: bool,
    ini: Vec<(String, PhpString)>,
    /// `$_SESSION`.
    pub data: SessionVar,
    /// The user save handler; `None` is PHP's files module.
    handler: Option<Arc<dyn SessionHandlerInterface>>,
    /// What the save handler read, for lazy writing.
    original: Option<PhpString>,
    warnings: Vec<Warning>,
    /// Inside the save handler's `create_sid()`, where
    /// `session_create_id()` makes the id itself.
    in_create_sid: bool,
    /// `session_register_shutdown()` was called.
    shutdown_registered: bool,
    /// A `session_start()` of this request set `$_SESSION` up, so
    /// `session_encode()` has a session to encode.
    tracked: bool,
    /// PHP still names the user handler's class in its write warning; a
    /// session that failed to decode leaves it nameless (`handler:
    /// write`), as recorded once.
    handler_named: bool,
    /// The class `NativeSessionStorage::save()`'s error handler names in
    /// the `session_write_close()` warnings raised meanwhile.
    write_close_handler: Option<String>,
    files: files::FilesState,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            status: Status::None,
            id: PhpString::default(),
            id_given: false,
            ini: DEFAULTS
                .iter()
                .map(|(k, v)| ((*k).to_owned(), PhpString::from(*v)))
                .chain([("sid_bits_per_character".to_owned(), PhpString::from("4"))])
                .collect(),
            data: SessionVar::Unset,
            handler: None,
            original: None,
            warnings: Vec::new(),
            in_create_sid: false,
            shutdown_registered: false,
            tracked: false,
            handler_named: false,
            write_close_handler: None,
            files: files::FilesState::default(),
        }
    }
}

impl SessionState {
    /// `ini_get('session.'.$name)`; `None` for a setting that does not
    /// exist (PHP's `false`).
    pub fn ini(&self, name: &str) -> Option<PhpString> {
        self.ini
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }

    /// Sets an ini value that exists, without `ini_set()`'s checks.
    pub fn set_ini(&mut self, name: &str, value: impl Into<PhpString>) {
        if let Some(entry) = self.ini.iter_mut().find(|(k, _)| k == name) {
            entry.1 = value.into();
        }
    }
}

pub(crate) fn with_session<R>(f: impl FnOnce(&mut SessionState) -> R) -> R {
    with_globals(|g| f(&mut g.session))
}

/// Raises `function(): message` as an `E_WARNING`.
pub(crate) fn warn(function: &str, message: impl std::fmt::Display) {
    let message = if function == "Unknown" {
        format!("Unknown: {message}")
    } else {
        format!("{function}(): {message}")
    };
    raise(E_WARNING, message);
}

/// Raises `message` as it is, at `level` (`E_NOTICE` is 8).
pub(crate) fn raise(level: i64, message: impl Into<String>) {
    let mut message = message.into();
    with_session(|s| {
        let mut html = true;
        if let Some(class) = &s.write_close_handler
            && level == E_WARNING
            && message.starts_with("session_write_close():")
        {
            message = format!(
                "session_write_close(): Failed to write session data with \"{class}\" handler"
            );
            html = false;
        }
        s.warnings.push(Warning {
            level,
            message,
            html,
        });
    });
}

/// Runs `f` with `NativeSessionStorage::save()`'s error handler, which
/// names `class` in each `session_write_close()` warning.
pub fn with_write_close_handler<R>(class: String, f: impl FnOnce() -> R) -> R {
    let outer = with_session(|s| s.write_close_handler.replace(class));
    let result = f();
    with_session(|s| s.write_close_handler = outer);
    result
}

/// PHP's warning for a header sent after the output started; where it
/// started is not known here.
const HEADERS_SENT: &str =
    "Cannot modify header information - headers already sent by (output started at )";

/// PHP's warning for reading `$_SESSION` before a session set it.
pub fn raise_undefined_session() {
    raise(E_WARNING, "Undefined global variable $_SESSION");
}

/// The warnings raised since the last call, oldest first.
pub fn take_warnings() -> Vec<Warning> {
    with_session(|s| std::mem::take(&mut s.warnings))
}

/// Reads or changes the warnings raised so far (an error handler
/// rewriting the ones a call raised).
pub fn with_warnings<R>(f: impl FnOnce(&mut Vec<Warning>) -> R) -> R {
    with_session(|s| f(&mut s.warnings))
}

/// Reads or changes `$_SESSION`.
pub fn with_session_var<R>(f: impl FnOnce(&mut SessionVar) -> R) -> R {
    with_session(|s| f(&mut s.data))
}

/// `ini_get('session.'.$name)`.
pub fn ini_get(name: &str) -> Option<PhpString> {
    with_session(|s| s.ini(name))
}

/// An ini setting as PHP reads a number: its leading digits.
pub(crate) fn ini_int(name: &str) -> i64 {
    php_value::numeric::int_argument(ini_get(name).unwrap_or_default().as_bytes()).unwrap_or(0)
}

/// An ini setting as PHP reads a boolean (`on`, `yes`, `true`, or a
/// number other than 0).
pub(crate) fn ini_bool(name: &str) -> bool {
    let value = ini_get(name).unwrap_or_default();
    let lower = value.as_bytes().to_ascii_lowercase();
    matches!(lower.as_slice(), b"on" | b"yes" | b"true") || ini_int(name) != 0
}

/// `session.name`'s rule: not numeric or empty, no NUL byte and none of
/// `=,;.[` and the whitespace characters.
fn valid_name(name: &[u8]) -> bool {
    !name.is_empty()
        && !php_value::numeric::is_numeric(name)
        && !name.iter().any(|b| b"=,;.[ \t\r\n\x0b\x0c\0".contains(b))
}

fn invalid_name(function: &str, name: &[u8]) {
    warn(
        function,
        format!(
            "session.name \"{}\" must not be numeric, empty, contain null bytes or any of the following characters \"=,;.[ \\t\\r\\n\\013\\014\"",
            String::from_utf8_lossy(name)
        ),
    );
}

/// `ini_set('session.'.$name, $value)`: the old value, or `None` (PHP's
/// `false`) for a setting that does not exist or that PHP refuses to
/// change, with its warning.
pub fn ini_set(name: &str, value: impl Into<PhpString>) -> Option<PhpString> {
    let value = value.into();
    let old = ini_get(name)?;
    if session_status() == Status::Active {
        warn(
            "ini_set",
            "Session ini settings cannot be changed when a session is active",
        );
        return None;
    }
    if headers_sent() {
        warn(
            "ini_set",
            "Session ini settings cannot be changed after headers have already been sent",
        );
        return None;
    }
    match name {
        "name" if !valid_name(value.as_bytes()) => {
            invalid_name("ini_set", value.as_bytes());
            return None;
        }
        "serialize_handler" if serializers::Serializer::from_name(value.as_bytes()).is_none() => {
            warn(
                "ini_set",
                format!(
                    "Serialization handler \"{}\" cannot be found",
                    String::from_utf8_lossy(value.as_bytes())
                ),
            );
            return None;
        }
        _ => {}
    }
    with_session(|s| s.set_ini(name, value));
    Some(old)
}

/// `ini_set('session.'.$name, $value)` for a PHP value: `true` is `"1"`,
/// `false` and `null` are `""`.
pub fn ini_set_value(name: &str, value: &Value) -> Option<PhpString> {
    let text = match value {
        Value::Bool(true) => PhpString::from("1"),
        Value::Bool(false) | Value::Null => PhpString::default(),
        other => other.to_php_string().unwrap_or_default(),
    };
    ini_set(name, text)
}

/// `session_status()`.
pub fn session_status() -> Status {
    with_session(|s| s.status)
}

/// `session_id()`.
pub fn session_id() -> PhpString {
    with_session(|s| s.id.clone())
}

/// `session_id($id)`: the previous id, or `None` (PHP's `false`) while a
/// session is active or, with `use_cookies`, once headers are sent.
pub fn set_session_id(id: impl Into<PhpString>) -> Option<PhpString> {
    if session_status() == Status::Active {
        warn(
            "session_id",
            "Session ID cannot be changed when a session is active",
        );
        return None;
    }
    if ini_bool("use_cookies") && headers_sent() {
        warn(
            "session_id",
            "Session ID cannot be changed after headers have already been sent",
        );
        return None;
    }
    let id = id.into();
    Some(with_session(|s| {
        s.id_given = true;
        std::mem::replace(&mut s.id, id)
    }))
}

/// `session_name()`.
pub fn session_name() -> PhpString {
    with_session(|s| s.ini("name").unwrap_or_default())
}

/// `session_name($name)`: the previous name, or `None` (PHP's `false`)
/// while a session is active or once headers are sent. An invalid name
/// is not set (the previous one is returned, with PHP's warning).
pub fn set_session_name(name: impl Into<PhpString>) -> Option<PhpString> {
    let name = name.into();
    if session_status() == Status::Active {
        warn(
            "session_name",
            "Session name cannot be changed when a session is active",
        );
        return None;
    }
    if headers_sent() {
        warn(
            "session_name",
            "Session name cannot be changed after headers have already been sent",
        );
        return None;
    }
    let previous = session_name();
    if valid_name(name.as_bytes()) {
        with_session(|s| s.set_ini("name", name));
    } else {
        invalid_name("session_name", name.as_bytes());
    }
    Some(previous)
}

/// 32 lowercase hex characters from the OS; `None` without randomness.
pub(crate) fn random_id() -> Option<PhpString> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    let mut id = Vec::with_capacity(32);
    for b in bytes {
        id.extend_from_slice(format!("{b:02x}").as_bytes());
    }
    Some(PhpString::from(id))
}

/// A number in `0..n` from the OS.
pub(crate) fn random_below(n: i64) -> i64 {
    let mut bytes = [0_u8; 8];
    if getrandom::fill(&mut bytes).is_err() || n <= 0 {
        return 0;
    }
    i64::try_from(u64::from_le_bytes(bytes) % n.unsigned_abs()).unwrap_or(0)
}

/// `[a-zA-Z0-9,-]*`.
pub(crate) fn valid_prefix(prefix: &[u8]) -> bool {
    prefix
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b',' || *b == b'-')
}

/// `session_get_cookie_params()`.
pub fn session_get_cookie_params() -> PhpArray {
    with_session(|s| {
        let text = |name: &str| s.ini(name).unwrap_or_default();
        let flag = |name: &str| Value::Bool(Value::String(text(name)).is_truthy());
        let mut params = PhpArray::new();
        params.insert(
            "lifetime",
            Value::Int(
                php_value::numeric::int_argument(text("cookie_lifetime").as_bytes()).unwrap_or(0),
            ),
        );
        params.insert("path", Value::String(text("cookie_path")));
        params.insert("domain", Value::String(text("cookie_domain")));
        params.insert("secure", flag("cookie_secure"));
        params.insert("httponly", flag("cookie_httponly"));
        params.insert("samesite", Value::String(text("cookie_samesite")));
        params
    })
}

/// `session_set_save_handler($handler, $registerShutdown)`: `false`, with
/// PHP's warning, while a session is active or once headers are sent.
/// `session.save_handler` becomes `user`.
pub fn session_set_save_handler(
    handler: Arc<dyn SessionHandlerInterface>,
    register_shutdown: bool,
) -> bool {
    if session_status() == Status::Active {
        warn(
            "session_set_save_handler",
            "Session save handler cannot be changed when a session is active",
        );
        return false;
    }
    if headers_sent() {
        warn(
            "session_set_save_handler",
            "Session save handler cannot be changed after headers have already been sent",
        );
        return false;
    }
    with_session(|s| {
        s.handler = Some(handler);
        s.handler_named = true;
        s.set_ini("save_handler", "user");
        if register_shutdown {
            s.shutdown_registered = true;
        }
    });
    true
}

/// `session_register_shutdown()`.
pub fn session_register_shutdown() {
    with_session(|s| s.shutdown_registered = true);
}

/// `headers_sent()`.
pub fn headers_sent() -> bool {
    with_output(|o| o.headers_sent)
}

/// `headers_list()`: the calls made so far, applied in order.
pub fn headers_list() -> Vec<PhpString> {
    let calls = with_output(|o| o.calls.clone());
    let mut sapi = Sapi::new(&Ini::default(), false);
    for call in &calls {
        sapi.apply(call);
    }
    sapi.headers_list()
}

/// `header($line, $replace)`: not sent, with PHP's warning, once the
/// headers are.
pub fn header(line: impl Into<PhpString>, replace: bool) {
    if headers_sent() {
        raise(E_WARNING, HEADERS_SENT);
        return;
    }
    let line = line.into();
    with_output(|o| {
        o.calls.push(OutputCall::Header {
            line,
            replace,
            response_code: 0,
        });
    });
}

/// `header_remove($name)`.
pub fn header_remove(name: impl Into<PhpString>) {
    let name = name.into();
    with_output(|o| o.calls.push(OutputCall::HeaderRemove(name)));
}

/// `setcookie(...)`: `false`, with PHP's warning, once the headers are
/// sent.
pub fn setcookie(cookie: SetCookie) -> bool {
    if headers_sent() {
        raise(E_WARNING, HEADERS_SENT);
        return false;
    }
    with_output(|o| o.calls.push(OutputCall::SetCookie(cookie)));
    true
}

/// The system time, in seconds.
pub(crate) fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

/// `D, d M Y H:i:s GMT`.
pub(crate) fn http_date(timestamp: i64) -> String {
    let mut text = gmdate("D, d M Y H:i:s", timestamp);
    text.push_str(" GMT");
    text
}

/// The session cookie for `id`, with the `session.cookie_*` settings; an
/// earlier one for the session's name is replaced where it stands.
pub(crate) fn send_session_cookie(id: &[u8]) {
    let (name, path, domain, samesite) = with_session(|s| {
        let text = |n: &str| s.ini(n).unwrap_or_default();
        (
            text("name"),
            text("cookie_path"),
            text("cookie_domain"),
            text("cookie_samesite"),
        )
    });
    let lifetime = ini_int("cookie_lifetime");
    let encoded_name = urlencode(name.as_bytes());
    let mut line = [&b"Set-Cookie: "[..], &encoded_name, b"=", &urlencode(id)].concat();
    if lifetime > 0 {
        line.extend_from_slice(b"; expires=");
        line.extend_from_slice(http_date(now().saturating_add(lifetime)).as_bytes());
        line.extend_from_slice(format!("; Max-Age={lifetime}").as_bytes());
    }
    for (attribute, value) in [(&b"; path="[..], &path), (b"; domain=", &domain)] {
        if !value.as_bytes().is_empty() {
            line.extend_from_slice(attribute);
            line.extend_from_slice(value.as_bytes());
        }
    }
    if ini_bool("cookie_secure") {
        line.extend_from_slice(b"; secure");
    }
    if ini_bool("cookie_httponly") {
        line.extend_from_slice(b"; HttpOnly");
    }
    if !samesite.as_bytes().is_empty() {
        line.extend_from_slice(b"; SameSite=");
        line.extend_from_slice(samesite.as_bytes());
    }
    with_output(|o| {
        o.calls.push(OutputCall::SessionCookie {
            name: PhpString::from(encoded_name),
            line: PhpString::from(line),
        });
    });
}

/// `session.cache_limiter`'s headers; `false` when they would be sent
/// after the headers were.
pub(crate) fn send_cache_limiter() -> bool {
    const PAST: &str = "Expires: Thu, 19 Nov 1981 08:52:00 GMT";
    let seconds = ini_int("cache_expire").saturating_mul(60);
    let headers: Vec<String> = match ini_get("cache_limiter").unwrap_or_default().as_bytes() {
        b"nocache" => vec![
            PAST.to_owned(),
            "Cache-Control: no-store, no-cache, must-revalidate".to_owned(),
            "Pragma: no-cache".to_owned(),
        ],
        b"private" => vec![
            PAST.to_owned(),
            format!("Cache-Control: private, max-age={seconds}"),
        ],
        b"private_no_expire" => vec![format!("Cache-Control: private, max-age={seconds}")],
        b"public" => vec![
            format!("Expires: {}", http_date(now().saturating_add(seconds))),
            format!("Cache-Control: public, max-age={seconds}"),
        ],
        _ => Vec::new(),
    };
    if headers.is_empty() {
        return true;
    }
    if headers_sent() {
        return false;
    }
    for line in headers {
        header(line, true);
    }
    true
}

/// `$_COOKIE[$name]`, when it is a string.
pub(crate) fn request_cookie(name: &[u8]) -> Option<PhpString> {
    with_globals(|g| match g.cookie.get(&Key::from_bytes(name)) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_32_hex_characters_after_the_prefix() {
        let id = session_create_id(b"pre-")
            .ok()
            .flatten()
            .unwrap_or_default();
        assert_eq!(36, id.as_bytes().len());
        assert!(id.as_bytes().starts_with(b"pre-"));
        assert!(id.as_bytes()[4..].iter().all(u8::is_ascii_hexdigit));
        assert_eq!(None, session_create_id(b"bad prefix").ok().flatten());
        assert!(session_create_id(b"bad,prefix").ok().flatten().is_some());
    }

    #[test]
    fn cookie_params_are_php_defaults() {
        let params = session_get_cookie_params();
        assert_eq!(Some(&Value::Int(0)), params.get(&"lifetime".into()));
        assert_eq!(Some(&Value::string("/")), params.get(&"path".into()));
        assert_eq!(Some(&Value::Bool(false)), params.get(&"secure".into()));
        assert_eq!(PhpString::from("PHPSESSID"), session_name());
        assert_eq!(Status::None, session_status());
    }

    #[test]
    fn names_follow_php_rules() {
        assert!(valid_name(b"sid"));
        assert!(!valid_name(b"1"));
        assert!(!valid_name(b"a b"));
        assert!(!valid_name(b""));
        assert_eq!(None, ini_set("name", "a.b"));
        assert_eq!(PhpString::from("PHPSESSID"), session_name());
    }
}
