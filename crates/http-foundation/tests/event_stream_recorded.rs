//! `EventStreamResponse` and `ServerEvent` against PHP 8.4.27 running the
//! pinned symfony/http-foundation (v8.1.11), recorded by
//! `oracle/recorders/http_foundation_event_stream.php` (`scripts/record.sh
//! http-foundation-event-stream`): 1,200 generated scripts. Each builds a
//! response from headers (keys in any case), a retry and a callback that
//! yields events, sends them itself (setting the response's retry between
//! some) or none. Then `send()` (once or twice), `getRetry()`,
//! `setRetry()`, `getCallback()` and `(string)` are called, with
//! `connection_aborted()` scripted to turn true after a given event.
//! Compared: each result, the echoed output and `header()` lines, then the
//! headers and the retry. The CLI's `PHP_SAPI` means no flushing here;
//! `tests/event_stream_response_test.rs` pins the other SAPIs' path.
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, clock, vectors};
use serde_json::{Value as Json, json};
use symfony_http_foundation::{
    event_stream_response::{self, EventCallback, Events},
    exception::Error,
    header_bag::HeaderValues,
    php::{Globals, OutputCall, set_globals, with_output},
    response::Response,
    server_event::{EventData, ServerEvent},
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/event_stream.json");

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<PhpString, TestError> {
    Ok(PhpString::from(vectors::base64(text(json)?)?))
}

fn nullable(json: &Json) -> Result<Option<PhpString>, TestError> {
    match json {
        Json::Null => Ok(None),
        other => bytes(other).map(Some),
    }
}

/// `[data, type, retry, id, comment]`.
fn event(spec: &Json) -> Result<ServerEvent, TestError> {
    let data = match text(&spec[0][0])? {
        "lines" => EventData::Lines(
            spec[0][1]
                .as_array()
                .ok_or("lines")?
                .iter()
                .map(bytes)
                .collect::<Result<_, _>>()?,
        ),
        _ => EventData::Text(bytes(&spec[0][1])?),
    };
    Ok(ServerEvent::new(
        data,
        nullable(&spec[1])?,
        spec[2].as_i64(),
        nullable(&spec[3])?,
        nullable(&spec[4])?,
    ))
}

/// The recorder's callback. With `abort_after = k`, `connection_aborted()`
/// is true from the k-th event on; the loop reads it after sending each
/// event, so the flag is set as that event is yielded.
fn callback(new: &Json, abort_after: Option<usize>) -> Result<Option<EventCallback>, TestError> {
    let events: Vec<ServerEvent> = new[3]
        .as_array()
        .ok_or("events")?
        .iter()
        .map(event)
        .collect::<Result<_, _>>()?;
    Ok(match text(&new[2])? {
        "null" => None,
        "yield" => Some(Arc::new(move |_| {
            let events: Events = Box::new(events.clone().into_iter().enumerate().map(
                move |(i, event)| {
                    if abort_after.is_some_and(|k| i + 1 >= k) {
                        with_output(|o| o.connection_aborted = true);
                    }
                    Ok(event)
                },
            ));
            Ok(Some(events))
        })),
        _ => {
            let set_at = new[4]
                .as_array()
                .and_then(|pair| Some((pair[0].as_u64()?, pair[1].as_i64()?)));
            Some(Arc::new(move |response| {
                for (i, event) in events.iter().enumerate() {
                    if let Some((at, retry)) = set_at
                        && u64::try_from(i).is_ok_and(|i| i == at)
                    {
                        response.set_retry(retry);
                    }
                    response.send_event(&mut event.clone());
                }
                Ok(None)
            }))
        }
    })
}

fn headers_value(response: &Response) -> Value {
    let mut headers = PhpArray::new();
    for (name, values) in response.headers().all_preserve_case().iter() {
        headers.insert(
            name.clone(),
            Value::Array(PhpArray::list(
                values
                    .iter()
                    .map(|v| v.clone().map_or(Value::Null, Value::String)),
            )),
        );
    }
    Value::Array(headers)
}

fn retry(response: &Response) -> Result<Value, TestError> {
    let events = response
        .stream()
        .and_then(|s| s.event_stream())
        .ok_or("not an event stream")?;
    Ok(events.get_retry().map_or(Value::Null, Value::Int))
}

/// The echoed bytes and the `header()` lines of the calls made.
fn output() -> (Vec<u8>, Json) {
    use base64::Engine;
    with_output(|o| {
        let mut echo = Vec::new();
        let mut headers = Vec::new();
        for call in &o.calls {
            match call {
                OutputCall::Echo(text) => echo.extend_from_slice(text.as_bytes()),
                OutputCall::Header {
                    line,
                    replace,
                    response_code,
                } => headers.push(json!([
                    "header",
                    base64::engine::general_purpose::STANDARD.encode(line.as_bytes()),
                    replace,
                    response_code
                ])),
                _ => {}
            }
        }
        (echo, Json::Array(headers))
    })
}

#[test]
#[allow(clippy::too_many_lines, reason = "one replay, in the recorder's order")]
fn event_streams_answer_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let scripts = data["scripts"].as_array().ok_or("scripts")?;
    assert_eq!(1200, scripts.len());
    let _clock = clock::mock(1_700_000_000);
    let mut failures = Vec::new();
    let mut answers = 0;

    for (n, script) in scripts.iter().enumerate() {
        set_globals(Globals::default());
        let new = &script["new"];
        let headers: Vec<(PhpString, HeaderValues)> = new[0]
            .as_array()
            .ok_or("headers")?
            .iter()
            .map(|pair| {
                Ok((
                    PhpString::from(text(&pair[0])?),
                    HeaderValues::from(text(&pair[1])?),
                ))
            })
            .collect::<Result<_, TestError>>()?;
        // PHP's array: a key given twice keeps its first position and its
        // last value.
        let mut array: Vec<(PhpString, HeaderValues)> = Vec::new();
        for (key, value) in headers {
            match array.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => array.push((key, value)),
            }
        }
        let abort_after = script["abort_after"]
            .as_u64()
            .and_then(|k| usize::try_from(k).ok());
        let mut response = event_stream_response::new(
            callback(new, abort_after)?,
            200,
            array.iter().map(|(k, v)| (k.as_bytes(), v.clone())),
            new[1].as_i64(),
        )?;

        let ops = script["ops"].as_array().ok_or("ops")?;
        let results = script["results"].as_array().ok_or("results")?;
        for (i, (step, result)) in ops.iter().zip(results).enumerate() {
            with_output(|o| o.calls.clear());
            let mut thrown = None;
            let got = match text(&step[0])? {
                "send" => match response.send(true) {
                    Ok(_) => None,
                    Err(e) => {
                        let e = e.downcast_ref::<Error>().ok_or("not an Error")?;
                        thrown = Some((e.class().name(), e.message().clone()));
                        None
                    }
                },
                "get_retry" => Some(retry(&response)?),
                "set_retry" => {
                    response
                        .stream()
                        .and_then(|s| s.event_stream())
                        .ok_or("not an event stream")?
                        .set_retry(step[1].as_i64().ok_or("retry")?);
                    Some(Value::Null)
                }
                "callback" => Some(Value::Bool(
                    response
                        .stream()
                        .and_then(symfony_http_foundation::streamed_response::Stream::get_callback)
                        .is_some(),
                )),
                "string" => Some(Value::String(response.to_php_string())),
                other => return Err(format!("unknown op {other}").into()),
            };
            answers += 1;
            let (echo, headers) = output();
            let expected_echo = vectors::base64(text(&result[1])?)?;
            // `None` is `$this` (send()).
            let same = match (&got, &thrown) {
                (_, Some((class, message))) => {
                    result[0][0] == "throws"
                        && result[0][1] == *class
                        && text(&result[0][2])?.as_bytes() == message.as_bytes()
                }
                (None, None) => result[0] == json!(["self"]),
                (Some(value), None) => vectors::decode(&result[0])?.identical(value),
            };
            if !same || echo != expected_echo || headers != result[2] {
                failures.push(format!(
                    "script {n} op {i} {step}: expected {} / {:?} / {}, got {got:?} / {:?} / {headers}",
                    result[0],
                    String::from_utf8_lossy(&expected_echo),
                    result[2],
                    String::from_utf8_lossy(&echo),
                ));
            }
        }

        answers += 2;
        if !vectors::decode(&script["headers"])?.identical(&headers_value(&response)) {
            failures.push(format!(
                "script {n} headers: expected {}",
                script["headers"]
            ));
        }
        if !vectors::decode(&script["retry"])?.identical(&retry(&response)?) {
            failures.push(format!("script {n} retry: expected {}", script["retry"]));
        }
    }
    let total = failures.len();
    failures.truncate(15);
    assert!(
        failures.is_empty(),
        "{total} of {answers} answers differ:\n{}",
        failures.join("\n")
    );
    Ok(())
}
