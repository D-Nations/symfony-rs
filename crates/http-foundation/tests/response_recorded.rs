//! `Response` against PHP 8.4.27 running the pinned symfony/http-foundation
//! (v8.1.11), recorded by `oracle/recorders/http_foundation_response.php`
//! (`scripts/record.sh http-foundation-response`): 1,200 generated
//! scripts of up to ten calls each (status, content, headers, cache
//! directives, dates, `ETag`, Vary, `setCache()` with values of every type,
//! `prepare()` and `isNotModified()` against generated requests, and the
//! send methods), each call's result and the process calls it made
//! (`header()`, `header_remove()`, `headers_send()`, `echo`, `ini_set()`),
//! then the response's state through every reader; and 300
//! `closeOutputBuffers()` calls on generated `ob_get_status()` answers;
//! and 121 `prepare()` calls crossing protocol versions with typed
//! `SERVER_PROTOCOL` values.
//! A value is compared with `===`; an exception by class and message.
//!
//! Upstream: none

use std::sync::Arc;

use php_value::{
    Key, PhpArray, PhpString, Value, clock,
    datetime::{DateTimeImmutable, TimeZone},
    vectors::{self, Outcome},
};
use serde_json::Value as Json;
use symfony_http_foundation::{
    cookie::{Cookie, Options},
    exception::{Class, Error},
    header_bag::{Cookies, HeaderValues, ResponseHeaderBag},
    php::{Globals, Output, OutputBuffer, OutputCall, set_globals, with_output},
    request::{Parameters, Request},
    response::Response,
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/response.json");

const NOW: i64 = 1_700_000_000;

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<PhpString, TestError> {
    Ok(PhpString::from(vectors::base64(text(json)?)?))
}

fn nullable_bytes(json: &Json) -> Result<Option<PhpString>, TestError> {
    match json {
        Json::Null => Ok(None),
        other => Ok(Some(bytes(other)?)),
    }
}

fn int(json: &Json) -> Result<i64, TestError> {
    json.as_i64().ok_or_else(|| "expected an int".into())
}

fn boolean(json: &Json) -> Result<bool, TestError> {
    json.as_bool().ok_or_else(|| "expected a bool".into())
}

/// A recorded header value: null, a string or a list of strings.
fn header_values(value: Value) -> Result<HeaderValues, TestError> {
    Ok(match value {
        Value::Null => HeaderValues::One(None),
        Value::String(s) => HeaderValues::One(Some(s)),
        Value::Array(a) => {
            HeaderValues::Many(a.iter().map(|(_, v)| v.as_string().cloned()).collect())
        }
        other => return Err(format!("unexpected header value {other:?}").into()),
    })
}

fn date(ts: &Json, offset: &Json) -> Result<DateTimeImmutable, TestError> {
    let offset = i32::try_from(int(offset)?)?;
    Ok(DateTimeImmutable::from_timestamp(
        int(ts)?,
        0,
        TimeZone::Offset(offset),
    ))
}

fn nullable_date(ts: &Json, offset: &Json) -> Result<Option<DateTimeImmutable>, TestError> {
    if ts.is_null() {
        Ok(None)
    } else {
        date(ts, offset).map(Some)
    }
}

/// `[server, format]`: `new Request([], [], [], [], [], $server)`, then
/// `setRequestFormat($format)`.
fn request(spec: &Json) -> Result<Request, TestError> {
    let Value::Array(server) = vectors::decode(&spec[0])? else {
        return Err("expected the server array".into());
    };
    let mut request = Request::new(Parameters {
        server,
        ..Parameters::default()
    })?;
    if let Some(format) = spec[1].as_str() {
        request.set_request_format(Some(PhpString::from(format)));
    }
    Ok(request)
}

/// `setCache()`'s options; a `["date", ts]` value is a date in UTC.
fn cache_options(pairs: &Json) -> Result<PhpArray, TestError> {
    let mut options = PhpArray::new();
    for pair in pairs.as_array().ok_or("options")? {
        let key = match vectors::decode(&pair[0])? {
            Value::Int(i) => Key::Int(i),
            Value::String(s) => Key::from_bytes(s.as_bytes()),
            other => return Err(format!("unexpected key {other:?}").into()),
        };
        let value = if pair[1][0] == "date" {
            Value::Object(Arc::new(DateTimeImmutable::from_timestamp(
                int(&pair[1][1])?,
                0,
                TimeZone::Offset(0),
            )))
        } else {
            vectors::decode(&pair[1])?
        };
        options.insert(key, value);
    }
    Ok(options)
}

/// What a call gave: the response itself, a value, or an exception.
enum Got {
    This,
    Value(Value),
    Throws(Error),
}

fn this<T>(result: Result<T, Error>) -> Got {
    match result {
        Ok(_) => Got::This,
        Err(e) => Got::Throws(e),
    }
}

fn value<T: Into<Value>>(result: Result<T, Error>) -> Got {
    match result {
        Ok(v) => Got::Value(v.into()),
        Err(e) => Got::Throws(e),
    }
}

fn op(response: &mut Response, op: &Json) -> Result<Got, TestError> {
    let name = text(&op[0])?;
    Ok(match name {
        "status" => {
            this(response.set_status_code(int(&op[1])?, op[2].as_str().map(PhpString::from)))
        }
        "content" => {
            response.set_content(nullable_bytes(&op[1])?)?;
            Got::This
        }
        "version" => {
            response.set_protocol_version(text(&op[1])?);
            Got::This
        }
        "charset" => {
            response.set_charset(text(&op[1])?);
            Got::This
        }
        "set" => {
            let values = header_values(vectors::decode(&op[2])?)?;
            match response
                .headers_mut()
                .set(text(&op[1])?.as_bytes(), values, boolean(&op[3])?)
            {
                Ok(()) => Got::Value(Value::Null),
                Err(e) => Got::Throws(e),
            }
        }
        "remove" => {
            response.headers_mut().remove(text(&op[1])?);
            Got::Value(Value::Null)
        }
        "private" => this(response.set_private()),
        "public" => this(response.set_public()),
        "immutable" => this(response.set_immutable(boolean(&op[1])?)),
        "date" => this(response.set_date(&date(&op[1], &op[2])?)),
        "expires" => this(response.set_expires(nullable_date(&op[1], &op[2])?.as_ref())),
        "last_modified" => {
            this(response.set_last_modified(nullable_date(&op[1], &op[2])?.as_ref()))
        }
        "expire" => this(response.expire()),
        "max_age" => this(response.set_max_age(int(&op[1])?)),
        "s_maxage" => this(response.set_shared_max_age(int(&op[1])?)),
        "sie" => this(response.set_stale_if_error(int(&op[1])?)),
        "swr" => this(response.set_stale_while_revalidate(int(&op[1])?)),
        "ttl" => this(response.set_ttl(int(&op[1])?)),
        "client_ttl" => this(response.set_client_ttl(int(&op[1])?)),
        "etag" => {
            let etag = op[1].as_str().map(str::as_bytes);
            this(response.set_etag(etag, boolean(&op[2])?))
        }
        "cache" => this(response.set_cache(&cache_options(&op[1])?)),
        "not_modified" => {
            response.set_not_modified();
            Got::This
        }
        "vary" => {
            let values = header_values(vectors::decode(&op[1])?)?;
            this(response.set_vary(values, boolean(&op[2])?))
        }
        "safe" => match response.set_content_safe(boolean(&op[1])?) {
            Ok(()) => Got::Value(Value::Null),
            Err(e) => Got::Throws(e),
        },
        "cookie" => {
            let cookie = Cookie::create(
                text(&op[1])?,
                Options {
                    value: Some(PhpString::from(text(&op[2])?)),
                    ..Options::default()
                },
            )?;
            response.headers_mut().set_cookie(cookie);
            Got::Value(Value::Null)
        }
        "prepare" => this(response.prepare(&mut request(&op[1])?)),
        "is_not_modified" => value(response.is_not_modified(&mut request(&op[1])?)),
        "send_headers" => {
            with_output(|o| o.headers_sent = op[2].as_bool().unwrap_or(false));
            this(response.send_headers(op[1].as_i64()))
        }
        "send_content" => {
            response.send_content()?;
            Got::This
        }
        "send" => {
            with_output(|o| o.headers_sent = op[2].as_bool().unwrap_or(false));
            match response.send(boolean(&op[1])?) {
                Ok(_) => Got::This,
                Err(e) => Got::Throws(e.downcast_ref::<Error>().ok_or("not an Error")?.clone()),
            }
        }
        other => return Err(format!("unknown op {other}").into()),
    })
}

/// The process calls as the recorder lists them: `ini_set()` when
/// `prepare()` emptied `default_mimetype`, the echoed text joined last.
fn calls(output: &Output) -> Json {
    let b64 = |s: &PhpString| Json::String(base64_encode(s.as_bytes()));
    let mut list = Vec::new();
    let mut echo = Vec::new();
    for call in &output.calls {
        match call {
            OutputCall::Header {
                line,
                replace,
                response_code,
            } => list.push(serde_json::json!([
                "header",
                b64(line),
                replace,
                response_code
            ])),
            OutputCall::HeaderRemove(name) => {
                list.push(serde_json::json!(["header_remove", b64(name)]));
            }
            OutputCall::HeadersSend(code) => list.push(serde_json::json!(["headers_send", code])),
            OutputCall::Echo(text) => echo.extend_from_slice(text.as_bytes()),
            OutputCall::ObEndFlush => list.push(serde_json::json!(["ob_end_flush"])),
            OutputCall::ObEndClean => list.push(serde_json::json!(["ob_end_clean"])),
            OutputCall::Flush => list.push(serde_json::json!(["flush"])),
            OutputCall::ObFlush => list.push(serde_json::json!(["ob_flush"])),
            OutputCall::FastcgiFinishRequest => {
                list.push(serde_json::json!(["fastcgi_finish_request"]));
            }
            OutputCall::LitespeedFinishRequest => {
                list.push(serde_json::json!(["litespeed_finish_request"]));
            }
        }
    }
    if output.default_mimetype.is_empty() {
        list.push(serde_json::json!(["ini_set", "default_mimetype", ""]));
    }
    if !echo.is_empty() {
        list.push(serde_json::json!(["echo", base64_encode(&echo)]));
    }
    Json::Array(list)
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn reset_output() {
    with_output(|o| {
        o.calls.clear();
        o.headers_sent = false;
        o.default_mimetype = PhpString::from("text/html");
    });
}

fn timestamp(result: Result<Option<DateTimeImmutable>, Error>) -> Got {
    value(result.map(|date| date.map_or(Value::Null, |d| Value::Int(d.timestamp()))))
}

fn bools(values: &[bool]) -> Value {
    Value::Array(PhpArray::list(values.iter().map(|b| Value::Bool(*b))))
}

fn reader(response: &Response, name: &str) -> Result<Got, TestError> {
    let nullable_int = |v: Option<i64>| v.map_or(Value::Null, Value::Int);
    Ok(match name {
        "string" => Got::Value(Value::String(response.to_php_string())),
        "status" => Got::Value(Value::Int(response.get_status_code())),
        "text" => Got::Value(Value::String(response.status_text().clone())),
        "version" => Got::Value(Value::String(response.get_protocol_version().clone())),
        "charset" => Got::Value(
            response
                .get_charset()
                .cloned()
                .map_or(Value::Null, Value::String),
        ),
        "content" => Got::Value(
            response
                .get_content()
                .map_or(Value::Bool(false), |c| Value::String(c.clone())),
        ),
        "headers" => {
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
            Got::Value(Value::Array(headers))
        }
        "cacheable" => value(response.is_cacheable()),
        "fresh" => value(response.is_fresh()),
        "validateable" => Got::Value(Value::Bool(response.is_validateable())),
        "revalidate" => Got::Value(Value::Bool(response.must_revalidate())),
        "immutable" => Got::Value(Value::Bool(response.is_immutable())),
        "age" => value(response.get_age()),
        "max_age" => value(response.get_max_age().map(nullable_int)),
        "ttl" => value(response.get_ttl().map(nullable_int)),
        "date" => timestamp(response.get_date()),
        "expires" => timestamp(response.get_expires()),
        "last_modified" => timestamp(response.get_last_modified()),
        "etag" => Got::Value(response.get_etag().map_or(Value::Null, Value::String)),
        "vary" => Got::Value(Value::Array(PhpArray::list(
            response.get_vary().into_iter().map(Value::String),
        ))),
        "has_vary" => Got::Value(Value::Bool(response.has_vary())),
        "predicates" => Got::Value(bools(&[
            response.is_invalid(),
            response.is_informational(),
            response.is_successful(),
            response.is_redirection(),
            response.is_client_error(),
            response.is_server_error(),
            response.is_ok(),
            response.is_forbidden(),
            response.is_not_found(),
            response.is_empty(),
            response.is_redirect(None),
            response.is_redirect(Some(b"/a")),
            response.is_redirect(Some(b"")),
        ])),
        "secure" => match response
            .headers()
            .get_cookies(ResponseHeaderBag::COOKIES_FLAT)?
        {
            Cookies::Flat(cookies) => Got::Value(bools(
                &cookies.iter().map(Cookie::is_secure).collect::<Vec<_>>(),
            )),
            Cookies::Array(_) => return Err("flat".into()),
        },
        other => return Err(format!("unknown reader {other}").into()),
    })
}

/// Whether `got` is what PHP recorded; `None` when it is, else the
/// difference.
fn compare(recorded: &Json, got: &Got) -> Result<Option<String>, TestError> {
    if recorded[0] == "self" {
        return Ok(match got {
            Got::This => None,
            Got::Value(v) => Some(format!("expected $this, got {v:?}")),
            Got::Throws(e) => Some(format!("expected $this, got {e:?}")),
        });
    }
    let expected = vectors::outcome(recorded)?;
    Ok(match (&expected, got) {
        (Outcome::Value(e), Got::Value(v)) if e.identical(v) => None,
        (Outcome::Throws { class, message }, Got::Throws(e))
            if e.class().name() == class && e.message().as_bytes() == message.as_bytes() =>
        {
            None
        }
        (expected, Got::This) => Some(format!("expected {expected:?}, got $this")),
        (expected, Got::Value(v)) => Some(format!("expected {expected:?}, got {v:?}")),
        (expected, Got::Throws(e)) => Some(format!("expected {expected:?}, got {e:?}")),
    })
}

#[test]
fn responses_answer_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let scripts = data["scripts"].as_array().ok_or("scripts")?;
    assert_eq!(1200, scripts.len());
    let _clock = clock::mock(NOW);
    let mut failures = Vec::new();
    let mut answers = 0;
    let mut refused = Vec::new();
    'scripts: for (n, script) in scripts.iter().enumerate() {
        set_globals(Globals {
            output: Output {
                headers_send_exists: true,
                ..Output::default()
            },
            ..Globals::default()
        });
        let new = &script["new"];
        let Value::Array(initial) = vectors::decode(&new[2])? else {
            return Err("headers".into());
        };
        let mut headers = Vec::new();
        for (key, value) in initial.iter() {
            let name = key.to_value().to_php_string().unwrap_or_default();
            headers.push((name.as_bytes().to_vec(), header_values(value.clone())?));
        }
        let (content, status) = (nullable_bytes(&new[0])?, int(&new[1])?);
        let created =
            ResponseHeaderBag::new(headers).and_then(|bag| Response::new(content, status, bag));
        let mut response = match (created, script.get("throws")) {
            (Err(e), Some(throws)) => {
                answers += 1;
                if e.class().name() != text(&throws[0])?
                    || e.message().as_bytes() != text(&throws[1])?.as_bytes()
                {
                    failures.push(format!("script {n}: new: expected {throws}, got {e:?}"));
                }
                continue;
            }
            (Ok(_), Some(throws)) => {
                failures.push(format!("script {n}: new: expected {throws}"));
                continue;
            }
            (Err(e), None) => {
                failures.push(format!("script {n}: new: {e:?}"));
                continue;
            }
            (Ok(response), None) => response,
        };

        let ops = script["ops"].as_array().ok_or("ops")?;
        let results = script["results"].as_array().ok_or("results")?;
        for (i, (step, result)) in ops.iter().zip(results).enumerate() {
            reset_output();
            let got = op(&mut response, step)?;
            answers += 1;
            if let Got::Throws(e) = &got
                && e.class() == Class::NotPorted
            {
                // What follows depends on the refused call.
                refused.push(e.message().to_string_lossy().into_owned());
                continue 'scripts;
            }
            if let Some(difference) = compare(&result[0], &got)? {
                failures.push(format!("script {n} op {i} {step}: {difference}"));
            }
            let made = with_output(|o| calls(o));
            if made != result[1] {
                failures.push(format!(
                    "script {n} op {i} {step}: calls: expected {}, got {made}",
                    result[1]
                ));
            }
        }

        for pair in script["state"].as_array().ok_or("state")? {
            let name = text(&pair[0])?;
            answers += 1;
            if let Some(difference) = compare(&pair[1], &reader(&response, name)?)? {
                failures.push(format!("script {n} state {name}: {difference}"));
            }
        }
    }
    // `strtotime()` texts php-value refuses (crates/php-value/tests/dates.rs):
    // the If-Modified-Since `-1`, which PHP reads as a zone offset.
    assert_eq!(vec!["-1"; 2], refused);
    let total = failures.len();
    failures.truncate(30);
    assert!(
        failures.is_empty(),
        "{total} of {answers} answers differ:\n{}",
        failures.join("\n")
    );
    Ok(())
}

