//! EventStreamResponseTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's.
//!
//! A PHP generator callback is a Rust callback returning its events as an
//! iterator; a generator of data lines is a list. What `send()` echoed is
//! read from the recorded output in place of `ob_start()`/`ob_get_clean()`;
//! `iterator_to_array()` on an event is `ServerEvent::parts`.
//!
//! Upstream: symfony/http-foundation Tests/EventStreamResponseTest.php
#![allow(
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::PhpString;
use symfony_http_foundation::{
    event_stream_response::{self, EventCallback, Events},
    header_bag::HeaderValues,
    php::{Globals, OutputCall, set_globals, with_output},
    response::Response,
    server_event::ServerEvent,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn s(text: &str) -> Option<PhpString> {
    Some(PhpString::from(text))
}

fn lines(items: &[&str]) -> Vec<PhpString> {
    items.iter().map(|t| PhpString::from(*t)).collect()
}

/// A generator callback yielding these events.
fn yields(events: Vec<ServerEvent>) -> Option<EventCallback> {
    Some(Arc::new(move |_| {
        let events: Events = Box::new(events.clone().into_iter().map(Ok));
        Ok(Some(events))
    }))
}

fn new(
    callback: Option<EventCallback>,
    retry: Option<i64>,
) -> Result<Response, Box<dyn std::error::Error>> {
    Ok(event_stream_response::new(
        callback,
        200,
        [("", HeaderValues::One(None)); 0],
        retry,
    )?)
}

fn assert_same_response_content(expected: &str, response: &mut Response) -> TestResult {
    set_globals(Globals::default());
    response.send(true)?;
    let actual: Vec<u8> = with_output(|o| {
        o.calls
            .iter()
            .filter_map(|call| match call {
                OutputCall::Echo(text) => Some(text.as_bytes().to_vec()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .concat()
    });

    assert_eq!(expected, String::from_utf8_lossy(&actual));
    Ok(())
}

#[test]
fn testInitializationWithDefaultValues() -> TestResult {
    let response = new(None, None)?;

    assert_eq!(
        s("text/event-stream"),
        response.headers().get("content-type", None)
    );
    assert_eq!(
        s("max-age=0, must-revalidate, no-cache, no-store, private"),
        response.headers().get("cache-control", None)
    );
    assert_eq!(s("keep-alive"), response.headers().get("connection", None));
    assert_eq!(s("?1"), response.headers().get("incremental", None));

    assert_eq!(200, response.get_status_code());
    let events = response
        .stream()
        .and_then(|s| s.event_stream())
        .ok_or("not an event stream")?;
    assert_eq!(None, events.get_retry());
    Ok(())
}

#[test]
fn testPresentOfExpiresHeader() -> TestResult {
    let response = new(None, None)?;

    assert!(response.headers().has("Expires"));
    assert_eq!(s("0"), response.headers().get("Expires", None));
    Ok(())
}

#[test]
fn testStreamSingleEvent() -> TestResult {
    let mut response = new(
        yields(vec![ServerEvent::new(
            "foo",
            s("bar"),
            Some(100),
            s("1"),
            s("bla bla"),
        )]),
        None,
    )?;

    let expected = ": bla bla\nid: 1\nretry: 100\nevent: bar\ndata: foo\n\n";

    assert_same_response_content(expected, &mut response)
}

#[test]
fn testStreamEventsAndData() -> TestResult {
    let data = lines(&["first line", "second line", "third line"]);

    let mut response = new(
        yields(vec![
            ServerEvent::data("single line"),
            ServerEvent::data(lines(&["first line", "second line"])),
            ServerEvent::data(data),
        ]),
        None,
    )?;

    let expected = "data: single line\n\ndata: first line\ndata: second line\n\ndata: first line\ndata: second line\ndata: third line\n\n";

    assert_same_response_content(expected, &mut response)
}

#[test]
fn testStreamEventsWithRetryFallback() -> TestResult {
    let mut response = new(
        yields(vec![
            ServerEvent::data("foo"),
            ServerEvent::data("bar"),
            ServerEvent::new("baz", None, Some(1000), None, None),
        ]),
        Some(1500),
    )?;

    let expected = "retry: 1500\ndata: foo\n\ndata: bar\n\nretry: 1000\ndata: baz\n\n";

    assert_same_response_content(expected, &mut response)
}

#[test]
fn testRetryIsNotSharedBetweenServerEvents() {
    let first = ServerEvent::new("foo", None, Some(3000), None, None);
    let second = ServerEvent::new("bar", None, Some(3000), None, None);

    assert_eq!(
        b"retry: 3000\ndata: foo\n\n".to_vec(),
        first
            .parts()
            .iter()
            .flat_map(|p| p.as_bytes().to_vec())
            .collect::<Vec<u8>>()
    );
    assert_eq!(
        b"retry: 3000\ndata: bar\n\n".to_vec(),
        second
            .parts()
            .iter()
            .flat_map(|p| p.as_bytes().to_vec())
            .collect::<Vec<u8>>()
    );
}

#[test]
fn testRetryIsNotSharedBetweenStreams() -> TestResult {
    let callback = yields(vec![ServerEvent::data("foo"), ServerEvent::data("bar")]);

    let expected = "retry: 2000\ndata: foo\n\ndata: bar\n\n";

    assert_same_response_content(expected, &mut new(callback.clone(), Some(2000))?)?;
    assert_same_response_content(expected, &mut new(callback, Some(2000))?)
}

#[test]
fn testStreamEventWithSendMethod() -> TestResult {
    let callback: EventCallback = Arc::new(|response| {
        response.send_event(&mut ServerEvent::data("foo"));
        Ok(None)
    });
    let mut response = new(Some(callback), None)?;

    assert_same_response_content("data: foo\n\n", &mut response)
}

#[test]
fn testStreamEventWith0Data() -> TestResult {
    let mut response = new(yields(vec![ServerEvent::data("0")]), None)?;

    assert_same_response_content("data: 0\n\n", &mut response)
}

#[test]
fn testStreamEventEmptyStringIgnored() -> TestResult {
    let mut response = new(yields(vec![ServerEvent::data("")]), None)?;

    assert_same_response_content("\n", &mut response)
}

#[test]
fn testStreamEventWithMultilineStringData() -> TestResult {
    let mut response = new(
        yields(vec![ServerEvent::data(
            "first line\nsecond line\rthird line\r\nfourth line",
        )]),
        None,
    )?;

    let expected = "data: first line\ndata: second line\ndata: third line\ndata: fourth line\n\n";

    assert_same_response_content(expected, &mut response)
}

#[test]
fn testStreamEventWithMultilineIterableData() -> TestResult {
    let mut response = new(
        yields(vec![ServerEvent::data(lines(&[
            "first line",
            "second line\nthird line",
        ]))]),
        None,
    )?;

    let expected = "data: first line\ndata: second line\ndata: third line\n\n";

    assert_same_response_content(expected, &mut response)
}

#[test]
fn testStreamEventCannotInjectFieldsThroughData() -> TestResult {
    let mut response = new(
        yields(vec![ServerEvent::new(
            "legit\nevent: adminAlert\ndata: hijacked",
            s("message"),
            None,
            None,
            None,
        )]),
        None,
    )?;

    let expected = "event: message\ndata: legit\ndata: event: adminAlert\ndata: data: hijacked\n\n";

    assert_same_response_content(expected, &mut response)
}

#[test]
fn testStreamEventCannotInjectFieldsThroughIdTypeAndComment() -> TestResult {
    let mut response = new(
        yields(vec![ServerEvent::new(
            "foo",
            s("message\nretry: 1"),
            None,
            s("1\revent: adminAlert"),
            s("bla\r\nbla"),
        )]),
        None,
    )?;

    let expected = ": bla\n: bla\nid: 1event: adminAlert\nevent: messageretry: 1\ndata: foo\n\n";

    assert_same_response_content(expected, &mut response)
}

/// Not upstream: outside the CLI, `sendEvent()` ends the output buffers
/// (`closeOutputBuffers(0, true)`) and calls `flush()` after each part
/// (the oracle's CLI cannot record it: `PHP_SAPI` is a constant).
#[test]
fn send_event_flushes_each_part_outside_the_cli() -> TestResult {
    use symfony_http_foundation::php::{Output, OutputBuffer};
    set_globals(Globals {
        output: Output {
            sapi: PhpString::from("fpm-fcgi"),
            buffers: vec![OutputBuffer {
                flags: Some(112),
                del: None,
            }],
            ..Output::default()
        },
        ..Globals::default()
    });
    let response = new(None, None)?;
    let events = response
        .stream()
        .and_then(|s| s.event_stream())
        .ok_or("not an event stream")?;
    events.send_event(&mut ServerEvent::data("foo"));

    let calls = with_output(|o| o.calls.clone());
    assert_eq!(
        vec![
            OutputCall::Echo(PhpString::default()),
            OutputCall::ObEndFlush,
            OutputCall::Flush,
            OutputCall::Echo(PhpString::from("data: foo\n")),
            OutputCall::Flush,
            OutputCall::Echo(PhpString::from("\n")),
            OutputCall::Flush,
        ],
        calls
    );
    Ok(())
}
