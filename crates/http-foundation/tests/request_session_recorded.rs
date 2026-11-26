//! Replays `tests/data/request_session.json`: 800 scripts recorded from PHP
//! 8.4.27 by `oracle/recorders/http_foundation_request_session.php`, on
//! `Request`'s and `RequestStack`'s session methods.
//!
//! Each script's operations run on requests that are handles, as PHP's
//! objects are: the stack holds the same requests the script changes.
//! Results are compared one by one, then each factory's call count.
//!
//! Upstream: none

use std::sync::{Arc, Mutex, PoisonError};

use php_value::{Handle, Key, PhpArray, Value};
use serde_json::{Value as Json, json};
use symfony_http_foundation::{
    exception::{Class, Error},
    request::{Create, Duplicate, Request, SessionFactory},
    request_stack::{RequestHandle, RequestStack},
    session::{
        Session, session_interface::SessionInterface,
        storage::mock_array_session_storage::MockArraySessionStorage,
    },
};

type TestError = Box<dyn std::error::Error>;

/// The sessions made so far, labelled `s1`, `s2`... in order.
#[derive(Default)]
struct Sessions(Mutex<Vec<Arc<dyn SessionInterface>>>);

impl Sessions {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Arc<dyn SessionInterface>>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `new Session(new MockArraySessionStorage($name))`.
    fn make(&self, name: &str) -> Result<Arc<dyn SessionInterface>, Error> {
        let session: Arc<dyn SessionInterface> = Arc::new(Session::new(
            Arc::new(MockArraySessionStorage::new(name, None)),
            None,
            None,
            None,
        )?);
        self.lock().push(Arc::clone(&session));
        Ok(session)
    }

    fn label(&self, session: &Arc<dyn SessionInterface>) -> Json {
        let at = self.lock().iter().position(|s| Arc::ptr_eq(s, session));
        at.map_or_else(|| json!("unknown"), |at| json!(format!("s{}", at + 1)))
    }

    fn get(&self, label: &Json) -> Result<Arc<dyn SessionInterface>, TestError> {
        let at = index(label)?.checked_sub(1).ok_or("label 0")?;
        Ok(Arc::clone(self.lock().get(at).ok_or("no such session")?))
    }
}

fn index(value: &Json) -> Result<usize, TestError> {
    Ok(usize::try_from(value.as_u64().ok_or("not an index")?)?)
}

fn text(value: &Json) -> Result<&str, TestError> {
    value.as_str().ok_or_else(|| "not a string".into())
}

fn thrown(error: &Error) -> Json {
    json!([
        "throws",
        error.class().name(),
        error.message().to_string_lossy()
    ])
}

/// A factory of the kind the recorder makes; its calls counted.
fn factory(
    kind: &str,
    name: &str,
    number: usize,
    sessions: &Arc<Sessions>,
    calls: &Arc<Mutex<Vec<u64>>>,
) -> SessionFactory {
    let kind = kind.to_owned();
    let name = name.to_owned();
    let sessions = Arc::clone(sessions);
    let calls = Arc::clone(calls);
    let same: Mutex<Option<Arc<dyn SessionInterface>>> = Mutex::new(None);
    Arc::new(move || {
        if let Some(count) = calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_mut(number)
        {
            *count += 1;
        }
        match kind.as_str() {
            "same" => {
                let mut same = same.lock().unwrap_or_else(PoisonError::into_inner);
                if let Some(session) = &*same {
                    return Ok(Some(Arc::clone(session)));
                }
                let session = sessions.make(&name)?;
                *same = Some(Arc::clone(&session));
                Ok(Some(session))
            }
            "fresh" => Ok(Some(sessions.make(&name)?)),
            "null" => Ok(None),
            _ => Err(Error::new(
                Class::PhpRuntime,
                format!("factory {} failed", number + 1),
            )),
        }
    })
}

