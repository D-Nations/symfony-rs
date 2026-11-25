//! PHP built-ins that File, `UploadedFile` and `FileBag` call, written from
//! the PHP manual and recorded answers (`tests/data/files.json`,
//! `oracle/recorders/http_foundation_files.php`); no PHP source code
//! (PLAN.md decision 8):
//!
//! - the `UPLOAD_ERR_*` constants;
//! - `is_uploaded_file()` and `move_uploaded_file()` on a process-wide
//!   list of uploaded files, which the code that writes an upload's
//!   temporary file fills ([`register_uploaded_file`]) and empties at the
//!   end of the request ([`forget_uploaded_files`]); PHP keeps that list
//!   per request. A moved file leaves the list, as in PHP;
//! - the warnings as a web server SAPI gives them (`html_errors` on,
//!   PHP's default outside the CLI): `&`, `<`, `>` and `"` escaped, which
//!   leaves nothing for upstream's `strip_tags()` to remove;
//! - `intval($s, 16)` and `intval($s, 8)` of the texts `parseFilesize()`
//!   gives them (a leading `0x` or `0`), saturating;
//! - `pathinfo($name, PATHINFO_EXTENSION)` and `SplFileInfo::getBasename()`;
//! - PHP's coercion of the values `FileBag` passes to `new
//!   UploadedFile()` (no `strict_types`), and its `TypeError` texts without
//!   the "called in" suffix;
//! - what `Request` reads from the PHP process: the superglobals,
//!   `php://input`, `request_parse_body()`'s answer and two ini settings
//!   ([`Globals`]), kept per thread as PHP keeps them per request; and the
//!   streams `getContent(true)` returns ([`Stream`]).
//!
//! Upstream: none

