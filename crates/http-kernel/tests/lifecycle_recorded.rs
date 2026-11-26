//! `HttpKernel` against PHP 8.4.27 running the pinned symfony/http-kernel
//! (v8.1.11), recorded by `oracle/recorders/http_kernel_lifecycle.php`
//! (`scripts/record.sh http-kernel-lifecycle`): 1,500 generated scripts,
//! each a controller (a response, another value, a throwable or none), an
//! argument resolver, `$catch` and `$handleAllThrowables`, and up to six
//! prioritized listeners that log what their event shows and act on it.
//! Compared: every listener call in order with what it saw, `handle()`'s
//! response (status, text, content, headers without Date) or throwable
//! (class, message, previous class), the request stack afterwards, then
//! `terminate()` or `terminateWithException()` with their listener calls,
//! result and output; and a throwable from `terminate()` given back to
//! `terminateWithException()`, which knows it as its own.
//!
//! Each script runs twice: on the plain `EventDispatcher`, and on a
//! `CompiledEventDispatcher` holding the same listeners (the listener seam:
//! priority, equal-priority order, propagation, controller replacement,
//! early response, exceptions and response filtering in both modes).
//!
//! Upstream: none

use std::sync::{Arc, Mutex};

use base64::Engine;
use php_value::{Handle, Key, PhpArray, PhpString, Value, clock, vectors};
use serde_json::{Value as Json, json};
use symfony_event_dispatcher::{
    CompiledEventDispatcher, ContractsEventDispatcherInterface, EventDispatcher,
    EventDispatcherInterface, EventObject, Listener, ListenerMap,
};
use symfony_http_foundation::{
    exception::{Class, Error as FoundationError},
    header_bag::ResponseHeaderBag,
    php::{Globals, OutputCall, set_globals, with_output},
    request::Request,
    request_stack::{RequestHandle, RequestStack},
    response::Response,
};
use symfony_http_kernel::{
    controller::{
        ArgumentResolverInterface, Controller, ControllerResolverInterface, ControllerResult,
        Parameter, ResponseHandle,
    },
    event::{
        ControllerArgumentsEvent, ControllerEvent, ControllerMetadata, ExceptionEvent,
        FinishRequestEvent, RequestEvent, ResponseEvent, TerminateEvent, ViewEvent,
    },
    exception::{HttpArgs, HttpException, PhpClass, PhpException, Throwable, class_of, message_of},
    http_kernel::{HttpKernel, HttpKernelInterface, KernelEvents, Options, TerminableInterface},
};

type TestError = Box<dyn std::error::Error + Send + Sync>;

const DATA: &str = include_str!("data/lifecycle.json");

