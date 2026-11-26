//! The session handlers against PHP 8.4.27 running the pinned
//! symfony/http-foundation (v8.1.11), recorded by
//! `oracle/recorders/http_foundation_session_handlers.php`
//! (`scripts/record.sh http-foundation-session-handlers`): 1,200 generated
//! scripts, 60 `IdentityMarshaller::marshall()` cases and the
//! constructors' errors.
//!
//! A script builds a chain over scripted leaves: a `RecorderBackend` (an
//! `AbstractSessionHandler` whose `doRead`/`doWrite`/`doDestroy` answer
//! from queues) or a `RecorderBareHandler` (a plain
//! `\SessionHandlerInterface`, given to `StrictSessionHandler`), under a
//! `MarshallingSessionHandler` (identity or scripted marshaller) or a
//! `MigratingSessionHandler` of two leaves, or alone. Each leaf's queues
//! come with the script. `session.use_cookies` is 0, as recorded.
//!
//! Compared: each operation's result or exception, and the calls each
//! leaf and the marshaller saw since the previous operation.
//!
//! Upstream: none

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, PoisonError},
};

use php_value::{PhpArray, PhpString, Value, vectors};
use serde_json::Value as Json;
use symfony_http_foundation::{
    exception::{Class, Error},
    php_session::ini_set,
    session::storage::handler::{
        HandlerResult, SessionHandlerInterface, SessionUpdateTimestampHandlerInterface,
        abstract_session_handler::{self, AbstractSessionHandler, Backend},
        identity_marshaller::IdentityMarshaller,
        marshaller_interface::MarshallerInterface,
        marshalling_session_handler::MarshallingSessionHandler,
        migrating_session_handler::MigratingSessionHandler,
        strict_session_handler::StrictSessionHandler,
    },
    streamed_response::Throwable,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/session_handlers.json");

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<Vec<u8>, TestError> {
    Ok(vectors::base64(text(json)?)?)
}

fn items(json: &Json) -> Result<&Vec<Json>, TestError> {
    json.as_array().ok_or_else(|| "expected an array".into())
}

fn show(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn class(name: &str) -> Result<Class, TestError> {
    Ok(match name {
        "RuntimeException" => Class::PhpRuntime,
        "DomainException" => Class::PhpDomain,
        "LogicException" => Class::PhpLogic,
        other => return Err(format!("unknown class {other}").into()),
    })
}

fn thrown(answer: &Json) -> Result<Throwable, TestError> {
    Ok(Arc::new(Error::new(
        class(text(&answer[1])?)?,
        text(&answer[2])?.to_owned(),
    )))
}

/// A scripted answer: `['s', b64]`, `['false']`, `['throw', class,
/// message]`, `['bool', b]`, `['int', n]`.
#[derive(Debug, Clone)]
enum Answer {
    Text(PhpString),
    False,
    Throw(Throwable),
    Bool(bool),
    Int(i64),
}

fn answer(json: &Json) -> Result<Answer, TestError> {
    Ok(match text(&json[0])? {
        "s" => Answer::Text(PhpString::from(bytes(&json[1])?)),
        "false" => Answer::False,
        "throw" => Answer::Throw(thrown(json)?),
        "bool" => Answer::Bool(json[1].as_bool().ok_or("bool")?),
        "int" => Answer::Int(json[1].as_i64().ok_or("int")?),
        other => return Err(format!("unknown answer {other}").into()),
    })
}

#[derive(Debug, Default)]
struct Script {
    calls: Mutex<Vec<String>>,
    queues: Mutex<std::collections::HashMap<String, VecDeque<Answer>>>,
}

impl Script {
    fn new(answers: &Json) -> Result<Self, TestError> {
        let script = Self::default();
        let map = answers.as_object().ok_or("answers")?;
        for (queue, list) in map {
            let list = items(list)?.iter().map(answer).collect::<Result<_, _>>()?;
            lock(&script.queues).insert(queue.clone(), list);
        }
        Ok(script)
    }

    fn log(&self, call: String) {
        lock(&self.calls).push(call);
    }

    fn take(&self, queue: &str) -> Option<Answer> {
        lock(&self.queues)
            .get_mut(queue)
            .and_then(VecDeque::pop_front)
    }

    fn bool(&self, queue: &str) -> HandlerResult<bool> {
        match self.take(queue) {
            None => Ok(true),
            Some(Answer::Bool(b)) => Ok(b),
            Some(Answer::Throw(e)) => Err(e),
            Some(other) => Err(Arc::new(Error::new(
                Class::PhpError,
                format!("bad bool answer {other:?}"),
            ))),
        }
    }

    fn read(&self) -> HandlerResult<Option<PhpString>> {
        match self.take("read") {
            None => Ok(Some(PhpString::default())),
            Some(Answer::Text(s)) => Ok(Some(s)),
            Some(Answer::False) => Ok(None),
            Some(Answer::Throw(e)) => Err(e),
            Some(other) => Err(Arc::new(Error::new(
                Class::PhpError,
                format!("bad read answer {other:?}"),
            ))),
        }
    }

    fn gc(&self) -> HandlerResult<Option<i64>> {
        match self.take("gc") {
            None => Ok(Some(1)),
            Some(Answer::Int(n)) => Ok(Some(n)),
            Some(Answer::False) => Ok(None),
            Some(Answer::Throw(e)) => Err(e),
            Some(other) => Err(Arc::new(Error::new(
                Class::PhpError,
                format!("bad gc answer {other:?}"),
            ))),
        }
    }

    fn drain(&self) -> Vec<String> {
        std::mem::take(&mut *lock(&self.calls))
    }
}

/// `RecorderBackend extends AbstractSessionHandler`.
#[derive(Debug)]
struct RecorderBackend {
    parent: AbstractSessionHandler,
    script: Script,
}

impl Backend for RecorderBackend {
    fn class(&self) -> String {
        "RecorderBackend".to_owned()
    }

    fn do_read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.script.log(format!("doRead {}", show(session_id)));
        self.script.read()
    }

    fn do_write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.script
            .log(format!("doWrite {} {}", show(session_id), show(data)));
        self.script.bool("write")
    }

    fn do_destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.script.log(format!("doDestroy {}", show(session_id)));
        self.script.bool("destroy")
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.parent.destroy(self, session_id)
    }
}

