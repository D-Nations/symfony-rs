//! Session/Storage/Handler/AbstractSessionHandlerTest.php at
//! symfony/http-foundation v8.1.11: `testSession`, one case per fixture
//! (`Fixtures/*.php`), each compared with its `.expected` file.
//!
//! PHPUnit runs each fixture in PHP's built-in server with the cookie
//! `sid=123abc`; here each runs on a thread of its own (a fresh request),
//! as `common.inc` sets it up: the same ini settings and headers, the
//! `TestSessionHandler` that echoes each call, and the shutdown function
//! that writes the session and prints `headers_list()`. A fixture's last
//! `ob_start()` callback changes only what is output after it, as PHP's
//! does; warnings are shown as `display_errors` shows them (`html_errors`
//! off), at the fixture's line. The comparison is upstream's, for PHP
//! before 8.6 (`write` where 8.6 calls `updateTimestamp`).
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Handler/AbstractSessionHandlerTest.php
#![allow(non_snake_case)]

use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use php_value::{PhpArray, PhpString, Value};
use symfony_http_foundation::{
    exception::{Class, Error},
    php::with_globals,
    php_session::{
        self, SessionVar, header, header_remove, headers_list, ini_set, session_id,
        session_set_save_handler, session_start, session_write_close, take_warnings,
    },
    sapi::SetCookie,
    session::{
        Data,
        flash::{flash_bag::FlashBag, flash_bag_interface::FlashBagInterface},
        storage::{
            handler::{
                HandlerResult, SessionHandlerInterface, SessionIdInterface,
                SessionUpdateTimestampHandlerInterface,
                abstract_session_handler::{AbstractSessionHandler, Backend},
                migrating_session_handler::MigratingSessionHandler,
                strict_session_handler::StrictSessionHandler,
            },
            native_session_storage::{NativeSessionStorage, SaveHandler},
            session_storage_interface::SessionStorageInterface,
        },
    },
    streamed_response::Throwable,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

thread_local! {
    /// What the request output so far.
    static OUT: RefCell<String> = const { RefCell::new(String::new()) };
}

fn echo(text: &str) {
    OUT.with(|o| o.borrow_mut().push_str(text));
}

fn out_len() -> usize {
    OUT.with(|o| o.borrow().len())
}

/// The fixture's `ob_start($callback)`: from here on, output goes
/// through `callback` at the end of the request.
type Callback = Box<dyn Fn(&str) -> String>;

/// `common.inc`'s `TestSessionHandler`.
#[derive(Debug)]
struct TestSessionHandler {
    parent: AbstractSessionHandler,
    data: String,
    session_id: Mutex<Option<String>>,
}

impl TestSessionHandler {
    fn new(data: &str, session_id: Option<&str>) -> Arc<Self> {
        Arc::new(Self {
            parent: AbstractSessionHandler::default(),
            data: data.to_owned(),
            session_id: Mutex::new(session_id.map(str::to_owned)),
        })
    }

    fn id(&self) -> std::sync::MutexGuard<'_, Option<String>> {
        self.session_id
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

impl Backend for TestSessionHandler {
    fn class(&self) -> String {
        "TestSessionHandler".to_owned()
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        echo("read\n");
        self.parent.read(self, session_id)
    }

    fn do_read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        let mut id = self.id();
        if id
            .as_deref()
            .is_some_and(|known| known.as_bytes() != session_id)
        {
            echo("doRead: invalid sessionId\n");
            return Ok(Some(PhpString::default()));
        }
        echo(&format!("doRead: {}\n", self.data));
        *id = Some(text(session_id));
        Ok(Some(PhpString::from(self.data.as_str())))
    }

    fn do_write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        echo(&format!("doWrite: {}\n", text(data)));
        *self.id() = Some(text(session_id));
        Ok(true)
    }

    fn do_destroy(&self, _session_id: &[u8]) -> HandlerResult<bool> {
        echo("doDestroy\n");
        *self.id() = Some(String::new());
        Ok(true)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        echo("destroy\n");
        self.parent.destroy(self, session_id)
    }
}

impl SessionHandlerInterface for TestSessionHandler {
    fn class(&self) -> String {
        "TestSessionHandler".to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        echo("open\n");
        Ok(self.parent.open(save_path, session_name))
    }

    fn close(&self) -> HandlerResult<bool> {
        echo("close\n");
        Ok(true)
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        Backend::read(self, session_id)
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        echo("write\n");
        self.parent.write(self, session_id, data)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        Backend::destroy(self, session_id)
    }

