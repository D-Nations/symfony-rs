//! Session/Storage/Handler/StrictSessionHandlerTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's;
//! `expects($this->once())->with(...)` is an assertion on the mock's call
//! log ([`session_handlers::MockHandler`]).
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Handler/StrictSessionHandlerTest.php
#![allow(non_snake_case)]

#[path = "session_handlers/mod.rs"]
mod session_handlers;

use std::sync::Arc;

use php_value::PhpString;
use session_handlers::{MockHandler, TestResult};
use symfony_http_foundation::{
    exception::{Class, Error},
    session::storage::handler::{
        SessionHandlerInterface, SessionUpdateTimestampHandlerInterface,
        strict_session_handler::StrictSessionHandler,
    },
    streamed_response::Throwable,
};

fn strict(handler: &Arc<MockHandler>) -> Result<StrictSessionHandler, Box<dyn std::error::Error>> {
    Ok(StrictSessionHandler::new(handler.clone())?)
}

fn s(text: &str) -> PhpString {
    PhpString::from(text)
}

#[test]
fn testOpen() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    let proxy = strict(&handler)?;
    // an instance of \SessionUpdateTimestampHandlerInterface and of AbstractSessionHandler
    assert!(proxy.as_update_timestamp().is_some());
    assert!(proxy.open(b"path", b"name").map_err(|x| x.to_string())?);
    assert_eq!(["open path name"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testCloseSession() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    let proxy = strict(&handler)?;
    assert!(proxy.close().map_err(|x| x.to_string())?);
    assert_eq!(["close"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testValidateIdOK() -> TestResult {
    let handler = MockHandler::bare().reads(Some("data"));
    let proxy = strict(&handler)?;
    assert!(proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    assert_eq!(["read id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testValidateIdKO() -> TestResult {
    let handler = MockHandler::bare().reads(Some(""));
    let proxy = strict(&handler)?;
    assert!(!proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    assert_eq!(["read id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testValidateIdKeepsIdWhenReadFails() -> TestResult {
    let handler = MockHandler::bare().reads(None);
    let proxy = strict(&handler)?;
    assert!(proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    assert_eq!(None, proxy.read(b"id").map_err(|x| x.to_string())?);
    assert_eq!(["read id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testValidateIdKeepsIdWhenReadThrows() -> TestResult {
    let exception: Throwable = Arc::new(Error::new(Class::PhpRuntime, "storage unreachable"));
    let handler = MockHandler::bare().read_throws(exception.clone());
    let proxy = strict(&handler)?;
    assert!(proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    let thrown = proxy.read(b"id").err().ok_or("expected the exception")?;
    // expectExceptionObject(): the same exception
    assert!(Arc::ptr_eq(&exception, &thrown));
    assert_eq!(["read id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testReadFailure() -> TestResult {
    let handler = MockHandler::bare().reads(None);
    let proxy = strict(&handler)?;
    assert_eq!(None, proxy.read(b"id").map_err(|x| x.to_string())?);
    assert_eq!(["read id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testRead() -> TestResult {
    let handler = MockHandler::bare().reads(Some("data"));
    let proxy = strict(&handler)?;
    assert_eq!(
        Some(s("data")),
        proxy.read(b"id").map_err(|x| x.to_string())?
    );
    assert_eq!(["read id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testReadWithValidateIdOK() -> TestResult {
    let handler = MockHandler::bare().reads(Some("data"));
    let proxy = strict(&handler)?;
    assert!(proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    assert_eq!(
        Some(s("data")),
        proxy.read(b"id").map_err(|x| x.to_string())?
    );
    assert_eq!(["read id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testReadWithValidateIdMismatch() -> TestResult {
    let handler = MockHandler::bare()
        .reads(Some("data1"))
        .reads(Some("data2"));
    let proxy = strict(&handler)?;
    assert!(proxy.validate_id(b"id1").map_err(|x| x.to_string())?);
    assert_eq!(
        Some(s("data2")),
        proxy.read(b"id2").map_err(|x| x.to_string())?
    );
    assert_eq!(["read id1", "read id2"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testUpdateTimestamp() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    let proxy = strict(&handler)?;
    assert!(
        proxy
            .update_timestamp(b"id", b"data")
            .map_err(|x| x.to_string())?
    );
    assert_eq!(["write id data"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testWrite() -> TestResult {
    let handler = MockHandler::bare().returns(true);
    let proxy = strict(&handler)?;
    assert!(proxy.write(b"id", b"data").map_err(|x| x.to_string())?);
    assert_eq!(["write id data"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testWriteEmptyNewSession() -> TestResult {
    // open() answers false, then destroy() true
    let handler = MockHandler::bare()
        .reads(Some(""))
        .returns(false)
        .returns(true);
    let proxy = strict(&handler)?;
    proxy.open(b"path", b"name").map_err(|x| x.to_string())?;
    assert!(!proxy.validate_id(b"id").map_err(|x| x.to_string())?);
    assert_eq!(Some(s("")), proxy.read(b"id").map_err(|x| x.to_string())?);
    assert!(proxy.write(b"id", b"").map_err(|x| x.to_string())?);
    let calls = handler.calls();
    assert_eq!(
        ["open path name", "read id", "destroy id"],
        calls.as_slice()
    );
    Ok(())
}

#[test]
fn testWriteEmptyExistingSession() -> TestResult {
    let handler = MockHandler::bare()
        .reads(Some("data"))
        .returns(false)
        .returns(true);
    let proxy = strict(&handler)?;
    proxy.open(b"path", b"name").map_err(|x| x.to_string())?;
    assert_eq!(
        Some(s("data")),
        proxy.read(b"id").map_err(|x| x.to_string())?
    );
    assert!(proxy.write(b"id", b"").map_err(|x| x.to_string())?);
    assert_eq!(
        ["open path name", "read id", "destroy id"],
        handler.calls().as_slice()
    );
    Ok(())
}

#[test]
fn testDestroy() -> TestResult {
    let handler = MockHandler::bare().returns(false).returns(true);
    let proxy = strict(&handler)?;
    proxy.open(b"path", b"name").map_err(|x| x.to_string())?;
    assert!(proxy.destroy(b"id").map_err(|x| x.to_string())?);
    assert_eq!(["open path name", "destroy id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testDestroyNewSession() -> TestResult {
    let handler = MockHandler::bare()
        .reads(Some(""))
        .returns(false)
        .returns(true);
    let proxy = strict(&handler)?;
    proxy.open(b"path", b"name").map_err(|x| x.to_string())?;
    assert_eq!(Some(s("")), proxy.read(b"id").map_err(|x| x.to_string())?);
    assert!(proxy.destroy(b"id").map_err(|x| x.to_string())?);
    assert_eq!(
        ["open path name", "read id", "destroy id"],
        handler.calls().as_slice()
    );
    Ok(())
}

#[test]
fn testDestroyNonEmptyNewSession() -> TestResult {
    let handler = MockHandler::bare()
        .reads(Some(""))
        .returns(false)
        .returns(true)
        .returns(true);
    let proxy = strict(&handler)?;
    proxy.open(b"path", b"name").map_err(|x| x.to_string())?;
    assert_eq!(Some(s("")), proxy.read(b"id").map_err(|x| x.to_string())?);
    assert!(proxy.write(b"id", b"data").map_err(|x| x.to_string())?);
    assert!(proxy.destroy(b"id").map_err(|x| x.to_string())?);
    assert_eq!(
        ["open path name", "read id", "write id data", "destroy id"],
        handler.calls().as_slice()
    );
    Ok(())
}

#[test]
fn testGc() -> TestResult {
    let handler = MockHandler::bare().gcs(Some(1));
    let proxy = strict(&handler)?;
    assert_eq!(Some(1), proxy.gc(123).map_err(|x| x.to_string())?);
    assert_eq!(["gc 123"], handler.calls().as_slice());
    Ok(())
}
