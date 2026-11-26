//! `StreamedResponse` against PHP 8.4.27 running the pinned
//! symfony/http-foundation (v8.1.11), recorded by
//! `oracle/recorders/http_foundation_streamed_response.php`
//! (`scripts/record.sh http-foundation-streamed-response`): 1,500
//! generated scripts. Each builds a streamed response from a callback
//! (echoing parts with `flush()` between some, counting its calls, or
//! throwing), a list of chunks or null. Then up to ten calls follow,
//! through every `Response` method the subclass changes: `prepare()`,
//! `setNotModified()`, `isNotModified()`, `sendHeaders()`,
//! `sendContent()`, `send()`, `setContent()`, `getContent()`,
//! `(string)`, `getCallback()` called, `setCallback()`, `setChunks()`,
//! status and header changes. Compared: each call's result, the process
//! calls it made in order (`echo` joined between other calls), then the
//! response's state. A value is compared with `===`; an exception by
//! class and message.
//!
//! Upstream: none

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use php_value::{PhpArray, PhpString, Value, clock, vectors};
use serde_json::{Value as Json, json};
use symfony_http_foundation::{
    exception::{Class, Error},
    header_bag::{HeaderValues, ResponseHeaderBag},
    php::{Globals, Output, OutputCall, set_globals, with_output},
    request::{Parameters, Request},
    response::Response,
    streamed_response::{self, Callback, CallbackOrChunks, Throwable, echo, flush},
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/streamed_response.json");

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn bytes(json: &Json) -> Result<PhpString, TestError> {
    Ok(PhpString::from(vectors::base64(text(json)?)?))
}

fn chunks(json: &Json) -> Result<Vec<PhpString>, TestError> {
    json.as_array().ok_or("chunks")?.iter().map(bytes).collect()
}

/// The recorder's callbacks; `count` shares the script's counter.
fn callback(spec: &Json, counter: &Arc<AtomicUsize>) -> Result<Callback, TestError> {
    Ok(match text(&spec[0])? {
        "count" => {
            let counter = counter.clone();
            Arc::new(move || {
                let n = counter.fetch_add(1, Ordering::SeqCst) + 1;
                echo(format!("call {n}"));
                Ok(())
            })
        }
        "throw" => {
            let before = bytes(&spec[1])?;
            Arc::new(move || {
                echo(before.clone());
                Err(Arc::new(Error::new(Class::PhpRuntime, "boom")) as Throwable)
            })
        }
        "parts" => {
            let parts = spec[1]
                .as_array()
                .ok_or("parts")?
                .iter()
                .map(|part| Ok((bytes(&part[0])?, part[1].as_bool().ok_or("flush")?)))
                .collect::<Result<Vec<_>, TestError>>()?;
            Arc::new(move || {
                for (text, then_flush) in &parts {
                    echo(text.clone());
                    if *then_flush {
                        flush();
                    }
                }
                Ok(())
            })
        }
        other => return Err(format!("unknown callback {other}").into()),
    })
}

fn argument(spec: &Json, counter: &Arc<AtomicUsize>) -> Result<CallbackOrChunks, TestError> {
    Ok(match text(&spec[0])? {
        "null" => CallbackOrChunks::None,
        "chunks" => CallbackOrChunks::Chunks(chunks(&spec[1])?),
        _ => CallbackOrChunks::Callback(callback(spec, counter)?),
    })
}

/// `[method, protocol, If-None-Match]`.
fn request(spec: &Json) -> Result<Request, TestError> {
    let mut server = PhpArray::new();
    server.insert("REQUEST_METHOD", Value::string(text(&spec[0])?));
    if let Some(protocol) = spec[1].as_str() {
        server.insert("SERVER_PROTOCOL", Value::string(protocol));
    }
    if let Some(etag) = spec[2].as_str() {
        server.insert("HTTP_IF_NONE_MATCH", Value::string(etag));
    }
    Ok(Request::new(Parameters {
        server,
        ..Parameters::default()
    })?)
}

/// What a call gave.
enum Got {
    This,
    Value(Value),
    Throws(&'static str, PhpString),
}

fn thrown(e: &(dyn std::error::Error + 'static)) -> Got {
    match e.downcast_ref::<Error>() {
        Some(e) => Got::Throws(e.class().name(), e.message().clone()),
        None => Got::Throws("unknown", PhpString::from(e.to_string())),
    }
}

fn this<T>(result: Result<T, Error>) -> Got {
    match result {
        Ok(_) => Got::This,
        Err(e) => thrown(&e),
    }
}

fn this_or_throwable<T>(result: Result<T, Throwable>) -> Got {
    match result {
        Ok(_) => Got::This,
        Err(e) => thrown(e.as_ref()),
    }
}

fn op(response: &mut Response, step: &Json, counter: &Arc<AtomicUsize>) -> Result<Got, TestError> {
    Ok(match text(&step[0])? {
        "prepare" => this(response.prepare(&mut request(&step[1])?)),
        "is_not_modified" => match response.is_not_modified(&mut request(&step[1])?) {
            Ok(b) => Got::Value(Value::Bool(b)),
            Err(e) => thrown(&e),
        },
        "not_modified" => {
            response.set_not_modified();
            Got::This
        }
        "send_headers" => {
            with_output(|o| o.headers_sent = step[2].as_bool().unwrap_or(false));
            this(response.send_headers(step[1].as_i64()))
        }
        "send_content" => this_or_throwable(response.send_content()),
        "send" => {
            with_output(|o| o.headers_sent = step[2].as_bool().unwrap_or(false));
            this_or_throwable(response.send(step[1].as_bool().ok_or("flush")?))
        }
        "set_content" => {
            let content = match &step[1] {
                Json::Null => None,
                other => Some(bytes(other)?),
            };
            this(response.set_content(content))
        }
        "get_content" => Got::Value(
            response
                .get_content()
                .map_or(Value::Bool(false), |c| Value::String(c.clone())),
        ),
        "string" => Got::Value(Value::String(response.to_php_string())),
        "invoke" => match response
            .stream()
            .and_then(streamed_response::Stream::get_callback)
        {
            None => Got::Value(Value::Null),
            Some(callback) => match callback() {
                Ok(()) => Got::Value(Value::Bool(true)),
                Err(e) => thrown(e.as_ref()),
            },
        },
        "set_callback" => {
            let callback = callback(&step[1], counter)?;
            response
                .stream_mut()
                .ok_or("not streamed")?
                .set_callback(callback);
            Got::This
        }
        "set_chunks" => {
            let chunks = chunks(&step[1])?;
            response
                .stream_mut()
                .ok_or("not streamed")?
                .set_chunks(chunks);
            Got::This
        }
        "status" => this(response.set_status_code(step[1].as_i64().ok_or("status")?, None)),
        "header" => {
            let value = HeaderValues::One(step[2].as_str().map(PhpString::from));
            match response
                .headers_mut()
                .set(text(&step[1])?.as_bytes(), value, true)
            {
                Ok(()) => Got::Value(Value::Null),
                Err(e) => thrown(&e),
            }
        }
        other => return Err(format!("unknown op {other}").into()),
    })
}

/// The process calls as the recorder lists them: consecutive echoes
/// joined, empty ones left out; `ini_set()` when `prepare()` emptied
/// `default_mimetype` (it makes no other call).
fn calls(output: &Output) -> Json {
    use base64::Engine;
    let b64 = |s: &[u8]| Json::String(base64::engine::general_purpose::STANDARD.encode(s));
    let mut list = Vec::new();
    let mut echo: Vec<u8> = Vec::new();
    let end_echo = |list: &mut Vec<Json>, echo: &mut Vec<u8>| {
        if !echo.is_empty() {
            list.push(json!(["echo", b64(echo)]));
            echo.clear();
        }
    };
    for call in &output.calls {
        if let OutputCall::Echo(text) = call {
            echo.extend_from_slice(text.as_bytes());
            continue;
        }
        end_echo(&mut list, &mut echo);
        list.push(match call {
            OutputCall::Header {
                line,
                replace,
                response_code,
            } => json!(["header", b64(line.as_bytes()), replace, response_code]),
            OutputCall::HeaderRemove(name) => json!(["header_remove", b64(name.as_bytes())]),
            OutputCall::HeadersSend(code) => json!(["headers_send", code]),
            OutputCall::ObEndFlush => json!(["ob_end_flush"]),
            OutputCall::ObEndClean => json!(["ob_end_clean"]),
            OutputCall::ObFlush => json!(["ob_flush"]),
            // never made by Response: shown, so a stray one differs
            OutputCall::SetCookie(cookie) => json!(["setcookie", b64(cookie.name.as_bytes())]),
            OutputCall::SessionCookie { name, .. } => {
                json!(["session_cookie", b64(name.as_bytes())])
            }
            OutputCall::Flush => json!(["flush"]),
            OutputCall::FastcgiFinishRequest => json!(["fastcgi_finish_request"]),
            OutputCall::LitespeedFinishRequest => json!(["litespeed_finish_request"]),
            OutputCall::Echo(_) => continue,
        });
    }
    end_echo(&mut list, &mut echo);
    if output.default_mimetype.is_empty() {
        list.push(json!(["ini_set", "default_mimetype", ""]));
    }
    Json::Array(list)
}

fn reader(response: &Response, name: &str) -> Result<Value, TestError> {
    Ok(match name {
        "string" => Value::String(response.to_php_string()),
        "status" => Value::Int(response.get_status_code()),
        "version" => Value::String(response.get_protocol_version().clone()),
        "content" => response
            .get_content()
            .map_or(Value::Bool(false), |c| Value::String(c.clone())),
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
            Value::Array(headers)
        }
        "callback" => Value::Bool(
            response
                .stream()
                .and_then(streamed_response::Stream::get_callback)
                .is_some(),
        ),
        other => return Err(format!("unknown reader {other}").into()),
    })
}

fn compare(recorded: &Json, got: &Got) -> Result<Option<String>, TestError> {
    if recorded[0] == "self" {
        return Ok(match got {
            Got::This => None,
            _ => Some("expected $this".to_owned()),
        });
    }
    if recorded[0] == "throws" {
        return Ok(match got {
            Got::Throws(class, message)
                if *class == text(&recorded[1])?
                    && message.as_bytes() == text(&recorded[2])?.as_bytes() =>
            {
                None
            }
            _ => Some(format!("expected {recorded}")),
        });
    }
    let expected = vectors::decode(recorded)?;
    Ok(match got {
        Got::Value(v) if expected.identical(v) => None,
        Got::Value(v) => Some(format!("expected {expected:?}, got {v:?}")),
        Got::This => Some(format!("expected {expected:?}, got $this")),
        Got::Throws(class, message) => Some(format!(
            "expected {expected:?}, got {class}: {}",
            message.to_string_lossy()
        )),
    })
}

#[test]
fn streamed_responses_answer_as_php() -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let scripts = data["scripts"].as_array().ok_or("scripts")?;
    assert_eq!(1500, scripts.len());
    let _clock = clock::mock(1_700_000_000);
    let mut failures = Vec::new();
    let mut answers = 0;

    for (n, script) in scripts.iter().enumerate() {
        set_globals(Globals {
            output: Output {
                headers_send_exists: true,
                ..Output::default()
            },
            ..Globals::default()
        });
        let counter = Arc::new(AtomicUsize::new(0));
        let new = &script["new"];
        let headers: Vec<(String, String)> = new[2]
            .as_array()
            .ok_or("headers")?
            .iter()
            .map(|pair| Ok((text(&pair[0])?.to_owned(), text(&pair[1])?.to_owned())))
            .collect::<Result<_, TestError>>()?;
        let bag = ResponseHeaderBag::new(headers.iter().map(|(k, v)| (k.as_str(), v.as_str())))?;
        let mut response = streamed_response::new(
            argument(&new[0], &counter)?,
            new[1].as_i64().ok_or("status")?,
            bag,
        )?;

        let ops = script["ops"].as_array().ok_or("ops")?;
        let results = script["results"].as_array().ok_or("results")?;
        for (i, (step, result)) in ops.iter().zip(results).enumerate() {
            with_output(|o| {
                o.calls.clear();
                o.headers_sent = false;
                o.default_mimetype = PhpString::from("text/html");
            });
            let got = op(&mut response, step, &counter)?;
            answers += 1;
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
            let got = Got::Value(reader(&response, name)?);
            if let Some(difference) = compare(&pair[1], &got)? {
                failures.push(format!("script {n} state {name}: {difference}"));
            }
        }
    }
    let total = failures.len();
    failures.truncate(20);
    assert!(
        failures.is_empty(),
        "{total} of {answers} answers differ:\n{}",
        failures.join("\n")
    );
    Ok(())
}
