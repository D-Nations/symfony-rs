//! PHP's session extension, `NativeSessionStorage` and
//! `PhpBridgeSessionStorage` against PHP 8.4.27 running the pinned
//! symfony/http-foundation (v8.1.11) in its built-in web server, recorded
//! by `oracle/recorders/http_foundation_native_session.php`
//! (`scripts/record.sh http-foundation-native-session`): 1,500 generated
//! scenarios.
//!
//! Each scenario is a request: a cookie, `session.*` ini settings, a save
//! handler (the server's `RecorderHandler`, an `AbstractSessionHandler`
//! subclass over a store that logs each call; bare handlers with or
//! without the timestamp and id methods; `StrictSessionHandler`,
//! `MigratingSessionHandler`, `NativeFileSessionHandler`,
//! `NullSessionHandler`, or PHP's files module), the calls that answer
//! `false`, and operations on PHP's session functions or a storage. Then
//! `session_write_close()` and `headers_list()`.
//!
//! Compared, after the recorder's normalization (generated ids, the save
//! path, dates, where output started): every handler call and answer,
//! every warning (as `html_errors` shows it) in the order raised, each
//! operation's result or exception, the headers, the session's status, id
//! and `$_SESSION`, the store and the session files.
//!
//! Upstream: none

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Write as _,
    path::PathBuf,
    sync::{Arc, Mutex, PoisonError},
};

use php_value::{Key, PhpArray, PhpString, Value, path::from_path};
use serde_json::{Value as Json, json};
use symfony_http_foundation::{
    exception::Error,
    php::{OutputCall, html_warning, with_globals, with_output},
    php_session::{self, SessionVar, Status},
    sapi::SetCookie,
    session::{
        Data,
        attribute::attribute_bag::AttributeBag,
        flash::flash_bag::FlashBag,
        storage::{
            handler::{
                HandlerResult, SessionHandlerInterface, SessionIdInterface,
                SessionUpdateTimestampHandlerInterface,
                abstract_session_handler::{AbstractSessionHandler, Backend},
                migrating_session_handler::MigratingSessionHandler,
                native_file_session_handler::NativeFileSessionHandler,
                null_session_handler::NullSessionHandler,
                strict_session_handler::StrictSessionHandler,
            },
            native_session_storage::{NativeSessionStorage, SaveHandler},
            php_bridge_session_storage,
            session_storage_interface::SessionStorageInterface,
        },
    },
    streamed_response::Throwable,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/native_session.json");
const SEED_A: &str = "0123456789abcdef0123456789abcdef";
const SEED_B: &str = "fedcba9876543210fedcba9876543210";

// ---------------------------------------------------------------- typed

/// The server's `t()`.
fn t(value: &Value) -> Json {
    match value {
        Value::Null => json!(["null"]),
        Value::Bool(b) => json!(["bool", b]),
        Value::Int(n) => json!(["int", n]),
        Value::Float(f) => json!(["float", format!("{f:?}")]),
        Value::String(s) => json!(["string", s.to_string_lossy()]),
        Value::Array(a) => json!([
            "array",
            a.iter()
                .map(|(k, v)| json!([t(&k.to_value()), t(v)]))
                .collect::<Vec<_>>()
        ]),
        other @ Value::Object(_) => json!(["object", other.value_name()]),
    }
}

/// A JSON operation argument as PHP's `json_decode(..., true)` gives it.
fn php(json: &Json) -> Value {
    match json {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => n.as_i64().map_or(Value::Null, Value::Int),
        Json::String(s) => Value::string(s.as_str()),
        Json::Array(items) => Value::Array(PhpArray::list(items.iter().map(php))),
        Json::Object(map) => {
            let mut array = PhpArray::new();
            for (k, v) in map {
                array.insert(Key::from_bytes(k.as_bytes()), php(v));
            }
            Value::Array(array)
        }
    }
}

// ---------------------------------------------------------------- the log

thread_local! {
    static LOG: RefCell<Vec<Json>> = const { RefCell::new(Vec::new()) };
    static STORE: RefCell<HashMap<String, Vec<(String, String)>>> = RefCell::new(HashMap::new());
    static FAIL: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The warnings raised so far, then `entry`.
fn log(entry: Json) {
    drain_warnings();
    LOG.with(|l| l.borrow_mut().push(entry));
}

fn drain_warnings() {
    for warning in php_session::take_warnings() {
        LOG.with(|l| {
            l.borrow_mut().push(json!([
                "warning",
                warning.level,
                if warning.html {
                    html_warning(&warning.message)
                } else {
                    warning.message
                }
            ]));
        });
    }
}

fn store_get(tag: &str, id: &[u8]) -> Option<String> {
    let id = String::from_utf8_lossy(id);
    STORE.with(|s| {
        s.borrow().get(tag).and_then(|entries| {
            entries
                .iter()
                .find(|(k, _)| *k == id)
                .map(|(_, v)| v.clone())
        })
    })
}

fn store_set(tag: &str, id: &[u8], data: &[u8]) {
    let id = String::from_utf8_lossy(id).into_owned();
    let data = String::from_utf8_lossy(data).into_owned();
    STORE.with(|s| {
        let mut s = s.borrow_mut();
        let entries = s.entry(tag.to_owned()).or_default();
        match entries.iter_mut().find(|(k, _)| *k == id) {
            Some(entry) => entry.1 = data,
            None => entries.push((id, data)),
        }
    });
}

fn store_unset(tag: &str, id: &[u8]) {
    let id = String::from_utf8_lossy(id);
    STORE.with(|s| {
        if let Some(entries) = s.borrow_mut().get_mut(tag) {
            entries.retain(|(k, _)| *k != id);
        }
    });
}

fn s(bytes: &[u8]) -> Json {
    t(&Value::String(PhpString::from(bytes)))
}

/// The server's `Logs::call()`: the call, its answer (`false` for a
/// method the scenario fails).
fn call<T>(
    tag: &str,
    method: &str,
    args: Vec<Json>,
    answer: T,
    value: impl Fn(&T) -> Value,
    failed: T,
) -> T {
    let mut entry = vec![json!("call"), json!(tag), json!(method)];
    entry.extend(args);
    log(Json::Array(entry));
    let answer = if FAIL.with(|f| f.borrow().iter().any(|m| m == method)) {
        failed
    } else {
        answer
    };
    log(json!(["return", tag, method, t(&value(&answer))]));
    answer
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "an answer callback takes the answer by reference"
)]
fn bool_value(b: &bool) -> Value {
    Value::Bool(*b)
}

#[expect(
    clippy::ref_option,
    reason = "an answer callback takes the answer by reference"
)]
fn read_value(r: &Option<PhpString>) -> Value {
    r.clone().map_or(Value::Bool(false), Value::String)
}