    fn gc(&self, _max_lifetime: i64) -> HandlerResult<Option<i64>> {
        echo("gc\n");
        Ok(Some(1))
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        Some(self)
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        Some(self)
    }
}

impl SessionUpdateTimestampHandlerInterface for TestSessionHandler {
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool> {
        echo("validateId\n");
        Ok(self.parent.validate_id(self, session_id))
    }

    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        echo("updateTimestamp\n");
        self.parent.update_timestamp(self, session_id, data)
    }
}

impl SessionIdInterface for TestSessionHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        match php_session::session_create_id(b"")? {
            Some(id) => Ok(id),
            None => Err(Arc::new(Error::new(
                Class::PhpRuntime,
                "Unable to create a session ID.",
            ))),
        }
    }
}

/// The fixtures' `UnreachableSessionHandler` (read gives `false`) and
/// `ThrowingSessionHandler` (read throws), which echo each call.
#[derive(Debug)]
struct FailingHandler {
    throws: bool,
}

impl SessionHandlerInterface for FailingHandler {
    fn class(&self) -> String {
        "FailingSessionHandler".to_owned()
    }

    fn open(&self, _save_path: &[u8], _session_name: &[u8]) -> HandlerResult<bool> {
        echo("open\n");
        Ok(true)
    }

    fn close(&self) -> HandlerResult<bool> {
        echo("close\n");
        Ok(true)
    }

    fn read(&self, _session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        echo("read\n");
        if self.throws {
            return Err(Arc::new(Error::new(
                Class::PhpRuntime,
                "storage unreachable",
            )));
        }
        Ok(None)
    }

    fn write(&self, _session_id: &[u8], _data: &[u8]) -> HandlerResult<bool> {
        echo("write\n");
        Ok(true)
    }

    fn destroy(&self, _session_id: &[u8]) -> HandlerResult<bool> {
        echo("destroy\n");
        Ok(true)
    }

    fn gc(&self, _max_lifetime: i64) -> HandlerResult<Option<i64>> {
        echo("gc\n");
        Ok(Some(0))
    }
}

/// The directory `common.inc` gives as `session.save_path` (`__DIR__`).
const DIR: &str = "/fixtures";

/// `common.inc`, before the fixture.
fn common() {
    with_globals(|g| {
        g.cookie.insert("sid", Value::string("123abc"));
    });
    for (name, value) in [
        ("gc_probability", "0"),
        ("serialize_handler", "php"),
        ("cookie_lifetime", "0"),
        ("cookie_domain", ""),
        ("cookie_secure", ""),
        ("cookie_httponly", ""),
        ("use_cookies", "1"),
        ("cache_expire", "180"),
        ("cookie_path", "/"),
        ("cookie_domain", ""),
        ("cookie_secure", "1"),
        ("cookie_httponly", "1"),
        ("cookie_samesite", "Lax"),
        ("use_strict_mode", "1"),
        ("lazy_write", "1"),
        ("name", "sid"),
        ("save_path", DIR),
        ("cache_limiter", ""),
    ] {
        ini_set(name, value);
    }
    header_remove("X-Powered-By");
    header("Content-Type: text/plain; charset=utf-8", true);
}

/// `display_errors` with `html_errors` off.
fn show_warnings(file: &str, line: u32) {
    for warning in take_warnings() {
        echo(&format!(
            "\nWarning: {} in {DIR}/{file} on line {line}\n",
            warning.message
        ));
    }
}

/// `print_r(headers_list())`.
fn print_headers() {
    echo("Array\n(\n");
    for (i, h) in headers_list().iter().enumerate() {
        echo(&format!("    [{i}] => {}\n", h.to_string_lossy()));
    }
    echo(")\n");
}

fn replace_session_id(out: &str) -> String {
    let id = session_id().to_string_lossy().into_owned();
    if id.is_empty() {
        out.to_owned()
    } else {
        out.replace(&id, "random_session_id")
    }
}

/// `preg_replace('~_sf2_meta.*$~m', '', ...)`.
fn without_meta(out: &str) -> String {
    out.split('\n')
        .map(|line| line.find("_sf2_meta").map_or(line, |i| &line[..i]))
        .collect::<Vec<_>>()
        .join("\n")
}

fn set_session(value: &PhpArray) {
    php_session::with_session_var(|s| *s = SessionVar::Array(Data::from_array(value)));
}

