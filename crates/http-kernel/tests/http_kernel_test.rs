//! HttpKernelTest.php at symfony/http-kernel v8.1.11, on `HttpKernel`.
//! Names and assertions are PHPUnit's; `assertSame()` on objects is handle
//! identity.
//!
//! Adapted where PHP has something Rust has not:
//! - the stubbed resolvers are small structs returning what the stub
//!   returns;
//! - a PHP throwable (`new \RuntimeException('foo')`) is a `PhpException`;
//! - `RedirectResponse('/login', 301)` (not ported) is a 301 `Response`
//!   with that `Location`, which is what the kernel reads of it;
//! - the mocked `RequestStack` of `testVerifyRequestStackPushPopDuringHandle`
//!   is a real stack observed during and after `handle()`;
//! - the `Response` subclass that records `sendHeaders()` is read from the
//!   recorded `php::Output` calls, and `headers_sent()` is its flag there.
//!
//! Upstream: symfony/http-kernel Tests/HttpKernelTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use php_value::{Handle, PhpArray, PhpString, Value, object::GenericObject, php_array};
use symfony_event_dispatcher::{EventDispatcher, EventDispatcherInterface, EventObject, Listener};
use symfony_http_foundation::{
    header_bag::ResponseHeaderBag,
    php::{Globals, OutputCall, set_globals, with_output},
    request::Request,
    request_stack::{RequestHandle, RequestStack},
    response::Response,
    streamed_response::{self, CallbackOrChunks},
};
use symfony_http_kernel::{
    controller::{
        ArgumentResolverInterface, Controller, ControllerResolverInterface, ControllerResult,
        Parameter, ResponseHandle,
    },
    event::{
        ControllerArgumentsEvent, ControllerEvent, ExceptionEvent, FinishRequestEvent,
        RequestEvent, ResponseEvent, TerminateEvent, ViewEvent,
    },
    exception::{
        HttpArgs, HttpException, PhpClass, PhpException, Throwable, class_of, message_of, same,
    },
    http_kernel::{
        HttpKernel, HttpKernelInterface, KernelEvents, KernelRef, Options, TerminableInterface,
        same_kernel,
    },
};

type Error = Box<dyn std::error::Error + Send + Sync>;

const MAIN_REQUEST: i64 = <dyn HttpKernelInterface>::MAIN_REQUEST;

fn response(content: &str) -> ResponseHandle {
    Handle::new(Response::new(Some(content), 200, ResponseHeaderBag::empty()).unwrap())
}

fn response_with(content: &str, status: i64) -> ResponseHandle {
    Handle::new(Response::new(Some(content), status, ResponseHeaderBag::empty()).unwrap())
}

fn new_request() -> RequestHandle {
    Handle::new(Request::default())
}

fn create(uri: &str) -> RequestHandle {
    Handle::new(Request::create(uri).unwrap())
}

fn runtime(message: &str) -> Throwable {
    PhpException::new(PhpClass::RuntimeException, message).throwable()
}

fn throws(throwable: Throwable) -> Controller {
    Controller::new(move |_| Err(throwable.clone()))
}

fn returns(response: ResponseHandle) -> Controller {
    Controller::new(move |_| Ok(ControllerResult::Response(response.clone())))
}

/// `$this->createStub(ControllerResolverInterface::class)` returning
/// `$controller` (`None` is `false`).
struct Resolver(Option<Controller>);

impl ControllerResolverInterface for Resolver {
    fn get_controller(&self, _request: &RequestHandle) -> Result<Option<Controller>, Throwable> {
        Ok(self.0.clone())
    }
}

/// `$this->createStub(ArgumentResolverInterface::class)` returning
/// `$arguments`, or throwing.
struct Arguments(Result<Vec<Value>, Throwable>);

impl ArgumentResolverInterface for Arguments {
    fn get_arguments(
        &self,
        _request: &RequestHandle,
        _controller: &Controller,
        _reflector: &[Parameter],
    ) -> Result<Vec<Value>, Throwable> {
        self.0.clone()
    }
}

/// `getHttpKernel()`'s `$controller`.
enum Ctl {
    /// `null`: `fn () => new Response('Hello')`.
    Default,
    /// `false`: no controller.
    False,
    Is(Controller),
}

/// A value a listener captures for the test to read.
type Slot<T> = Arc<Mutex<Option<T>>>;