// ---------------------------------------------------------------- handlers

/// The server's `RecorderHandler`.
#[derive(Debug)]
struct RecorderHandler {
    tag: &'static str,
    parent: AbstractSessionHandler,
}

impl RecorderHandler {
    fn new(tag: &'static str) -> Arc<Self> {
        Arc::new(Self {
            tag,
            parent: AbstractSessionHandler::default(),
        })
    }
}

/// Logs the call, runs `f` (whose own calls are logged inside), logs the
/// answer.
fn around<T>(
    tag: &str,
    method: &str,
    args: Vec<Json>,
    f: impl FnOnce() -> HandlerResult<T>,
    value: impl Fn(&T) -> Value,
    failed: T,
) -> HandlerResult<T> {
    let mut entry = vec![json!("call"), json!(tag), json!(method)];
    entry.extend(args);
    log(Json::Array(entry));
    let answer = f()?;
    let answer = if FAIL.with(|f| f.borrow().iter().any(|m| m == method)) {
        failed
    } else {
        answer
    };
    log(json!(["return", tag, method, t(&value(&answer))]));
    Ok(answer)
}

impl Backend for RecorderHandler {
    fn class(&self) -> String {
        "RecorderHandler".to_owned()
    }

    fn read(&self, id: &[u8]) -> HandlerResult<Option<PhpString>> {
        around(
            self.tag,
            "read",
            vec![s(id)],
            || self.parent.read(self, id),
            read_value,
            None,
        )
    }

    fn do_read(&self, id: &[u8]) -> HandlerResult<Option<PhpString>> {
        let data = store_get(self.tag, id).unwrap_or_default();
        Ok(call(
            self.tag,
            "doRead",
            vec![s(id)],
            Some(PhpString::from(data)),
            read_value,
            None,
        ))
    }

    fn do_write(&self, id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        store_set(self.tag, id, data);
        Ok(call(
            self.tag,
            "doWrite",
            vec![s(id), s(data)],
            true,
            bool_value,
            false,
        ))
    }

    fn do_destroy(&self, id: &[u8]) -> HandlerResult<bool> {
        store_unset(self.tag, id);
        Ok(call(
            self.tag,
            "doDestroy",
            vec![s(id)],
            true,
            bool_value,
            false,
        ))
    }

    fn destroy(&self, id: &[u8]) -> HandlerResult<bool> {
        around(
            self.tag,
            "destroy",
            vec![s(id)],
            || self.parent.destroy(self, id),
            bool_value,
            false,
        )
    }
}

impl SessionHandlerInterface for RecorderHandler {
    fn class(&self) -> String {
        "RecorderHandler".to_owned()
    }

    fn write_class(&self) -> String {
        symfony_http_foundation::session::storage::handler::abstract_session_handler::CLASS
            .to_owned()
    }