fn run(script: &Json) -> Result<(Vec<Json>, Vec<u64>), TestError> {
    let mut cookies = PhpArray::new();
    for (name, value) in script["cookies"].as_object().into_iter().flatten() {
        cookies.insert(Key::from(name.as_str()), Value::string(text(value)?));
    }
    let first = Request::create_with(
        "/",
        Create {
            cookies,
            ..Create::default()
        },
    )?;
    let mut requests: Vec<RequestHandle> = vec![Handle::new(first)];
    let stack = RequestStack::default();
    let sessions = Arc::new(Sessions::default());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut factories: Vec<SessionFactory> = Vec::new();
    let mut results = Vec::new();

    for op in script["ops"].as_array().ok_or("no ops")? {
        let name = text(&op[0])?;
        let request = || -> Result<RequestHandle, TestError> {
            Ok(requests
                .get(index(&op[1])?)
                .ok_or("no such request")?
                .clone())
        };
        let result: Result<Json, Error> = match name {
            "has" => {
                let skip = op[2].as_bool().ok_or("not a bool")?;
                Ok(json!(request()?.with(|r| r.has_session(skip))))
            }
            "get" => request()?
                .with_mut(Request::get_session)
                .map(|s| sessions.label(&s)),
            "prev" => request()?
                .with_mut(Request::has_previous_session)
                .map(|b| json!(b)),
            "set" => {
                let session = if text(&op[2])? == "old" {
                    sessions.get(&op[3])?
                } else {
                    sessions.make(text(&op[3])?)?
                };
                request()?.with_mut(|r| r.set_session(session));
                Ok(Json::Null)
            }
            "factory" => {
                let f = if text(&op[2])? == "old" {
                    let at = index(&op[3])?.checked_sub(1).ok_or("factory 0")?;
                    Arc::clone(factories.get(at).ok_or("no such factory")?)
                } else {
                    let number = factories.len();
                    calls.lock().unwrap_or_else(PoisonError::into_inner).push(0);
                    let f = factory(text(&op[3])?, text(&op[4])?, number, &sessions, &calls);
                    factories.push(Arc::clone(&f));
                    f
                };
                request()?.with_mut(|r| r.set_session_factory(f));
                Ok(Json::Null)
            }
            "dup" => {
                let dup = request()?.with_mut(|r| r.duplicate(Duplicate::default()))?;
                requests.push(Handle::new(dup));
                Ok(json!(requests.len() - 1))
            }
            "clone" => {
                let copy = request()?.with(Clone::clone);
                requests.push(Handle::new(copy));
                Ok(json!(requests.len() - 1))
            }
            "cookie_set" => {
                let key = text(&op[2])?;
                request()?
                    .with_mut(|r| r.cookies_mut().set(key, Value::string("x")))
                    .map(|()| Json::Null)
            }
            "cookie_remove" => {
                let key = text(&op[2])?;
                request()?.with_mut(|r| r.cookies_mut().remove(key));
                Ok(Json::Null)
            }
            "rename" => sessions
                .get(&op[1])?
                .set_name(text(&op[2])?)
                .map(|()| Json::Null),
            "push" => {
                stack.push(request()?);
                Ok(Json::Null)
            }
            "pop" => Ok(json!(stack.pop().is_some())),
            "stack_get" => stack.get_session().map(|s| sessions.label(&s)),
            other => return Err(format!("unknown op {other}").into()),
        };
        results.push(result.unwrap_or_else(|e| thrown(&e)));
    }
    let calls = calls.lock().unwrap_or_else(PoisonError::into_inner).clone();
    Ok((results, calls))
}

#[test]
fn recorded_request_sessions() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(include_str!("data/request_session.json"))?;
    let scripts = data["scripts"].as_array().ok_or("no scripts")?;
    assert_eq!(800, scripts.len());
    let mut failures = Vec::new();
    for (n, script) in scripts.iter().enumerate() {
        let (results, calls) = run(script)?;
        let expected: Vec<u64> = script["calls"]
            .as_array()
            .ok_or("no calls")?
            .iter()
            .map(|c| c.as_u64().ok_or("not a count"))
            .collect::<Result<_, _>>()?;
        if Json::Array(results.clone()) != script["results"] || calls != expected {
            failures.push(format!(
                "script {n}: ops {}\n  php:  {} calls {expected:?}\n  rust: {} calls {calls:?}",
                script["ops"],
                script["results"],
                Json::Array(results),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 800 differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    Ok(())
}