/// `getHttpKernel($eventDispatcher, $controller, $requestStack,
/// $arguments, $handleAllThrowables)`.
fn get_http_kernel(
    dispatcher: &Arc<EventDispatcher>,
    controller: Ctl,
    request_stack: Option<RequestStack>,
    arguments: Vec<Value>,
    handle_all_throwables: bool,
) -> Arc<HttpKernel> {
    set_globals(Globals::default());
    let controller = match controller {
        Ctl::Default => Some(returns(response("Hello"))),
        Ctl::False => None,
        Ctl::Is(controller) => Some(controller),
    };
    HttpKernel::new(
        dispatcher.clone(),
        Arc::new(Resolver(controller)),
        Options {
            request_stack,
            argument_resolver: Arc::new(Arguments(Ok(arguments))),
            handle_all_throwables,
        },
    )
}

fn kernel_with(dispatcher: &Arc<EventDispatcher>, controller: Controller) -> Arc<HttpKernel> {
    get_http_kernel(dispatcher, Ctl::Is(controller), None, Vec::new(), false)
}

fn listen<E: EventObject>(
    dispatcher: &Arc<EventDispatcher>,
    name: &str,
    f: impl Fn(&mut E) -> Result<(), Error> + Send + Sync + 'static,
) {
    dispatcher
        .add_listener(
            name,
            Listener::closure(move |event, _, _| {
                let event = event.downcast_mut::<E>().ok_or("unexpected event type")?;
                f(event)
            }),
            0,
        )
        .unwrap();
}

/// `$event->setResponse(new Response($event->getThrowable()->getMessage()))`.
fn message_listener(dispatcher: &Arc<EventDispatcher>) {
    listen(
        dispatcher,
        KernelEvents::EXCEPTION,
        |event: &mut ExceptionEvent| {
            let message = message_of(event.get_throwable().as_ref());
            event.set_response(response(&message.to_string_lossy()));
            Ok(())
        },
    );
}

fn content(response: &ResponseHandle) -> String {
    response.with(|r| {
        r.get_content()
            .cloned()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    })
}

fn status(response: &ResponseHandle) -> i64 {
    response.with(Response::get_status_code)
}

fn header(response: &ResponseHandle, name: &str) -> Option<PhpString> {
    response.with(|r| r.headers().get(name, None))
}

/// Catch exceptions: true; Throwable type: `RuntimeException`; Listener: false.
#[test]
fn testHandleWhenControllerThrowsAnExceptionAndCatchIsTrue() {
    let kernel = kernel_with(&Arc::new(EventDispatcher::new()), throws(runtime("")));

    let e = kernel
        .handle(&new_request(), MAIN_REQUEST, true)
        .unwrap_err();
    assert_eq!("RuntimeException", class_of(e.as_ref()));
}

#[test]
fn testRequestStackIsNotBrokenWhenControllerThrowsAnExceptionAndCatchIsTrue() {
    let request_stack = RequestStack::default();
    let kernel = get_http_kernel(
        &Arc::new(EventDispatcher::new()),
        Ctl::Is(throws(runtime(""))),
        Some(request_stack.clone()),
        Vec::new(),
        false,
    );

    let _ = kernel.handle(&new_request(), MAIN_REQUEST, true);

    assert!(request_stack.get_current_request().is_none());
}

#[test]
fn testRequestStackIsNotBrokenWhenControllerThrowsAnExceptionAndCatchIsFalse() {
    let request_stack = RequestStack::default();
    let kernel = get_http_kernel(
        &Arc::new(EventDispatcher::new()),
        Ctl::Is(throws(runtime(""))),
        Some(request_stack.clone()),
        Vec::new(),
        false,
    );

    let _ = kernel.handle(&new_request(), MAIN_REQUEST, false);

    assert!(request_stack.get_current_request().is_none());
}

#[test]
fn testRequestStackIsNotBrokenWhenControllerThrowsAnThrowable() {
    let request_stack = RequestStack::default();
    let kernel = get_http_kernel(
        &Arc::new(EventDispatcher::new()),
        Ctl::Is(throws(PhpException::new(PhpClass::Error, "").throwable())),
        Some(request_stack.clone()),
        Vec::new(),
        false,
    );

    let _ = kernel.handle(&new_request(), MAIN_REQUEST, true);

    assert!(request_stack.get_current_request().is_none());
}