    fn open(&self, save_path: &[u8], name: &[u8]) -> HandlerResult<bool> {
        around(
            self.tag,
            "open",
            vec![s(name)],
            || Ok(self.parent.open(save_path, name)),
            bool_value,
            false,
        )
    }

    fn close(&self) -> HandlerResult<bool> {
        around(self.tag, "close", vec![], || Ok(true), bool_value, false)
    }

    fn read(&self, id: &[u8]) -> HandlerResult<Option<PhpString>> {
        Backend::read(self, id)
    }

    fn write(&self, id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        around(
            self.tag,
            "write",
            vec![s(id), s(data)],
            || self.parent.write(self, id, data),
            bool_value,
            false,
        )
    }

    fn destroy(&self, id: &[u8]) -> HandlerResult<bool> {
        Backend::destroy(self, id)
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        around(
            self.tag,
            "gc",
            vec![t(&Value::Int(max_lifetime))],
            || Ok(Some(0)),
            |n: &Option<i64>| n.map_or(Value::Bool(false), Value::Int),
            None,
        )
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        Some(self)
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        Some(self)
    }
}

impl SessionUpdateTimestampHandlerInterface for RecorderHandler {
    fn validate_id(&self, id: &[u8]) -> HandlerResult<bool> {
        around(
            self.tag,
            "validateId",
            vec![s(id)],
            || Ok(self.parent.validate_id(self, id)),
            bool_value,
            false,
        )
    }

    fn update_timestamp(&self, id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        around(
            self.tag,
            "updateTimestamp",
            vec![s(id), s(data)],
            || self.parent.update_timestamp(self, id, data),
            bool_value,
            false,
        )
    }
}

impl SessionIdInterface for RecorderHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        symfony_http_foundation::session::storage::handler::create_sid()
    }
}

/// The server's `BareHandler` and its subclasses.
#[derive(Debug)]
struct BareHandler {
    class: &'static str,
    missing: Option<PhpString>,
    timestamp: bool,
    ids: Option<Mutex<u32>>,
}

impl BareHandler {
    fn new(class: &'static str, missing: Option<&str>) -> Self {
        Self {
            class,
            missing: missing.map(PhpString::from),
            timestamp: false,
            ids: None,
        }
    }
}

impl SessionHandlerInterface for BareHandler {
    fn class(&self) -> String {
        self.class.to_owned()
    }

    fn open(&self, _save_path: &[u8], name: &[u8]) -> HandlerResult<bool> {
        Ok(call("a", "open", vec![s(name)], true, bool_value, false))
    }

    fn close(&self) -> HandlerResult<bool> {
        Ok(call("a", "close", vec![], true, bool_value, false))
    }

    fn read(&self, id: &[u8]) -> HandlerResult<Option<PhpString>> {
        let data = store_get("a", id)
            .map(PhpString::from)
            .or_else(|| self.missing.clone());
        Ok(call("a", "read", vec![s(id)], data, read_value, None))
    }

    fn write(&self, id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        store_set("a", id, data);
        Ok(call(
            "a",
            "write",
            vec![s(id), s(data)],
            true,
            bool_value,
            false,
        ))
    }

    fn destroy(&self, id: &[u8]) -> HandlerResult<bool> {
        store_unset("a", id);
        Ok(call("a", "destroy", vec![s(id)], true, bool_value, false))
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        Ok(call(
            "a",
            "gc",
            vec![t(&Value::Int(max_lifetime))],
            Some(0),
            |n: &Option<i64>| n.map_or(Value::Bool(false), Value::Int),
            None,
        ))
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        self.timestamp
            .then_some(self as &dyn SessionUpdateTimestampHandlerInterface)
    }

    fn as_session_id(&self) -> Option<&dyn SessionIdInterface> {
        self.ids.as_ref().map(|_| self as &dyn SessionIdInterface)
    }
}

impl SessionUpdateTimestampHandlerInterface for BareHandler {
    fn validate_id(&self, id: &[u8]) -> HandlerResult<bool> {
        let known = store_get("a", id).is_some();
        Ok(call(
            "a",
            "validateId",
            vec![s(id)],
            known,
            bool_value,
            false,
        ))
    }

    fn update_timestamp(&self, id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        Ok(call(
            "a",
            "updateTimestamp",
            vec![s(id), s(data)],
            true,
            bool_value,
            false,
        ))
    }
}

impl SessionIdInterface for BareHandler {
    fn create_sid(&self) -> HandlerResult<PhpString> {
        let next = self.ids.as_ref().map_or(0, |n| {
            let mut n = n.lock().unwrap_or_else(PoisonError::into_inner);
            *n += 1;
            *n
        });
        let id = PhpString::from(format!("custom{next:026}"));
        Ok(call(
            "a",
            "create_sid",
            vec![],
            id,
            |id: &PhpString| Value::String(id.clone()),
            PhpString::default(),
        ))
    }
}