fn b64(bytes: &[u8]) -> Json {
    Json::String(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// `typed_value()` of oracle/recorders/lib/typed.php.
fn typed(value: &Value) -> Json {
    match value {
        Value::Null => json!(["null"]),
        Value::Bool(b) => json!(["bool", b]),
        Value::Int(i) => json!(["int", i]),
        Value::Float(f) => json!(["float", format!("{:016x}", f.to_bits())]),
        Value::String(s) => json!(["string", b64(s.as_bytes())]),
        Value::Array(a) => json!(["array", pairs(a)]),
        Value::Object(o) => json!(["object", o.class(), pairs(&o.properties())]),
    }
}

fn pairs(array: &PhpArray) -> Json {
    Json::Array(
        array
            .iter()
            .map(|(k, v)| json!([typed(&k.to_value()), typed(v)]))
            .collect(),
    )
}

fn list(values: &[Value]) -> Json {
    typed(&Value::Array(PhpArray::list(values.iter().cloned())))
}

fn text(json: &Json) -> Result<&str, TestError> {
    json.as_str().ok_or_else(|| "expected a string".into())
}

fn int(json: &Json) -> Result<i64, TestError> {
    json.as_i64().ok_or_else(|| "expected an int".into())
}

/// A PHP array from its JSON form (an object, or `[]`).
fn php_array(json: &Json) -> Result<PhpArray, TestError> {
    let mut array = PhpArray::new();
    if let Some(object) = json.as_object() {
        for (key, value) in object {
            let value = match value {
                Json::String(s) => Value::string(s.as_str()),
                Json::Number(n) => Value::Int(n.as_i64().ok_or("int")?),
                Json::Array(items) => Value::Array(PhpArray::list(
                    items
                        .iter()
                        .map(|i| Ok(Value::string(text(i)?)))
                        .collect::<Result<Vec<_>, TestError>>()?,
                )),
                other => return Err(format!("header value {other}").into()),
            };
            array.insert(Key::from_bytes(key.as_bytes()), value);
        }
    }
    Ok(array)
}

/// The recorder's `$build($spec)`.
fn build(spec: &Json) -> Result<Throwable, TestError> {
    let php = |class, message: &Json| -> Result<Throwable, TestError> {
        Ok(PhpException::new(class, text(message)?).throwable())
    };
    Ok(match text(&spec[0])? {
        "runtime" => php(PhpClass::RuntimeException, &spec[1])?,
        "type_error" => php(PhpClass::TypeError, &spec[1])?,
        "error" => php(PhpClass::Error, &spec[1])?,
        "logic" => php(PhpClass::LogicException, &spec[1])?,
        "http" => HttpException::from_status_code(
            int(&spec[1])?,
            HttpArgs {
                message: PhpString::from(text(&spec[2])?),
                headers: php_array(&spec[3])?,
                ..HttpArgs::default()
            },
        )
        .throwable(),
        "method_not_allowed" => {
            let methods: Vec<String> = spec[1]
                .as_array()
                .ok_or("methods")?
                .iter()
                .map(|m| Ok(text(m)?.to_owned()))
                .collect::<Result<_, TestError>>()?;
            HttpException::method_not_allowed(methods, HttpArgs::default()).throwable()
        }
        "suspicious" => Arc::new(FoundationError::new(
            Class::SuspiciousOperation,
            text(&spec[1])?,
        )),
        other => return Err(format!("unknown exception {other}").into()),
    })
}

/// `[$e::class, $e->getMessage(), $e->getPrevious()::class]`.
fn describe(e: &Throwable) -> Json {
    let previous = e.source().map(class_of);
    json!([
        class_of(e.as_ref()),
        message_of(e.as_ref()).to_string_lossy(),
        previous
    ])
}

fn kind(metadata: Option<&ControllerMetadata>) -> Json {
    match metadata {
        None => Json::Null,
        Some(ControllerMetadata::Controller(_)) => json!("controller"),
        Some(ControllerMetadata::Arguments(_)) => json!("arguments"),
    }
}

/// The recorder's `$state($response)`.
fn state(response: &ResponseHandle) -> Json {
    response.with(|r| {
        let mut headers = PhpArray::new();
        let mut all: Vec<(Key, Value)> = r
            .headers()
            .all_preserve_case()
            .iter()
            .filter(|(k, _)| {
                k.to_value()
                    .to_php_string()
                    .is_none_or(|n| n.as_bytes() != b"Date")
            })
            .map(|(k, values)| {
                (
                    k.clone(),
                    Value::Array(PhpArray::list(
                        values
                            .iter()
                            .map(|v| v.clone().map_or(Value::Null, Value::String)),
                    )),
                )
            })
            .collect();
        all.sort_by(|(a, _), (b, _)| php_value::array::compare_keys(a, b));
        for (k, v) in all {
            headers.insert(k, v);
        }
        json!([
            r.get_status_code(),
            b64(r.status_text().as_bytes()),
            b64(r.get_content().cloned().unwrap_or_default().as_bytes()),
            typed(&Value::Array(headers))
        ])
    })
}

fn new_response(
    content: &str,
    status: i64,
    from: Option<&str>,
) -> Result<ResponseHandle, TestError> {
    let headers = match from {
        Some(event) => ResponseHeaderBag::new([("X-From", event)])?,
        None => ResponseHeaderBag::empty(),
    };
    Ok(Handle::new(Response::new(Some(content), status, headers)?))
}

type Log = Arc<Mutex<Vec<Json>>>;

/// One recorded listener: log what the event shows, then act.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per event and action, as the recorder"
)]
fn listener(
    index: usize,
    event_name: String,
    action: Json,
    log: Log,
    stack: RequestStack,
) -> Listener {
    Listener::closure(move |event, _, _| {
        let seen = match event_name.as_str() {
            KernelEvents::REQUEST => {
                let e = event.downcast_ref::<RequestEvent>().ok_or("request")?;
                let depth = usize::from(stack.get_current_request().is_some())
                    + usize::from(stack.get_parent_request().is_some());
                json!([depth, e.has_response(), e.is_main_request()])
            }
            KernelEvents::CONTROLLER => {
                let e = event
                    .downcast_ref::<ControllerEvent>()
                    .ok_or("controller")?;
                json!([e.get_controller_reflector().len()])
            }
            KernelEvents::CONTROLLER_ARGUMENTS => {
                let e = event
                    .downcast_ref::<ControllerArgumentsEvent>()
                    .ok_or("arguments")?;
                json!([
                    list(&e.get_arguments()),
                    typed(&Value::Array(e.get_named_arguments()))
                ])
            }
            KernelEvents::VIEW => {
                let e = event.downcast_ref::<ViewEvent>().ok_or("view")?;
                let arguments = e
                    .controller_metadata
                    .as_ref()
                    .map_or(json!(["null"]), |m| list(&m.get_arguments()));
                json!([typed(e.get_controller_result()), arguments])
            }
            KernelEvents::RESPONSE => {
                let e = event.downcast_ref::<ResponseEvent>().ok_or("response")?;
                json!([
                    e.get_response().with(Response::get_status_code),
                    e.controller_metadata.is_some()
                ])
            }
            KernelEvents::FINISH_REQUEST => {
                let e = event.downcast_ref::<FinishRequestEvent>().ok_or("finish")?;
                json!([kind(e.controller_metadata.as_ref())])
            }
            KernelEvents::EXCEPTION => {
                let e = event.downcast_ref::<ExceptionEvent>().ok_or("exception")?;
                json!([
                    describe(e.get_throwable()),
                    e.is_kernel_terminating(),
                    kind(e.controller_metadata.as_ref()),
                    e.has_response()
                ])
            }
            KernelEvents::TERMINATE => {
                let e = event.downcast_ref::<TerminateEvent>().ok_or("terminate")?;
                json!([e.get_response().with(Response::get_status_code)])
            }
            other => return Err(format!("unknown event {other}").into()),
        };
        log.lock()
            .map_err(|_| "log")?
            .push(json!([event_name, index, seen]));

        let from = Some(event_name.as_str());
        match text(&action[0])? {
            "log" => {}
            "response" => {
                let response = new_response(text(&action[1])?, int(&action[2])?, from)?;
                if let Some(e) = event.downcast_mut::<RequestEvent>() {
                    e.set_response(response);
                } else if let Some(e) = event.downcast_mut::<ViewEvent>() {
                    e.set_response(response);
                } else if let Some(e) = event.downcast_mut::<ExceptionEvent>() {
                    e.set_response(response);
                } else if let Some(e) = event.downcast_mut::<ResponseEvent>() {
                    e.set_response(response);
                }
            }
            "stop" => stop(event),
            "throw" => return Err(Box::new(build(&action[1])?)),
            "replace" => {
                let e = event.downcast_mut::<ExceptionEvent>().ok_or("exception")?;
                e.set_throwable(build(&action[1])?);
            }
            "allow" => {
                let e = event.downcast_mut::<ExceptionEvent>().ok_or("exception")?;
                e.allow_custom_response_code();
            }
            "arguments" => {
                let e = event
                    .downcast_ref::<ControllerArgumentsEvent>()
                    .ok_or("arguments")?;
                let arguments = action[1]
                    .as_array()
                    .ok_or("arguments")?
                    .iter()
                    .map(|a| Ok(Value::string(text(a)?)))
                    .collect::<Result<Vec<_>, TestError>>()?;
                e.set_arguments(arguments);
            }
            "controller" => {
                let label = text(&action[1])?.to_owned();
                let controller = Controller::new(move |arguments| {
                    let content = format!("{label}{}", arguments.len());
                    let response = Response::new(Some(content), 200, ResponseHeaderBag::empty())
                        .map_err(|e| Arc::new(e) as Throwable)?;
                    Ok(ControllerResult::Response(Handle::new(response)))
                })
                .with_parameters([Parameter::new("args").variadic()]);
                if let Some(e) = event.downcast_ref::<ControllerEvent>() {
                    e.set_controller(controller, None);
                } else if let Some(e) = event.downcast_ref::<ControllerArgumentsEvent>() {
                    e.set_controller(controller, None);
                }
            }
            "status" => {
                let e = event.downcast_ref::<ResponseEvent>().ok_or("response")?;
                e.get_response().with_mut(|r| {
                    r.set_status_code(int(&action[1])?, None)
                        .map(|_| ())
                        .map_err(TestError::from)
                })?;
            }
            "header" => {
                let e = event.downcast_ref::<ResponseEvent>().ok_or("response")?;
                let (name, value) = (text(&action[1])?, text(&action[2])?);
                e.get_response()
                    .with_mut(|r| r.headers_mut().set(name.as_bytes(), value, true))?;
            }
            other => return Err(format!("unknown action {other}").into()),
        }
        Ok(())
    })
}