/// Catch exceptions: false; Throwable type: `RuntimeException`; Listener: false.
#[test]
fn testHandleWhenControllerThrowsAnExceptionAndCatchIsFalseAndNoListenerIsRegistered() {
    let kernel = kernel_with(&Arc::new(EventDispatcher::new()), throws(runtime("")));

    let e = kernel
        .handle(&new_request(), MAIN_REQUEST, false)
        .unwrap_err();
    assert_eq!("RuntimeException", class_of(e.as_ref()));
}

/// Catch exceptions: true; Throwable type: `RuntimeException`; Listener: true.
#[test]
fn testHandleWhenControllerThrowsAnExceptionAndCatchIsTrueWithAHandlingListener() {
    let dispatcher = Arc::new(EventDispatcher::new());
    message_listener(&dispatcher);

    let kernel = kernel_with(&dispatcher, throws(runtime("foo")));
    let response = kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap();

    assert_eq!(500, status(&response));
    assert_eq!("foo", content(&response));
}

/// Catch exceptions: true; Throwable type: `TypeError`; Listener: true.
#[test]
fn testHandleWhenControllerThrowsAThrowableAndCatchIsTrueWithAHandlingListener() {
    let dispatcher = Arc::new(EventDispatcher::new());
    message_listener(&dispatcher);

    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Is(throws(
            PhpException::new(PhpClass::TypeError, "foo").throwable(),
        )),
        None,
        Vec::new(),
        true,
    );
    let response = kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap();

    assert_eq!(500, status(&response));
    assert_eq!("foo", content(&response));
}

/// Catch exceptions: false; Throwable type: `TypeError`; Listener: true.
#[test]
fn testHandleWhenControllerThrowsAThrowableAndCatchIsFalseWithAHandlingListener() {
    let dispatcher = Arc::new(EventDispatcher::new());
    message_listener(&dispatcher);

    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Is(throws(
            PhpException::new(PhpClass::TypeError, "foo").throwable(),
        )),
        None,
        Vec::new(),
        true,
    );
    let e = kernel
        .handle(&new_request(), MAIN_REQUEST, false)
        .unwrap_err();
    assert_eq!("TypeError", class_of(e.as_ref()));
}

/// Catch exceptions: true; Throwable type: `TypeError`; Listener: true.
#[test]
fn testHandleWhenControllerThrowsAThrowableAndCatchIsTrueNotHandlingThrowables() {
    let dispatcher = Arc::new(EventDispatcher::new());
    message_listener(&dispatcher);

    set_globals(Globals::default());
    let kernel = HttpKernel::new(
        dispatcher,
        Arc::new(Resolver(Some(throws(
            PhpException::new(PhpClass::TypeError, "foo").throwable(),
        )))),
        Options {
            request_stack: None,
            argument_resolver: Arc::new(Arguments(Ok(Vec::new()))),
            handle_all_throwables: false,
        },
    );

    let e = kernel
        .handle(&new_request(), MAIN_REQUEST, true)
        .unwrap_err();
    assert_eq!("TypeError", class_of(e.as_ref()));
}

/// Catch exceptions: true; Throwable type: `RuntimeException`; Listener: true.
#[test]
fn testHandleWhenControllerThrowsAnExceptionAndCatchIsTrueWithANonHandlingListener() {
    let exception = runtime("");

    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::EXCEPTION,
        |_: &mut ExceptionEvent| {
            // should set a response, but does not
            Ok(())
        },
    );

    let kernel = kernel_with(&dispatcher, throws(exception.clone()));

    match kernel.handle(&new_request(), MAIN_REQUEST, true) {
        Ok(_) => panic!("LogicException expected"),
        Err(e) => assert!(same(&exception, &e)),
    }
}

#[test]
fn testHandleExceptionWithARedirectionResponse() {
    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::EXCEPTION,
        |event: &mut ExceptionEvent| {
            // new RedirectResponse('/login', 301)
            let redirect = Response::new(
                Some(""),
                301,
                ResponseHeaderBag::new([("Location", "/login")])?,
            )?;
            event.set_response(Handle::new(redirect));
            Ok(())
        },
    );

    let kernel = kernel_with(
        &dispatcher,
        throws(HttpException::access_denied(HttpArgs::default()).throwable()),
    );
    let response = kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap();

    assert_eq!(301, status(&response));
    assert_eq!(
        Some(PhpString::from("/login")),
        header(&response, "Location")
    );
}