fn handler(kind: &str, dir: &[u8]) -> Result<Option<Arc<dyn SessionHandlerInterface>>, TestError> {
    Ok(Some(match kind {
        "test" => RecorderHandler::new("a"),
        "bare" => Arc::new(BareHandler::new("BareHandler", Some(""))),
        "bare_false" => Arc::new(BareHandler::new("BareHandler", None)),
        "bare_ts" => Arc::new(BareHandler {
            timestamp: true,
            ..BareHandler::new("BareTimestampHandler", Some(""))
        }),
        "bare_id" => Arc::new(BareHandler {
            ids: Some(Mutex::new(0)),
            ..BareHandler::new("BareIdHandler", Some(""))
        }),
        "strict_bare" => Arc::new(StrictSessionHandler::new(Arc::new(BareHandler::new(
            "BareHandler",
            None,
        )))?),
        "migrating" => Arc::new(MigratingSessionHandler::new(
            RecorderHandler::new("a"),
            RecorderHandler::new("b"),
        )?),
        "files" => Arc::new(NativeFileSessionHandler::new(Some(dir))?),
        "null" => Arc::new(NullSessionHandler::default()),
        "none" => return Ok(None),
        other => return Err(format!("unknown handler {other}").into()),
    }))
}

// ---------------------------------------------------------------- the run

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    Ok(text(json)?.as_bytes().to_vec())
}

fn thrown_json(error: &Error) -> Json {
    match error.thrown() {
        Some(other) => thrown_other(other),
        None => json!([
            "throws",
            error.class().name(),
            error.message().to_string_lossy()
        ]),
    }
}

fn thrown_other(error: &Throwable) -> Json {
    match error.downcast_ref::<Error>() {
        Some(e) => thrown_json(e),
        None => json!(["throws", "Throwable", error.to_string()]),
    }
}

/// `['ok', t($value)]` or `['throws', class, message]`.
fn outcome(result: Result<Value, Error>) -> Json {
    match result {
        Ok(value) => json!(["ok", t(&value)]),
        Err(error) => thrown_json(&error),
    }
}

fn session_result(result: Result<Value, Throwable>) -> Json {
    match result {
        Ok(value) => json!(["ok", t(&value)]),
        Err(error) => thrown_other(&error),
    }
}

fn opt_string(r: Option<PhpString>) -> Value {
    r.map_or(Value::Bool(false), Value::String)
}

fn session_var() -> Value {
    php_session::with_session_var(|s| match s {
        SessionVar::Unset => Value::Null,
        other => other.to_value().unwrap_or(Value::Null),
    })
}

fn php_error(message: &str) -> Throwable {
    Arc::new(Error::new(
        symfony_http_foundation::exception::Class::PhpError,
        message,
    ))
}

/// A session function or `$_SESSION` operation.
fn raw_op(op: &[Json]) -> Result<Json, TestError> {
    let arg = |i: usize| op.get(i).ok_or("missing argument");
    Ok(match text(&op[0])? {
        "start" => session_result(php_session::session_start().map(Value::Bool)),
        "write_close" => session_result(php_session::session_write_close().map(Value::Bool)),
        "abort" => session_result(php_session::session_abort().map(Value::Bool)),
        "reset" => session_result(php_session::session_reset().map(Value::Bool)),
        "destroy" => session_result(php_session::session_destroy().map(Value::Bool)),
        "regenerate" => session_result(
            php_session::session_regenerate_id(arg(1)?.as_bool().ok_or("bool")?).map(Value::Bool),
        ),
        "id" => json!(["ok", t(&Value::String(php_session::session_id()))]),
        "set_id" => json!([
            "ok",
            t(&opt_string(php_session::set_session_id(bytes(arg(1)?)?)))
        ]),
        "name" => json!(["ok", t(&Value::String(php_session::session_name()))]),
        "set_name" => json!([
            "ok",
            t(&opt_string(php_session::set_session_name(bytes(arg(1)?)?)))
        ]),
        "status" => json!(["ok", t(&Value::Int(php_session::session_status().value()))]),
        "create_id" => {
            session_result(php_session::session_create_id(&bytes(arg(1)?)?).map(opt_string))
        }
        "encode" => session_result(php_session::session_encode().map(opt_string)),
        "decode" => session_result(php_session::session_decode(&bytes(arg(1)?)?).map(Value::Bool)),
        "gc" => session_result(
            php_session::session_gc().map(|n| n.map_or(Value::Bool(false), Value::Int)),
        ),
        "ini" => {
            let name = text(arg(1)?)?.to_owned();
            json!([
                "ok",
                t(&opt_string(php_session::ini_set(&name, bytes(arg(2)?)?)))
            ])
        }
        "ini_get" => json!(["ok", t(&opt_string(php_session::ini_get(text(arg(1)?)?)))]),
        "cookie_params" => json!([
            "ok",
            t(&Value::Array(php_session::session_get_cookie_params()))
        ]),
        "tick" => {
            let n = arg(1)?.as_i64().ok_or("tick")?;
            php_value::clock::sleep(n);
            json!(["ok", t(&Value::Int(php_value::clock::time()))])
        }
        other => return global_op(other, op),
    })
}