fn unset_session(key: &str) {
    php_session::with_session_var(|s| {
        if let SessionVar::Array(data) = s {
            data.remove(key);
        }
    });
}

fn native(
    options: &PhpArray,
    handler: Arc<TestSessionHandler>,
) -> Result<NativeSessionStorage, TestError> {
    let storage = NativeSessionStorage::new(options, None, None)?;
    storage.set_save_handler(Some(SaveHandler::Handler(handler)))?;
    Ok(storage)
}

fn uncaught(error: &Throwable) {
    let (class, message) = match error.downcast_ref::<Error>() {
        Some(e) => (
            e.class().name().to_owned(),
            e.message().to_string_lossy().into_owned(),
        ),
        None => ("Throwable".to_owned(), error.to_string()),
    };
    echo(&format!("{class}: {message}\n"));
}

/// What the fixture's `ob_start($callback)` buffered: from where, and
/// through what.
type Buffered = Option<(usize, Callback)>;

/// `ob_start($callback)` from here on.
fn from_here(callback: Callback) -> (usize, Callback) {
    (out_len(), callback)
}

/// The fixtures that use `NativeSessionStorage`.
fn storage_fixture(fixture: &str) -> Result<(usize, Callback), TestError> {
    let callback;
    match fixture {
        "invalid_regenerate" => {
            let storage = native(
                &PhpArray::new(),
                TestSessionHandler::new("", Some("abc123")),
            )?;
            let flash = Arc::new(FlashBag::default());
            storage.register_bag(flash.clone())?;
            storage.start()?;
            flash.add("foo", Value::string("bar"))?;
            echo(if php_session::with_session_var(|s| s.is_truthy()) {
                "$_SESSION is not empty"
            } else {
                "$_SESSION is empty"
            });
            echo("\n");
            callback = from_here(Box::new(|out| without_meta(&replace_session_id(out))));
        }
        "storage" => {
            let storage = native(&PhpArray::new(), TestSessionHandler::new("", None))?;
            let flash = Arc::new(FlashBag::default());
            storage.register_bag(flash.clone())?;
            storage.start()?;
            flash.add("foo", Value::string("bar"))?;
            echo("Array\n(\n");
            for (key, v) in flash.get("foo", PhpArray::new())?.iter() {
                echo(&format!(
                    "    [{}] => {}\n",
                    String::from_utf8_lossy(&key.bytes()),
                    v.to_php_string().unwrap_or_default().to_string_lossy()
                ));
            }
            echo(")\n");
            let empty = |s: &SessionVar| !s.is_truthy();
            echo(if php_session::with_session_var(|s| empty(s)) {
                "$_SESSION is empty"
            } else {
                "$_SESSION is not empty"
            });
            echo("\n");
            storage.save()?;
            echo(if php_session::with_session_var(|s| empty(s)) {
                "$_SESSION is empty"
            } else {
                "$_SESSION is not empty"
            });
            callback = from_here(Box::new(replace_session_id));
        }
        "with_samesite" | "with_samesite_and_migration" => {
            let mut options = PhpArray::new();
            options.insert("cookie_samesite", Value::string("lax"));
            let storage = native(&options, TestSessionHandler::new("", None))?;
            storage.start()?;
            let mut session = PhpArray::new();
            session.insert("foo", Value::string("bar"));
            set_session(&session);
            if fixture == "with_samesite" {
                callback = from_here(Box::new(replace_session_id));
            } else {
                storage.regenerate(true, None)?;
                callback = from_here(Box::new(|out| without_meta(&replace_session_id(out))));
            }
        }
        other => return Err(format!("no fixture {other}").into()),
    }
    Ok(callback)
}

