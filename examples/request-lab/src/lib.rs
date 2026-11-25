//! A second application using the request crates, without demo
//! dependencies, on the ported Symfony kernel: its routes are a routing
//! `RouteCollection` matched by a `RedirectableUrlMatcher` through the
//! `RouterListener`; its controllers are methods of a declared class
//! (`App\Controller\LabController`) found by the `ControllerResolver`, with
//! their arguments from the `ArgumentResolver`, run by `HttpKernel`; an
//! exception listener answers its HTTP exceptions. `symfony-runtime`'s
//! `FrontController` serves it as nginx and PHP-FPM would. These
//! diagnostic routes are examples, not ports of demo controllers.
//!
//! Upstream: none

#![forbid(unsafe_code)]

use std::{
    net::SocketAddr,
    sync::{Arc, LazyLock},
};

use axum::Router;
use php_value::{
    Handle, PhpArray, PhpString, Value,
    json::{
        JSON_FORCE_OBJECT, JSON_INVALID_UTF8_SUBSTITUTE, JSON_UNESCAPED_SLASHES,
        JSON_UNESCAPED_UNICODE,
    },
};
use symfony_event_dispatcher::{EventDispatcher, EventDispatcherInterface, Listener};
use symfony_http_foundation::{
    POST_MAX_SIZE,
    header_bag::ResponseHeaderBag,
    json_response::JsonResponse,
    request::Request,
    request_stack::{RequestHandle, RequestStack},
    response::Response,
    sapi::Ini,
};
use symfony_http_kernel::{
    argument_resolver::ArgumentResolver,
    controller::{Controller, ControllerResult, Parameter, REQUEST_CLASS, as_request},
    controller_resolver::{Class, ClassRegistry, ControllerResolver, Method},
    event::ExceptionEvent,
    exception::{
        HTTP_EXCEPTION_INTERFACE, HttpArgs, HttpException, Throwable, instance_of, message_of,
    },
    http_kernel::{HttpKernel, KernelEvents, Options as KernelOptions, response_handle},
    router_listener::{Matcher, Options as RouterOptions, RouterListener},
};
use symfony_routing::{
    exception::Error as RoutingError,
    redirectable_url_matcher::{RedirectableUrlMatcher, RedirectableUrlMatcherInterface},
    request_context::RequestContext,
    route::Route,
    route_collection::RouteCollection,
};
use symfony_runtime::{
    http_kernel_runner::RunnableKernel,
    server::{FrontController, ServerConfig},
};

/// The controller class the routes name.
const LAB: &str = "App\\Controller\\LabController";

const BAD_REQUEST_HTTP: &str = "Symfony\\Component\\HttpKernel\\Exception\\BadRequestHttpException";

/// The JSON the lab writes, as it wrote it before it ran on the kernel:
/// every array an object, `/` and non-ASCII unescaped, invalid UTF-8
/// replaced.
const JSON_OPTIONS: i64 = JSON_FORCE_OBJECT
    | JSON_UNESCAPED_SLASHES
    | JSON_UNESCAPED_UNICODE
    | JSON_INVALID_UTF8_SUBSTITUTE;

/// A requirement for `1` to `PHP_INT_MAX` (`i64::MAX`): an id that fits.
pub fn positive_int64() -> String {
    let max = i64::MAX.to_string();
    let digits = max.as_bytes();
    let mut alternatives = vec![format!("[1-9][0-9]{{0,{}}}", digits.len() - 2)];
    for (at, &digit) in digits.iter().enumerate() {
        let low = if at == 0 { b'1' } else { b'0' };
        if digit > low {
            alternatives.push(format!(
                "{}[{}-{}][0-9]{{{}}}",
                &max[..at],
                char::from(low),
                char::from(digit - 1),
                digits.len() - at - 1
            ));
        }
    }
    alternatives.push(max.clone());
    alternatives.join("|")
}

fn route(path: &str, methods: &[&str], action: &str) -> Result<Route, RoutingError> {
    let mut route = Route::new(path)?;
    route.set_methods(methods.iter().copied());
    route.set_default(
        b"_controller",
        Value::String(PhpString::from(format!("{LAB}::{action}"))),
    )?;
    Ok(route)
}

fn routes() -> Result<RouteCollection, RoutingError> {
    let mut routes = RouteCollection::new();
    routes.add("inspect", route("/inspect", &[], "inspect")?, 0);
    routes.add("redirect", route("/redirect", &["GET"], "redirect")?, 0);
    routes.add("error", route("/error", &["GET"], "error")?, 0);
    let mut item = route("/item/{id}", &["GET"], "item")?;
    item.set_requirement(b"id", positive_int64().as_bytes())?;
    routes.add("item", item, 0);
    Ok(routes)
}