/// An operation on `$_SESSION` or the output.
fn global_op(name: &str, op: &[Json]) -> Result<Json, TestError> {
    let arg = |i: usize| op.get(i).ok_or("missing argument");
    Ok(match name {
        "set" => {
            let key = Key::from_bytes(&bytes(arg(1)?)?);
            let value = php(arg(2)?);
            let set = php_session::with_session_var(|s| {
                match s {
                    SessionVar::Unset | SessionVar::Other(Value::Null) => {
                        *s = SessionVar::Array(Data::default());
                    }
                    SessionVar::Other(_) => return false,
                    SessionVar::Array(_) => {}
                }
                if let SessionVar::Array(data) = s {
                    data.assign(key, value.clone());
                }
                true
            });
            if set {
                json!(["ok", t(&value)])
            } else {
                session_result(Err(php_error("Cannot use a scalar value as an array")))
            }
        }
        "unset" => {
            let key = Key::from_bytes(&bytes(arg(1)?)?);
            let unset = php_session::with_session_var(|s| match s {
                SessionVar::Array(data) => {
                    data.unset(&key);
                    true
                }
                SessionVar::Unset | SessionVar::Other(Value::Null) => true,
                SessionVar::Other(_) => false,
            });
            if unset {
                json!(["ok", ["null"]])
            } else {
                session_result(Err(php_error(
                    "Cannot unset offset in a non-array variable",
                )))
            }
        }
        "assign" => {
            let value = php(arg(1)?);
            php_session::with_session_var(|s| *s = SessionVar::from_value(value.clone()));
            json!(["ok", t(&value)])
        }
        "session" => json!(["ok", t(&session_var())]),
        "echo" => {
            with_output(|o| {
                o.calls.push(OutputCall::Echo(PhpString::from("output\n")));
                o.headers_sent = true;
            });
            json!(["ok", ["int", 1]])
        }
        "setcookie" => {
            let sent = php_session::setcookie(SetCookie {
                name: PhpString::from(bytes(arg(1)?)?),
                value: PhpString::from(bytes(arg(2)?)?),
                ..SetCookie::default()
            });
            json!(["ok", ["bool", sent]])
        }
        other => return Err(format!("unknown op {other}").into()),
    })
}

fn bag_op(storage: &NativeSessionStorage, op: &[Json]) -> Result<Json, TestError> {
    let arg = |i: usize| op.get(i).ok_or("missing argument");
    let attributes = || -> Result<_, Error> {
        storage
            .get_bag("attributes")?
            .into_attribute_bag()
            .ok_or_else(|| {
                Error::new(
                    symfony_http_foundation::exception::Class::NotPorted,
                    "not an attribute bag",
                )
            })
    };
    let flashes = || -> Result<_, Error> {
        storage.get_bag("flashes")?.into_flash_bag().ok_or_else(|| {
            Error::new(
                symfony_http_foundation::exception::Class::NotPorted,
                "not a flash bag",
            )
        })
    };
    Ok(match text(&op[0])? {
        "s_attr_set" => {
            let (name, value) = (text(arg(1)?)?.to_owned(), php(arg(2)?));
            outcome(attributes().map(|a| {
                a.set(&name, value);
                Value::Null
            }))
        }
        "s_attr_get" => {
            let name = text(arg(1)?)?.to_owned();
            outcome(attributes().map(|a| a.get(&name, Value::Null)))
        }
        "s_attr_all" => outcome(attributes().map(|a| Value::Array(a.all()))),
        "s_flash_add" => {
            let (kind, message) = (text(arg(1)?)?.to_owned(), php(arg(2)?));
            outcome(flashes().and_then(|f| f.add(&kind, message).map(|()| Value::Null)))
        }
        "s_flash_get" => {
            let kind = text(arg(1)?)?.to_owned();
            outcome(flashes().and_then(|f| f.get(&kind, PhpArray::new()).map(Value::Array)))
        }
        other => return Err(format!("unknown op {other}").into()),
    })
}

