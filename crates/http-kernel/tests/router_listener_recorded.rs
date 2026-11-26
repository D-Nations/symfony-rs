//! `RouterListener` against PHP 8.4.27 running the pinned
//! symfony/http-kernel (v8.1.11), recorded by
//! `oracle/recorders/http_kernel_router_listener.php`
//! (`scripts/record.sh http-kernel-router-listener`): 2,000 generated
//! scripts. Each builds a listener on a URL matcher, a request matcher or
//! one that is both (with or without a context of its own), runs a chain
//! of one to three nested requests through `onKernelRequest()` and unwinds
//! it with `onKernelFinishRequest()` (given the event or null); the
//! matcher returns generated parameters (any `_route_mapping` shape) or
//! throws. Compared: the constructor's exception, each call's exception
//! (class, message, code, status, headers, previous), the request
//! attributes, the matcher's calls, the logger's calls and the listener's
//! context after every step; then `onKernelException()`: whether it sets a
//! response, its status, and whether its page is the recorded welcome page
//! (hue and project directory taken out).
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::{
    collections::VecDeque,
    fmt::Write as _,
    sync::{Arc, Mutex},
};

use base64::Engine;
use php_value::{Handle, Key, PhpArray, PhpString, Value, vectors};
use psr_log::{Context, ContextValue, LogLevel, LoggerInterface};
use serde_json::{Value as Json, json};
use symfony_http_foundation::{
    request::{Create, Request},
    request_stack::{RequestHandle, RequestStack},
    response::Response,
};
use symfony_http_kernel::{
    controller::ResponseHandle,
    event::{ExceptionEvent, FinishRequestEvent, RequestEvent},
    exception::{HttpArgs, HttpException, PhpClass, PhpException, Throwable, class_of, message_of},
    http_kernel::{HttpKernelInterface, KernelRef},
    router_listener::{
        Matcher, Options, RequestMatcherInterface, RouterListener, UrlMatcherInterface,
    },
};
use symfony_routing::{
    exception::{Class, Error as RoutingError},
    request_context::RequestContext,
};

const DATA: &str = include_str!("data/router_listener.json");