fn stop(event: &mut dyn EventObject) {
    if let Some(e) = event.downcast_mut::<RequestEvent>() {
        e.stop_propagation();
    } else if let Some(e) = event.downcast_mut::<ControllerEvent>() {
        e.stop_propagation();
    } else if let Some(e) = event.downcast_mut::<ViewEvent>() {
        e.stop_propagation();
    } else if let Some(e) = event.downcast_mut::<ExceptionEvent>() {
        e.stop_propagation();
    }
}

struct Resolver(Option<Controller>);

impl ControllerResolverInterface for Resolver {
    fn get_controller(&self, _request: &RequestHandle) -> Result<Option<Controller>, Throwable> {
        Ok(self.0.clone())
    }
}

struct Arguments(Json);

impl ArgumentResolverInterface for Arguments {
    fn get_arguments(
        &self,
        _request: &RequestHandle,
        _controller: &Controller,
        _reflector: &[Parameter],
    ) -> Result<Vec<Value>, Throwable> {
        let failed = |e: TestError| -> Throwable { Arc::from(e) };
        if self.0[0] == "throw" {
            return Err(build(&self.0[1]).map_err(failed)?);
        }
        self.0[1]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|i| Value::string(i.as_str().unwrap_or_default()))
                    .collect()
            })
            .ok_or_else(|| failed("arguments".into()))
    }
}