#[test]
fn testHandleHttpException() {
    let dispatcher = Arc::new(EventDispatcher::new());
    message_listener(&dispatcher);

    let kernel = kernel_with(
        &dispatcher,
        throws(HttpException::method_not_allowed(["POST"], HttpArgs::default()).throwable()),
    );
    let response = kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap();

    assert_eq!(405, status(&response));
    assert_eq!(Some(PhpString::from("POST")), header(&response, "Allow"));
}

#[test]
fn testHandleWhenAnExceptionIsHandledWithASpecificStatusCode() {
    for expected_status_code in [200, 302, 403] {
        let dispatcher = Arc::new(EventDispatcher::new());
        listen(
            &dispatcher,
            KernelEvents::EXCEPTION,
            move |event: &mut ExceptionEvent| {
                event.allow_custom_response_code();
                event.set_response(response_with("", expected_status_code));
                Ok(())
            },
        );

        let kernel = kernel_with(&dispatcher, throws(runtime("")));
        let response = kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap();

        assert_eq!(expected_status_code, status(&response));
    }
}

#[test]
fn testHandleWhenAListenerReturnsAResponse() {
    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::REQUEST,
        |event: &mut RequestEvent| {
            event.set_response(response("hello"));
            Ok(())
        },
    );

    let kernel = get_http_kernel(&dispatcher, Ctl::Default, None, Vec::new(), false);

    assert_eq!(
        "hello",
        content(&kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap())
    );
}

#[test]
fn testHandleWhenNoControllerIsFound() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let kernel = get_http_kernel(&dispatcher, Ctl::False, None, Vec::new(), false);

    let e = kernel
        .handle(&new_request(), MAIN_REQUEST, true)
        .unwrap_err();
    assert_eq!(
        "Symfony\\Component\\HttpKernel\\Exception\\NotFoundHttpException",
        class_of(e.as_ref())
    );
}

#[test]
fn testHandleWhenTheControllerIsAClosure() {
    let response = response("foo");
    let dispatcher = Arc::new(EventDispatcher::new());
    let kernel = kernel_with(&dispatcher, returns(response.clone()));

    assert!(response.same(&kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap()));
}

#[test]
fn testHandleWhenTheControllerDoesNotReturnAResponseButAViewIsRegistered() {
    let dispatcher = Arc::new(EventDispatcher::new());
    listen(&dispatcher, KernelEvents::VIEW, |event: &mut ViewEvent| {
        let result = event
            .get_controller_result()
            .to_php_string()
            .unwrap_or_default();
        event.set_response(response(&result.to_string_lossy()));
        Ok(())
    });

    let kernel = kernel_with(
        &dispatcher,
        Controller::new(|_| Ok(ControllerResult::Value(Value::string("foo")))),
    );

    assert_eq!(
        "foo",
        content(&kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap())
    );
}

#[test]
fn testHandleWithAResponseListener() {
    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::RESPONSE,
        |event: &mut ResponseEvent| {
            event.set_response(response("foo"));
            Ok(())
        },
    );
    let kernel = get_http_kernel(&dispatcher, Ctl::Default, None, Vec::new(), false);

    assert_eq!(
        "foo",
        content(&kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap())
    );
}

/// `fn ($content) => new Response($content)`.
fn content_controller() -> Controller {
    Controller::new(|arguments| {
        let content = arguments
            .first()
            .and_then(Value::to_php_string)
            .unwrap_or_default();
        Ok(ControllerResult::Response(response(
            &content.to_string_lossy(),
        )))
    })
    .with_parameters([Parameter::new("content")])
}

/// `assertResponseEquals()`: equal once the dates are the same.
/// PHPUnit compares the header arrays by key, in any order; the string
/// form sorts them.
fn assert_response_equals(expected: &ResponseHandle, actual: &ResponseHandle) {
    let date = actual.with(|r| r.get_date().unwrap().unwrap());
    expected.with_mut(|r| {
        r.set_date(&date).unwrap();
    });
    let form = |r: &Response| (r.to_php_string(), r.get_charset().cloned());
    assert_eq!(expected.with(form), actual.with(form));
}

#[test]
fn testHandleAllowChangingControllerArguments() {
    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::CONTROLLER_ARGUMENTS,
        |event: &mut ControllerArgumentsEvent| {
            event.set_arguments(vec![Value::string("foo")]);
            Ok(())
        },
    );

    let kernel = kernel_with(&dispatcher, content_controller());

    assert_response_equals(
        &response("foo"),
        &kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap(),
    );
}