/// The fixtures that call PHP's session functions.
fn session_fixture(fixture: &str) -> Result<Buffered, TestError> {
    let mut callback = None;
    match fixture {
        "empty_destroys" => {
            session_set_save_handler(TestSessionHandler::new("abc|i:123;", None), false);
            session_start()?;
            unset_session("abc");
        }
        "migrating_empty_destroys" | "migrating_new_empty" => {
            let data = if fixture == "migrating_empty_destroys" {
                "abc|i:123;"
            } else {
                ini_set("name", "new_sid");
                ini_set("lazy_write", "0");
                ""
            };
            let handler = MigratingSessionHandler::new(
                TestSessionHandler::new(data, None),
                TestSessionHandler::new(data, None),
            )?;
            session_set_save_handler(Arc::new(handler), false);
            session_start()?;
            if fixture == "migrating_empty_destroys" {
                unset_session("abc");
            }
        }
        "read_only" => {
            session_set_save_handler(TestSessionHandler::new("abc|i:123;", None), false);
            session_start()?;
            let value = php_session::with_session_var(|s| match s {
                SessionVar::Array(d) => d.get("abc"),
                _ => None,
            });
            echo(
                &value
                    .and_then(|v| v.to_php_string())
                    .unwrap_or_default()
                    .to_string_lossy(),
            );
        }
        "regenerate" => {
            session_set_save_handler(TestSessionHandler::new("abc|i:123;", None), false);
            session_start()?;
            php_session::session_regenerate_id(true)?;
            callback = Some(from_here(Box::new(replace_session_id)));
        }
        "strict_read_exception" => {
            let handler = StrictSessionHandler::new(Arc::new(FailingHandler { throws: true }))?;
            session_set_save_handler(Arc::new(handler), false);
            if let Err(error) = session_start() {
                uncaught(&error);
            } else {
                echo("not reached\n");
            }
        }
        "strict_read_failure" => {
            callback = Some(from_here(Box::new(|out| out.replace(DIR, "Fixtures"))));
            let handler = StrictSessionHandler::new(Arc::new(FailingHandler { throws: false }))?;
            session_set_save_handler(Arc::new(handler), false);
            let started = session_start()?;
            show_warnings("strict_read_failure.php", 65);
            echo(&format!("bool({started})\n"));
        }
        "with_cookie" => {
            session_set_save_handler(TestSessionHandler::new("abc|i:123;", None), false);
            session_start()?;
            php_session::setcookie(SetCookie {
                name: PhpString::from("abc"),
                value: PhpString::from("def"),
                ..SetCookie::default()
            });
        }
        "with_cookie_and_session" => {
            php_session::setcookie(SetCookie {
                name: PhpString::from("abc"),
                value: PhpString::from("def"),
                ..SetCookie::default()
            });
            session_set_save_handler(TestSessionHandler::new("abc|i:123;", None), false);
            session_start()?;
            session_write_close()?;
            session_start()?;
            php_session::with_session_var(|s| {
                if let SessionVar::Array(d) = s {
                    d.assign("abc".into(), Value::Int(234));
                }
            });
            unset_session("abc");
        }
        other => return Err(format!("no fixture {other}").into()),
    }
    Ok(callback)
}

/// Runs a fixture as a request: `common.inc`, the fixture, then the
/// shutdown function; the output, with the fixture's callback applied to
/// what it buffered.
fn run(fixture: &str) -> Result<String, TestError> {
    common();
    let callback = match fixture {
        "invalid_regenerate" | "storage" | "with_samesite" | "with_samesite_and_migration" => {
            Some(storage_fixture(fixture)?)
        }
        _ => session_fixture(fixture)?,
    };
    // the shutdown function common.inc registers
    echo("\n");
    session_write_close()?;
    print_headers();
    echo("shutdown\n");

    let out = OUT.with(|o| o.borrow().clone());
    Ok(match callback {
        Some((start, f)) => format!("{}{}", &out[..start], f(&out[start..])),
        None => out,
    })
}

fn fixtures() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../upstream/symfony/http-foundation/Tests/Session/Storage/Handler/Fixtures");
    dir.is_dir().then_some(dir)
}

#[test]
fn testSession() -> Result<(), TestError> {
    let Some(dir) = fixtures() else {
        eprintln!("upstream/ is not fetched: AbstractSessionHandlerTest::testSession skipped");
        return Ok(());
    };
    let mut names: Vec<String> = std::fs::read_dir(&dir)?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".php").map(str::to_owned)
        })
        .collect();
    names.sort();
    assert_eq!(13, names.len());
    let mut failures = Vec::new();
    for name in names {
        let fixture = name.clone();
        let result = std::thread::spawn(move || run(&fixture).map_err(|e| e.to_string()))
            .join()
            .map_err(|_| format!("{name} panicked"))?;
        let result = result.map_err(|e| format!("{name}: {e}"))?;
        // upstream's comparison: `expires` dates with spaces, and PHP
        // before 8.6 calling write() where 8.6 calls updateTimestamp()
        let result = result.replace(
            "write\ndestroy\nclose\n",
            "updateTimestamp\ndestroy\nclose\n",
        );
        let expected = std::fs::read_to_string(dir.join(format!("{name}.expected")))?;
        if result != expected {
            failures.push(format!(
                "== {name}\n-- expected\n{expected}\n-- got\n{result}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}