fn storage_op(storage: &NativeSessionStorage, op: &[Json]) -> Result<Json, TestError> {
    let arg = |i: usize| op.get(i).ok_or("missing argument");
    Ok(match text(&op[0])? {
        "s_start" => outcome(storage.start().map(Value::Bool)),
        "s_save" => outcome(storage.save().map(|()| Value::Null)),
        "s_regenerate" => {
            let destroy = arg(1)?.as_bool().ok_or("bool")?;
            let lifetime = arg(2)?.as_i64();
            outcome(storage.regenerate(destroy, lifetime).map(Value::Bool))
        }
        "s_clear" => outcome(storage.clear().map(|()| Value::Null)),
        "s_set_id" => outcome(storage.set_id(text(arg(1)?)?).map(|()| Value::Null)),
        "s_get_id" => json!(["ok", t(&Value::string(storage.get_id()))]),
        "s_set_name" => outcome(storage.set_name(text(arg(1)?)?).map(|()| Value::Null)),
        "s_get_name" => json!(["ok", t(&Value::string(storage.get_name()))]),
        "s_is_started" => json!(["ok", t(&Value::Bool(storage.is_started()))]),
        "s_meta" => {
            let meta = storage.get_metadata_bag();
            outcome((|| {
                Ok(Value::Array(PhpArray::list([
                    Value::Int(meta.get_created()?),
                    Value::Int(meta.get_last_used()?),
                    Value::Int(meta.get_lifetime()?),
                ])))
            })())
        }
        "s_set_options" => {
            let Value::Array(options) = php(arg(1)?) else {
                return Err("options".into());
            };
            storage.set_options(&options);
            json!(["ok", ["null"]])
        }
        "s_register_bag" => {
            // new AttributeBag('_late', 'late'): the constructor has one parameter
            outcome(
                storage
                    .register_bag(Arc::new(AttributeBag::new("_late")))
                    .map(|()| Value::Null),
            )
        }
        "s_handler_name" => json!([
            "ok",
            t(&storage
                .get_save_handler()
                .proxy()
                .get_save_handler_name()
                .map_or(Value::Null, Value::string))
        ]),
        name if name.starts_with("s_") => bag_op(storage, op)?,
        _ => raw_op(op)?,
    })
}

/// The request's `$_COOKIE` from its `Cookie` header.
fn cookies(header: &str) {
    for pair in header.split("; ") {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = php_value::url::urldecode(value.as_bytes());
        with_globals(|g| {
            g.cookie.insert(
                Key::from_bytes(name.as_bytes()),
                Value::String(PhpString::from(value)),
            );
        });
    }
}

/// The request as the server starts it: cookie, headers, store, files,
/// ini settings; the save path.
fn set_up(scenario: &Json, dir: &std::path::Path) -> Result<Vec<u8>, TestError> {
    if let Some(cookie) = scenario["cookie"].as_str() {
        cookies(cookie);
    }
    php_session::header_remove("X-Powered-By");
    php_session::header("Content-Type: text/plain; charset=utf-8", true);
    FAIL.with(|f| {
        *f.borrow_mut() = scenario["fail"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|m| m.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
    });
    if let Some(store) = scenario["store"].as_object() {
        for (tag, entries) in store {
            for (id, data) in entries.as_object().into_iter().flatten() {
                store_set(tag, id.as_bytes(), text(data)?.as_bytes());
            }
        }
    }
    std::fs::create_dir_all(dir)?;
    for (id, data) in scenario["files"].as_object().into_iter().flatten() {
        std::fs::write(dir.join(format!("sess_{id}")), text(data)?)?;
    }
    for pair in scenario["ini"].as_array().into_iter().flatten() {
        php_session::ini_set(text(&pair[0])?, bytes(&pair[1])?);
    }
    let dir_bytes = from_path(dir);
    php_session::ini_set("save_path", dir_bytes.clone());
    Ok(dir_bytes)
}

/// What the server answers once the request ends.
fn transcript(dir: &std::path::Path) -> Result<Json, TestError> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(id) = name.strip_prefix("sess_") {
            files.push(json!([
                id,
                String::from_utf8_lossy(&std::fs::read(entry.path())?)
            ]));
        }
    }
    let store = STORE.with(|s| {
        let s = s.borrow();
        let mut tags: Vec<_> = s.keys().cloned().collect();
        tags.sort();
        let mut array = PhpArray::new();
        for tag in tags {
            let mut entries = PhpArray::new();
            for (id, data) in &s[&tag] {
                entries.insert(Key::from_bytes(id.as_bytes()), Value::string(data.as_str()));
            }
            array.insert(Key::from_bytes(tag.as_bytes()), Value::Array(entries));
        }
        array
    });
    let session = php_session::with_session_var(|s| match s {
        SessionVar::Unset => json!(["unset"]),
        other => t(&other.to_value().unwrap_or(Value::Null)),
    });
    Ok(json!({
        "log": LOG.with(|l| l.borrow().clone()),
        "headers": php_session::headers_list().iter().map(|h| h.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "status": php_session::session_status().value(),
        "id": php_session::session_id().to_string_lossy(),
        "session": session,
        "store": t(&Value::Array(store)),
        "files": files,
    }))
}