use std::{
    cell::RefCell,
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

use php_value::{PhpArray, PhpString, Value, numeric::int_argument};

use crate::exception::{Class, Error, Result};

pub const UPLOAD_ERR_OK: i64 = 0;
pub const UPLOAD_ERR_INI_SIZE: i64 = 1;
pub const UPLOAD_ERR_FORM_SIZE: i64 = 2;
pub const UPLOAD_ERR_PARTIAL: i64 = 3;
pub const UPLOAD_ERR_NO_FILE: i64 = 4;
pub const UPLOAD_ERR_NO_TMP_DIR: i64 = 6;
pub const UPLOAD_ERR_CANT_WRITE: i64 = 7;
pub const UPLOAD_ERR_EXTENSION: i64 = 8;

/// `DIRECTORY_SEPARATOR`.
pub const DIRECTORY_SEPARATOR: u8 = if cfg!(windows) { b'\\' } else { b'/' };

static UPLOADED: Mutex<BTreeSet<Vec<u8>>> = Mutex::new(BTreeSet::new());

fn uploaded() -> std::sync::MutexGuard<'static, BTreeSet<Vec<u8>>> {
    UPLOADED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Marks `path` as a file PHP received in this request's body.
pub fn register_uploaded_file(path: impl Into<Vec<u8>>) {
    uploaded().insert(path.into());
}

/// Ends the request: no file counts as uploaded any more.
pub fn forget_uploaded_files() {
    uploaded().clear();
}

/// `is_uploaded_file($filename)`.
pub fn is_uploaded_file(filename: &[u8]) -> bool {
    uploaded().contains(filename)
}

/// A PHP warning's text as a web SAPI shows it to an error handler.
pub fn html_warning(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// `move_uploaded_file($from, $to)`: `false` with no warning for a file
/// that was not uploaded; otherwise a rename, with PHP's warning when it
/// fails (recorded from `php -S`).
pub fn move_uploaded_file(from: &[u8], to: &[u8]) -> (bool, Option<String>) {
    if !is_uploaded_file(from) {
        return (false, None);
    }
    let renamed = symfony_filesystem::php::rename(from, to);
    if renamed.value {
        uploaded().remove(from);
        return (true, None);
    }
    let warning = format!(
        "move_uploaded_file(): Unable to move \"{}\" to \"{}\"",
        String::from_utf8_lossy(from),
        String::from_utf8_lossy(to)
    );
    (false, Some(html_warning(&warning)))
}

/// `intval($s, 16)` of a text starting with `0x`, or `intval($s, 8)` of
/// one starting with `0`: the digits after the prefix, up to the first
/// other character, saturating at `PHP_INT_MAX`.
pub fn intval_prefixed(s: &[u8], base: u32) -> i64 {
    let digits = if base == 16 {
        s.strip_prefix(b"0x").unwrap_or(s)
    } else {
        s
    };
    let mut value: i64 = 0;
    for &b in digits {
        let Some(digit) = char::from(b).to_digit(base) else {
            break;
        };
        match value
            .checked_mul(i64::from(base))
            .and_then(|v| v.checked_add(i64::from(digit)))
        {
            Some(v) => value = v,
            None => return i64::MAX,
        }
    }
    value
}

/// `pathinfo($name, PATHINFO_EXTENSION)` of a name without `/`: what
/// follows its last `.`, or `""`.
pub use php_value::path::pathinfo_extension;

/// `basename($path)` (no suffix), `/` being the separator.
pub use php_value::path::basename;

/// The `TypeError` PHP throws for argument `n` of `new UploadedFile()`.
fn type_error(n: usize, name: &str, expected: &str, given: &Value) -> Error {
    Error::new(
        Class::PhpTypeError,
        format!(
            "Symfony\\Component\\HttpFoundation\\File\\UploadedFile::__construct(): Argument #{n} (${name}) must be of type {expected}, {} given",
            given.debug_type()
        ),
    )
}

/// A value passed to a `string` parameter.
pub fn string_argument(n: usize, name: &str, value: &Value) -> Result<PhpString> {
    match value {
        Value::Null | Value::Array(_) => Err(type_error(n, name, "string", value)),
        Value::Object(object) => object
            .to_php_string()
            .ok_or_else(|| type_error(n, name, "string", value)),
        scalar => scalar
            .to_php_string()
            .ok_or_else(|| type_error(n, name, "string", value)),
    }
}

/// A value passed to a `?string` parameter.
pub fn nullable_string_argument(n: usize, name: &str, value: &Value) -> Result<Option<PhpString>> {
    match value {
        Value::Null => Ok(None),
        Value::Array(_) => Err(type_error(n, name, "?string", value)),
        Value::Object(object) => object
            .to_php_string()
            .map(Some)
            .ok_or_else(|| type_error(n, name, "?string", value)),
        scalar => Ok(scalar.to_php_string()),
    }
}

/// A value passed to a `?int` parameter: a float or numeric string is
/// truncated if it fits (PHP 8.1+ deprecates a fraction).
pub fn nullable_int_argument(n: usize, name: &str, value: &Value) -> Result<Option<i64>> {
    const LIMIT: f64 = 9_223_372_036_854_775_808.0; // 2^63
    match value {
        Value::Null => Ok(None),
        Value::Bool(b) => Ok(Some(i64::from(*b))),
        Value::Int(i) => Ok(Some(*i)),
        Value::Float(f) if f.is_finite() && (-LIMIT..LIMIT).contains(f) => {
            Ok(Some(f.trunc() as i64))
        }
        Value::String(s) => int_argument(s.as_bytes())
            .map(Some)
            .ok_or_else(|| type_error(n, name, "?int", value)),
        _ => Err(type_error(n, name, "?int", value)),
    }
}

/// What a PHP request sees of the process: the superglobals `Request`
/// reads and writes, `php://input`, and the two ini settings
/// `overrideGlobals()` reads. The HTTP adapter fills it for each request
/// ([`set_globals`]); a test does as PHPUnit does with `$_SERVER`.
#[derive(Debug, Clone)]
pub struct Globals {
    pub get: PhpArray,
    pub post: PhpArray,
    pub cookie: PhpArray,
    pub files: PhpArray,
    pub server: PhpArray,
    /// `$_REQUEST`, which `overrideGlobals()` writes.
    pub request: PhpArray,
    /// The body, as `php://input` reads it.
    pub input: Arc<[u8]>,
    /// What `request_parse_body()` returns for this request (`[$post,
    /// $files]`), or `None` where it throws `\RequestParseBodyException`.
    /// `createFromGlobals()` calls it for PUT, DELETE, PATCH and QUERY.
    pub parsed_body: Option<(PhpArray, PhpArray)>,
    /// `ini_get('request_order')`.
    pub request_order: PhpString,
    /// `ini_get('variables_order')`.
    pub variables_order: PhpString,
}

impl Default for Globals {
    /// PHP 8.4.27's in the oracle: empty superglobals and body,
    /// `request_order` empty and `variables_order` `EGPCS` (no php.ini).
    fn default() -> Self {
        Self {
            get: PhpArray::new(),
            post: PhpArray::new(),
            cookie: PhpArray::new(),
            files: PhpArray::new(),
            server: PhpArray::new(),
            request: PhpArray::new(),
            input: Arc::from(&b""[..]),
            parsed_body: None,
            request_order: PhpString::default(),
            variables_order: PhpString::from("EGPCS"),
        }
    }
}

thread_local! {
    static GLOBALS: RefCell<Globals> = RefCell::new(Globals::default());
}

/// Starts a request on this thread: these are its superglobals.
pub fn set_globals(globals: Globals) {
    GLOBALS.with(|g| *g.borrow_mut() = globals);
}

/// Ends the request on this thread, returning what it left in the
/// superglobals.
pub fn take_globals() -> Globals {
    GLOBALS.with(|g| std::mem::take(&mut *g.borrow_mut()))
}

/// Reads or changes this thread's superglobals.
pub fn with_globals<R>(f: impl FnOnce(&mut Globals) -> R) -> R {
    GLOBALS.with(|g| f(&mut g.borrow_mut()))
}

/// A PHP stream on bytes (`php://memory`, `php://temp`, `php://input`):
/// a position and an end-of-file flag. Clones are the same stream, as a
/// PHP resource passed around is.
#[derive(Debug, Clone, Default)]
pub struct Stream(Arc<Mutex<StreamState>>);

#[derive(Debug, Default)]
struct StreamState {
    bytes: Vec<u8>,
    position: usize,
    eof: bool,
}

impl Stream {
    /// A stream on `bytes`, at its start (`fopen('php://memory', 'r+')`,
    /// `fwrite()`, `rewind()`).
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self(Arc::new(Mutex::new(StreamState {
            bytes: bytes.into(),
            position: 0,
            eof: false,
        })))
    }

    fn state(&self) -> std::sync::MutexGuard<'_, StreamState> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `rewind()`.
    pub fn rewind(&self) {
        let mut state = self.state();
        state.position = 0;
        state.eof = false;
    }

    /// `fwrite()` at the position.
    pub fn write(&self, bytes: &[u8]) {
        let mut state = self.state();
        let at = state.position;
        let end = at + bytes.len();
        if state.bytes.len() < end {
            state.bytes.resize(end, 0);
        }
        state.bytes[at..end].copy_from_slice(bytes);
        state.position = end;
    }

    /// `fread($stream, $length)`: at most `length` bytes; reading at the
    /// end sets the end-of-file flag.
    pub fn read(&self, length: usize) -> Vec<u8> {
        let mut state = self.state();
        let at = state.position.min(state.bytes.len());
        let end = at.saturating_add(length).min(state.bytes.len());
        let out = state.bytes[at..end].to_vec();
        state.position = end;
        if out.len() < length {
            state.eof = true;
        }
        out
    }

    /// `stream_get_contents()`: the rest, from the position.
    pub fn contents(&self) -> Vec<u8> {
        let mut state = self.state();
        let at = state.position.min(state.bytes.len());
        let out = state.bytes[at..].to_vec();
        state.position = state.bytes.len();
        state.eof = true;
        out
    }

    /// `feof()`.
    pub fn eof(&self) -> bool {
        self.state().eof
    }

    /// Whether two handles are the same stream (`===` on resources).
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_read_as_php_does() {
        let stream = Stream::default();
        assert_eq!(stream.read(1), b"");
        assert!(stream.eof());
        let stream = Stream::new(*b"abc");
        assert_eq!(stream.read(2), b"ab");
        assert!(!stream.eof());
        assert_eq!(stream.contents(), b"c");
        stream.rewind();
        assert_eq!(stream.contents(), b"abc");
    }

    #[test]
    fn reads_prefixed_integers() {
        assert_eq!(intval_prefixed(b"0x10", 16), 16);
        assert_eq!(intval_prefixed(b"0x", 16), 0);
        assert_eq!(intval_prefixed(b"0xg", 16), 0);
        assert_eq!(intval_prefixed(b"0xffffffffffffffffffff", 16), i64::MAX);
        assert_eq!(intval_prefixed(b"010", 8), 8);
        assert_eq!(intval_prefixed(b"08", 8), 0);
    }

    #[test]
    fn finds_extensions() {
        assert_eq!(pathinfo_extension(b"a.tar.gz"), b"gz");
        assert_eq!(pathinfo_extension(b"a"), b"");
        assert_eq!(pathinfo_extension(b".htaccess"), b"htaccess");
        assert_eq!(pathinfo_extension(b"a."), b"");
    }

    #[test]
    fn escapes_warnings() {
        assert_eq!(html_warning("<a>&'\"x"), "&lt;a&gt;&amp;'&quot;x");
    }
}