#[test]
fn testHandleAllowChangingControllerAndArguments() {
    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::CONTROLLER_ARGUMENTS,
        |event: &mut ControllerArgumentsEvent| {
            let old_controller = event.get_controller();
            let old_arguments = event.get_arguments();

            let new_controller = Controller::new(move |arguments| {
                let ControllerResult::Response(response) = old_controller.call(&old_arguments)?
                else {
                    return Err(
                        PhpException::new(PhpClass::LogicException, "no response").throwable()
                    );
                };

                let id = arguments
                    .first()
                    .and_then(Value::to_php_string)
                    .unwrap_or_default();
                response
                    .with_mut(|r| r.headers_mut().set(b"X-Id", id, true))
                    .map_err(|e| Arc::new(e) as Throwable)?;

                Ok(ControllerResult::Response(response))
            })
            .with_parameters([Parameter::new("id")]);

            event.set_controller(new_controller, None);
            event.set_arguments(vec![Value::string("bar")]);
            Ok(())
        },
    );

    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Is(content_controller()),
        None,
        vec![Value::string("foo")],
        false,
    );

    let expected = Handle::new(
        Response::new(
            Some("foo"),
            200,
            ResponseHeaderBag::new([("X-Id", "bar")]).unwrap(),
        )
        .unwrap(),
    );
    assert_response_equals(
        &expected,
        &kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap(),
    );
}

#[test]
fn testTerminate() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let kernel = get_http_kernel(&dispatcher, Ctl::Default, None, Vec::new(), false);
    let captured: Slot<(KernelRef, RequestHandle, ResponseHandle)> = Arc::default();
    let sink = captured.clone();
    listen(
        &dispatcher,
        KernelEvents::TERMINATE,
        move |event: &mut TerminateEvent| {
            *sink.lock().unwrap() = Some((
                event.get_kernel().clone(),
                event.get_request().clone(),
                event.get_response().clone(),
            ));
            Ok(())
        },
    );

    let request = create("/");
    let response = response("");
    kernel.terminate(&request, &response).unwrap();
    let (captured_kernel, captured_request, captured_response) =
        captured.lock().unwrap().clone().expect("called");
    let kernel: KernelRef = kernel;
    assert!(same_kernel(&kernel, &captured_kernel));
    // assertEquals(): the same object
    assert!(request.same(&captured_request));
    assert_eq!(response, captured_response);
}

#[test]
fn testTerminateWithException() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let request_stack = RequestStack::default();
    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Default,
        Some(request_stack.clone()),
        Vec::new(),
        false,
    );

    let captured: Arc<Mutex<Option<RequestHandle>>> = Arc::default();
    let sink = captured.clone();
    let stack = request_stack.clone();
    listen(
        &dispatcher,
        KernelEvents::EXCEPTION,
        move |event: &mut ExceptionEvent| {
            *sink.lock().unwrap() = stack.get_current_request();
            event.set_response(response(""));
            Ok(())
        },
    );

    let request = create("/");
    kernel
        .terminate_with_exception(
            PhpException::new(PhpClass::Exception, "boo").throwable(),
            Some(request.clone()),
        )
        .unwrap();
    assert!(request.same(captured.lock().unwrap().as_ref().unwrap()));
    assert!(request_stack.get_current_request().is_none());
}

#[test]
fn testTerminateWithExceptionThrownByTerminateListener() {
    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::TERMINATE,
        |_: &mut TerminateEvent| {
            Err(Box::new(PhpException::new(
                PhpClass::RuntimeException,
                "boo",
            )))
        },
    );
    let is_kernel_terminating: Arc<Mutex<Vec<bool>>> = Arc::default();
    let sink = is_kernel_terminating.clone();
    listen(
        &dispatcher,
        KernelEvents::EXCEPTION,
        move |event: &mut ExceptionEvent| {
            sink.lock().unwrap().push(event.is_kernel_terminating());
            Ok(())
        },
    );
    let kernel = get_http_kernel(&dispatcher, Ctl::Default, None, Vec::new(), false);
    let request = create("/");

    let e = kernel.terminate(&request, &response("")).unwrap_err();

    let _ = kernel.terminate_with_exception(e, Some(request.clone()));

    let _ = kernel.terminate_with_exception(runtime("bar"), Some(request));

    assert_eq!(vec![true, false], *is_kernel_terminating.lock().unwrap());
}