impl SessionHandlerInterface for RecorderBackend {
    fn class(&self) -> String {
        "RecorderBackend".to_owned()
    }

    fn write_class(&self) -> String {
        abstract_session_handler::CLASS.to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        Ok(self.parent.open(save_path, session_name))
    }

    fn close(&self) -> HandlerResult<bool> {
        self.script.log("close".to_owned());
        self.script.bool("close")
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.parent.read(self, session_id)
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.parent.write(self, session_id, data)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        Backend::destroy(self, session_id)
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        self.script.log(format!("gc {max_lifetime}"));
        self.script.gc()
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        Some(self)
    }
}

impl SessionUpdateTimestampHandlerInterface for RecorderBackend {
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool> {
        Ok(self.parent.validate_id(self, session_id))
    }

    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.parent.update_timestamp(self, session_id, data)
    }
}

/// `RecorderBareHandler implements \SessionHandlerInterface`.
#[derive(Debug)]
struct RecorderBareHandler {
    script: Script,
}

impl SessionHandlerInterface for RecorderBareHandler {
    fn class(&self) -> String {
        "RecorderBareHandler".to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        self.script
            .log(format!("open {} {}", show(save_path), show(session_name)));
        self.script.bool("open")
    }

    fn close(&self) -> HandlerResult<bool> {
        self.script.log("close".to_owned());
        self.script.bool("close")
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.script.log(format!("read {}", show(session_id)));
        self.script.read()
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.script
            .log(format!("write {} {}", show(session_id), show(data)));
        self.script.bool("write")
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.script.log(format!("destroy {}", show(session_id)));
        self.script.bool("destroy")
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        self.script.log(format!("gc {max_lifetime}"));
        self.script.gc()
    }
}