fn controller(spec: &Json, values: &[Value]) -> Result<Option<Controller>, TestError> {
    let parameters = [
        Parameter::new("first").with_default(Value::Null),
        Parameter::new("rest").variadic(),
    ];
    let controller = match text(&spec[0])? {
        "response" => {
            let (content, status) = (text(&spec[1])?.to_owned(), int(&spec[2])?);
            Controller::new(move |_| {
                let response =
                    Response::new(Some(content.clone()), status, ResponseHeaderBag::empty())
                        .map_err(|e| Arc::new(e) as Throwable)?;
                Ok(ControllerResult::Response(Handle::new(response)))
            })
        }
        "value" => {
            let value = values
                .get(usize::try_from(int(&spec[1])?)?)
                .cloned()
                .ok_or("value index")?;
            Controller::new(move |_| Ok(ControllerResult::Value(value.clone())))
        }
        "throw" => {
            let exception = spec[1].clone();
            Controller::new(move |_| {
                Err(build(&exception).map_err(Arc::<dyn std::error::Error + Send + Sync>::from)?)
            })
        }
        "false" => return Ok(None),
        other => return Err(format!("unknown controller {other}").into()),
    };
    Ok(Some(controller.with_parameters(parameters)))
}

fn calls() -> Json {
    with_output(|o| {
        let mut list = Vec::new();
        let mut echo = Vec::new();
        for call in &o.calls {
            match call {
                OutputCall::Header {
                    line,
                    replace,
                    response_code,
                } => list.push(json!([
                    "header",
                    b64(line.as_bytes()),
                    replace,
                    response_code
                ])),
                OutputCall::HeaderRemove(name) => {
                    list.push(json!(["header_remove", b64(name.as_bytes())]));
                }
                OutputCall::Echo(text) => echo.extend_from_slice(text.as_bytes()),
                other => list.push(json!([format!("{other:?}")])),
            }
        }
        if !echo.is_empty() {
            list.push(json!(["echo", b64(&echo)]));
        }
        Json::Array(list)
    })
}

#[test]
fn lifecycles_run_as_in_php() -> Result<(), TestError> {
    replay(Mode::Plain)
}

/// The same scripts on the listener seam: the listeners compiled into a
/// `CompiledEventDispatcher` before the kernel is built, as the converter
/// emits them. No listener here changes the dispatcher, so every dispatch
/// goes through the compiled table.
#[test]
fn lifecycles_run_as_in_php_on_a_compiled_dispatcher() -> Result<(), TestError> {
    replay(Mode::Compiled)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Plain,
    Compiled,
}