/// `provideHeadersSent()`.
#[test]
fn testTerminateWithExceptionSendsHeadersUnlessAlreadySent() {
    for headers_sent in [false, true] {
        let response = response_with("boo", 500);

        let dispatcher = Arc::new(EventDispatcher::new());
        let set = response.clone();
        listen(
            &dispatcher,
            KernelEvents::EXCEPTION,
            move |event: &mut ExceptionEvent| {
                event.set_response(set.clone());
                Ok(())
            },
        );
        let terminated = Arc::new(Mutex::new(false));
        let sink = terminated.clone();
        listen(
            &dispatcher,
            KernelEvents::TERMINATE,
            move |_: &mut TerminateEvent| {
                *sink.lock().unwrap() = true;
                Ok(())
            },
        );

        let kernel = get_http_kernel(&dispatcher, Ctl::Default, None, Vec::new(), false);
        with_output(|o| o.headers_sent = headers_sent);
        kernel
            .terminate_with_exception(
                PhpException::new(PhpClass::Exception, "boo").throwable(),
                Some(create("/")),
            )
            .unwrap();

        let (echoed, send_headers_called) = with_output(|o| {
            let echoed: Vec<u8> = o
                .calls
                .iter()
                .filter_map(|c| match c {
                    OutputCall::Echo(text) => Some(text.as_bytes().to_vec()),
                    _ => None,
                })
                .flatten()
                .collect();
            let headers = o
                .calls
                .iter()
                .any(|c| matches!(c, OutputCall::Header { .. }));
            (echoed, headers)
        });
        assert_eq!(b"boo".to_vec(), echoed);
        assert_eq!(!headers_sent, send_headers_called);
        assert!(*terminated.lock().unwrap());
    }
}

/// The mocked stack's expectations (`push` once with the request, `pop`
/// once), observed on a real stack.
#[test]
fn testVerifyRequestStackPushPopDuringHandle() {
    let request = new_request();

    let stack = RequestStack::default();
    let seen: Arc<Mutex<Vec<RequestHandle>>> = Arc::default();

    let dispatcher = Arc::new(EventDispatcher::new());
    let observed = stack.clone();
    let sink = seen.clone();
    listen(
        &dispatcher,
        KernelEvents::REQUEST,
        move |_: &mut RequestEvent| {
            sink.lock().unwrap().extend(observed.get_current_request());
            assert!(observed.get_parent_request().is_none());
            Ok(())
        },
    );
    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Default,
        Some(stack.clone()),
        Vec::new(),
        false,
    );

    kernel.handle(&request, MAIN_REQUEST, true).unwrap();

    let seen = seen.lock().unwrap();
    assert_eq!(1, seen.len());
    assert!(request.same(&seen[0]));
    assert!(stack.get_current_request().is_none());
}