/// `RecorderMarshaller implements MarshallerInterface`.
#[derive(Debug, Default)]
struct RecorderMarshaller {
    calls: Mutex<Vec<String>>,
    marshalls: Mutex<VecDeque<Json>>,
    unmarshalls: Mutex<VecDeque<Json>>,
}

fn not_recorded(what: String) -> Throwable {
    Arc::new(Error::new(Class::PhpError, what))
}

impl MarshallerInterface for RecorderMarshaller {
    fn marshall(&self, values: &PhpArray, failed: &mut PhpArray) -> HandlerResult<PhpArray> {
        let data = values
            .get(&"data".into())
            .and_then(Value::to_php_string)
            .unwrap_or_default();
        lock(&self.calls).push(format!("marshall {}", show(data.as_bytes())));
        let answer = lock(&self.marshalls).pop_front();
        let mut out = PhpArray::new();
        let Some(answer) = answer else {
            let mut text = b"m:".to_vec();
            text.extend_from_slice(data.as_bytes());
            out.insert("data", Value::String(PhpString::from(text)));
            return Ok(out);
        };
        match answer[0].as_str() {
            Some("string") => {
                let text = vectors::base64(answer[1].as_str().unwrap_or_default())
                    .map_err(|e| not_recorded(e.to_string()))?;
                out.insert("data", Value::String(PhpString::from(text)));
            }
            Some("int") => {
                out.insert("data", Value::Int(answer[1].as_i64().unwrap_or_default()));
            }
            Some("missing") => {}
            Some("failed-key") => {
                *failed = PhpArray::new();
                failed.insert("data", Value::Bool(true));
            }
            Some("failed-list") => {
                *failed = PhpArray::list([Value::string("data")]);
            }
            other => return Err(not_recorded(format!("marshall answer {other:?}"))),
        }
        Ok(out)
    }

    fn unmarshall(&self, value: &PhpString) -> HandlerResult<Value> {
        lock(&self.calls).push(format!("unmarshall {}", show(value.as_bytes())));
        let answer = lock(&self.unmarshalls).pop_front();
        let Some(answer) = answer else {
            let mut text = b"u:".to_vec();
            text.extend_from_slice(value.as_bytes());
            return Ok(Value::String(PhpString::from(text)));
        };
        Ok(match answer[0].as_str() {
            Some("string") => Value::String(PhpString::from(
                vectors::base64(answer[1].as_str().unwrap_or_default())
                    .map_err(|e| not_recorded(e.to_string()))?,
            )),
            Some("false") => Value::Bool(false),
            Some("true") => Value::Bool(true),
            Some("int") => Value::Int(answer[1].as_i64().unwrap_or_default()),
            Some("float") => vectors::decode(&serde_json::json!(["float", answer[1]]))
                .map_err(|e| not_recorded(e.to_string()))?,
            Some("null") => Value::Null,
            Some("array") => Value::Array(PhpArray::new()),
            Some("throw") => return Err(thrown(&answer).map_err(|e| not_recorded(e.to_string()))?),
            other => return Err(not_recorded(format!("unmarshall answer {other:?}"))),
        })
    }
}

enum Leaf {
    Backend(Arc<RecorderBackend>),
    Bare(Arc<RecorderBareHandler>),
}

impl Leaf {
    fn new(spec: &Json) -> Result<Self, TestError> {
        let script = Script::new(&spec["answers"])?;
        Ok(match text(&spec["kind"])? {
            "abstract" => Self::Backend(Arc::new(RecorderBackend {
                parent: AbstractSessionHandler::default(),
                script,
            })),
            "bare" => Self::Bare(Arc::new(RecorderBareHandler { script })),
            other => return Err(format!("unknown leaf {other}").into()),
        })
    }

    /// The leaf as given to `MigratingSessionHandler`.
    fn raw(&self) -> Arc<dyn SessionHandlerInterface> {
        match self {
            Self::Backend(b) => b.clone(),
            Self::Bare(b) => b.clone(),
        }
    }

    /// The leaf as a handler on its own: a bare one under `Strict`.
    fn handler(&self) -> Result<Arc<dyn SessionHandlerInterface>, Error> {
        Ok(match self {
            Self::Backend(b) => b.clone(),
            Self::Bare(b) => Arc::new(StrictSessionHandler::new(b.clone())?),
        })
    }

