//! `ResponseListener` against PHP 8.4.27 running the pinned
//! symfony/http-kernel (v8.1.11), recorded by
//! `oracle/recorders/http_kernel_response_listener.php`
//! (`scripts/record.sh http-kernel-response-listener`): 1,500 generated
//! scripts. Each builds a listener (a charset, with or without the
//! `Content-Language` header), a request (URI, method, `SERVER_PROTOCOL`,
//! a locale, a request format, a `_vary_by_language` attribute of any
//! type) and a response (content, status, headers, its own charset or
//! none), and dispatches a `ResponseEvent` for a main or sub request.
//! Compared: the exception, or the response's status, protocol version,
//! charset, content and headers in order (`Date` left out: the clock).
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::sync::Arc;

use base64::Engine;
use php_value::{Handle, PhpArray, PhpString, Value, vectors};
use serde_json::{Value as Json, json};
use symfony_event_dispatcher::{
    ContractsEventDispatcherInterface as _, EventDispatcher, EventDispatcherInterface, Listener,
};
use symfony_http_foundation::{
    header_bag::ResponseHeaderBag,
    request::{Create, Request},
    request_stack::RequestHandle,
    response::Response,
};
use symfony_http_kernel::{
    controller::ResponseHandle,
    event::ResponseEvent,
    exception::{Throwable, class_of, from_box, message_of},
    http_kernel::{HttpKernelInterface, KernelEvents},
    response_listener::ResponseListener,
};

struct StubKernel;

impl HttpKernelInterface for StubKernel {
    fn handle(&self, _: &RequestHandle, _: i64, _: bool) -> Result<ResponseHandle, Throwable> {
        Ok(Handle::new(Response::default()))
    }
}

fn text(json: &Json) -> &str {
    json.as_str().expect("a string")
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn run(script: &Json) -> Json {
    let dispatcher = EventDispatcher::new();
    let listener = &script["listener"];
    let object: symfony_event_dispatcher::Object = Arc::new(ResponseListener::new(
        text(&listener["charset"]),
        listener["content_language"].as_bool().unwrap(),
    ));
    dispatcher
        .add_listener(
            KernelEvents::RESPONSE,
            Listener::method(&object, "onKernelResponse"),
            0,
        )
        .unwrap();

    let r = &script["request"];
    let mut server = PhpArray::new();
    server.insert("SERVER_PROTOCOL", Value::string(text(&r["protocol"])));
    let mut request = Request::create_with(
        text(&r["uri"]),
        Create {
            method: Some(PhpString::from(text(&r["method"]))),
            server,
            ..Create::default()
        },
    )
    .unwrap();
    if let Some(locale) = r["locale"].as_str() {
        request.set_locale(locale);
    }
    if let Some(format) = r["format"].as_str() {
        request.set_request_format(Some(PhpString::from(format)));
    }
    if !r["vary"].is_null() {
        let vary = vectors::decode(&r["vary"]).unwrap();
        request.attributes_mut().set("_vary_by_language", vary);
    }

    let p = &script["response"];
    let mut response = Response::new(
        Some(text(&p["content"])),
        p["status"].as_i64().unwrap(),
        ResponseHeaderBag::empty(),
    )
    .unwrap();
    for pair in p["headers"].as_array().unwrap() {
        response
            .headers_mut()
            .set(text(&pair[0]).as_bytes(), text(&pair[1]), true)
            .unwrap();
    }
    if let Some(charset) = p["charset"].as_str() {
        response.set_charset(charset);
    }

    let request_type = if script["main"].as_bool().unwrap() {
        <dyn HttpKernelInterface>::MAIN_REQUEST
    } else {
        <dyn HttpKernelInterface>::SUB_REQUEST
    };
    let mut event = ResponseEvent::new(
        Arc::new(StubKernel),
        Handle::new(request),
        request_type,
        Handle::new(response),
        None,
    );
    if let Err(e) = dispatcher.dispatch(&mut event, Some(KernelEvents::RESPONSE)) {
        let e = from_box(e);
        return json!({"throws": [class_of(e.as_ref()), lossy(message_of(e.as_ref()).as_bytes())]});
    }

    event.get_response().with(|response| {
        let headers: Vec<Json> = response
            .headers()
            .all_preserve_case_without_cookies()
            .iter()
            .filter(|(name, _)| name.bytes().as_ref() != b"Date")
            .map(|(name, values)| {
                let values: Vec<Json> = values
                    .iter()
                    .map(|v| {
                        v.as_ref()
                            .map_or(Json::Null, |v| json!(lossy(v.as_bytes())))
                    })
                    .collect();
                json!([lossy(&name.bytes()), values])
            })
            .collect();
        json!({
            "status": response.get_status_code(),
            "version": lossy(response.get_protocol_version().as_bytes()),
            "charset": response.get_charset().map(|c| lossy(c.as_bytes())),
            "content": base64::engine::general_purpose::STANDARD
                .encode(response.get_content().cloned().unwrap_or_default().as_bytes()),
            "headers": headers,
        })
    })
}

#[test]
fn every_recorded_script_matches() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/response_listener.json"
    );
    let data: Json = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let scripts = data["scripts"].as_array().unwrap();
    assert_eq!(scripts.len(), 1500);
    let mut failures = Vec::new();
    for (at, case) in scripts.iter().enumerate() {
        let actual = run(&case["script"]);
        if actual != case["outcome"] {
            failures.push(format!(
                "script {at}: {}\n  Rust {actual}\n  PHP  {}",
                case["script"], case["outcome"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        scripts.len(),
        failures
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