#[test]
fn testVerifyRequestStackPushPopWithStreamedResponse() -> Result<(), Error> {
    let request = new_request();
    let stack = RequestStack::default();
    let dispatcher = Arc::new(EventDispatcher::new());
    let observed = stack.clone();
    let streamed = streamed_response::new(
        CallbackOrChunks::Callback(Arc::new(move || {
            // `echo $stack->getMainRequest()::class`; `null::class` is an
            // `\Error`.
            let Some(_) = observed.get_main_request() else {
                return Err(
                    PhpException::new(PhpClass::Error, "Cannot use \"::class\" on null")
                        .throwable(),
                );
            };
            streamed_response::echo(symfony_http_foundation::request::CLASS);
            Ok(())
        })),
        200,
        ResponseHeaderBag::empty(),
    )?;
    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Is(returns(Handle::new(streamed))),
        Some(stack.clone()),
        Vec::new(),
        false,
    );

    let response = kernel.handle(&request, MAIN_REQUEST, true)?;
    assert!(stack.get_main_request().is_none());
    with_output(|o| o.calls.clear());
    response.with_mut(|r| r.send(true).map(|_| ()))?;
    let echoed: Vec<u8> = with_output(|o| {
        o.calls
            .iter()
            .filter_map(|call| match call {
                OutputCall::Echo(text) => Some(text.as_bytes().to_vec()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .concat()
    });
    assert_eq!(
        symfony_http_foundation::request::CLASS.as_bytes(),
        echoed.as_slice()
    );
    assert!(stack.get_main_request().is_none());
    Ok(())
}

#[test]
fn testInconsistentClientIpsOnMainRequests() {
    let request = new_request();
    Request::set_trusted_proxies(
        ["1.1.1.1"],
        Request::HEADER_X_FORWARDED_FOR | Request::HEADER_FORWARDED,
    )
    .unwrap();
    request.with_mut(|r| {
        r.server_mut().set("REMOTE_ADDR", Value::string("1.1.1.1"));
        r.headers_mut()
            .set(b"FORWARDED", "for=2.2.2.2", true)
            .unwrap();
        r.headers_mut()
            .set(b"X_FORWARDED_FOR", "3.3.3.3", true)
            .unwrap();
    });

    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::REQUEST,
        |event: &mut RequestEvent| {
            event.get_request().with_mut(Request::get_client_ip)?;
            Ok(())
        },
    );

    let kernel = get_http_kernel(&dispatcher, Ctl::Default, None, Vec::new(), false);
    let result = kernel.handle(&request, MAIN_REQUEST, false);

    Request::set_trusted_proxies(Vec::<&str>::new(), -1).unwrap();

    let e = result.unwrap_err();
    assert_eq!(
        "Symfony\\Component\\HttpKernel\\Exception\\BadRequestHttpException",
        class_of(e.as_ref())
    );
}

/// `new Bar('test')`.
fn bar(value: &str) -> Value {
    Value::Object(Arc::new(GenericObject::new(
        "Symfony\\Component\\HttpKernel\\Tests\\Fixtures\\Attribute\\Bar",
        php_array!["foo" => value],
    )))
}

#[test]
fn testResponseEventCanAccessControllerAttributes() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let captured_attributes: Arc<Mutex<Option<PhpArray>>> = Arc::default();

    listen(
        &dispatcher,
        KernelEvents::CONTROLLER,
        |event: &mut ControllerEvent| {
            event.set_controller(event.get_controller(), Some(PhpArray::list([bar("test")])));
            Ok(())
        },
    );

    let sink = captured_attributes.clone();
    listen(
        &dispatcher,
        KernelEvents::RESPONSE,
        move |event: &mut ResponseEvent| {
            *sink.lock().unwrap() = event
                .controller_metadata
                .as_ref()
                .map(|m| m.get_attributes(Some("*")));
            Ok(())
        },
    );

    let kernel = get_http_kernel(&dispatcher, Ctl::Default, None, Vec::new(), false);

    kernel.handle(&new_request(), MAIN_REQUEST, false).unwrap();

    let captured = captured_attributes.lock().unwrap().clone().unwrap();
    assert!(php_value::vectors::phpunit_equals(
        &Value::Array(PhpArray::list([bar("test")])),
        &Value::Array(captured)
    ));
}

#[test]
fn testViewEventProvidesControllerArgumentsViaMetadata() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let captured: Slot<(Vec<Value>, PhpArray)> = Arc::default();

    let sink = captured.clone();
    listen(
        &dispatcher,
        KernelEvents::VIEW,
        move |event: &mut ViewEvent| {
            let metadata = event.controller_metadata.clone().ok_or("metadata")?;
            *sink.lock().unwrap() =
                Some((metadata.get_arguments(), metadata.get_named_arguments()));
            event.set_response(response("ok"));
            Ok(())
        },
    );

    // fn ($value) => $value
    let controller = Controller::new(|arguments| {
        Ok(ControllerResult::Value(
            arguments.first().cloned().unwrap_or_default(),
        ))
    })
    .with_parameters([Parameter::new("value")]);
    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Is(controller),
        None,
        vec![Value::string("resolved")],
        false,
    );

    kernel.handle(&new_request(), MAIN_REQUEST, false).unwrap();

    let (captured_arguments, captured_named_arguments) = captured.lock().unwrap().clone().unwrap();
    assert!(
        Value::Array(PhpArray::list(captured_arguments))
            .identical(&Value::Array(PhpArray::list([Value::string("resolved")])))
    );
    assert!(
        Value::Array(captured_named_arguments)
            .identical(&Value::Array(php_array!["value" => "resolved"]))
    );
}