fn run(scenario: &Json, dir: &std::path::Path) -> Result<Json, TestError> {
    let _clock = php_value::clock::mock(1_700_000_000);
    let dir_bytes = set_up(scenario, dir)?;
    let mode = text(&scenario["mode"])?;
    let handler = handler(text(&scenario["handler"])?, &dir_bytes);
    let mut storage = None;
    let build = (|| -> Result<(), Error> {
        let handler = handler.map_err(|e| {
            Error::new(
                symfony_http_foundation::exception::Class::NotPorted,
                e.to_string(),
            )
        })?;
        if let Some(h) = &handler
            && (mode == "raw" || mode == "bridge")
        {
            php_session::session_set_save_handler(Arc::clone(h), false);
        }
        let save_handler = handler.map(SaveHandler::Handler);
        let built = match mode {
            "native" => {
                let options = match php(&scenario["options"]) {
                    Value::Array(a) => a,
                    _ => PhpArray::new(),
                };
                Some(NativeSessionStorage::new(&options, save_handler, None)?)
            }
            "bridge" => {
                if scenario["bridge_started"].as_bool() == Some(true) {
                    php_session::session_start().map_err(Error::from_throwable)?;
                }
                Some(php_bridge_session_storage::new(save_handler, None)?)
            }
            _ => None,
        };
        if let Some(s) = &built {
            s.register_bag(Arc::new(AttributeBag::default()))?;
            s.register_bag(Arc::new(FlashBag::default()))?;
        }
        storage = built;
        Ok(())
    })();
    log(json!(["build", outcome(build.map(|()| Value::Null))]));

    if !LOG.with(|l| l.borrow().last().is_some_and(|e| e[1][0] == "throws")) {
        for op in scenario["ops"].as_array().into_iter().flatten() {
            let op = op.as_array().ok_or("op")?;
            let result = match &storage {
                Some(storage) => storage_op(storage, op)?,
                None => raw_op(op)?,
            };
            log(json!(["op", op, result]));
        }
    }

    let end = if php_session::session_status() == Status::Active {
        session_result(php_session::session_write_close().map(Value::Bool))
    } else {
        json!(["ok", ["null"]])
    };
    log(json!(["end", end]));
    drain_warnings();

    transcript(dir)
}

// ---------------------------------------------------------------- normalizing

/// The recorder's `TranscriptNormalizer`.
struct Normalizer {
    dir: String,
    ids: Vec<String>,
}

fn is_hex_id(s: &[u8]) -> bool {
    s.len() == 32
        && s.iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
}

