//! Session/Storage/Handler/MarshallingSessionHandlerTest.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's;
//! the mocks are [`session_handlers`]'. The serialization tests and the
//! igbinary one are `n/a` (parity manifest): a Rust handler cannot be
//! given to `serialize()`, and igbinary is absent here as in the oracle.
//!
//! Upstream: symfony/http-foundation Tests/Session/Storage/Handler/MarshallingSessionHandlerTest.php
#![allow(non_snake_case)]

#[path = "session_handlers/mod.rs"]
mod session_handlers;

use std::sync::Arc;

use php_value::{PhpString, Value};
use session_handlers::{MockHandler, MockMarshaller, TestResult};
use symfony_http_foundation::{
    exception::{Class, Error},
    session::storage::handler::{
        SessionHandlerInterface, SessionUpdateTimestampHandlerInterface,
        marshalling_session_handler::MarshallingSessionHandler,
    },
};

/// `setUp()`: `createMock(AbstractSessionHandler::class)`.
fn set_up() -> Arc<MockHandler> {
    MockHandler::abstract_handler()
}

fn marshalling(
    handler: &Arc<MockHandler>,
    marshaller: &Arc<MockMarshaller>,
) -> Result<MarshallingSessionHandler, Box<dyn std::error::Error>> {
    Ok(MarshallingSessionHandler::new(
        handler.clone(),
        marshaller.clone(),
    )?)
}

#[test]
fn testOpen() -> TestResult {
    let handler = set_up().returns(true);
    let marshalling_session_handler = marshalling(&handler, &MockMarshaller::new())?;
    marshalling_session_handler
        .open(b"path", b"name")
        .map_err(|x| x.to_string())?;
    assert_eq!(["open path name"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testClose() -> TestResult {
    let handler = set_up().returns(true);
    let marshalling_session_handler = marshalling(&handler, &MockMarshaller::new())?;
    assert!(
        marshalling_session_handler
            .close()
            .map_err(|x| x.to_string())?
    );
    assert_eq!(["close"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testDestroy() -> TestResult {
    let handler = set_up().returns(true);
    let marshalling_session_handler = marshalling(&handler, &MockMarshaller::new())?;
    marshalling_session_handler
        .destroy(b"session_id")
        .map_err(|x| x.to_string())?;
    assert_eq!(["destroy session_id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testGc() -> TestResult {
    let handler = set_up().gcs(Some(1));
    let marshalling_session_handler = marshalling(&handler, &MockMarshaller::new())?;
    marshalling_session_handler
        .gc(4711)
        .map_err(|x| x.to_string())?;
    assert_eq!(["gc 4711"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testRead() -> TestResult {
    let marshaller = MockMarshaller::new().unmarshalls_to(Ok(Value::string("unmarshalled_data")));
    let handler = set_up().reads(Some("data"));
    let marshalling_session_handler = marshalling(&handler, &marshaller)?;

    let result = marshalling_session_handler
        .read(b"session_id")
        .map_err(|x| x.to_string())?;
    assert_eq!(Some(PhpString::from("unmarshalled_data")), result);
    assert_eq!(["read session_id"], handler.calls().as_slice());
    assert_eq!(["unmarshall data"], marshaller.calls().as_slice());
    Ok(())
}

#[test]
fn testReadReturnsEmptyStringWhenUnmarshallingFails() -> TestResult {
    let marshaller = MockMarshaller::new().unmarshalls_to(Err(Arc::new(Error::new(
        Class::PhpDomain,
        "Failed to decrypt value.",
    ))));
    let handler = set_up().reads(Some("data"));
    let marshalling_session_handler = marshalling(&handler, &marshaller)?;

    assert_eq!(
        Some(PhpString::default()),
        marshalling_session_handler
            .read(b"session_id")
            .map_err(|x| x.to_string())?
    );
    assert_eq!(["unmarshall data"], marshaller.calls().as_slice());
    Ok(())
}

#[test]
fn testReadFailure() -> TestResult {
    let marshaller = MockMarshaller::new();
    let handler = set_up().reads(None);
    let marshalling_session_handler = marshalling(&handler, &marshaller)?;

    assert_eq!(
        None,
        marshalling_session_handler
            .read(b"session_id")
            .map_err(|x| x.to_string())?
    );
    assert_eq!(Vec::<String>::new(), marshaller.calls());
    Ok(())
}

#[test]
fn testWrite() -> TestResult {
    let marshaller = MockMarshaller::new().marshalls_to("marshalled_data");
    let handler = set_up();
    let marshalling_session_handler = marshalling(&handler, &marshaller)?;

    marshalling_session_handler
        .write(b"session_id", b"data")
        .map_err(|x| x.to_string())?;
    assert_eq!(
        ["marshall data=data failed=0"],
        marshaller.calls().as_slice()
    );
    assert_eq!(
        ["write session_id marshalled_data"],
        handler.calls().as_slice()
    );
    Ok(())
}

#[test]
fn testWriteDoesNotMarshallEmptyData() -> TestResult {
    let marshaller = MockMarshaller::new();
    let handler = set_up().returns(true);
    let marshalling_session_handler = marshalling(&handler, &marshaller)?;

    assert!(
        marshalling_session_handler
            .write(b"session_id", b"")
            .map_err(|x| x.to_string())?
    );
    assert_eq!(Vec::<String>::new(), marshaller.calls());
    assert_eq!(["write session_id "], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testValidateId() -> TestResult {
    let handler = set_up().returns(true);
    let marshalling_session_handler = marshalling(&handler, &MockMarshaller::new())?;
    marshalling_session_handler
        .validate_id(b"session_id")
        .map_err(|x| x.to_string())?;
    assert_eq!(["validateId session_id"], handler.calls().as_slice());
    Ok(())
}

#[test]
fn testUpdateTimestamp() -> TestResult {
    let handler = set_up().returns(true);
    let marshalling_session_handler = marshalling(&handler, &MockMarshaller::new())?;
    marshalling_session_handler
        .update_timestamp(b"session_id", b"data")
        .map_err(|x| x.to_string())?;
    assert_eq!(
        ["updateTimestamp session_id data"],
        handler.calls().as_slice()
    );
    Ok(())
}