/// `RedirectableUrlMatcher::redirect()`: the lab's own permanent redirect
/// to the path with its trailing slash fixed.
struct SlashRedirect;

impl RedirectableUrlMatcherInterface for SlashRedirect {
    fn redirect(
        &self,
        path: &[u8],
        route: &PhpString,
        _scheme: Option<&[u8]>,
    ) -> Result<PhpArray, RoutingError> {
        let mut attributes = PhpArray::new();
        attributes.insert(
            "_controller",
            Value::String(PhpString::from(format!("{LAB}::slashRedirect"))),
        );
        attributes.insert("path", Value::String(PhpString::from(path)));
        attributes.insert("_route", Value::String(route.clone()));
        Ok(attributes)
    }
}

/// The matcher and the declared controller class, built once.
struct App {
    matcher: Arc<RedirectableUrlMatcher<SlashRedirect>>,
    classes: Arc<ClassRegistry>,
}

static APP: LazyLock<Result<App, String>> = LazyLock::new(|| {
    let routes = routes().map_err(|e| e.to_string())?;
    let matcher = Arc::new(RedirectableUrlMatcher::new(
        routes,
        RequestContext::default(),
        SlashRedirect,
    ));
    let mut classes = ClassRegistry::new();
    classes
        .declare_class(lab_controller())
        .map_err(|e| e.to_string())?;
    Ok(App {
        matcher,
        classes: Arc::new(classes),
    })
});

fn throwable(error: impl std::error::Error + Send + Sync + 'static) -> Throwable {
    Arc::new(error)
}

fn request_parameter() -> Parameter {
    Parameter::new("request").typed(REQUEST_CLASS)
}

fn action(
    name: &str,
    parameters: Vec<Parameter>,
    body: impl Fn(&[Value]) -> Result<ControllerResult, Throwable> + Send + Sync + 'static,
) -> Method {
    Method::new(
        name,
        Controller::new(body)
            .with_name(format!("{LAB}::{name}"))
            .with_parameters(parameters),
    )
}

fn request_of(arguments: &[Value]) -> Result<RequestHandle, Throwable> {
    arguments
        .first()
        .and_then(as_request)
        .ok_or_else(|| throwable(std::io::Error::other("the request argument is missing")))
}

/// `$response = new JsonResponse(null, $status);
/// $response->setEncodingOptions(...); $response->setData($data);`
fn json_response(data: PhpArray, status: i64) -> Result<Response, Throwable> {
    let mut response = JsonResponse::new(Value::Null, status, Vec::<(&str, &str)>::new(), false)
        .map_err(throwable)?;
    response
        .set_encoding_options(JSON_OPTIONS)
        .map_err(throwable)?;
    response.set_data(&Value::Array(data)).map_err(throwable)?;
    Ok(response.into())
}

fn json(data: PhpArray, status: i64) -> Result<ControllerResult, Throwable> {
    Ok(ControllerResult::Response(response_handle(json_response(
        data, status,
    )?)))
}

/// An empty response with a `Location`.
fn redirect(status: i64, location: PhpString) -> Result<ControllerResult, Throwable> {
    let headers = ResponseHeaderBag::new([("Location", location)]).map_err(throwable)?;
    let response = Response::new(Some(""), status, headers).map_err(throwable)?;
    Ok(ControllerResult::Response(response_handle(response)))
}

/// `App\Controller\LabController`.
fn lab_controller() -> Class {
    Class::new(LAB)
        .with_method(action("inspect", vec![request_parameter()], |args| {
            request_of(args)?.with_mut(inspect)
        }))
        .with_method(action("redirect", Vec::new(), |_| {
            redirect(303, PhpString::from("/inspect"))
        }))
        .with_method(action("error", Vec::new(), |_| {
            Err(HttpException::bad_request(HttpArgs::message("Example bad request")).throwable())
        }))
        .with_method(action("item", vec![request_parameter()], |args| {
            let path = request_of(args)?
                .with_mut(Request::get_path_info)
                .map_err(throwable)?;
            let mut data = PhpArray::new();
            data.insert("path", Value::String(path));
            json(data, 200)
        }))
        .with_method(action(
            "slashRedirect",
            vec![request_parameter(), Parameter::new("path").typed("string")],
            |args| {
                let request = request_of(args)?;
                let Some(Value::String(path)) = args.get(1) else {
                    return Err(throwable(std::io::Error::other("the path is missing")));
                };
                // The query string as sent, as the lab redirected before.
                let mut location = path.as_bytes().to_vec();
                let query = request.with(|r| r.server().get("QUERY_STRING", Value::Null));
                if let Value::String(query) = query
                    && !query.as_bytes().is_empty()
                {
                    location.push(b'?');
                    location.extend_from_slice(query.as_bytes());
                }
                redirect(301, PhpString::from(location))
            },
        ))
}