fn date_shape(date: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = date.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_uppercase()
            && chars.get(i + 1).is_some_and(char::is_ascii_lowercase)
            && chars.get(i + 2).is_some_and(char::is_ascii_lowercase)
        {
            out.push_str("Xxx");
            i += 3;
        } else if chars[i].is_ascii_digit() {
            out.push('9');
            i += 1;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}

impl Pipe for String {}

/// Each text between `start` and `end` replaced.
fn replace_between(s: &str, start: &str, end: &str, with: impl Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find(start) {
        out.push_str(&rest[..at + start.len()]);
        rest = &rest[at + start.len()..];
        match rest.find(end) {
            Some(stop) => {
                out.push_str(&with(&rest[..stop]));
                rest = &rest[stop..];
            }
            None => break,
        }
    }
    out.push_str(rest);
    out
}

/// The digits after each `after` replaced.
fn replace_number_after(s: &str, after: &str, with: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(at) = rest.find(after) {
        out.push_str(&rest[..at + after.len()]);
        rest = &rest[at + after.len()..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 {
            out.push_str(with);
            rest = &rest[digits..];
        }
    }
    out.push_str(rest);
    out
}

impl Normalizer {
    fn text(&mut self, s: &str, file: bool) -> String {
        let s = s.replace(&self.dir, "{dir}");
        // generated ids
        let bytes = s.as_bytes();
        let mut out = String::new();
        let mut i = 0;
        while i < bytes.len() {
            let hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
            if i + 32 <= bytes.len()
                && is_hex_id(&bytes[i..i + 32])
                && (i == 0 || !hex(bytes[i - 1]))
                && bytes.get(i + 32).is_none_or(|b| !hex(*b))
            {
                let id = &s[i..i + 32];
                if id == SEED_A || id == SEED_B {
                    out.push_str(id);
                } else if let Some(n) = self.ids.iter().position(|known| known == id) {
                    let _ = write!(out, "{{id{}}}", n + 1);
                } else if file {
                    out.push_str("{file}");
                } else {
                    self.ids.push(id.to_owned());
                    let _ = write!(out, "{{id{}}}", self.ids.len());
                }
                i += 32;
                continue;
            }
            let c = s[i..].chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
        // dates
        let mut s = out;
        for prefix in ["expires=", "Expires: "] {
            let mut done = String::new();
            let mut rest = s.as_str();
            while let Some(at) = rest.find(prefix) {
                done.push_str(&rest[..at + prefix.len()]);
                rest = &rest[at + prefix.len()..];
                let end = rest.find(';').unwrap_or(rest.len());
                let date = &rest[..end];
                if date.contains("1970") || date.contains("1981") {
                    done.push_str(date);
                } else {
                    done.push_str(&date_shape(date));
                }
                rest = &rest[end..];
            }
            done.push_str(rest);
            s = done;
        }
        replace_between(&s, "by \"", "\" at line ", |_| "{file}".to_owned())
            .pipe(|s| replace_number_after(&s, "\" at line ", "{line}"))
            .pipe(|s| replace_between(&s, "output started at ", ")", |_| "{output}".to_owned()))
            .pipe(|s| replace_between(&s, "(started from ", " on line ", |_| "{file}".to_owned()))
            .pipe(|s| replace_number_after(&s, "{file} on line ", "{line}"))
    }

    fn walk(&mut self, v: &Json, file: bool) -> Json {
        match v {
            Json::String(s) => Json::String(self.text(s, file)),
            Json::Array(items) => Json::Array(items.iter().map(|i| self.walk(i, file)).collect()),
            Json::Object(map) => Json::Object(
                map.iter()
                    .map(|(k, v)| (self.text(k, file), self.walk(v, file)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}

fn normalize(result: &Json, dir: &str) -> Json {
    let mut n = Normalizer {
        dir: dir.to_owned(),
        ids: Vec::new(),
    };
    let mut out = serde_json::Map::new();
    for field in ["log", "headers", "status", "id", "session", "store"] {
        out.insert(field.to_owned(), n.walk(&result[field], false));
    }
    let mut files = n
        .walk(&result["files"], true)
        .as_array()
        .cloned()
        .unwrap_or_default();
    files.sort_by_key(std::string::ToString::to_string);
    out.insert("files".to_owned(), Json::Array(files));
    Json::Object(out)
}

/// The recorded transcript, its files sorted as this test sorts them.
fn recorded(result: &Json) -> Json {
    let mut out = result.as_object().cloned().unwrap_or_default();
    if let Some(Json::Array(files)) = out.get_mut("files") {
        files.sort_by_key(std::string::ToString::to_string);
    }
    Json::Object(out)
}

fn first_difference(expected: &Json, got: &Json) -> String {
    for field in [
        "log", "headers", "status", "id", "session", "store", "files",
    ] {
        let (e, g) = (&expected[field], &got[field]);
        if e == g {
            continue;
        }
        if let (Some(e), Some(g)) = (e.as_array(), g.as_array()) {
            let i = e
                .iter()
                .zip(g)
                .position(|(a, b)| a != b)
                .unwrap_or(e.len().min(g.len()));
            return format!(
                "{field}[{i}]: expected {}, got {}",
                e.get(i).map_or("(end)".to_owned(), Json::to_string),
                g.get(i).map_or("(end)".to_owned(), Json::to_string)
            );
        }
        return format!("{field}: expected {e}, got {g}");
    }
    "same".to_owned()
}

fn temp_dir(n: u64) -> PathBuf {
    std::env::temp_dir().join(format!(
        "symfony-rs-native-session-{}-{n}",
        std::process::id()
    ))
}

#[test]
fn native_sessions_answer_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let scenarios = data["scenarios"].as_array().ok_or("scenarios")?;
    assert_eq!(1500, scenarios.len());
    let mut failures = Vec::new();
    for entry in scenarios {
        let scenario = entry["scenario"].clone();
        let n = scenario["n"].as_u64().ok_or("n")?;
        let dir = temp_dir(n);
        let dir_for_run = dir.clone();
        let got =
            std::thread::spawn(move || run(&scenario, &dir_for_run).map_err(|e| e.to_string()))
                .join()
                .map_err(|_| format!("scenario {n} panicked"))?;
        let dir_text = String::from_utf8_lossy(&from_path(&dir)).into_owned();
        let _ = std::fs::remove_dir_all(&dir);
        let got = match got {
            Ok(got) => normalize(&got, &dir_text),
            Err(error) => {
                failures.push(format!("scenario {n}: {error}"));
                continue;
            }
        };
        let expected = recorded(&entry["result"]);
        // NATIVE_SESSION_DUMP=<n> prints both transcripts of scenario n
        if std::env::var("NATIVE_SESSION_DUMP").ok() == Some(n.to_string()) {
            for (name, log) in [("expected", &expected["log"]), ("got", &got["log"])] {
                eprintln!("== {name}");
                for line in log.as_array().into_iter().flatten() {
                    if line[0] != "return" {
                        eprintln!("{line}");
                    }
                }
            }
        }
        if expected != got {
            failures.push(format!(
                "scenario {n} ({} {}): {}",
                entry["scenario"]["mode"],
                entry["scenario"]["handler"],
                first_difference(&expected, &got)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        scenarios.len(),
        failures
            .iter()
            .take(25)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    Ok(())
}
