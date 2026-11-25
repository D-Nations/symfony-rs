//! RouterListenerTest.php at symfony/http-kernel v8.1.11. Names and
//! assertions are PHPUnit's; a data provider's sets run in one test.
//! PHPUnit's stubs and mocks are small structs that count their calls.
//!
//! `new ControllerResolver()` and `new ArgumentResolver()` are not ported:
//! where a test hands them to the kernel and never reaches them (routing
//! fails first), a resolver without a controller stands in; where it
//! resolves a closure in `_controller`, the closure is looked up by the
//! name the attribute holds.
//!
//! Upstream: symfony/http-kernel Tests/EventListener/RouterListenerTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock, Weak,
        atomic::{AtomicUsize, Ordering},
    },
};

use php_value::{Handle, PhpArray, PhpString, Value, php_array};
use psr_log::{Context, ContextValue, LogLevel, LoggerInterface};
use symfony_event_dispatcher::{EventDispatcher, EventDispatcherInterface};
use symfony_http_foundation::{
    header_bag::ResponseHeaderBag,
    request::Request,
    request_stack::{RequestHandle, RequestStack},
    response::Response,
};
use symfony_http_kernel::{
    controller::{
        ArgumentResolverInterface, Controller, ControllerResolverInterface, ControllerResult,
        Parameter, ResponseHandle,
    },
    event::RequestEvent,
    exception::{HttpException, Throwable, class_of, instance_of, message_of},
    http_kernel::{HttpKernel, HttpKernelInterface, KernelRef, Options as KernelOptions},
    router_listener::{
        Matcher, Options, RequestMatcherInterface, RouterListener, UrlMatcherInterface,
    },
};
use symfony_routing::{
    exception::{Class, Error as RoutingError},
    request_context::RequestContext,
};

const BAD_REQUEST_HTTP: &str = "Symfony\\Component\\HttpKernel\\Exception\\BadRequestHttpException";
const BAD_REQUEST: &str = "Symfony\\Component\\HttpFoundation\\Exception\\BadRequestException";
const NOT_FOUND_HTTP: &str = "Symfony\\Component\\HttpKernel\\Exception\\NotFoundHttpException";
const METHOD_NOT_ALLOWED_HTTP: &str =
    "Symfony\\Component\\HttpKernel\\Exception\\MethodNotAllowedHttpException";

/// `$this->createStub(HttpKernelInterface::class)`.
struct StubKernel;

impl HttpKernelInterface for StubKernel {
    fn handle(&self, _: &RequestHandle, _: i64, _: bool) -> Result<ResponseHandle, Throwable> {
        Ok(Handle::new(Response::default()))
    }
}

fn stub_kernel() -> KernelRef {
    Arc::new(StubKernel)
}

/// A stub or mock of `UrlMatcherInterface`.
struct UrlStub {
    context: Handle<RequestContext>,
    result: Result<PhpArray, RoutingError>,
}

impl UrlMatcherInterface for UrlStub {
    fn get_context(&self) -> Handle<RequestContext> {
        self.context.clone()
    }

    fn match_path(&self, _: &[u8]) -> Result<PhpArray, Throwable> {
        self.result.clone().map_err(|e| Arc::new(e) as Throwable)
    }
}

/// A stub or mock of `RequestMatcherInterface`, counting its calls.
struct RequestStub {
    result: Result<PhpArray, RoutingError>,
    calls: AtomicUsize,
}

