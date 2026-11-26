//! PHP built-ins that File, `UploadedFile` and `FileBag` call, written from
//! the PHP manual and recorded answers (`tests/data/files.json`,
//! `oracle/recorders/http_foundation_files.php`); no PHP source code
//! (PLAN.md decision 8):
//!
//! - the `UPLOAD_ERR_*` constants;
//! - `is_uploaded_file()` and `move_uploaded_file()` on a per-thread
//!   list of uploaded files (a request runs on one thread), which the code that writes an upload's
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
//!   streams `getContent(true)` returns ([`Stream`]);
//! - what `Response` sends: its `header()`, `header_remove()`, `echo`,
//!   output-buffer and finish-request calls, recorded in order with the
//!   SAPI facts it asks about (`PHP_SAPI`, `headers_sent()`, which
//!   functions exist, `ob_get_status()`) ([`Output`]).
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

thread_local! {
    /// The uploaded files of the request this thread runs: PHP keeps the
    /// list per request, so one request never sees another's files.
    static UPLOADED: RefCell<BTreeSet<Vec<u8>>> = const { RefCell::new(BTreeSet::new()) };
}

fn with_uploaded<R>(f: impl FnOnce(&mut BTreeSet<Vec<u8>>) -> R) -> R {
    UPLOADED.with(|u| f(&mut u.borrow_mut()))
}

/// Marks `path` as a file PHP received in this request's body.
pub fn register_uploaded_file(path: impl Into<Vec<u8>>) {
    with_uploaded(|u| u.insert(path.into()));
}

/// Ends the request: no file counts as uploaded any more.
pub fn forget_uploaded_files() {
    with_uploaded(BTreeSet::clear);
}

/// `is_uploaded_file($filename)`.
pub fn is_uploaded_file(filename: &[u8]) -> bool {
    with_uploaded(|u| u.contains(filename))
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
        with_uploaded(|u| u.remove(from));
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
#[expect(
    clippy::cast_possible_truncation,
    reason = "PHP's (int) on a float in range"
)]
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
    /// What `Response` sends, and the SAPI it sends it to.
    pub output: Output,
}

/// `ob_get_status(true)`'s entry for one output buffer, as far as
/// `Response::closeOutputBuffers()` reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OutputBuffer {
    /// `flags` (`PHP_OUTPUT_HANDLER_*`), when the entry has it.
    pub flags: Option<i64>,
    /// `del`, when the entry has it (PHP before 5.4 gave it).
    pub del: Option<bool>,
}

/// `PHP_OUTPUT_HANDLER_CLEANABLE`.
pub const PHP_OUTPUT_HANDLER_CLEANABLE: i64 = 16;
/// `PHP_OUTPUT_HANDLER_FLUSHABLE`.
pub const PHP_OUTPUT_HANDLER_FLUSHABLE: i64 = 32;
/// `PHP_OUTPUT_HANDLER_REMOVABLE`.
pub const PHP_OUTPUT_HANDLER_REMOVABLE: i64 = 64;

/// The function the SAPI gives to end the request early, as
/// `Response::send()` looks for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FinishRequest {
    /// Neither exists (the CLI, Apache's module).
    #[default]
    None,
    /// `fastcgi_finish_request()` (PHP-FPM).
    FastCgi,
    /// `litespeed_finish_request()` (`LiteSpeed`).
    LiteSpeed,
}

/// A call `Response` made to send itself, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputCall {
    /// `header($line, $replace, $responseCode)`.
    Header {
        line: PhpString,
        replace: bool,
        response_code: i64,
    },
    /// `header_remove($name)`.
    HeaderRemove(PhpString),
    /// `headers_send($statusCode)` (`FrankenPHP`: sends a 1xx response).
    HeadersSend(i64),
    /// `echo $content`.
    Echo(PhpString),
    /// `fastcgi_finish_request()`.
    FastcgiFinishRequest,
    /// `litespeed_finish_request()`.
    LitespeedFinishRequest,
    /// `ob_end_flush()`.
    ObEndFlush,
    /// `ob_end_clean()`.
    ObEndClean,
    /// `flush()`.
    Flush,
    /// `@ob_flush()`: the innermost buffer's content passed to the next
    /// level; without a buffer, nothing (the notice is silenced).
    ObFlush,
}

/// The SAPI as `Response` sees it, and the calls it made. Nothing here
/// applies a call as PHP's SAPI would (status lines, `Location`'s 302,
/// replacing a header); the HTTP adapter does that from [`Output::calls`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// `PHP_SAPI`.
    pub sapi: PhpString,
    /// What `headers_sent()` answers.
    pub headers_sent: bool,
    /// `function_exists('headers_send')`.
    pub headers_send_exists: bool,
    pub finish_request: FinishRequest,
    /// `ob_get_status(true)`, innermost buffer last; `ob_end_*()` removes
    /// the last.
    pub buffers: Vec<OutputBuffer>,
    /// `ini_get('default_mimetype')`, which `Response::prepare()` empties
    /// for a response without a body.
    pub default_mimetype: PhpString,
    /// `connection_aborted()`: the HTTP adapter sets it once the client is
    /// gone.
    pub connection_aborted: bool,
    pub calls: Vec<OutputCall>,
}