#[test]
fn close_output_buffers_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let cases = data["close"].as_array().ok_or("close")?;
    assert_eq!(300, cases.len());
    for (n, case) in cases.iter().enumerate() {
        let buffers = case[0]
            .as_array()
            .ok_or("status")?
            .iter()
            .map(|entry| OutputBuffer {
                flags: entry[0].as_i64(),
                del: entry[1].as_bool(),
            })
            .collect();
        set_globals(Globals {
            output: Output {
                buffers,
                ..Output::default()
            },
            ..Globals::default()
        });
        Response::close_output_buffers(int(&case[1])?, boolean(&case[2])?);
        let made = with_output(|o| calls(o));
        assert_eq!(case[3], made, "case {n}: {case}");
    }
    Ok(())
}

/// `prepare()`'s loose comparisons: a typed `SERVER_PROTOCOL` against
/// `'HTTP/1.0'`, and `'1.0'` against the protocol version (HTTP/1.0's
/// `Pragma` and `Expires`).
#[test]
fn prepare_compares_protocols_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let cases = data["protocols"].as_array().ok_or("protocols")?;
    assert_eq!(121, cases.len());
    let _clock = clock::mock(NOW);
    for case in cases {
        set_globals(Globals::default());
        let mut response = Response::new(Some("foo"), 200, ResponseHeaderBag::empty())?;
        response.set_protocol_version(text(&case[0])?);
        let mut server = PhpArray::new();
        server.insert("SERVER_PROTOCOL", vectors::decode(&case[1])?);
        let mut request = Request::new(Parameters {
            server,
            ..Parameters::default()
        })?;
        response.prepare(&mut request)?;
        let header = |name: &str| {
            response
                .headers()
                .get(name, None)
                .map_or(Value::Null, Value::String)
        };
        assert_eq!(
            text(&case[2])?.as_bytes(),
            response.get_protocol_version().as_bytes(),
            "{case}"
        );
        assert!(
            vectors::decode(&case[3])?.identical(&header("pragma")),
            "{case}"
        );
        assert!(
            vectors::decode(&case[4])?.identical(&header("expires")),
            "{case}"
        );
    }
    Ok(())
}