    fn drain(&self) -> Vec<String> {
        match self {
            Self::Backend(b) => b.script.drain(),
            Self::Bare(b) => b.script.drain(),
        }
    }
}

fn throws_as(recorded: &Json, error: &Error) -> Result<bool, TestError> {
    Ok(recorded[0] == "throws"
        && recorded[1] == error.class().name()
        && bytes(&recorded[2])? == error.message().as_bytes())
}

fn same(recorded: &Json, got: &HandlerResult<Value>) -> Result<bool, TestError> {
    match got {
        Err(e) => match e.downcast_ref::<Error>() {
            Some(e) => throws_as(recorded, e),
            None => Ok(false),
        },
        Ok(value) => Ok(recorded[0] != "throws" && vectors::decode(recorded)?.identical(value)),
    }
}

fn bool_value(r: HandlerResult<bool>) -> HandlerResult<Value> {
    r.map(Value::Bool)
}

fn read_value(r: HandlerResult<Option<PhpString>>) -> HandlerResult<Value> {
    r.map(|d| d.map_or(Value::Bool(false), Value::String))
}

fn run(
    handler: &dyn SessionHandlerInterface,
    op: &Json,
) -> Result<HandlerResult<Value>, TestError> {
    let arg = |i: usize| bytes(&op[i]);
    let timestamp = || {
        handler
            .as_update_timestamp()
            .ok_or_else(|| TestError::from("not a timestamp handler"))
    };
    Ok(match text(&op[0])? {
        "open" => bool_value(handler.open(&arg(1)?, &arg(2)?)),
        "close" => bool_value(handler.close()),
        "validateId" => bool_value(timestamp()?.validate_id(&arg(1)?)),
        "read" => read_value(handler.read(&arg(1)?)),
        "write" => bool_value(handler.write(&arg(1)?, &arg(2)?)),
        "destroy" => bool_value(handler.destroy(&arg(1)?)),
        "updateTimestamp" => bool_value(timestamp()?.update_timestamp(&arg(1)?, &arg(2)?)),
        "gc" => handler
            .gc(op[1].as_i64().ok_or("gc lifetime")?)
            .map(|n| n.map_or(Value::Bool(false), Value::Int)),
        other => return Err(format!("unknown op {other}").into()),
    })
}

fn strings(json: &Json) -> Result<Vec<String>, TestError> {
    items(json)?.iter().map(|c| Ok(show(&bytes(c)?))).collect()
}

