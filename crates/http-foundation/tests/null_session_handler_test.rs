//! Session/Storage/Handler/NullSessionHandlerTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's;
//! each test runs on a thread of its own (PHPUnit: a process of its own).
//! The anonymous subclass that records `destroy()` is a handler on
//! `AbstractSessionHandler` with `NullSessionHandler`'s abstract methods.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Handler/NullSessionHandlerTest.php
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex, PoisonError};

use php_value::{PhpArray, PhpString, Value};
use symfony_http_foundation::{
    php_session::{ini_get, set_session_id},
    session::{
        Session,
        session_interface::SessionInterface,
        storage::{
            handler::{
                HandlerResult, SessionHandlerInterface, SessionUpdateTimestampHandlerInterface,
                abstract_session_handler::{AbstractSessionHandler, Backend},
                null_session_handler::NullSessionHandler,
            },
            native_session_storage::{NativeSessionStorage, SaveHandler},
        },
    },
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `getStorage()`.
fn get_storage() -> Result<Arc<NativeSessionStorage>, Box<dyn std::error::Error>> {
    Ok(Arc::new(NativeSessionStorage::new(
        &PhpArray::new(),
        Some(SaveHandler::Handler(
            Arc::new(NullSessionHandler::default()),
        )),
        None,
    )?))
}

#[test]
fn testSaveHandlers() -> TestResult {
    get_storage()?;
    assert_eq!(Some(PhpString::from("user")), ini_get("save_handler"));
    Ok(())
}

#[test]
fn testSession() -> TestResult {
    set_session_id("nullsessionstorage");
    let storage = get_storage()?;
    let session = Session::new(storage, None, None, None)?;
    assert_eq!(Value::Null, session.get("something", Value::Null)?);
    session.set("something", Value::string("unique"))?;
    assert_eq!(
        Value::string("unique"),
        session.get("something", Value::Null)?
    );
    Ok(())
}

#[test]
fn testNothingIsPersisted() -> TestResult {
    set_session_id("nullsessionstorage");
    let storage = get_storage()?;
    let session = Session::new(storage, None, None, None)?;
    session.start()?;
    assert_eq!("nullsessionstorage", session.get_id());
    assert_eq!(Value::Null, session.get("something", Value::Null)?);
    Ok(())
}

/// `new class extends NullSessionHandler { ... destroy() ... }`.
#[derive(Debug, Default)]
struct RecordingHandler {
    parent: AbstractSessionHandler,
    null: NullSessionHandler,
    destroyed_ids: Mutex<Vec<String>>,
}

impl RecordingHandler {
    fn destroyed_ids(&self) -> Vec<String> {
        self.destroyed_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Backend for RecordingHandler {
    fn class(&self) -> String {
        "NullSessionHandler@anonymous".to_owned()
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        self.parent.read(self, session_id)
    }

    fn do_read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        Backend::do_read(&self.null, session_id)
    }

    fn do_write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        Backend::do_write(&self.null, session_id, data)
    }

    fn do_destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        Backend::do_destroy(&self.null, session_id)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.destroyed_ids
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(String::from_utf8_lossy(session_id).into_owned());
        self.parent.destroy(self, session_id)
    }
}

impl SessionHandlerInterface for RecordingHandler {
    fn class(&self) -> String {
        Backend::class(self)
    }

    fn open(&self, save_path: &[u8], session_name: &[u8]) -> HandlerResult<bool> {
        Ok(self.parent.open(save_path, session_name))
    }

    fn close(&self) -> HandlerResult<bool> {
        self.null.close()
    }

    fn read(&self, session_id: &[u8]) -> HandlerResult<Option<PhpString>> {
        Backend::read(self, session_id)
    }

    fn write(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.parent.write(self, session_id, data)
    }

    fn destroy(&self, session_id: &[u8]) -> HandlerResult<bool> {
        Backend::destroy(self, session_id)
    }

    fn gc(&self, max_lifetime: i64) -> HandlerResult<Option<i64>> {
        self.null.gc(max_lifetime)
    }
}

impl SessionUpdateTimestampHandlerInterface for RecordingHandler {
    fn validate_id(&self, session_id: &[u8]) -> HandlerResult<bool> {
        self.null.validate_id(session_id)
    }

    fn update_timestamp(&self, session_id: &[u8], data: &[u8]) -> HandlerResult<bool> {
        self.parent.update_timestamp(self, session_id, data)
    }
}

#[test]
fn testUpdateTimestampWithEmptyDataDestroysSession() -> TestResult {
    let handler = RecordingHandler::default();
    SessionHandlerInterface::open(&handler, b"", b"sid").map_err(|x| x.to_string())?;
    assert_eq!(
        Some(PhpString::default()),
        SessionHandlerInterface::read(&handler, b"id").map_err(|x| x.to_string())?
    );
    assert!(
        handler
            .update_timestamp(b"id", b"data")
            .map_err(|x| x.to_string())?
    );
    assert_eq!(Vec::<String>::new(), handler.destroyed_ids());
    assert!(
        handler
            .update_timestamp(b"id", b"")
            .map_err(|x| x.to_string())?
    );
    assert_eq!(["id"], handler.destroyed_ids().as_slice());
    Ok(())
}