#[allow(clippy::too_many_lines, reason = "one replay, in the recorder's order")]
fn replay(kind: Mode) -> Result<(), TestError> {
    let data: Json = serde_json::from_str(DATA)?;
    let values: Vec<Value> = data["values"]
        .as_array()
        .ok_or("values")?
        .iter()
        .map(vectors::decode)
        .collect::<Result<_, _>>()?;
    let scripts = data["scripts"].as_array().ok_or("scripts")?;
    assert_eq!(1500, scripts.len());
    let _clock = clock::mock(1_700_000_000);
    let mut failures = Vec::new();

    for (n, recorded) in scripts.iter().enumerate() {
        let script = &recorded["script"];
        set_globals(Globals::default());
        Request::reset_statics();

        let log: Log = Arc::default();
        let stack = RequestStack::default();
        let mut registrations = Vec::new();
        for (index, spec) in script["listeners"]
            .as_array()
            .ok_or("listeners")?
            .iter()
            .enumerate()
        {
            registrations.push((
                text(&spec[0])?.to_owned(),
                listener(
                    index,
                    text(&spec[0])?.to_owned(),
                    spec[2].clone(),
                    log.clone(),
                    stack.clone(),
                ),
                int(&spec[1])?,
            ));
        }
        let mut compiled = None;
        let dispatcher: Arc<dyn ContractsEventDispatcherInterface> = match kind {
            Mode::Plain => {
                let dispatcher = EventDispatcher::new();
                for (name, listener, priority) in registrations {
                    dispatcher.add_listener(&name, listener, priority)?;
                }
                Arc::new(dispatcher)
            }
            Mode::Compiled => {
                let mut map = ListenerMap::new();
                for (name, listener, priority) in registrations {
                    map.add_listener(&name, listener, priority);
                }
                let dispatcher = Arc::new(CompiledEventDispatcher::from(map));
                compiled = Some(dispatcher.clone());
                dispatcher
            }
        };
        let kernel = HttpKernel::new(
            dispatcher,
            Arc::new(Resolver(controller(&script["controller"], &values)?)),
            Options {
                request_stack: Some(stack.clone()),
                argument_resolver: Arc::new(Arguments(script["arguments"].clone())),
                handle_all_throwables: script["all"].as_bool().ok_or("all")?,
            },
        );
        let request: RequestHandle = Handle::new(Request::create("/path")?);

        let handled = kernel.handle(
            &request,
            <dyn HttpKernelInterface>::MAIN_REQUEST,
            script["catch"].as_bool().ok_or("catch")?,
        );
        let got = match &handled {
            Ok(response) => json!(["response", state(response)]),
            Err(e) => json!(["throws", describe(e)]),
        };
        let compare = |what: &str, expected: &Json, actual: &Json, failures: &mut Vec<String>| {
            if expected != actual {
                failures.push(format!(
                    "script {n} {what}: expected {expected}, got {actual}\n  {script}"
                ));
            }
        };
        compare(
            "log",
            &recorded["log"],
            &Json::Array(log.lock().map_err(|_| "log")?.clone()),
            &mut failures,
        );
        compare("handled", &recorded["handled"], &got, &mut failures);
        let level = request.with(|r| r.headers().get("X-Php-Ob-Level", None).unwrap_or_default());
        compare(
            "after",
            &recorded["after"],
            &json!([stack.get_current_request().is_some(), b64(level.as_bytes())]),
            &mut failures,
        );

        log.lock().map_err(|_| "log")?.clear();
        with_output(|o| {
            o.calls.clear();
            o.headers_sent = script["headers_sent"].as_bool().unwrap_or(false);
        });
        let terminated = match handled {
            Ok(response) => kernel.terminate(&request, &response),
            Err(e) => kernel.terminate_with_exception(e, Some(request.clone())),
        };
        let made = calls();
        let mut again = Json::Null;
        let terminated = match terminated {
            Ok(()) => json!(["ok"]),
            Err(e) => {
                let described = describe(&e);
                if recorded["handled"][0] == "response" {
                    let terminate_log = log.lock().map_err(|_| "log")?.clone();
                    log.lock().map_err(|_| "log")?.clear();
                    let result = kernel.terminate_with_exception(e, Some(request.clone()));
                    let calls = Json::Array(log.lock().map_err(|_| "log")?.clone());
                    again = match result {
                        Ok(()) => json!(["ok", calls]),
                        Err(u) => json!(["throws", describe(&u), calls]),
                    };
                    *log.lock().map_err(|_| "log")? = terminate_log;
                }
                json!(["throws", described])
            }
        };
        compare("again", &recorded["again"], &again, &mut failures);
        compare(
            "terminate_log",
            &recorded["terminate_log"],
            &Json::Array(log.lock().map_err(|_| "log")?.clone()),
            &mut failures,
        );
        compare(
            "terminated",
            &recorded["terminated"],
            &terminated,
            &mut failures,
        );
        compare("calls", &recorded["calls"], &made, &mut failures);
        if compiled.is_some_and(|d| d.is_thawed()) {
            failures.push(format!("script {n}: the compiled dispatcher thawed"));
        }
    }
    Request::reset_statics();
    let total = failures.len();
    failures.truncate(10);
    assert!(
        failures.is_empty(),
        "{total} differences:\n{}",
        failures.join("\n")
    );
    Ok(())
}
