//! Session/Storage/Handler/MigratingSessionHandlerTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's;
//! the mocks are [`session_handlers`]'. The serialization tests are `n/a`
//! (parity manifest): a Rust handler cannot be given to `serialize()`.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Handler/MigratingSessionHandlerTest.php
#![allow(non_snake_case)]

#[path = "session_handlers/mod.rs"]
mod session_handlers;

use std::sync::Arc;

use php_value::PhpString;
use session_handlers::{MockHandler, TestResult};
use symfony_http_foundation::session::storage::handler::{
    SessionHandlerInterface, SessionUpdateTimestampHandlerInterface,
    migrating_session_handler::MigratingSessionHandler,
};

fn dual(
    current: &Arc<MockHandler>,
    write_only: &Arc<MockHandler>,
) -> Result<MigratingSessionHandler, Box<dyn std::error::Error>> {
    Ok(MigratingSessionHandler::new(
        current.clone(),
        write_only.clone(),
    )?)
}

#[test]
fn testInstanceOf() -> TestResult {
    let dual_handler = dual(&MockHandler::bare(), &MockHandler::bare())?;
    // \SessionHandlerInterface and \SessionUpdateTimestampHandlerInterface
    assert!(dual_handler.as_update_timestamp().is_some());
    Ok(())
}

#[test]
fn testClose() -> TestResult {
    let current_handler = MockHandler::bare().returns(true);
    let write_only_handler = MockHandler::bare().returns(false);
    let dual_handler = dual(&current_handler, &write_only_handler)?;

    let result = dual_handler.close().map_err(|x| x.to_string())?;

    assert!(result);
    assert_eq!(["close"], current_handler.calls().as_slice());
    assert_eq!(["close"], write_only_handler.calls().as_slice());
    Ok(())
}

#[test]
fn testDestroy() -> TestResult {
    // open() answers false, then destroy() as the test says
    let current_handler = MockHandler::bare().returns(false).returns(true);
    let write_only_handler = MockHandler::bare().returns(false).returns(false);
    let dual_handler = dual(&current_handler, &write_only_handler)?;
    dual_handler
        .open(b"/path/to/save/location", b"xyz")
        .map_err(|x| x.to_string())?;

    let session_id = b"xyz";

    let result = dual_handler
        .destroy(session_id)
        .map_err(|x| x.to_string())?;

    assert!(result);
    assert_eq!(
        ["open /path/to/save/location xyz", "destroy xyz"],
        current_handler.calls().as_slice()
    );
    assert_eq!(
        ["open /path/to/save/location xyz", "destroy xyz"],
        write_only_handler.calls().as_slice()
    );
    Ok(())
}

#[test]
fn testGc() -> TestResult {
    let maxlifetime = 357;

    let current_handler = MockHandler::bare().gcs(Some(1));
    let write_only_handler = MockHandler::bare().gcs(None);

    let dual_handler = dual(&current_handler, &write_only_handler)?;
    assert_eq!(
        Some(1),
        dual_handler.gc(maxlifetime).map_err(|x| x.to_string())?
    );
    assert_eq!(["gc 357"], current_handler.calls().as_slice());
    assert_eq!(["gc 357"], write_only_handler.calls().as_slice());
    Ok(())
}

#[test]
fn testOpen() -> TestResult {
    let current_handler = MockHandler::bare().returns(true);
    let write_only_handler = MockHandler::bare().returns(false);
    let dual_handler = dual(&current_handler, &write_only_handler)?;

    let result = dual_handler
        .open(b"/path/to/save/location", b"xyz")
        .map_err(|x| x.to_string())?;

    assert!(result);
    assert_eq!(
        ["open /path/to/save/location xyz"],
        current_handler.calls().as_slice()
    );
    assert_eq!(
        ["open /path/to/save/location xyz"],
        write_only_handler.calls().as_slice()
    );
    Ok(())
}

#[test]
fn testRead() -> TestResult {
    let current_handler = MockHandler::bare().reads(Some("something"));
    let write_only_handler = MockHandler::bare();
    let dual_handler = dual(&current_handler, &write_only_handler)?;

    let result = dual_handler.read(b"xyz").map_err(|x| x.to_string())?;

    assert_eq!(Some(PhpString::from("something")), result);
    assert_eq!(["read xyz"], current_handler.calls().as_slice());
    assert_eq!(Vec::<String>::new(), write_only_handler.calls());
    Ok(())
}

#[test]
fn testWrite() -> TestResult {
    let current_handler = MockHandler::bare().returns(true);
    let write_only_handler = MockHandler::bare().returns(false);
    let dual_handler = dual(&current_handler, &write_only_handler)?;

    let result = dual_handler
        .write(b"xyz", b"my-serialized-data")
        .map_err(|x| x.to_string())?;

    assert!(result);
    assert_eq!(
        ["write xyz my-serialized-data"],
        current_handler.calls().as_slice()
    );
    assert_eq!(
        ["write xyz my-serialized-data"],
        write_only_handler.calls().as_slice()
    );
    Ok(())
}

#[test]
fn testValidateId() -> TestResult {
    let current_handler = MockHandler::bare().reads(Some("something"));
    let write_only_handler = MockHandler::bare();
    let dual_handler = dual(&current_handler, &write_only_handler)?;

    let result = dual_handler
        .validate_id(b"xyz")
        .map_err(|x| x.to_string())?;

    assert!(result);
    assert_eq!(["read xyz"], current_handler.calls().as_slice());
    assert_eq!(Vec::<String>::new(), write_only_handler.calls());
    Ok(())
}

#[test]
fn testUpdateTimestamp() -> TestResult {
    let current_handler = MockHandler::bare().returns(true);
    let write_only_handler = MockHandler::bare().returns(false);
    let dual_handler = dual(&current_handler, &write_only_handler)?;

    let result = dual_handler
        .update_timestamp(b"xyz", b"my-serialized-data")
        .map_err(|x| x.to_string())?;

    assert!(result);
    assert_eq!(
        ["write xyz my-serialized-data"],
        current_handler.calls().as_slice()
    );
    assert_eq!(
        ["write xyz my-serialized-data"],
        write_only_handler.calls().as_slice()
    );
    Ok(())
}