impl Default for Output {
    /// The CLI's, before anything is sent; `default_mimetype` is PHP's
    /// default.
    fn default() -> Self {
        Self {
            sapi: PhpString::from("cli"),
            headers_sent: false,
            headers_send_exists: false,
            finish_request: FinishRequest::None,
            buffers: Vec::new(),
            default_mimetype: PhpString::from("text/html"),
            connection_aborted: false,
            calls: Vec::new(),
        }
    }
}

/// Reads or changes this thread's output.
pub fn with_output<R>(f: impl FnOnce(&mut Output) -> R) -> R {
    with_globals(|g| f(&mut g.output))
}

/// What the SAPI does at `flush()`: the HTTP adapter sends what was
/// output so far.
pub type FlushHook = Box<dyn FnMut()>;

thread_local! {
    static FLUSH: RefCell<Option<FlushHook>> = const { RefCell::new(None) };
}

/// Sets what `flush()` does on this thread for the request it runs.
pub fn set_flush_hook(hook: Option<FlushHook>) {
    FLUSH.with(|f| *f.borrow_mut() = hook);
}

/// `flush()`: the call is recorded, then the SAPI's hook runs (no output
/// is borrowed while it runs).
pub fn flush() {
    with_output(|o| o.calls.push(OutputCall::Flush));
    let hook = FLUSH.with(|f| f.borrow_mut().take());
    if let Some(mut hook) = hook {
        hook();
        FLUSH.with(|f| {
            let mut slot = f.borrow_mut();
            // The hook may have replaced itself.
            if slot.is_none() {
                *slot = Some(hook);
            }
        });
    }
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
            output: Output::default(),
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

    /// `fseek($stream, $offset)` from the start; past the end is allowed,
    /// as on a file.
    pub fn seek(&self, offset: usize) {
        let mut state = self.state();
        state.position = offset;
        state.eof = false;
    }

    /// `fstat($stream)['size']`.
    pub fn size(&self) -> usize {
        self.state().bytes.len()
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

/// A PHP reference (`&$value`): a value several holders see and change
/// together, as a session's bags share the storage's data. Clones are the
/// same reference.
#[derive(Debug, Clone, Default)]
pub struct Reference(Arc<Mutex<Value>>);

impl Reference {
    pub fn new(value: Value) -> Self {
        Self(Arc::new(Mutex::new(value)))
    }

    /// The value now.
    pub fn get(&self) -> Value {
        self.lock().clone()
    }

    /// `$reference = $value`.
    pub fn set(&self, value: Value) {
        *self.lock() = value;
    }

    /// The value, to read or change in place.
    pub fn with<R>(&self, f: impl FnOnce(&mut Value) -> R) -> R {
        f(&mut self.lock())
    }

    /// Whether two references are the same (`$a = &$b`).
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Value> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// `$array[$key][] = $value` on the element `key` of `array`: a missing,
/// null or `false` element becomes a list; a string or another scalar
/// throws PHP's `\Error`.
pub fn push_onto(array: &mut PhpArray, key: &php_value::Key, value: Value) -> Result<()> {
    if array.get(key).is_none() {
        array.insert(key.clone(), Value::Array(PhpArray::new()));
    }
    let Some(element) = array.get_mut(key) else {
        return Ok(());
    };
    if matches!(element, Value::Null | Value::Bool(false)) {
        *element = Value::Array(PhpArray::new());
    }
    match element {
        Value::Array(list) => list.push(value).map(|_| ()).map_err(|_| {
            Error::new(
                Class::PhpError,
                "Cannot add element to the array as the next element is already occupied",
            )
        }),
        Value::String(_) => Err(Error::new(
            Class::PhpError,
            "[] operator not supported for strings",
        )),
        Value::Object(object) => Err(Error::new(
            Class::PhpError,
            format!("Cannot use object of type {} as array", object.class()),
        )),
        _ => Err(Error::new(
            Class::PhpError,
            "Cannot use a scalar value as an array",
        )),
    }
}

/// A value returned from a method declared `: array`; anything else is
/// PHP's `\TypeError`.
pub fn array_return(method: &str, value: Value) -> Result<PhpArray> {
    match value {
        Value::Array(array) => Ok(array),
        other => Err(Error::new(
            Class::PhpTypeError,
            format!(
                "{method}(): Return value must be of type array, {} returned",
                other.value_name()
            ),
        )),
    }
}

/// A value returned from a method declared `: int`, in coercive mode: a
/// bool, a float in range (truncated, which PHP 8.1+ deprecates) or a
/// numeric string converts.
pub fn int_return(method: &str, value: &Value) -> Result<i64> {
    let converted = match value {
        Value::Int(i) => Some(*i),
        Value::Bool(b) => Some(i64::from(*b)),
        Value::Float(f) if f.is_finite() => float_to_int(f.trunc()),
        Value::String(s) => int_argument(s.as_bytes()),
        _ => None,
    };
    converted.ok_or_else(|| {
        Error::new(
            Class::PhpTypeError,
            format!(
                "{method}(): Return value must be of type int, {} returned",
                value.value_name()
            ),
        )
    })
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a truncated float in range, as PHP converts it"
)]
fn float_to_int(f: f64) -> Option<i64> {
    const LIMIT: f64 = 9_223_372_036_854_775_808.0; // 2^63
    (-LIMIT..LIMIT).contains(&f).then_some(f as i64)
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