/// `/inspect`: what the request holds, through the Request's bags.
fn inspect(request: &mut Request) -> Result<ControllerResult, Throwable> {
    // InputBag's scalar contract as well as its array view.
    let Ok(scalar) = request.query().get("scalar", Value::Null) else {
        let mut data = PhpArray::new();
        data.insert("error", Value::string("scalar must be a string"));
        return json(data, 400);
    };
    let language = request
        .get_preferred_language(Some(&["en", "fr"]))
        .map_err(throwable)?;
    let scheme = request.get_scheme().map_err(throwable)?;
    let mut data = PhpArray::new();
    data.insert("query", Value::Array(request.query().all().clone()));
    data.insert("form", Value::Array(request.request().all().clone()));
    data.insert("cookies", Value::Array(request.cookies().all().clone()));
    data.insert("scalar", scalar);
    data.insert("language", language.map_or(Value::Null, Value::String));
    data.insert("scheme", Value::string(scheme));
    json(data, 200)
}

/// The lab's `kernel.exception` listener: a bad request answers with its
/// message as JSON, any other HTTP exception with no body (the kernel
/// sets the exception's status and headers); other throwables are left to
/// PHP (a 500). This lab deliberately exposes its fixed example error;
/// applications should log internal failures and use their own public
/// error pages.
fn on_exception(event: &mut ExceptionEvent) -> Result<(), Throwable> {
    let error = event.get_throwable().clone();
    if !instance_of(error.as_ref(), HTTP_EXCEPTION_INTERFACE) {
        return Ok(());
    }
    let response = if instance_of(error.as_ref(), BAD_REQUEST_HTTP) {
        let mut data = PhpArray::new();
        data.insert("error", Value::String(message_of(error.as_ref())));
        json_response(data, 200)?
    } else {
        Response::new(
            Some(""),
            200,
            ResponseHeaderBag::new(Vec::<(&str, &str)>::new()).map_err(throwable)?,
        )
        .map_err(throwable)?
    };
    event.set_response(response_handle(response));
    Ok(())
}

/// The kernel of one request, as the front controller builds it.
fn kernel() -> Result<Arc<dyn RunnableKernel>, Throwable> {
    let app = APP
        .as_ref()
        .map_err(|e| throwable(std::io::Error::other(e.clone())))?;
    let request_stack = RequestStack::default();
    let dispatcher = Arc::new(EventDispatcher::new());
    let listener = RouterListener::new(
        Matcher::Request(app.matcher.clone()),
        request_stack.clone(),
        RouterOptions {
            context: Some(Handle::new(RequestContext::default())),
            debug: false,
            ..RouterOptions::default()
        },
    )?;
    dispatcher
        .add_subscriber(Arc::new(listener))
        .map_err(throwable)?;
    dispatcher
        .add_listener(
            KernelEvents::EXCEPTION,
            Listener::closure(|event, _, _| {
                let Some(event) = event.downcast_mut::<ExceptionEvent>() else {
                    return Ok(());
                };
                // The kernel unboxes the shared throwable (`from_box`).
                on_exception(event).map_err(|e| Box::new(e) as _)
            }),
            0,
        )
        .map_err(throwable)?;
    let kernel: Arc<dyn RunnableKernel> = HttpKernel::new(
        dispatcher,
        Arc::new(ControllerResolver::new(app.classes.clone(), None)),
        KernelOptions {
            request_stack: Some(request_stack),
            argument_resolver: Arc::new(ArgumentResolver::default()),
            handle_all_throwables: true,
        },
    );
    Ok(kernel)
}

/// The lab served at `address` (`SERVER_ADDR` and `SERVER_PORT`).
pub fn router_at(address: SocketAddr) -> Router {
    let server = ServerConfig {
        server_addr: address,
        // The lab read bodies up to `post_max_size` before it ran on the
        // kernel; PHP drops a form past it, the web server answers 413
        // past twice that.
        client_max_body_size: 2 * POST_MAX_SIZE,
        ..ServerConfig::default()
    };
    Arc::new(FrontController::new(
        server,
        Ini::default(),
        Arc::new(kernel),
    ))
    .router()
}

/// The lab on its default address, `127.0.0.1:8012`.
pub fn router() -> Router {
    router_at(SocketAddr::from(([127, 0, 0, 1], 8012)))
}