impl RequestStub {
    fn new(result: Result<PhpArray, RoutingError>) -> Arc<Self> {
        Arc::new(Self {
            result,
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl RequestMatcherInterface for RequestStub {
    fn match_request(&self, _: &RequestHandle) -> Result<PhpArray, Throwable> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone().map_err(|e| Arc::new(e) as Throwable)
    }
}

/// A mock of `LoggerInterface`.
#[derive(Default)]
struct Logger(Mutex<Vec<(LogLevel, String, Context)>>);

impl LoggerInterface for Logger {
    fn log(&self, level: LogLevel, message: &str, context: Context) {
        self.0
            .lock()
            .unwrap()
            .push((level, message.to_owned(), context));
    }
}

/// A controller resolver for `_controller` closures, which the request
/// names: `$request->attributes->get('_controller')` is the closure.
#[derive(Default)]
struct ClosureResolver(Mutex<HashMap<Vec<u8>, Controller>>);

impl ClosureResolver {
    fn set(&self, request: &RequestHandle, name: &str, controller: Controller) {
        self.0
            .lock()
            .unwrap()
            .insert(name.as_bytes().to_vec(), controller);
        request.with_mut(|r| r.attributes_mut().set("_controller", Value::string(name)));
    }
}

impl ControllerResolverInterface for ClosureResolver {
    fn get_controller(&self, request: &RequestHandle) -> Result<Option<Controller>, Throwable> {
        let name = request.with(|r| r.attributes().get("_controller", Value::Null));
        let Value::String(name) = name else {
            return Ok(None);
        };
        Ok(self.0.lock().unwrap().get(name.as_bytes()).cloned())
    }
}

/// `new ArgumentResolver()` for controllers without parameters.
struct NoArguments;

impl ArgumentResolverInterface for NoArguments {
    fn get_arguments(
        &self,
        _: &RequestHandle,
        _: &Controller,
        _: &[Parameter],
    ) -> Result<Vec<Value>, Throwable> {
        Ok(Vec::new())
    }
}

fn kernel(
    dispatcher: &Arc<EventDispatcher>,
    resolver: Arc<dyn ControllerResolverInterface>,
    request_stack: &RequestStack,
) -> Arc<HttpKernel> {
    HttpKernel::new(
        dispatcher.clone(),
        resolver,
        KernelOptions {
            request_stack: Some(request_stack.clone()),
            argument_resolver: Arc::new(NoArguments),
            handle_all_throwables: false,
        },
    )
}

fn create(uri: &str) -> RequestHandle {
    Handle::new(Request::create(uri).unwrap())
}

fn with_context(context: RequestContext) -> Options {
    Options {
        context: Some(Handle::new(context)),
        ..Options::default()
    }
}

fn set_header(request: &RequestHandle, name: &str, value: &str) {
    request.with_mut(|r| {
        r.headers_mut().set(name.as_bytes(), value, true).unwrap();
    });
}

fn create_request_event_for_uri(uri: &str) -> RequestEvent {
    let request = create(uri);
    // Prevents going in to routing process
    request.with_mut(|r| r.attributes_mut().set("_controller", Value::Null));
    RequestEvent::new(
        stub_kernel(),
        request,
        Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
    )
}

fn get_port_data() -> Vec<(i64, i64, &'static str, i64, i64)> {
    vec![
        (80, 443, "http://localhost/", 80, 443),
        (80, 443, "http://localhost:90/", 90, 443),
        (80, 443, "https://localhost/", 80, 443),
        (80, 443, "https://localhost:90/", 80, 90),
    ]
}

#[test]
fn testPort() {
    for (default_http_port, default_https_port, uri, expected_http_port, expected_https_port) in
        get_port_data()
    {
        let context = Handle::new(RequestContext::default());
        context.with_mut(|c| {
            c.set_http_port(default_http_port);
            c.set_https_port(default_https_port);
        });
        let url_matcher = Arc::new(UrlStub {
            context: context.clone(),
            result: Ok(PhpArray::new()),
        });

        let listener = RouterListener::new(
            Matcher::Url(url_matcher),
            RequestStack::default(),
            Options::default(),
        )
        .unwrap();
        let mut event = create_request_event_for_uri(uri);
        listener.on_kernel_request(&mut event).unwrap();

        assert_eq!(expected_http_port, context.with(RequestContext::http_port));
        assert_eq!(
            expected_https_port,
            context.with(RequestContext::https_port)
        );
        assert_eq!(
            if uri.starts_with("https") {
                "https"
            } else {
                "http"
            },
            context.with(|c| c.scheme().to_string_lossy().into_owned())
        );
    }
}

#[test]
fn testRequestMatcher() {
    let request = create("http://localhost/");
    let mut event = RequestEvent::new(
        stub_kernel(),
        request,
        Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
    );

    let request_matcher = RequestStub::new(Ok(PhpArray::new()));

    let listener = RouterListener::new(
        Matcher::Request(request_matcher.clone()),
        RequestStack::default(),
        with_context(RequestContext::default()),
    )
    .unwrap();
    listener.on_kernel_request(&mut event).unwrap();

    assert_eq!(1, request_matcher.calls());
}

#[test]
fn testSubRequestWithDifferentMethod() {
    let request = Handle::new(
        Request::create_with(
            "http://localhost/",
            symfony_http_foundation::request::Create {
                method: Some(PhpString::from("post")),
                ..Default::default()
            },
        )
        .unwrap(),
    );
    let mut event = RequestEvent::new(
        stub_kernel(),
        request,
        Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
    );

    let request_matcher = RequestStub::new(Ok(PhpArray::new()));

    let context = RequestContext::default();

    let listener = RouterListener::new(
        Matcher::Request(request_matcher),
        RequestStack::default(),
        with_context(RequestContext::default()),
    )
    .unwrap();
    listener.on_kernel_request(&mut event).unwrap();

    // sub-request with another HTTP method
    let request = Handle::new(
        Request::create_with(
            "http://localhost/",
            symfony_http_foundation::request::Create {
                method: Some(PhpString::from("get")),
                ..Default::default()
            },
        )
        .unwrap(),
    );
    let mut event = RequestEvent::new(
        stub_kernel(),
        request,
        Some(<dyn HttpKernelInterface>::SUB_REQUEST),
    );

    listener.on_kernel_request(&mut event).unwrap();

    assert_eq!("GET", context.method().to_string_lossy());
}

fn get_logging_parameter_data() -> Vec<(PhpArray, &'static str, Context)> {
    let text = |s: &str| ContextValue::Value(Value::string(s));
    vec![
        (
            php_array!["_route" => "foo"],
            "Matched route \"{route}\".",
            vec![
                ("route".to_owned(), text("foo")),
                (
                    "route_parameters".to_owned(),
                    ContextValue::Value(Value::Array(php_array!["_route" => "foo"])),
                ),
                ("request_uri".to_owned(), text("http://localhost/")),
                ("method".to_owned(), text("GET")),
            ],
        ),
        (
            PhpArray::new(),
            "Matched route \"{route}\".",
            vec![
                ("route".to_owned(), text("n/a")),
                (
                    "route_parameters".to_owned(),
                    ContextValue::Value(Value::Array(PhpArray::new())),
                ),
                ("request_uri".to_owned(), text("http://localhost/")),
                ("method".to_owned(), text("GET")),
            ],
        ),
    ]
}

#[test]
fn testLoggingParameter() {
    for (parameter, log, parameters) in get_logging_parameter_data() {
        let request_matcher = RequestStub::new(Ok(parameter));

        let logger = Arc::new(Logger::default());

        let request = create("http://localhost/");

        let listener = RouterListener::new(
            Matcher::Request(request_matcher.clone()),
            RequestStack::default(),
            Options {
                logger: Some(logger.clone()),
                ..with_context(RequestContext::default())
            },
        )
        .unwrap();
        listener
            .on_kernel_request(&mut RequestEvent::new(
                stub_kernel(),
                request,
                Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
            ))
            .unwrap();

        assert_eq!(1, request_matcher.calls());
        let records = logger.0.lock().unwrap();
        assert_eq!(1, records.len());
        assert_eq!(LogLevel::Info, records[0].0);
        assert_eq!(log, records[0].1);
        assert_eq!(parameters, records[0].2);
    }
}

#[test]
fn testNoRoutingConfigurationResponse() {
    let request_stack = RequestStack::default();

    let request_matcher = RequestStub::new(Err(RoutingError::new(Class::NoConfiguration, "")));

    let dispatcher = Arc::new(EventDispatcher::new());
    dispatcher
        .add_subscriber(Arc::new(
            RouterListener::new(
                Matcher::Request(request_matcher.clone()),
                request_stack.clone(),
                with_context(RequestContext::default()),
            )
            .unwrap(),
        ))
        .unwrap();

    let kernel = kernel(
        &dispatcher,
        Arc::new(ClosureResolver::default()),
        &request_stack,
    );

    let request = create("http://localhost/");

    for _ in 0..2 {
        let response = kernel.handle(&request, 1, true).unwrap();
        assert_eq!(404, response.with(Response::get_status_code));
        assert!(
            response
                .with(|r| r.get_content().to_string_lossy().into_owned())
                .contains("Welcome")
        );
    }
    assert_eq!(2, request_matcher.calls());
}

#[test]
fn testRoutingContextOfTheParentIsRestoredAfterNestedMainRequest() {
    let context = Handle::new(RequestContext::default());
    let matcher = Arc::new(UrlStub {
        context: context.clone(),
        result: Ok(PhpArray::new()),
    });

    let request_stack = RequestStack::default();
    let dispatcher = Arc::new(EventDispatcher::new());
    dispatcher
        .add_subscriber(Arc::new(
            RouterListener::new(
                Matcher::Url(matcher),
                request_stack.clone(),
                Options {
                    context: Some(context.clone()),
                    ..Options::default()
                },
            )
            .unwrap(),
        ))
        .unwrap();
    let resolver = Arc::new(ClosureResolver::default());
    let kernel = kernel(&dispatcher, resolver.clone(), &request_stack);
    let weak: Arc<OnceLock<Weak<HttpKernel>>> = Arc::new(OnceLock::new());
    weak.set(Arc::downgrade(&kernel)).unwrap();

    let request = create("http://example.com/");
    let main = {
        let (weak, context, resolver) = (weak.clone(), context.clone(), resolver.clone());
        Controller::new(move |_| {
            let kernel = weak.get().and_then(Weak::upgrade).unwrap();
            let nested_request = create("https://example.org/");
            resolver.set(
                &nested_request,
                "nested",
                Controller::new(|_| {
                    Ok(ControllerResult::Response(Handle::new(Response::default())))
                }),
            );
            kernel.handle(&nested_request, 1, true)?;

            let body = context.with(|c| {
                format!(
                    "{}://{}",
                    c.scheme().to_string_lossy(),
                    c.host().to_string_lossy()
                )
            });
            Ok(ControllerResult::Response(Handle::new(
                Response::new(Some(body), 200, ResponseHeaderBag::empty()).unwrap(),
            )))
        })
    };
    resolver.set(&request, "main", main);

    let response = kernel.handle(&request, 1, true).unwrap();
    assert_eq!(
        "http://example.com",
        response.with(|r| r.get_content().to_string_lossy().into_owned())
    );
}

#[test]
fn testRequestWithBadHost() {
    let request = create("/");
    set_header(&request, "host", "bad host %22");
    let mut event = RequestEvent::new(
        stub_kernel(),
        request,
        Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
    );

    let request_matcher = RequestStub::new(Ok(PhpArray::new()));

    let listener = RouterListener::new(
        Matcher::Request(request_matcher),
        RequestStack::default(),
        with_context(RequestContext::default()),
    )
    .unwrap();
    match listener.on_kernel_request(&mut event) {
        Ok(()) => panic!("Expected \"{BAD_REQUEST_HTTP}\" or \"{BAD_REQUEST}\" to be thrown."),
        Err(e) => assert!(
            instance_of(e.as_ref(), BAD_REQUEST_HTTP) || instance_of(e.as_ref(), BAD_REQUEST),
            "{}",
            class_of(e.as_ref())
        ),
    }
}

#[test]
fn testResourceNotFoundException() {
    let context = Handle::new(RequestContext::default());

    let url_matcher = Arc::new(UrlStub {
        context,
        result: Err(RoutingError::new(Class::ResourceNotFound, "")),
    });

    let request = create("https://www.symfony.com/path");
    set_header(&request, "referer", "https://www.google.com");

    let mut event = RequestEvent::new(
        stub_kernel(),
        request,
        Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
    );

    let listener = RouterListener::new(
        Matcher::Url(url_matcher),
        RequestStack::default(),
        Options::default(),
    )
    .unwrap();
    let e = listener.on_kernel_request(&mut event).unwrap_err();
    assert!(instance_of(e.as_ref(), NOT_FOUND_HTTP));
    assert!(message_of(e.as_ref()).to_string_lossy().contains(
        "No route found for \"GET https://www.symfony.com/path\" (from \"https://www.google.com\")"
    ));
}

#[test]
fn testMethodNotAllowedException() {
    let context = Handle::new(RequestContext::default());

    let url_matcher = Arc::new(UrlStub {
        context,
        result: Err(RoutingError::method_not_allowed(["POST"], "")),
    });

    let request = create("https://www.symfony.com/path");

    let mut event = RequestEvent::new(
        stub_kernel(),
        request,
        Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
    );

    let listener = RouterListener::new(
        Matcher::Url(url_matcher),
        RequestStack::default(),
        Options::default(),
    )
    .unwrap();
    let e = listener.on_kernel_request(&mut event).unwrap_err();
    assert!(instance_of(e.as_ref(), METHOD_NOT_ALLOWED_HTTP));
    assert!(message_of(e.as_ref()).to_string_lossy().contains(
        "No route found for \"GET https://www.symfony.com/path\": Method Not Allowed (Allow: POST)"
    ));
    let headers = e.downcast_ref::<HttpException>().unwrap().get_headers();
    assert_eq!(&php_array!["Allow" => "POST"], headers);
}

fn provide_route_mapping() -> Vec<(PhpArray, PhpArray)> {
    vec![
        (
            php_array!["conference" => "vienna-2024"],
            php_array![
                "slug" => "vienna-2024",
                "_route_mapping" => php_array!["slug" => "conference"],
            ],
        ),
        (
            php_array![
                "article" => php_array![
                    "id" => "abc123",
                    "date" => "2024-04-24",
                    "slug" => "symfony-rocks",
                ],
            ],
            php_array![
                "id" => "abc123",
                "date" => "2024-04-24",
                "slug" => "symfony-rocks",
                "_route_mapping" => php_array![
                    "id" => "article",
                    "date" => "article",
                    "slug" => "article",
                ],
            ],
        ),
        (
            php_array!["conference" => php_array!["slug" => "vienna-2024"]],
            php_array![
                "slug" => "vienna-2024",
                "_route_mapping" => php_array![
                    "slug" => php_array!["conference", "slug"],
                ],
            ],
        ),
        (
            php_array![
                "article" => php_array![
                    "id" => "abc123",
                    "date" => "2024-04-24",
                    "slug" => "symfony-rocks",
                ],
            ],
            php_array![
                "id" => "abc123",
                "date" => "2024-04-24",
                "slug" => "symfony-rocks",
                "_route_mapping" => php_array![
                    "id" => php_array!["article", "id"],
                    "date" => php_array!["article", "date"],
                    "slug" => php_array!["article", "slug"],
                ],
            ],
        ),
    ]
}

#[test]
fn testRouteMapping() {
    for (mut expected, mut parameters) in provide_route_mapping() {
        let request = create("http://localhost/");
        let mut event = RequestEvent::new(
            stub_kernel(),
            request.clone(),
            Some(<dyn HttpKernelInterface>::MAIN_REQUEST),
        );

        let request_matcher = RequestStub::new(Ok(parameters.clone()));

        let listener = RouterListener::new(
            Matcher::Request(request_matcher),
            RequestStack::default(),
            with_context(RequestContext::default()),
        )
        .unwrap();
        listener.on_kernel_request(&mut event).unwrap();

        let mapping = parameters.get(&"_route_mapping".into()).cloned().unwrap();
        expected.insert("_route_mapping", mapping);
        parameters.remove(&"_route_mapping".into());
        expected.insert("_route_params", Value::Array(parameters));

        // assertEquals() on arrays: same pairs, in any order.
        let mut actual = request.with(|r| r.attributes().all().clone());
        actual.ksort();
        expected.ksort();
        assert_eq!(expected, actual);
    }
}

/// A `RouterListener` without a context and a matcher that does not give
/// one cannot be built.
#[test]
fn constructor_requires_a_context() {
    let e = RouterListener::new(
        Matcher::Request(RequestStub::new(Ok(PhpArray::new()))),
        RequestStack::default(),
        Options::default(),
    )
    .unwrap_err();
    assert_eq!("InvalidArgumentException", class_of(e.as_ref()));
    assert_eq!(
        "You must either pass a RequestContext or the matcher must implement RequestContextAwareInterface.",
        message_of(e.as_ref()).to_string_lossy()
    );
}

/// The welcome page is upstream's, filled in.
#[test]
fn welcome_page_is_filled() {
    let page = symfony_http_kernel::router_listener::welcome_page(42, b"/app/");
    let page = String::from_utf8(page).unwrap();
    assert!(page.contains("--hue: 42;"));
    assert!(
        page.contains("<code translate=\"no\" class=\"notranslate project_dir_path\">/app/</code>")
    );
    assert!(page.contains("You are using Symfony <strong>8.1</strong> version"));
    assert!(!page.contains("@@"));
}
