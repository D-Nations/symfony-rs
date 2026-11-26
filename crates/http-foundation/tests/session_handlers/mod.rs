//! The session handler tests' mocks: PHPUnit's `createMock()` of a bare
//! `\SessionHandlerInterface` (`Tests\Fixtures\BareSessionHandler`) or of
//! `AbstractSessionHandler`, and of `MarshallerInterface`.
//!
//! A mock logs each call with its arguments (`"read id"`) and answers
//! from a queue per method, PHPUnit's default (`false`, `''`, `0`) when
//! the queue is empty. A test asserts the log, as `expects(...)->with()`
//! does.
//!
//! Upstream: symfony/http-foundation Tests/Fixtures/BareSessionHandler.php
#![allow(dead_code)]

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, PoisonError},
};

use php_value::{PhpArray, PhpString, Value};
use symfony_http_foundation::{
    session::storage::handler::{
        HandlerResult, SessionHandlerInterface, SessionUpdateTimestampHandlerInterface,
        marshaller_interface::MarshallerInterface,
    },
    streamed_response::Throwable,
};

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[derive(Debug, Default)]
struct Answers {
    bools: VecDeque<bool>,
    reads: VecDeque<HandlerResult<Option<PhpString>>>,
    gcs: VecDeque<Option<i64>>,
}

/// A mocked handler; `timestamp` makes it an `AbstractSessionHandler`
/// mock (with `validateId()` and `updateTimestamp()`).
#[derive(Debug, Default)]
pub struct MockHandler {
    timestamp: bool,
    calls: Mutex<Vec<String>>,
    answers: Mutex<Answers>,
}

impl MockHandler {
    /// `createMock(BareSessionHandler::class)`.
    pub fn bare() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// `createMock(AbstractSessionHandler::class)`.
    pub fn abstract_handler() -> Arc<Self> {
        Arc::new(Self {
            timestamp: true,
            ..Self::default()
        })
    }

    /// `->method(...)->willReturn($bool)` for the next bool answer.
    pub fn returns(self: &Arc<Self>, answer: bool) -> Arc<Self> {
        lock(&self.answers).bools.push_back(answer);
        Arc::clone(self)
    }

    /// `->method('read')->willReturn($data)` (`None` is `false`).
    pub fn reads(self: &Arc<Self>, data: Option<&str>) -> Arc<Self> {
        lock(&self.answers)
            .reads
            .push_back(Ok(data.map(PhpString::from)));
        Arc::clone(self)
    }

    /// `->method('read')->willThrowException($e)`.
    pub fn read_throws(self: &Arc<Self>, error: Throwable) -> Arc<Self> {
        lock(&self.answers).reads.push_back(Err(error));
        Arc::clone(self)
    }

    /// `->method('gc')->willReturn($n)`.
    pub fn gcs(self: &Arc<Self>, answer: Option<i64>) -> Arc<Self> {
        lock(&self.answers).gcs.push_back(answer);
        Arc::clone(self)
    }

    /// The calls made, in order.
    pub fn calls(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }

    fn log(&self, call: String) {
        lock(&self.calls).push(call);
    }

    fn bool(&self) -> bool {
        lock(&self.answers).bools.pop_front().unwrap_or(false)
    }
}

impl SessionHandlerInterface for MockHandler {
    fn class(&self) -> String {
        "MockObject_BareSessionHandler".to_owned()
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        self.log(format!("open {} {}", text(save_path), text(session_name)));
        Ok(self.bool())
    }

    fn close(&self) -> HandlerResult<bool> {
        self.log("close".to_owned());
        Ok(self.bool())
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.log(format!("read {}", text(session_id)));
        lock(&self.answers)
            .reads
            .pop_front()
            .unwrap_or_else(|| Ok(Some(PhpString::default())))
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.log(format!("write {} {}", text(session_id), text(data)));
        Ok(self.bool())
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.log(format!("destroy {}", text(session_id)));
        Ok(self.bool())
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        self.log(format!("gc {max_lifetime}"));
        Ok(lock(&self.answers).gcs.pop_front().unwrap_or(Some(0)))
    }

    fn as_update_timestamp(&self) -> Option<&dyn SessionUpdateTimestampHandlerInterface> {
        self.timestamp
            .then_some(self as &dyn SessionUpdateTimestampHandlerInterface)
    }
}

impl SessionUpdateTimestampHandlerInterface for MockHandler {
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.log(format!("validateId {}", text(session_id)));
        Ok(self.bool())
    }

    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.log(format!(
            "updateTimestamp {} {}",
            text(session_id),
            text(data)
        ));
        Ok(self.bool())
    }
}

/// `createMock(MarshallerInterface::class)`.
#[derive(Debug, Default)]
pub struct MockMarshaller {
    calls: Mutex<Vec<String>>,
    marshalled: Mutex<VecDeque<PhpArray>>,
    unmarshalled: Mutex<VecDeque<HandlerResult<Value>>>,
}

impl MockMarshaller {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn marshalls_to(self: &Arc<Self>, data: &str) -> Arc<Self> {
        let mut array = PhpArray::new();
        array.insert("data", Value::string(data));
        lock(&self.marshalled).push_back(array);
        Arc::clone(self)
    }

    pub fn unmarshalls_to(self: &Arc<Self>, answer: HandlerResult<Value>) -> Arc<Self> {
        lock(&self.unmarshalled).push_back(answer);
        Arc::clone(self)
    }

    pub fn calls(&self) -> Vec<String> {
        lock(&self.calls).clone()
    }
}

impl MarshallerInterface for MockMarshaller {
    fn marshall(&self, values: &PhpArray, failed: &mut PhpArray) -> HandlerResult<PhpArray> {
        let shown: Vec<String> = values
            .iter()
            .map(|(k, v)| {
                format!(
                    "{}={}",
                    text(k.to_value().to_php_string().unwrap_or_default().as_bytes()),
                    text(v.to_php_string().unwrap_or_default().as_bytes())
                )
            })
            .collect();
        lock(&self.calls).push(format!(
            "marshall {} failed={}",
            shown.join(","),
            failed.len()
        ));
        Ok(lock(&self.marshalled).pop_front().unwrap_or_default())
    }

    fn unmarshall(&self, value: &PhpString) -> HandlerResult<Value> {
        lock(&self.calls).push(format!("unmarshall {}", text(value.as_bytes())));
        lock(&self.unmarshalled)
            .pop_front()
            .unwrap_or(Ok(Value::Null))
    }
}