#[test]
fn testExceptionEventProvidesControllerMetadata() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let captured: Slot<(Controller, Vec<Value>)> = Arc::default();
    // static fn (string $value) => throw new \RuntimeException('boom')
    let controller = throws(runtime("boom")).with_parameters([Parameter::new("value")]);

    let sink = captured.clone();
    listen(
        &dispatcher,
        KernelEvents::EXCEPTION,
        move |event: &mut ExceptionEvent| {
            let metadata = event.controller_metadata.clone().ok_or("metadata")?;
            let arguments = metadata.as_arguments().ok_or("arguments")?.get_arguments();
            *sink.lock().unwrap() = Some((metadata.get_controller(), arguments));
            event.set_response(response("handled"));
            Ok(())
        },
    );

    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Is(controller.clone()),
        None,
        vec![Value::string("meta")],
        false,
    );

    let response = kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap();

    assert_eq!("handled", content(&response));
    let (captured_controller, captured_arguments) = captured.lock().unwrap().clone().unwrap();
    assert!(controller.same(&captured_controller));
    assert!(
        Value::Array(PhpArray::list(captured_arguments))
            .identical(&Value::Array(PhpArray::list([Value::string("meta")])))
    );
}

#[test]
fn testResponseEventWhenArgumentResolutionThrows() {
    let dispatcher = Arc::new(EventDispatcher::new());
    listen(
        &dispatcher,
        KernelEvents::EXCEPTION,
        |event: &mut ExceptionEvent| {
            event.set_response(response_with("handled", 400));
            Ok(())
        },
    );

    set_globals(Globals::default());
    let kernel = HttpKernel::new(
        dispatcher,
        Arc::new(Resolver(Some(returns(response("never"))))),
        Options {
            request_stack: None,
            argument_resolver: Arc::new(Arguments(Err(HttpException::bad_request(
                HttpArgs::message("boom"),
            )
            .throwable()))),
            handle_all_throwables: false,
        },
    );

    let response = kernel.handle(&new_request(), MAIN_REQUEST, true).unwrap();

    assert_eq!("handled", content(&response));
    assert_eq!(400, status(&response));
}

#[test]
fn testFinishRequestEventKeepsControllerMetadata() {
    let dispatcher = Arc::new(EventDispatcher::new());
    let captured: Arc<Mutex<Option<Vec<Value>>>> = Arc::default();

    let sink = captured.clone();
    listen(
        &dispatcher,
        KernelEvents::FINISH_REQUEST,
        move |event: &mut FinishRequestEvent| {
            let metadata = event.controller_metadata.clone().ok_or("metadata")?;
            *sink.lock().unwrap() =
                Some(metadata.as_arguments().ok_or("arguments")?.get_arguments());
            Ok(())
        },
    );

    let kernel = get_http_kernel(
        &dispatcher,
        Ctl::Is(content_controller()),
        None,
        vec![Value::string("done")],
        false,
    );

    let response = kernel.handle(&new_request(), MAIN_REQUEST, false).unwrap();

    assert_eq!("done", content(&response));
    assert!(
        Value::Array(PhpArray::list(captured.lock().unwrap().clone().unwrap()))
            .identical(&Value::Array(PhpArray::list([Value::string("done")])))
    );
}

/// Not upstream: the message for a controller result that is not a
/// response (`varToString()`), which upstream's test reads only through
/// the exception's trace.
#[test]
fn controller_result_messages() {
    let cases: Vec<(Value, &str)> = vec![
        (
            Value::Null,
            "null. Did you forget to add a return statement somewhere in your controller?",
        ),
        (Value::Bool(false), "a boolean value (false)."),
        (Value::Bool(true), "a boolean value (true)."),
        (Value::string("foo"), "a string (\"foo\")."),
        (Value::Int(3), "a number (3)."),
        (Value::Float(1.5), "a number (1.5)."),
        (
            Value::Array(php_array!["a" => 1_i64, 2_i64 => 3_i64]),
            "an array ([a => ..., 2 => ...]).",
        ),
        (
            bar("x"),
            "an object of type Symfony\\Component\\HttpKernel\\Tests\\Fixtures\\Attribute\\Bar.",
        ),
    ];
    for (result, tail) in cases {
        let dispatcher = Arc::new(EventDispatcher::new());
        let kernel = kernel_with(
            &dispatcher,
            Controller::new(move |_| Ok(ControllerResult::Value(result.clone()))),
        );
        let e = kernel
            .handle(&new_request(), MAIN_REQUEST, false)
            .unwrap_err();
        assert_eq!(
            "Symfony\\Component\\HttpKernel\\Exception\\ControllerDoesNotReturnResponseException",
            class_of(e.as_ref())
        );
        assert_eq!(
            format!(
                "The controller must return a \"Symfony\\Component\\HttpFoundation\\Response\" object but it returned {tail}"
            ),
            message_of(e.as_ref()).to_string_lossy()
        );
    }
}