#[test]
fn handler_chains_answer_as_php() -> Result<(), TestError> {
    ini_set("use_cookies", "0");
    let data: Json = serde_json::from_str(DATA)?;
    let scripts = items(&data["scripts"])?;
    assert_eq!(1200, scripts.len());
    let mut failures = Vec::new();
    let mut answers = 0;

    for (n, script) in scripts.iter().enumerate() {
        let leaves = items(&script["leaves"])?
            .iter()
            .map(Leaf::new)
            .collect::<Result<Vec<_>, _>>()?;
        let leaf0 = leaves.first().ok_or("no leaf")?;
        let mut marshaller = None;
        let built: Result<Arc<dyn SessionHandlerInterface>, Error> = match text(&script["top"])? {
            "leaf" => leaf0.handler(),
            "marshalling" => {
                let spec = &script["marshaller"];
                let m: Arc<dyn MarshallerInterface> = if spec["kind"] == "identity" {
                    Arc::new(IdentityMarshaller)
                } else {
                    let m = Arc::new(RecorderMarshaller {
                        marshalls: Mutex::new(items(&spec["marshall"])?.iter().cloned().collect()),
                        unmarshalls: Mutex::new(
                            items(&spec["unmarshall"])?.iter().cloned().collect(),
                        ),
                        ..RecorderMarshaller::default()
                    });
                    marshaller = Some(m.clone());
                    m
                };
                leaf0
                    .handler()
                    .and_then(|h| Ok(Arc::new(MarshallingSessionHandler::new(h, m)?) as Arc<_>))
            }
            "migrating" => {
                let leaf1 = leaves.get(1).ok_or("no second leaf")?;
                MigratingSessionHandler::new(leaf0.raw(), leaf1.raw())
                    .map(|h| Arc::new(h) as Arc<dyn SessionHandlerInterface>)
            }
            other => return Err(format!("unknown top {other}").into()),
        };
        let build = &script["build"];
        let handler = match built {
            Ok(h) if build[0] == "ok" => h,
            Err(e) if throws_as(build, &e)? => continue,
            other => {
                failures.push(format!("script {n}: build {build}, got {:?}", other.err()));
                continue;
            }
        };

        for (i, (op, recorded)) in items(&script["ops"])?
            .iter()
            .zip(items(&script["results"])?)
            .enumerate()
        {
            answers += 1;
            let got = run(handler.as_ref(), op)?;
            if !same(&recorded["result"], &got)? {
                failures.push(format!(
                    "script {n} op {i} {op}: expected {}, got {got:?}",
                    recorded["result"]
                ));
            }
            for (l, (leaf, calls)) in leaves.iter().zip(items(&recorded["calls"])?).enumerate() {
                let (expected, got) = (strings(calls)?, leaf.drain());
                if expected != got {
                    failures.push(format!(
                        "script {n} op {i} {op}: leaf {l} calls {expected:?}, got {got:?}"
                    ));
                }
            }
            let expected = strings(&recorded["marshaller"])?;
            let got = marshaller
                .as_ref()
                .map(|m| std::mem::take(&mut *lock(&m.calls)))
                .unwrap_or_default();
            if expected != got {
                failures.push(format!(
                    "script {n} op {i} {op}: marshaller calls {expected:?}, got {got:?}"
                ));
            }
        }
    }
    assert!(answers > 5000, "only {answers} answers");
    assert!(
        failures.is_empty(),
        "{} of {answers} differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    Ok(())
}

#[test]
fn identity_marshaller_answers_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let cases = items(&data["identity"])?;
    assert_eq!(60, cases.len());
    let mut failures = Vec::new();
    for (n, case) in cases.iter().enumerate() {
        let values = items(&case["values"])?
            .iter()
            .map(|v| {
                Ok(match text(&v[0])? {
                    "string" => Value::string(text(&v[1])?),
                    "int" => Value::Int(v[1].as_i64().ok_or("int")?),
                    "null" => Value::Null,
                    "array" => Value::Array(PhpArray::new()),
                    "bool" => Value::Bool(v[1].as_bool().ok_or("bool")?),
                    "float" => Value::Float(text(&v[1])?.parse()?),
                    other => return Err(format!("unknown value {other}").into()),
                })
            })
            .collect::<Result<Vec<_>, TestError>>()?;
        let mut failed = PhpArray::new();
        let got = IdentityMarshaller
            .marshall(&PhpArray::list(values), &mut failed)
            .map(Value::Array);
        if !same(&case["result"], &got)? {
            failures.push(format!(
                "case {n}: expected {}, got {got:?}",
                case["result"]
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    Ok(())
}

#[test]
fn constructors_fail_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let recorded = &data["constructors"];
    let bare = || {
        Arc::new(RecorderBareHandler {
            script: Script::default(),
        })
    };
    let backend = Arc::new(RecorderBackend {
        parent: AbstractSessionHandler::default(),
        script: Script::default(),
    });

    let marshalling = MarshallingSessionHandler::new(bare(), Arc::new(IdentityMarshaller))
        .err()
        .ok_or("expected a TypeError")?;
    assert!(throws_as(&recorded["marshalling_bare"], &marshalling)?);

    let strict = StrictSessionHandler::new(backend)
        .err()
        .ok_or("expected a LogicException")?;
    assert!(throws_as(&recorded["strict_abstract"], &strict)?);

    let inner = Arc::new(StrictSessionHandler::new(bare())?);
    let strict = StrictSessionHandler::new(inner)
        .err()
        .ok_or("expected a LogicException")?;
    assert!(throws_as(&recorded["strict_strict"], &strict)?);
    Ok(())
}
