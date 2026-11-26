//! Session/Storage/Proxy/SessionHandlerProxyTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's;
//! the mocks are [`session_handlers`]': a bare handler, and one with
//! `\SessionUpdateTimestampHandlerInterface` (upstream's
//! `TestSessionHandler`). Each data set of the provider is a case of the
//! same test.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Proxy/SessionHandlerProxyTest.php
#![allow(non_snake_case)]

#[path = "session_handlers/mod.rs"]
mod session_handlers;

use std::sync::Arc;

use php_value::PhpArray;
use session_handlers::{MockHandler, TestResult};
use symfony_http_foundation::{
    php_session::SessionHandler,
    session::storage::{
        handler::{
            SessionHandlerInterface, SessionUpdateTimestampHandlerInterface,
            strict_session_handler::StrictSessionHandler,
        },
        native_session_storage::{NativeSessionStorage, SaveHandler},
        proxy::session_handler_proxy::SessionHandlerProxy,
    },
};

fn proxy(handler: &Arc<MockHandler>) -> SessionHandlerProxy {
    SessionHandlerProxy::new(handler.clone())
}

#[test]
fn testOpenTrue() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    let proxy = proxy(&handler);
    assert!(!proxy.is_active());
    proxy.open(b"name", b"id").map_err(|x| x.to_string())?;
    assert!(!proxy.is_active());
    assert_eq!(["open name id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testOpenFalse() -> TestResult {
    let handler = MockHandler::bare().returns(false);
    let proxy = proxy(&handler);
    assert!(!proxy.is_active());
    proxy.open(b"name", b"id").map_err(|x| x.to_string())?;
    assert!(!proxy.is_active());
    assert_eq!(["open name id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testClose() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    let proxy = proxy(&handler);
    assert!(!proxy.is_active());
    proxy.close().map_err(|x| x.to_string())?;
    assert!(!proxy.is_active());
    assert_eq!(["close"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testCloseFalse() -> TestResult {
    let handler = MockHandler::bare().returns(false);
    let proxy = proxy(&handler);
    assert!(!proxy.is_active());
    proxy.close().map_err(|x| x.to_string())?;
    assert!(!proxy.is_active());
    assert_eq!(["close"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testRead() -> TestResult {
    let handler = MockHandler::bare().reads(Some("foo"));
    proxy(&handler).read(b"id").map_err(|x| x.to_string())?;
    assert_eq!(["read id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testWrite() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    assert!(
        proxy(&handler)
            .write(b"id", b"data")
            .map_err(|x| x.to_string())?
    );
    assert_eq!(["write id data"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testDestroy() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    assert!(proxy(&handler).destroy(b"id").map_err(|x| x.to_string())?);
    assert_eq!(["destroy id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testGc() -> TestResult {
    let handler = MockHandler::bare().gcs(Some(1));
    proxy(&handler).gc(86400).map_err(|x| x.to_string())?;
    assert_eq!(["gc 86400"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testValidateIdWithoutUpdateTimestampHandler() -> TestResult {
    let proxy = proxy(&MockHandler::bare());
    assert!(proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    Ok(())
}

#[test]
fn testValidateIdWithUpdateTimestampHandlerAndValidId() -> TestResult {
    let handler = MockHandler::abstract_handler().returns(true);
    let proxy = proxy(&handler);
    assert!(proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    assert_eq!(["validateId id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testValidateIdWithUpdateTimestampHandlerAndInvalidId() -> TestResult {
    let handler = MockHandler::abstract_handler().returns(false);
    let proxy = proxy(&handler);
    assert!(!proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    assert_eq!(["validateId id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testUpdateTimestampWithUpdateTimestampHandler() -> TestResult {
    let handler = MockHandler::abstract_handler().returns(false);
    proxy(&handler)
        .update_timestamp(b"id", b"data")
        .map_err(|x| x.to_string())?;
    assert_eq!(["updateTimestamp id data"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testUpdateTimestampWithoutUpdateTimestampHandler() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    assert!(
        proxy(&handler)
            .update_timestamp(b"id", b"data")
            .map_err(|x| x.to_string())?
    );
    assert_eq!(["write id data"], handler.calls().as_slice());
    Ok(())
}

/// One data set, as a request of its own: in PHPUnit's process the
/// headers are already sent (PHPUnit printed), so no data set registers
/// its handler and `session.save_handler` stays `files` for the next.
fn save_handler_name(handler_type: &str) -> Result<Option<String>, String> {
    let run = || -> Result<Option<String>, Box<dyn std::error::Error>> {
        let handler = match handler_type {
            "strict" => SaveHandler::Handler(Arc::new(StrictSessionHandler::new(Arc::new(
                SessionHandler,
            ))?)),
            "proxy" => SaveHandler::HandlerProxy(Arc::new(SessionHandlerProxy::new(Arc::new(
                StrictSessionHandler::new(Arc::new(SessionHandler))?,
            )))),
            _ => SaveHandler::Handler(Arc::new(SessionHandler)),
        };
        let storage = NativeSessionStorage::new(&PhpArray::new(), Some(handler), None)?;
        Ok(storage.get_save_handler().proxy().get_save_handler_name())
    };
    run().map_err(|e| e.to_string())
}

#[test]
fn testNativeSessionStorageSaveHandlerName() -> TestResult {
    for handler_type in ["native", "strict", "proxy"] {
        let name = std::thread::spawn(move || save_handler_name(handler_type))
            .join()
            .map_err(|_| "the data set panicked")??;
        assert_eq!(Some("files".to_owned()), name, "{handler_type}");
    }
    Ok(())
}