fn b64(bytes: &[u8]) -> Json {
    Json::String(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn unb64(json: &Json) -> Vec<u8> {
    vectors::base64(json.as_str().unwrap()).unwrap()
}

/// `typed_value()` of oracle/recorders/lib/typed.php.
fn typed(value: &Value) -> Json {
    match value {
        Value::Null => json!(["null"]),
        Value::Bool(b) => json!(["bool", b]),
        Value::Int(i) => json!(["int", i]),
        Value::Float(f) => {
            let hex: String = f.to_be_bytes().iter().fold(String::new(), |mut hex, b| {
                let _ = write!(hex, "{b:02x}");
                hex
            });
            json!(["float", hex])
        }
        Value::String(s) => json!(["string", b64(s.as_bytes())]),
        Value::Array(a) => json!(["array", typed_pairs(a)]),
        Value::Object(o) => json!(["unsupported", o.class()]),
    }
}

fn typed_pairs(array: &PhpArray) -> Json {
    Json::Array(
        array
            .iter()
            .map(|(k, v)| json!([typed(&k.to_value()), typed(v)]))
            .collect(),
    )
}

/// What the matcher answers, one result per call.
struct Queue {
    results: Mutex<VecDeque<Json>>,
    calls: Mutex<Vec<Json>>,
}

impl Queue {
    fn next(&self, call: Json) -> Result<PhpArray, Throwable> {
        self.calls.lock().unwrap().push(call);
        let Some(result) = self.results.lock().unwrap().pop_front() else {
            return Ok(PhpArray::new());
        };
        if result[0] == "returns" {
            let Value::Array(parameters) = vectors::decode(&result[1]).unwrap() else {
                panic!("parameters are an array");
            };
            return Ok(parameters);
        }
        let message = || PhpString::from(unb64(&result[2]));
        Err(match result[1].as_str().unwrap() {
            "ResourceNotFound" => Arc::new(RoutingError::new(Class::ResourceNotFound, message())),
            "NoConfiguration" => Arc::new(RoutingError::new(Class::NoConfiguration, "")),
            "MethodNotAllowed" => {
                let methods: Vec<String> = result[2]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| m.as_str().unwrap().to_owned())
                    .collect();
                Arc::new(RoutingError::method_not_allowed(methods, ""))
            }
            "RouteNotFound" => Arc::new(RoutingError::new(Class::RouteNotFound, message())),
            "RuntimeException" => {
                PhpException::new(PhpClass::RuntimeException, message()).throwable()
            }
            other => panic!("unknown result {other}"),
        })
    }
}

struct UrlStub(Handle<RequestContext>, Arc<Queue>);

impl UrlMatcherInterface for UrlStub {
    fn get_context(&self) -> Handle<RequestContext> {
        self.0.clone()
    }

    fn match_path(&self, pathinfo: &[u8]) -> Result<PhpArray, Throwable> {
        self.1.next(b64(pathinfo))
    }
}

struct RequestStub(Option<Handle<RequestContext>>, Arc<Queue>);

impl RequestMatcherInterface for RequestStub {
    fn match_request(&self, _: &RequestHandle) -> Result<PhpArray, Throwable> {
        self.1.next(json!("request"))
    }

    fn get_context(&self) -> Option<Handle<RequestContext>> {
        self.0.clone()
    }
}

#[derive(Default)]
struct Logger(Mutex<Vec<Json>>);

impl LoggerInterface for Logger {
    fn log(&self, level: LogLevel, message: &str, context: Context) {
        let mut array = PhpArray::new();
        for (key, value) in context {
            let value = match value {
                ContextValue::Value(v) => v,
                ContextValue::String(s) => Value::string(s),
                ContextValue::Exception(e) => Value::string(e.to_string()),
            };
            array.insert(Key::from(key), value);
        }
        self.0.lock().unwrap().push(json!([
            level.as_str(),
            b64(message.as_bytes()),
            typed_pairs(&array)
        ]));
    }
}

struct Kernel;

impl HttpKernelInterface for Kernel {
    fn handle(&self, _: &RequestHandle, _: i64, _: bool) -> Result<ResponseHandle, Throwable> {
        Ok(Handle::new(Response::default()))
    }
}

fn context_state(context: &Handle<RequestContext>) -> Json {
    context.with(|c| {
        json!([
            b64(c.base_url().as_bytes()),
            b64(c.path_info().as_bytes()),
            c.method().to_string_lossy(),
            b64(c.host().as_bytes()),
            c.scheme().to_string_lossy(),
            c.http_port(),
            c.https_port(),
            b64(c.query_string().as_bytes()),
        ])
    })
}

fn code_of(e: &(dyn std::error::Error + 'static)) -> i64 {
    if let Some(e) = e.downcast_ref::<HttpException>() {
        return e.get_code();
    }
    if let Some(e) = e.downcast_ref::<PhpException>() {
        return e.get_code();
    }
    0
}

fn throwable(e: &Throwable) -> Json {
    let http = e.downcast_ref::<HttpException>();
    let previous = http
        .and_then(HttpException::get_previous)
        .map(|p| json!([class_of(p.as_ref()), b64(message_of(p.as_ref()).as_bytes())]));
    json!([
        class_of(e.as_ref()),
        b64(message_of(e.as_ref()).as_bytes()),
        code_of(e.as_ref()),
        http.map(HttpException::get_status_code),
        http.map(|h| typed_pairs(h.get_headers())),
        previous,
    ])
}

fn build_request(r: &Json) -> RequestHandle {
    let mut request = Request::create_with(
        unb64(&r["uri"]),
        Create {
            method: Some(PhpString::from(r["method"].as_str().unwrap())),
            ..Create::default()
        },
    )
    .unwrap();
    for header in r["headers"].as_array().unwrap() {
        request
            .headers_mut()
            .set(
                header[0].as_str().unwrap().as_bytes(),
                PhpString::from(unb64(&header[1])),
                true,
            )
            .unwrap();
    }
    if r["preset_controller"].as_bool().unwrap() {
        request.attributes_mut().set("_controller", Value::Null);
    }
    Handle::new(request)
}

fn build_exception(spec: &Json) -> Throwable {
    let previous: Option<Throwable> = match spec[1].as_str() {
        Some("NoConfiguration") => Some(Arc::new(RoutingError::new(Class::NoConfiguration, ""))),
        Some("ResourceNotFound") => Some(Arc::new(RoutingError::new(Class::ResourceNotFound, ""))),
        _ => None,
    };
    let args = HttpArgs {
        previous,
        ..HttpArgs::default()
    };
    match spec[0].as_str().unwrap() {
        "NotFoundHttp" => HttpException::not_found(args),
        "MethodNotAllowedHttp" => HttpException::method_not_allowed(["GET"], args),
        "BadRequestHttp" => HttpException::bad_request(args),
        other => panic!("unknown exception {other}"),
    }
    .throwable()
}

/// The welcome page with the per-response parts taken out, as the
/// recorder's `normalize_welcome()` does.
fn normalize_welcome(page: &[u8]) -> Vec<u8> {
    fn between(page: &[u8], start: &[u8], end: &[u8], with: &[u8]) -> Vec<u8> {
        let Some(at) = page.windows(start.len()).position(|w| w == start) else {
            return page.to_vec();
        };
        let from = at + start.len();
        let Some(len) = page[from..].iter().position(|&b| b == end[0]) else {
            return page.to_vec();
        };
        [&page[..from], with, &page[from + len..]].concat()
    }
    let page = between(page, b"--hue: ", b";", b"@@HUE@@");
    between(
        &page,
        b"class=\"notranslate project_dir_path\">",
        b"<",
        b"@@PROJECT_DIR@@",
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one script, step by step as the recorder runs it"
)]
fn run(script: &Json, welcome: &[u8]) -> Json {
    let queue = Arc::new(Queue {
        results: Mutex::new(
            script["requests"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["result"].clone())
                .collect(),
        ),
        calls: Mutex::new(Vec::new()),
    });
    let ports = (
        script["ports"][0].as_i64().unwrap(),
        script["ports"][1].as_i64().unwrap(),
    );
    let new_context = || {
        let mut c = RequestContext::default();
        c.set_http_port(ports.0).set_https_port(ports.1);
        Handle::new(c)
    };
    let matcher_context = new_context();
    let passed = script["pass_context"].as_bool().unwrap().then(new_context);
    let matcher = match script["matcher"].as_str().unwrap() {
        "url" => Matcher::Url(Arc::new(UrlStub(matcher_context.clone(), queue.clone()))),
        "request" => Matcher::Request(Arc::new(RequestStub(None, queue.clone()))),
        "both" => Matcher::Request(Arc::new(RequestStub(
            Some(matcher_context.clone()),
            queue.clone(),
        ))),
        other => panic!("unknown matcher {other}"),
    };
    let logger = script["logger"]
        .as_bool()
        .unwrap()
        .then(|| Arc::new(Logger::default()));
    let stack = RequestStack::default();
    let mut out = json!({"construct": null, "steps": [], "exception": null});

    let listener = match RouterListener::new(
        matcher,
        stack.clone(),
        Options {
            context: passed.clone(),
            logger: logger.clone().map(|l| l as Arc<dyn LoggerInterface>),
            project_dir: Some(PhpString::from("/work")),
            debug: script["debug"].as_bool().unwrap(),
        },
    ) {
        Ok(listener) => listener,
        Err(e) => {
            out["construct"] = throwable(&e);
            return out;
        }
    };

    let context = passed.unwrap_or(matcher_context);
    let kernel: KernelRef = Arc::new(Kernel);
    let mut requests = Vec::new();
    let mut steps = Vec::new();
    let request_type = |i: usize| {
        if i == 0 {
            <dyn HttpKernelInterface>::MAIN_REQUEST
        } else {
            <dyn HttpKernelInterface>::SUB_REQUEST
        }
    };
    for (i, r) in script["requests"].as_array().unwrap().iter().enumerate() {
        let request = build_request(r);
        requests.push(request.clone());
        stack.push(request.clone());
        let calls = queue.calls.lock().unwrap().len();
        let logs = logger.as_ref().map_or(0, |l| l.0.lock().unwrap().len());
        let mut event = RequestEvent::new(kernel.clone(), request.clone(), Some(request_type(i)));
        let thrown = match listener.on_kernel_request(&mut event) {
            Ok(()) => Json::Null,
            Err(e) => throwable(&e),
        };
        steps.push(json!({
            "thrown": thrown,
            "attributes": request.with(|r| typed(&Value::Array(r.attributes().all().clone()))),
            "calls": queue.calls.lock().unwrap()[calls..].to_vec(),
            "logs": logger.as_ref().map_or_else(Vec::new, |l| l.0.lock().unwrap()[logs..].to_vec()),
            "context": context_state(&context),
        }));
    }
    for i in (0..requests.len()).rev() {
        let result = if script["finish"][i] == "event" {
            let event = FinishRequestEvent::new(
                kernel.clone(),
                requests[i].clone(),
                Some(request_type(i)),
                None,
            );
            listener.on_kernel_finish_request(Some(&event))
        } else {
            listener.on_kernel_finish_request(None)
        };
        let thrown = match result {
            Ok(()) => Json::Null,
            Err(e) => throwable(&e),
        };
        stack.pop();
        steps.push(json!({"thrown": thrown, "context": context_state(&context)}));
    }
    out["steps"] = Json::Array(steps);

    let mut event = ExceptionEvent::new(
        kernel,
        Handle::new(Request::create("/").unwrap()),
        <dyn HttpKernelInterface>::MAIN_REQUEST,
        build_exception(&script["exception"]),
        false,
        None,
    );
    listener.on_kernel_exception(&mut event).unwrap();
    if let Some(response) = event.get_response() {
        let (status, page) = response.with(|r| {
            (
                r.get_status_code(),
                r.get_content()
                    .cloned()
                    .unwrap_or_default()
                    .as_bytes()
                    .to_vec(),
            )
        });
        out["exception"] = json!([status, normalize_welcome(&page) == welcome]);
    }
    out
}

#[test]
fn router_listener_matches_php() {
    let data: Json = serde_json::from_str(DATA).unwrap();
    let welcome = unb64(&data["welcome"]);
    let scripts = data["scripts"].as_array().unwrap();
    assert_eq!(2000, scripts.len());
    let mut failures = Vec::new();
    for (n, script) in scripts.iter().enumerate() {
        let actual = run(script, &welcome);
        if actual != script["out"] {
            failures.push(n);
            if failures.len() <= 3 {
                eprintln!(
                    "script {n}: {}\nexpected {}\nactual   {}",
                    script["requests"], script["out"], actual
                );
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} scripts differ: {:?}",
        failures.len(),
        scripts.len(),
        &failures[..failures.len().min(20)]
    );
}

/// The welcome page the listener serves is the one PHP renders.
#[test]
fn welcome_page_is_upstreams() {
    let data: Json = serde_json::from_str(DATA).unwrap();
    let welcome = unb64(&data["welcome"]);
    let page = symfony_http_kernel::router_listener::welcome_page(0, b"/work/");
    assert_eq!(welcome, normalize_welcome(&page));
}
