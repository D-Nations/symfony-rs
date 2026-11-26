//! StreamedResponseTest.php at symfony/http-foundation v8.1.11. Names and
//! assertions are PHPUnit's.
//!
//! A PHP closure echoing is a Rust closure writing through
//! `streamed_response::echo`; what it wrote is read from the recorded
//! output in place of `ob_start()`/`ob_get_clean()`. `++$called` through a
//! reference is an atomic counter.
//!
//! Upstream: symfony/http-foundation Tests/StreamedResponseTest.php
#![allow(non_snake_case)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use php_value::{PhpString, Value};
use symfony_http_foundation::{
    exception::{Class, Error},
    header_bag::ResponseHeaderBag,
    php::{Globals, OutputCall, set_globals, with_output},
    request::{Create, Request},
    response::Response,
    streamed_response::{self, Callback, CallbackOrChunks, echo},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fresh() {
    set_globals(Globals::default());
}

fn echoes(text: &'static str) -> CallbackOrChunks {
    CallbackOrChunks::Callback(Arc::new(move || {
        echo(text);
        Ok(())
    }))
}

fn streamed<const N: usize>(
    content: CallbackOrChunks,
    status: i64,
    headers: [(&str, &str); N],
) -> Result<Response, Error> {
    streamed_response::new(content, status, ResponseHeaderBag::new(headers)?)
}

/// What was echoed, joined.
fn echoed() -> Vec<u8> {
    with_output(|o| {
        o.calls
            .iter()
            .filter_map(|call| match call {
                OutputCall::Echo(text) => Some(text.as_bytes().to_vec()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .concat()
    })
}

fn is_logic_exception(error: &(dyn std::error::Error + 'static)) -> bool {
    error
        .downcast_ref::<Error>()
        .is_some_and(|e| e.class() == Class::PhpLogic)
}

#[test]
fn testConstructor() -> TestResult {
    fresh();
    let response = streamed(echoes("foo"), 404, [("Content-Type", "text/plain")])?;

    assert_eq!(404, response.get_status_code());
    assert_eq!(
        Some(PhpString::from("text/plain")),
        response.headers().get("Content-Type", None)
    );
    Ok(())
}

#[test]
fn testConstructorWithChunks() -> TestResult {
    fresh();
    let chunks = ["foo", "bar", "baz"].map(PhpString::from).to_vec();
    let response = streamed(CallbackOrChunks::Chunks(chunks), 200, [])?;
    let callback = response
        .stream()
        .and_then(streamed_response::Stream::get_callback)
        .ok_or("no callback")?;

    callback()?;

    assert_eq!(b"foobarbaz".to_vec(), echoed());
    Ok(())
}

#[test]
fn testPrepareWith11Protocol() -> TestResult {
    fresh();
    let mut response = streamed(echoes("foo"), 200, [])?;
    let mut request = Request::create("/")?;
    request
        .server_mut()
        .set("SERVER_PROTOCOL", Value::string("HTTP/1.1"));

    response.prepare(&mut request)?;

    assert_eq!(&PhpString::from("1.1"), response.get_protocol_version());
    assert_ne!(
        Some(PhpString::from("chunked")),
        response.headers().get("Transfer-Encoding", None),
        "Apache assumes responses with a Transfer-Encoding header set to chunked to already be encoded."
    );
    Ok(())
}

#[test]
fn testPrepareWith10Protocol() -> TestResult {
    fresh();
    let mut response = streamed(echoes("foo"), 200, [])?;
    let mut request = Request::create("/")?;
    request
        .server_mut()
        .set("SERVER_PROTOCOL", Value::string("HTTP/1.0"));

    response.prepare(&mut request)?;

    assert_eq!(&PhpString::from("1.0"), response.get_protocol_version());
    assert_eq!(None, response.headers().get("Transfer-Encoding", None));
    Ok(())
}

#[test]
fn testPrepareWithHeadRequest() -> TestResult {
    fresh();
    let mut response = streamed(echoes("foo"), 200, [("Content-Length", "123")])?;
    let mut request = Request::create_with(
        "/",
        Create {
            method: Some(PhpString::from("HEAD")),
            ..Create::default()
        },
    )?;

    response.prepare(&mut request)?;

    assert_eq!(
        Some(PhpString::from("123")),
        response.headers().get("Content-Length", None)
    );
    Ok(())
}

#[test]
fn testPrepareWithCacheHeaders() -> TestResult {
    fresh();
    let mut response = streamed(
        echoes("foo"),
        200,
        [("Cache-Control", "max-age=600, public")],
    )?;
    let mut request = Request::create_with(
        "/",
        Create {
            method: Some(PhpString::from("GET")),
            ..Create::default()
        },
    )?;

    response.prepare(&mut request)?;
    assert_eq!(
        Some(PhpString::from("max-age=600, public")),
        response.headers().get("Cache-Control", None)
    );
    Ok(())
}

#[test]
fn testSendContent() -> TestResult {
    fresh();
    let called = Arc::new(AtomicUsize::new(0));
    let counter = called.clone();
    let callback: Callback = Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });

    let mut response = streamed(CallbackOrChunks::Callback(callback), 200, [])?;

    response.send_content()?;
    assert_eq!(1, called.load(Ordering::SeqCst));

    response.send_content()?;
    assert_eq!(1, called.load(Ordering::SeqCst));
    Ok(())
}

#[test]
fn testSendContentWithNonCallable() -> TestResult {
    fresh();
    let mut response = streamed(CallbackOrChunks::None, 200, [])?;
    let Err(e) = response.send_content() else {
        return Err("expected a LogicException".into());
    };
    assert!(is_logic_exception(e.as_ref()));
    Ok(())
}

#[test]
fn testSetContent() -> TestResult {
    fresh();
    let mut response = streamed(echoes("foo"), 200, [])?;
    let Err(e) = response.set_content(Some(PhpString::from("foo"))) else {
        return Err("expected a LogicException".into());
    };
    assert!(is_logic_exception(&e));
    Ok(())
}

#[test]
fn testGetContent() -> TestResult {
    fresh();
    let response = streamed(echoes("foo"), 200, [])?;
    assert_eq!(None, response.get_content());
    Ok(())
}

#[test]
fn testReturnThis() -> TestResult {
    fresh();
    let mut response = streamed(CallbackOrChunks::Callback(Arc::new(|| Ok(()))), 200, [])?;
    assert!(response.send_content()?.stream().is_some());
    assert!(response.send_content()?.stream().is_some());

    let mut response = streamed(CallbackOrChunks::Callback(Arc::new(|| Ok(()))), 200, [])?;
    assert!(response.send_headers(None)?.stream().is_some());
    assert!(response.send_headers(None)?.stream().is_some());
    Ok(())
}

#[test]
fn testSetNotModified() -> TestResult {
    fresh();
    let mut response = streamed(echoes("foo"), 200, [])?;
    let pointer: *const Response = &raw const response;
    let modified = response.set_not_modified();
    assert!(std::ptr::eq(pointer, modified));
    assert_eq!(304, modified.get_status_code());

    modified.send_content()?;
    let string = echoed();
    assert_eq!(Vec::<u8>::new(), string);
    Ok(())
}

#[test]
fn testSendInformationalResponse() -> TestResult {
    fresh();
    let mut response = streamed(CallbackOrChunks::None, 200, [])?;
    response.send_headers(Some(103))?;

    // Informational responses must not override the main status code
    assert_eq!(200, response.get_status_code());

    response.send_headers(None)?;
    Ok(())
}
