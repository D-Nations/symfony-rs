//! `FrameworkBundle`'s two pieces on the fixture's path: the redirect a
//! `RedirectableCompiledUrlMatcher` makes of a trailing-slash match, and
//! `RedirectController::urlRedirectAction()`, the controller it names. They
//! belong in a `FrameworkBundle` crate (M3); `parse_url()` is not ported, so a
//! `path` that does not start with `/` is refused (`NotPorted`) unless
//! `//` made it a full URL.
//!
//! The matcher is the ported `RedirectableUrlMatcher` with this `redirect()`;
//! Symfony's compiled matcher answers as it does (routing's recorded
//! vectors).
//!
//! Upstream: symfony/framework-bundle Routing/RedirectableCompiledUrlMatcher.php Controller/RedirectController.php

use std::sync::Arc;

use php_value::{PhpArray, PhpObject, PhpString, Value};
use symfony_http_foundation::{
    redirect_response::RedirectResponse, request::Request, request_stack::RequestHandle,
    response::Response,
};
use symfony_http_kernel::{
    controller::{Controller, ControllerResult, Parameter, REQUEST_CLASS, as_request},
    controller_resolver::{Class, Method},
    exception::{HttpArgs, HttpException, Throwable},
    http_kernel::response_handle,
};
use symfony_routing::{
    exception::Error as RoutingError, redirectable_url_matcher::RedirectableUrlMatcherInterface,
    request_context::RequestContext,
};

use super::NotPorted;

/// `Symfony\Bundle\FrameworkBundle\Controller\RedirectController`.
pub const REDIRECT_CONTROLLER: &str =
    "Symfony\\Bundle\\FrameworkBundle\\Controller\\RedirectController";

/// `RedirectableCompiledUrlMatcher::redirect()`.
#[derive(Debug, Default)]
pub struct RedirectableCompiledUrlMatcher;

impl RedirectableUrlMatcherInterface for RedirectableCompiledUrlMatcher {
    fn redirect(
        &self,
        path: &[u8],
        route: &PhpString,
        scheme: Option<&[u8]>,
    ) -> Result<PhpArray, RoutingError> {
        self.redirect_in(path, route, scheme, &RequestContext::default())
    }

    fn redirect_in(
        &self,
        path: &[u8],
        route: &PhpString,
        scheme: Option<&[u8]>,
        context: &RequestContext,
    ) -> Result<PhpArray, RoutingError> {
        let mut attributes = PhpArray::new();
        attributes.insert(
            "_controller",
            Value::String(PhpString::from(format!(
                "{REDIRECT_CONTROLLER}::urlRedirectAction"
            ))),
        );
        attributes.insert("path", Value::String(PhpString::from(path)));
        attributes.insert("permanent", Value::Bool(true));
        attributes.insert(
            "scheme",
            scheme.map_or(Value::Null, |s| Value::String(PhpString::from(s))),
        );
        attributes.insert("httpPort", Value::Int(context.http_port()));
        attributes.insert("httpsPort", Value::Int(context.https_port()));
        attributes.insert("_route", Value::String(route.clone()));
        attributes.insert("_route_mapping", Value::Array(PhpArray::new()));
        Ok(attributes)
    }
}

/// The `redirect_controller` service: `RedirectController` with
/// `%router.request_context.http_port%` and `https_port%` (80 and 443).
#[derive(Debug)]
pub struct RedirectController {
    http_port: Option<i64>,
    https_port: Option<i64>,
}

impl Default for RedirectController {
    fn default() -> Self {
        Self {
            http_port: Some(80),
            https_port: Some(443),
        }
    }
}

