//! What Symfony's session handlers call of PHP's session extension and of
//! the header functions it relies on, written from the PHP manual and
//! PHP 8.4.27's answers in the oracle (no php.ini); no PHP source code
//! (PLAN.md decision 8).
//!
//! The state is this thread's (a request runs on one thread), in
//! [`crate::php::Globals`]:
//!
//! - the session's status, id and name (`PHPSESSID`);
//! - its `session.*` ini settings, at PHP's defaults (`cache_limiter`
//!   `nocache`, `cache_expire` 180, `use_cookies` 1, `save_handler`
//!   `files`, `sid_length` 32 with 4 bits per character, the cookie
//!   parameters `0`, `/`, `''`, off, off, `''`).
//!
//! `session_create_id()` gives 32 lowercase hex characters from the OS
//! (`getrandom`), after a prefix of `[a-zA-Z0-9,-]` (any other prefix is
//! PHP's `false`). `headers_list()` replays the calls made so far through
//! a `Sapi` ([`crate::sapi`]); `header()`, `header_remove()` and
//! `setcookie()` are recorded as calls, as `Response` records its own.
//!
//! Starting a session, its save handler and its data are not here yet:
//! `NativeSessionStorage` will need them.
//!
//! Upstream: none

use php_value::{PhpArray, PhpString, Value};

use crate::{
    php::{OutputCall, with_globals, with_output},
    sapi::{Ini, Sapi, SetCookie},
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
pub struct SessionState {
    pub status: Status,
    pub id: PhpString,
    ini: Vec<(String, PhpString)>,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            status: Status::None,
            id: PhpString::default(),
            ini: DEFAULTS
                .iter()
                .map(|(k, v)| ((*k).to_owned(), PhpString::from(*v)))
                .chain([("sid_bits_per_character".to_owned(), PhpString::from("4"))])
                .collect(),
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

    /// `ini_set('session.'.$name, $value)`, for a setting that exists.
    pub fn set_ini(&mut self, name: &str, value: impl Into<PhpString>) {
        if let Some(entry) = self.ini.iter_mut().find(|(k, _)| k == name) {
            entry.1 = value.into();
        }
    }
}

fn with_session<R>(f: impl FnOnce(&mut SessionState) -> R) -> R {
    with_globals(|g| f(&mut g.session))
}

/// `ini_get('session.'.$name)`.
pub fn ini_get(name: &str) -> Option<PhpString> {
    with_session(|s| s.ini(name))
}

/// `ini_set('session.'.$name, $value)`.
pub fn ini_set(name: &str, value: impl Into<PhpString>) {
    with_session(|s| s.set_ini(name, value));
}

/// `session_status()`.
pub fn session_status() -> Status {
    with_session(|s| s.status)
}

/// `session_id()`.
pub fn session_id() -> PhpString {
    with_session(|s| s.id.clone())
}

/// `session_id($id)`: the id set, the previous one returned.
pub fn set_session_id(id: impl Into<PhpString>) -> PhpString {
    with_session(|s| std::mem::replace(&mut s.id, id.into()))
}

/// `session_name()`.
pub fn session_name() -> PhpString {
    with_session(|s| s.ini("name").unwrap_or_default())
}

/// `session_name($name)`: the name set, the previous one returned.
pub fn set_session_name(name: impl Into<PhpString>) -> PhpString {
    let name = name.into();
    with_session(|s| {
        let previous = s.ini("name").unwrap_or_default();
        s.set_ini("name", name);
        previous
    })
}

/// `session_create_id($prefix)`: `None` is PHP's `false`, for a prefix
/// other than `[a-zA-Z0-9,-]` or no randomness.
pub fn session_create_id(prefix: &[u8]) -> Option<PhpString> {
    if !prefix
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || *b == b',' || *b == b'-')
    {
        return None;
    }
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).ok()?;
    let mut id = prefix.to_vec();
    for b in bytes {
        id.extend_from_slice(format!("{b:02x}").as_bytes());
    }
    Some(PhpString::from(id))
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

/// `header($line, $replace)`.
pub fn header(line: impl Into<PhpString>, replace: bool) {
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

/// `setcookie(...)`.
pub fn setcookie(cookie: SetCookie) {
    with_output(|o| o.calls.push(OutputCall::SetCookie(cookie)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_32_hex_characters_after_the_prefix() {
        let id = session_create_id(b"pre-").unwrap_or_default();
        assert_eq!(36, id.as_bytes().len());
        assert!(id.as_bytes().starts_with(b"pre-"));
        assert!(id.as_bytes()[4..].iter().all(u8::is_ascii_hexdigit));
        assert_eq!(None, session_create_id(b"bad prefix"));
        assert!(session_create_id(b"bad,prefix").is_some());
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
}
