//! ResponseListenerTest.php at symfony/http-kernel v8.1.11. Names and
//! assertions are PHPUnit's. `$listener->onKernelResponse(...)` is the
//! listener `[$listener, 'onKernelResponse']`; the kernel is a stub.
//!
//! Upstream: symfony/http-kernel Tests/EventListener/ResponseListenerTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::sync::Arc;

use php_value::{Handle, PhpString};
use symfony_event_dispatcher::ContractsEventDispatcherInterface as _;
use symfony_event_dispatcher::{EventDispatcher, EventDispatcherInterface, Listener};
use symfony_http_foundation::{
    header_bag::ResponseHeaderBag, request::Request, request_stack::RequestHandle,
    response::Response,
};
use symfony_http_kernel::{
    controller::ResponseHandle,
    event::ResponseEvent,
    exception::Throwable,
    http_kernel::{HttpKernelInterface, KernelEvents, KernelRef},
    response_listener::ResponseListener,
};

/// `$this->createStub(HttpKernelInterface::class)`.
struct StubKernel;

impl HttpKernelInterface for StubKernel {
    fn handle(&self, _: &RequestHandle, _: i64, _: bool) -> Result<ResponseHandle, Throwable> {
        Ok(Handle::new(Response::default()))
    }
}

/// `setUp()`: a dispatcher with `new ResponseListener('UTF-8')` at
/// priority 0.
fn set_up() -> (EventDispatcher, KernelRef) {
    let dispatcher = EventDispatcher::new();
    add(&dispatcher, ResponseListener::new("UTF-8", false), 0);
    (dispatcher, Arc::new(StubKernel))
}

fn add(dispatcher: &EventDispatcher, listener: ResponseListener, priority: i64) {
    let object: symfony_event_dispatcher::Object = Arc::new(listener);
    dispatcher
        .add_listener(
            KernelEvents::RESPONSE,
            Listener::method(&object, "onKernelResponse"),
            priority,
        )
        .unwrap();
}

fn response(content: &str) -> ResponseHandle {
    Handle::new(Response::new(Some(content), 200, ResponseHeaderBag::empty()).unwrap())
}

fn dispatch(
    dispatcher: &EventDispatcher,
    kernel: &KernelRef,
    request: Request,
    request_type: i64,
    response: &ResponseHandle,
) -> ResponseEvent {
    let mut event = ResponseEvent::new(
        kernel.clone(),
        Handle::new(request),
        request_type,
        response.clone(),
        None,
    );
    dispatcher
        .dispatch(&mut event, Some(KernelEvents::RESPONSE))
        .unwrap();
    event
}

fn header(response: &ResponseHandle, name: &str) -> Option<PhpString> {
    response.with(|r| r.headers().get(name, None))
}

#[test]
fn testFilterDoesNothingForSubRequests() {
    let (dispatcher, kernel) = set_up();
    let response = response("foo");

    let event = dispatch(
        &dispatcher,
        &kernel,
        Request::default(),
        <dyn HttpKernelInterface>::SUB_REQUEST,
        &response,
    );

    // assertEquals('', null): the header is not set.
    assert_eq!(None, header(event.get_response(), "content-type"));
}

#[test]
fn testFilterSetsNonDefaultCharsetIfNotOverridden() {
    let (dispatcher, kernel) = set_up();
    add(&dispatcher, ResponseListener::new("ISO-8859-15", false), 1);

    let response = response("foo");

    dispatch(
        &dispatcher,
        &kernel,
        Request::create("/").unwrap(),
        <dyn HttpKernelInterface>::MAIN_REQUEST,
        &response,
    );

    assert_eq!(
        Some(PhpString::from("ISO-8859-15")),
        response.with(|r| r.get_charset().cloned())
    );
}

#[test]
fn testFilterDoesNothingIfCharsetIsOverridden() {
    let (dispatcher, kernel) = set_up();
    add(&dispatcher, ResponseListener::new("ISO-8859-15", false), 1);

    let response = response("foo");
    response.with_mut(|r| {
        r.set_charset("ISO-8859-1");
    });

    dispatch(
        &dispatcher,
        &kernel,
        Request::create("/").unwrap(),
        <dyn HttpKernelInterface>::MAIN_REQUEST,
        &response,
    );

    assert_eq!(
        Some(PhpString::from("ISO-8859-1")),
        response.with(|r| r.get_charset().cloned())
    );
}

#[test]
fn testFiltersSetsNonDefaultCharsetIfNotOverriddenOnNonTextContentType() {
    let (dispatcher, kernel) = set_up();
    add(&dispatcher, ResponseListener::new("ISO-8859-15", false), 1);

    let response = response("foo");
    let mut request = Request::create("/").unwrap();
    request.set_request_format(Some(PhpString::from("application/json")));

    dispatch(
        &dispatcher,
        &kernel,
        request,
        <dyn HttpKernelInterface>::MAIN_REQUEST,
        &response,
    );

    assert_eq!(
        Some(PhpString::from("ISO-8859-15")),
        response.with(|r| r.get_charset().cloned())
    );
}

/// The test passes a third argument, `['fr', 'en']`, which the constructor
/// does not declare and PHP ignores.
#[test]
fn testSetContentLanguageHeaderWhenEmptyAndAtLeast2EnabledLocalesAreConfigured() {
    let (dispatcher, kernel) = set_up();
    add(&dispatcher, ResponseListener::new("ISO-8859-15", true), 1);

    let response = response("content");
    let mut request = Request::create("/").unwrap();
    request.set_locale("fr");

    dispatch(
        &dispatcher,
        &kernel,
        request,
        <dyn HttpKernelInterface>::MAIN_REQUEST,
        &response,
    );

    assert_eq!(
        Some(PhpString::from("fr")),
        header(&response, "Content-Language")
    );
}

/// The third argument, `['de']`, is ignored as above.
#[test]
fn testNotOverrideContentLanguageHeaderWhenNotEmpty() {
    let (dispatcher, kernel) = set_up();
    add(&dispatcher, ResponseListener::new("ISO-8859-15", true), 1);

    let response = response("content");
    response.with_mut(|r| {
        r.headers_mut()
            .set(b"Content-Language", "mi, en", true)
            .unwrap();
    });
    let mut request = Request::create("/").unwrap();
    request.set_locale("de");

    dispatch(
        &dispatcher,
        &kernel,
        request,
        <dyn HttpKernelInterface>::MAIN_REQUEST,
        &response,
    );

    assert_eq!(
        Some(PhpString::from("mi, en")),
        header(&response, "Content-Language")
    );
}

#[test]
fn testNotSetContentLanguageHeaderWhenDisabled() {
    let (dispatcher, kernel) = set_up();
    add(&dispatcher, ResponseListener::new("ISO-8859-15", false), 1);

    let response = response("content");
    let mut request = Request::create("/").unwrap();
    request.set_locale("fr");

    dispatch(
        &dispatcher,
        &kernel,
        request,
        <dyn HttpKernelInterface>::MAIN_REQUEST,
        &response,
    );

    assert_eq!(None, header(&response, "Content-Language"));
}