impl PhpObject for RedirectController {
    fn class(&self) -> &str {
        REDIRECT_CONTROLLER
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// The arguments of `urlRedirectAction()`.
struct UrlRedirect {
    path: PhpString,
    permanent: bool,
    scheme: Option<PhpString>,
    http_port: Option<i64>,
    https_port: Option<i64>,
    keep_request_method: bool,
}

fn not_ported(what: &'static str) -> Throwable {
    Arc::new(NotPorted(what))
}

fn foundation(error: symfony_http_foundation::exception::Error) -> Throwable {
    Arc::new(error)
}

impl UrlRedirect {
    /// The resolved arguments, in the declared order. Only the types the
    /// matcher's attributes and the defaults give are accepted; PHP's
    /// coercion of other scalars is not ported.
    fn from_arguments(arguments: &[Value]) -> Result<(RequestHandle, Self), Throwable> {
        let request = arguments
            .first()
            .and_then(as_request)
            .ok_or_else(|| not_ported("urlRedirectAction() without its request"))?;
        let int = |at: usize| match arguments.get(at) {
            Some(Value::Int(i)) => Ok(Some(*i)),
            Some(Value::Null) => Ok(None),
            _ => Err(not_ported(
                "urlRedirectAction() with a port that is not an int",
            )),
        };
        let boolean = |at: usize| match arguments.get(at) {
            Some(Value::Bool(b)) => Ok(*b),
            _ => Err(not_ported(
                "urlRedirectAction() with a flag that is not a bool",
            )),
        };
        let Some(Value::String(path)) = arguments.get(1) else {
            return Err(not_ported(
                "urlRedirectAction() with a path that is not a string",
            ));
        };
        let scheme = match arguments.get(3) {
            Some(Value::String(s)) => Some(s.clone()),
            Some(Value::Null) => None,
            _ => {
                return Err(not_ported(
                    "urlRedirectAction() with a scheme that is not a string",
                ));
            }
        };
        Ok((
            request,
            Self {
                path: path.clone(),
                permanent: boolean(2)?,
                scheme,
                http_port: int(4)?,
                https_port: int(5)?,
                keep_request_method: boolean(6)?,
            },
        ))
    }
}

impl RedirectController {
    /// `urlRedirectAction(Request $request, string $path, bool $permanent =
    /// false, ?string $scheme = null, ?int $httpPort = null, ?int $httpsPort
    /// = null, bool $keepRequestMethod = false)`.
    fn url_redirect_action(
        &self,
        request: &mut Request,
        arguments: UrlRedirect,
    ) -> Result<Response, Throwable> {
        let permanent = arguments.permanent;
        if arguments.path.is_empty() {
            let status = if permanent { 410 } else { 404 };
            return Err(HttpException::new(status, HttpArgs::message("")).throwable());
        }

        let status = match (arguments.keep_request_method, permanent) {
            (true, true) => 308,
            (true, false) => 307,
            (false, true) => 301,
            (false, false) => 302,
        };

        let scheme = match arguments.scheme {
            Some(scheme) => scheme,
            None => PhpString::from(request.get_scheme().map_err(foundation)?),
        };

        let mut path = arguments.path.to_vec();
        if path.starts_with(b"//") {
            path = [scheme.as_bytes(), b":", &path].concat();
            // parse_url() finds the scheme just added: redirect to it as is.
            return redirect(path, status);
        }
        if !path.starts_with(b"/") {
            return Err(not_ported(
                "urlRedirectAction() with a path not starting with '/' (parse_url())",
            ));
        }

        // `$request->server->get('QUERY_STRING') ?: $request->getQueryString()`
        let raw = request.server().get("QUERY_STRING", Value::Null);
        let qs = if raw.is_truthy() {
            raw.to_php_string()
        } else {
            request.get_query_string().map_err(foundation)?
        };
        let qs = match qs {
            Some(qs) if Value::String(qs.clone()).is_truthy() => {
                let separator: &[u8] = if path.contains(&b'?') { b"&" } else { b"?" };
                [separator, qs.as_bytes()].concat()
            }
            _ => Vec::new(),
        };

        let request_scheme = request.get_scheme().map_err(foundation)?;
        let port = match scheme.as_bytes() {
            b"http" => {
                let port = match arguments.http_port {
                    Some(port) => Some(port),
                    None if request_scheme == "http" => request_port(request)?,
                    None => self.http_port,
                };
                port.filter(|&p| p != 80)
            }
            b"https" => {
                let port = match arguments.https_port {
                    Some(port) => Some(port),
                    None if request_scheme == "https" => request_port(request)?,
                    None => self.https_port,
                };
                port.filter(|&p| p != 443)
            }
            _ => None,
        };
        let port = port.map_or_else(Vec::new, |p| format!(":{p}").into_bytes());

        let host = request.get_host().map_err(foundation)?;
        let base_url = request.get_base_url().map_err(foundation)?;
        let url = [
            scheme.as_bytes(),
            b"://",
            host.as_bytes(),
            &port,
            base_url.as_bytes(),
            &path,
            &qs,
        ]
        .concat();
        redirect(url, status)
    }
}

/// `$request->getPort()`, an int or null.
fn request_port(request: &mut Request) -> Result<Option<i64>, Throwable> {
    match request.get_port().map_err(foundation)? {
        Value::Int(port) => Ok(Some(port)),
        Value::Null => Ok(None),
        _ => Err(not_ported(
            "a port that is not an int (`$request->getPort()`)",
        )),
    }
}

/// `new RedirectResponse($url, $status)`.
fn redirect(url: Vec<u8>, status: i64) -> Result<Response, Throwable> {
    let response = RedirectResponse::new(PhpString::from(url), status, Vec::<(&str, &str)>::new())
        .map_err(foundation)?;
    Ok(response.into())
}

/// The declared `RedirectController` class, as the resolver sees it.
pub fn redirect_controller_class() -> Class {
    let declared = Controller::new(|_| {
        Err(not_ported(
            "RedirectController::urlRedirectAction() without its object",
        ))
    })
    .with_parameters([
        Parameter::new("request").typed(REQUEST_CLASS),
        Parameter::new("path").typed("string"),
        Parameter::new("permanent")
            .typed("bool")
            .with_default(Value::Bool(false)),
        Parameter::new("scheme")
            .typed("string")
            .nullable()
            .with_default(Value::Null),
        Parameter::new("httpPort")
            .typed("int")
            .nullable()
            .with_default(Value::Null),
        Parameter::new("httpsPort")
            .typed("int")
            .nullable()
            .with_default(Value::Null),
        Parameter::new("keepRequestMethod")
            .typed("bool")
            .with_default(Value::Bool(false)),
    ]);
    Class::new(REDIRECT_CONTROLLER).with_method(
        Method::new("urlRedirectAction", declared).with_this(|this, arguments| {
            let Value::Object(object) = this else {
                return Err(not_ported(
                    "urlRedirectAction() on a value that is not an object",
                ));
            };
            let controller = object
                .downcast_ref::<RedirectController>()
                .ok_or_else(|| not_ported("urlRedirectAction() on another object"))?;
            let (request, redirect) = UrlRedirect::from_arguments(arguments)?;
            let response = request.with_mut(|r| controller.url_redirect_action(r, redirect))?;
            Ok(ControllerResult::Response(response_handle(response)))
        }),
    )
}
