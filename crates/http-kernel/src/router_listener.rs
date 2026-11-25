//! Port of Symfony\Component\HttpKernel\EventListener\RouterListener.php:
//! initializes the routing context from the request and sets request
//! attributes based on a matching route, with the welcome page shown in
//! debug mode when no route is configured.
//!
//! The matcher is a [`Matcher`]: PHP's `UrlMatcherInterface|
//! RequestMatcherInterface`, told apart as `instanceof
//! RequestMatcherInterface` does (a matcher that implements both, as the
//! Router does, is a [`Matcher::Request`]). The routing context is a PHP
//! object the listener and the matcher share, so it is a
//! [`Handle<RequestContext>`]: `fromRequest()` through the listener is
//! what the matcher (and a URL generator holding the same context) sees.
//! The routing crate's `UrlMatcher` and `RedirectableUrlMatcher` keep
//! their context by value; as request matchers they match in a copy filled
//! from the request, as their `matchRequest()` does, and the listener
//! needs its own context for them.
//!
//! What a matcher throws is a [`Throwable`]; `ResourceNotFoundException`
//! (and so `NoConfigurationException`) and `MethodNotAllowedException` are
//! recognized by class, the allowed methods read from the routing error.
//!
//! The route mapping (`_route_mapping`) is applied with PHP's array and
//! string offset rules for what a matcher may return: `isset()` on a
//! string, writes into a string attribute (`Cannot access offset of type
//! string on string`, `Cannot use a scalar value as an array`, a one-byte
//! string offset write), offsets that are not keys. PHP's warnings and
//! deprecations on the way (an undefined index, a float offset) are not
//! reproduced.
//!
//! The parent contexts that `onKernelFinishRequest()` restores are kept
//! by sub-request object: PHP keys them by `spl_object_id()`, which a
//! later object may reuse once a sub-request is freed without finishing;
//! here the entry holds the request, so it is never confused with
//! another.
//!
//! The welcome page is upstream's `Resources/welcome.html.php` rendered in
//! the oracle (`resources/welcome.html`, see its licence file); the hue
//! (`random_int(0, 360)`) and the project directory (`realpath()` and `/`,
//! as Linux PHP gives `DIRECTORY_SEPARATOR`) are filled per response.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel EventListener/RouterListener.php Resources/welcome.html.php

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::{
    Handle, Key, Number, PhpArray, PhpString, Value,
    numeric::{leading_numeric_value, numeric_value},
};
use psr_log::{ContextValue, LoggerInterface};
use symfony_event_dispatcher::{
    Callable, EventDispatcherInterface, EventObject, EventSubscriber, Subscription, UndefinedMethod,
};
use symfony_http_foundation::{
    header_bag::ResponseHeaderBag,
    request::Request,
    request_stack::{RequestHandle, RequestStack},
    response::Response,
};
use symfony_routing::{
    exception::{Detail, Error as RoutingError},
    redirectable_url_matcher::{RedirectableUrlMatcher, RedirectableUrlMatcherInterface},
    request_context::RequestContext,
    url_matcher::UrlMatcher,
};

use crate::{
    event::{ExceptionEvent, FinishRequestEvent, RequestEvent},
    exception::{HttpArgs, HttpException, PhpClass, PhpException, Throwable, instance_of},
    http_kernel::KernelEvents,
};

/// `Symfony\Component\HttpKernel\EventListener\RouterListener`.
pub const CLASS: &str = "Symfony\\Component\\HttpKernel\\EventListener\\RouterListener";

/// `Kernel::MAJOR_VERSION.'.'.Kernel::MINOR_VERSION` at the pin.
pub const KERNEL_VERSION: &str = "8.1";

const RESOURCE_NOT_FOUND: &str =
    "Symfony\\Component\\Routing\\Exception\\ResourceNotFoundException";
const METHOD_NOT_ALLOWED: &str =
    "Symfony\\Component\\Routing\\Exception\\MethodNotAllowedException";
const NO_CONFIGURATION: &str = "Symfony\\Component\\Routing\\Exception\\NoConfigurationException";
const NOT_FOUND_HTTP: &str = "Symfony\\Component\\HttpKernel\\Exception\\NotFoundHttpException";

const WELCOME: &[u8] = include_bytes!("../resources/welcome.html");

/// `Symfony\Component\Routing\Matcher\UrlMatcherInterface`, which extends
/// `RequestContextAwareInterface`.
pub trait UrlMatcherInterface: Send + Sync {
    /// `getContext()`: the context the matcher matches in.
    fn get_context(&self) -> Handle<RequestContext>;

    /// `match($pathinfo)`: the route's attributes.
    fn match_path(&self, pathinfo: &[u8]) -> Result<PhpArray, Throwable>;
}

/// `Symfony\Component\Routing\Matcher\RequestMatcherInterface`.
pub trait RequestMatcherInterface: Send + Sync {
    /// `matchRequest($request)`: the route's attributes.
    fn match_request(&self, request: &RequestHandle) -> Result<PhpArray, Throwable>;

    /// `getContext()` when the matcher is also a
    /// `RequestContextAwareInterface` (the Router); `None` otherwise.
    fn get_context(&self) -> Option<Handle<RequestContext>> {
        None
    }
}

fn routing_error(error: RoutingError) -> Throwable {
    Arc::new(error)
}

impl RequestMatcherInterface for UrlMatcher {
    fn match_request(&self, request: &RequestHandle) -> Result<PhpArray, Throwable> {
        request
            .with_mut(|r| UrlMatcher::match_request(self, r))
            .map_err(routing_error)
    }
}

impl<R: RedirectableUrlMatcherInterface + Send + Sync> RequestMatcherInterface
    for RedirectableUrlMatcher<R>
{
    fn match_request(&self, request: &RequestHandle) -> Result<PhpArray, Throwable> {
        request
            .with_mut(|r| RedirectableUrlMatcher::match_request(self, r))
            .map_err(routing_error)
    }
}

/// `UrlMatcherInterface|RequestMatcherInterface $matcher`.
#[derive(Clone)]
pub enum Matcher {
    /// A matcher of paths in its context.
    Url(Arc<dyn UrlMatcherInterface>),
    /// A matcher of requests, tried first when a matcher is both.
    Request(Arc<dyn RequestMatcherInterface>),
}

impl Matcher {
    /// `$matcher->getContext()` when it is a `RequestContextAwareInterface`.
    fn get_context(&self) -> Option<Handle<RequestContext>> {
        match self {
            Self::Url(m) => Some(m.get_context()),
            Self::Request(m) => m.get_context(),
        }
    }
}

/// The constructor's optional arguments.
#[derive(Clone)]
pub struct Options {
    /// `?RequestContext $context` (can be null when the matcher implements
    /// `RequestContextAwareInterface`).
    pub context: Option<Handle<RequestContext>>,
    pub logger: Option<Arc<dyn LoggerInterface>>,
    /// `?string $projectDir`, shown on the welcome page.
    pub project_dir: Option<PhpString>,
    pub debug: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            context: None,
            logger: None,
            project_dir: None,
            debug: true,
        }
    }
}

/// `RouterListener`.
pub struct RouterListener {
    matcher: Matcher,
    request_stack: RequestStack,
    context: Handle<RequestContext>,
    logger: Option<Arc<dyn LoggerInterface>>,
    project_dir: Option<PhpString>,
    debug: bool,
    /// `$parentContexts`: the parent's context when a sub-request
    /// started, by sub-request (`spl_object_id()`).
    parent_contexts: Mutex<Vec<(RequestHandle, RequestContext)>>,
}

impl std::fmt::Debug for RouterListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RouterListener")
            .field("context", &self.context)
            .field("project_dir", &self.project_dir)
            .field("debug", &self.debug)
            .finish_non_exhaustive()
    }
}

impl RouterListener {
    /// `new RouterListener($matcher, $requestStack, $context, $logger,
    /// $projectDir, $debug)`: `\InvalidArgumentException` without a
    /// context from either the argument or the matcher.
    pub fn new(
        matcher: Matcher,
        request_stack: RequestStack,
        options: Options,
    ) -> Result<Self, Throwable> {
        let Some(context) = options.context.or_else(|| matcher.get_context()) else {
            return Err(PhpException::new(
                PhpClass::InvalidArgumentException,
                "You must either pass a RequestContext or the matcher must implement RequestContextAwareInterface.",
            )
            .throwable());
        };
        Ok(Self {
            matcher,
            request_stack,
            context,
            logger: options.logger,
            project_dir: options.project_dir,
            debug: options.debug,
            parent_contexts: Mutex::new(Vec::new()),
        })
    }

    /// The routing context the listener fills.
    pub fn context(&self) -> &Handle<RequestContext> {
        &self.context
    }

    fn set_current_request(&self, request: Option<&RequestHandle>) -> Result<(), Throwable> {
        let Some(request) = request else {
            return Ok(());
        };
        let filled = self
            .context
            .with_mut(|context| request.with_mut(|r| context.from_request(r).map(|_| ())));
        match filled {
            Ok(()) => Ok(()),
            Err(e) if e.is_a("UnexpectedValueException") => {
                let message = e.message().clone();
                Err(HttpException::bad_request(HttpArgs {
                    message,
                    previous: Some(routing_error(e)),
                    // `getCode()`: 0 for every exception `fromRequest()`
                    // throws.
                    code: 0,
                    headers: PhpArray::new(),
                })
                .throwable())
            }
            Err(e) => Err(routing_error(e)),
        }
    }

    fn parent_contexts(&self) -> MutexGuard<'_, Vec<(RequestHandle, RequestContext)>> {
        self.parent_contexts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// `onKernelFinishRequest($event)`: after a sub-request is done, the
    /// routing context goes back to the parent request so that the URL
    /// generator operates on the correct context again. With an event, the
    /// parent's context saved when the sub-request started is restored
    /// instead of being computed again: while a sub-request runs, its
    /// trusted proxies also apply to the parent request. Without one
    /// (`null`), it is computed from the parent request.
    pub fn on_kernel_finish_request(
        &self,
        event: Option<&FinishRequestEvent>,
    ) -> Result<(), Throwable> {
        let Some(event) = event else {
            return self.set_current_request(self.request_stack.get_parent_request().as_ref());
        };

        let request = event.get_request();
        let parent_context = {
            let mut parent_contexts = self.parent_contexts();
            let Some(at) = parent_contexts.iter().position(|(r, _)| r.same(request)) else {
                return Ok(());
            };
            parent_contexts.remove(at).1
        };

        self.context.with_mut(|context| {
            context
                .set_base_url(parent_context.base_url())
                .set_path_info(parent_context.path_info())
                .set_method(parent_context.method())
                .set_host(parent_context.host())
                .set_scheme(parent_context.scheme())
                .set_http_port(parent_context.http_port())
                .set_https_port(parent_context.https_port())
                .set_query_string(Some(parent_context.query_string()));
        });
        Ok(())
    }

    /// `onKernelRequest($event)`.
    pub fn on_kernel_request(&self, event: &mut RequestEvent) -> Result<(), Throwable> {
        let request = event.get_request().clone();

        if self.request_stack.get_parent_request().is_some() {
            let context = self.context.get();
            let mut parent_contexts = self.parent_contexts();
            match parent_contexts.iter_mut().find(|(r, _)| r.same(&request)) {
                Some(entry) => entry.1 = context,
                None => parent_contexts.push((request.clone(), context)),
            }
        }

        self.set_current_request(Some(&request))?;

        if request.with(|r| r.attributes().has("_controller")) {
            // routing is already done
            return Ok(());
        }

        match self.route(&request) {
            Ok(()) => Ok(()),
            Err(e) if instance_of(e.as_ref(), RESOURCE_NOT_FOUND) => {
                let mut message = b"No route found for \"".to_vec();
                message.extend_from_slice(&method_and_uri(&request)?);
                message.push(b'"');
                let referer = request.with(|r| r.headers().get("referer", None));
                if let Some(referer) = referer.filter(|r| php_value::string::truthy(r)) {
                    message.extend_from_slice(b" (from \"");
                    message.extend_from_slice(&referer);
                    message.extend_from_slice(b"\")");
                }
                Err(HttpException::not_found(HttpArgs {
                    message: PhpString::from(message),
                    previous: Some(e),
                    ..HttpArgs::default()
                })
                .throwable())
            }
            Err(e) if instance_of(e.as_ref(), METHOD_NOT_ALLOWED) => {
                let allowed = match e.downcast_ref::<RoutingError>().map(RoutingError::detail) {
                    Some(Detail::AllowedMethods(methods)) => methods.clone(),
                    _ => Vec::new(),
                };
                let joined: Vec<&[u8]> = allowed.iter().map(PhpString::as_bytes).collect();
                let mut message = b"No route found for \"".to_vec();
                message.extend_from_slice(&method_and_uri(&request)?);
                message.extend_from_slice(b"\": Method Not Allowed (Allow: ");
                message.extend_from_slice(&joined.join(&b", "[..]));
                message.push(b')');
                Err(HttpException::method_not_allowed(
                    allowed.iter().map(PhpString::as_bytes),
                    HttpArgs {
                        message: PhpString::from(message),
                        previous: Some(e),
                        ..HttpArgs::default()
                    },
                )
                .throwable())
            }
            Err(e) => Err(e),
        }
    }

    /// The `try` block of `onKernelRequest()`.
    fn route(&self, request: &RequestHandle) -> Result<(), Throwable> {
        // matching a request is more powerful than matching a URL path +
        // context, so try that first
        let mut parameters = match &self.matcher {
            Matcher::Request(matcher) => matcher.match_request(request)?,
            Matcher::Url(matcher) => {
                let pathinfo = request
                    .with_mut(Request::get_path_info)
                    .map_err(foundation)?;
                matcher.match_path(pathinfo.as_bytes())?
            }
        };

        if let Some(logger) = &self.logger {
            let route = match parameters.get(&Key::from("_route")) {
                Some(route) if !matches!(route, Value::Null) => route.clone(),
                _ => Value::string("n/a"),
            };
            let request_uri = request.with_mut(Request::get_uri).map_err(foundation)?;
            let method = request.with_mut(Request::get_method).map_err(foundation)?;
            logger.info(
                "Matched route \"{route}\".",
                vec![
                    ("route".to_owned(), ContextValue::Value(route)),
                    (
                        "route_parameters".to_owned(),
                        ContextValue::Value(Value::Array(parameters.clone())),
                    ),
                    (
                        "request_uri".to_owned(),
                        ContextValue::Value(Value::String(request_uri)),
                    ),
                    (
                        "method".to_owned(),
                        ContextValue::Value(Value::String(method)),
                    ),
                ],
            );
        }

        let mut attributes = parameters.clone();
        let mapping_key = Key::from("_route_mapping");
        let mapping = parameters.get(&mapping_key).cloned().unwrap_or(Value::Null);
        if mapping.is_truthy() {
            parameters.remove(&mapping_key);
            attributes = map_attributes(&parameters, &mapping)?;
            attributes.insert(mapping_key, mapping);
        }

        request.with_mut(|r| r.attributes_mut().add(attributes));
        parameters.remove(&Key::from("_route"));
        parameters.remove(&Key::from("_controller"));
        request.with_mut(|r| {
            r.attributes_mut()
                .set("_route_params", Value::Array(parameters));
        });
        Ok(())
    }

    /// `onKernelException($event)`: the welcome page, in debug mode, for a
    /// `NotFoundHttpException` caused by a `NoConfigurationException`.
    pub fn on_kernel_exception(&self, event: &mut ExceptionEvent) -> Result<(), Throwable> {
        if !self.debug {
            return Ok(());
        }
        let throwable = event.get_throwable();
        if !instance_of(throwable.as_ref(), NOT_FOUND_HTTP) {
            return Ok(());
        }
        let no_configuration = throwable
            .downcast_ref::<HttpException>()
            .and_then(HttpException::get_previous)
            .is_some_and(|previous| instance_of(previous.as_ref(), NO_CONFIGURATION));
        if no_configuration {
            event.set_response(Handle::new(self.create_welcome_response()?));
        }
        Ok(())
    }

    /// `getSubscribedEvents()`.
    pub fn get_subscribed_events() -> Vec<(&'static str, Subscription)> {
        vec![
            (
                KernelEvents::REQUEST,
                Subscription::Methods(vec![("onKernelRequest", 32)]),
            ),
            (
                KernelEvents::FINISH_REQUEST,
                Subscription::Methods(vec![("onKernelFinishRequest", 0)]),
            ),
            (
                KernelEvents::EXCEPTION,
                Subscription::Prioritized("onKernelException", -64),
            ),
        ]
    }

    fn create_welcome_response(&self) -> Result<Response, Throwable> {
        let project_dir = self
            .project_dir
            .as_ref()
            .map_or(&b""[..], PhpString::as_bytes);
        // `realpath('')` is the working directory; `false` is `''`.
        let real = symfony_finder::fs::realpath(if project_dir.is_empty() {
            b"."
        } else {
            project_dir
        });
        let mut project_dir = real.unwrap_or_default();
        project_dir.push(b'/');
        let page = welcome_page(random_hue(), &project_dir);
        Response::new(
            Some(PhpString::from(page)),
            Response::HTTP_NOT_FOUND,
            ResponseHeaderBag::new(std::iter::empty::<(&str, &str)>()).map_err(foundation)?,
        )
        .map_err(foundation)
    }
}

/// The welcome page for a hue and a project directory.
pub fn welcome_page(hue: u16, project_dir: &[u8]) -> Vec<u8> {
    let mut page = Vec::with_capacity(WELCOME.len() + project_dir.len());
    let mut rest = WELCOME;
    for (placeholder, value) in [
        (&b"@@HUE@@"[..], hue.to_string().into_bytes()),
        (&b"@@PROJECT_DIR@@"[..], project_dir.to_vec()),
    ] {
        match find(rest, placeholder) {
            Some(at) => {
                page.extend_from_slice(&rest[..at]);
                page.extend_from_slice(&value);
                rest = &rest[at + placeholder.len()..];
            }
            None => break,
        }
    }
    page.extend_from_slice(rest);
    page
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// `random_int(0, 360)`, from the standard library's random hash keys.
fn random_hue() -> u16 {
    use std::hash::{BuildHasher, RandomState};
    let random = RandomState::new().hash_one(0u8);
    u16::try_from(random % 361).unwrap_or(0)
}

fn foundation(error: symfony_http_foundation::exception::Error) -> Throwable {
    Arc::new(error)
}

/// `$request->getMethod().' '.$request->getUriForPath($request->getPathInfo())`.
fn method_and_uri(request: &RequestHandle) -> Result<Vec<u8>, Throwable> {
    request
        .with_mut(|r| {
            let method = r.get_method()?;
            let pathinfo = r.get_path_info()?;
            let uri = r.get_uri_for_path(pathinfo.as_bytes())?;
            let mut out = method.as_bytes().to_vec();
            out.push(b' ');
            out.extend_from_slice(uri.as_bytes());
            Ok(out)
        })
        .map_err(foundation)
}

fn type_error(message: String) -> Throwable {
    PhpException::new(PhpClass::TypeError, message).throwable()
}

fn error(message: impl Into<PhpString>) -> Throwable {
    PhpException::new(PhpClass::Error, message).throwable()
}

/// `$array[$offset]` as a key for a write or a read: PHP's `TypeError`
/// for an array or object.
fn key(offset: &Value) -> Result<Key, Throwable> {
    Key::from_value(offset).ok_or_else(|| {
        type_error(format!(
            "Cannot access offset of type {} on array",
            offset.debug_type()
        ))
    })
}

/// `isset($array[$offset])`.
fn isset(array: &PhpArray, offset: &Value) -> Result<bool, Throwable> {
    let key = Key::from_value(offset).ok_or_else(|| {
        type_error(format!(
            "Cannot access offset of type {} in isset or empty",
            offset.debug_type()
        ))
    })?;
    Ok(array.get(&key).is_some_and(|v| !matches!(v, Value::Null)))
}

/// A string offset given as a string: an integer numeric string, or a
/// leading-integer one (PHP warns); `None` for any other string.
fn string_offset(s: &[u8]) -> Option<i64> {
    match numeric_value(s) {
        Some(Number::Int(i)) => Some(i),
        Some(Number::Float(_)) => None,
        None => match leading_numeric_value(s) {
            Some(Number::Int(i)) => Some(i),
            _ => None,
        },
    }
}

/// The byte at `offset` of `s`, counting from the end when negative.
fn string_position(len: usize, offset: i64) -> Option<usize> {
    let len = i64::try_from(len).ok()?;
    let at = if offset < 0 { len + offset } else { offset };
    (at >= 0 && at < len)
        .then(|| usize::try_from(at).ok())
        .flatten()
}

/// `isset($mapping[$key])` and `$mapping[$key]` for the mapping a matcher
/// returned, which need not be an array: a string's offsets are its bytes,
/// any other scalar has none.
fn mapping_entry(mapping: &Value, key: &Key) -> Option<Value> {
    match mapping {
        Value::Array(array) => array
            .get(key)
            .filter(|v| !matches!(v, Value::Null))
            .cloned(),
        Value::String(s) => {
            let offset = match key {
                Key::Int(i) => Some(*i),
                Key::Str(k) => match numeric_value(k) {
                    Some(Number::Int(i)) => Some(i),
                    _ => None,
                },
            }?;
            let at = string_position(s.len(), offset)?;
            Some(Value::String(PhpString::from(&s.as_bytes()[at..=at])))
        }
        _ => None,
    }
}

/// `$value[0]` and `$value[1]` of `[$a, $b] = $value` (null when missing).
fn pair(array: &PhpArray) -> (Value, Value) {
    let at = |i| array.get(&Key::Int(i)).cloned().unwrap_or(Value::Null);
    (at(0), at(1))
}

/// `(string) $value` as a string offset write converts it.
fn offset_value(value: &Value) -> Result<PhpString, Throwable> {
    match value {
        Value::Array(_) => Ok(PhpString::from("Array")),
        other => other.to_php_string_or_stringable().ok_or_else(|| {
            error(format!(
                "Object of class {} could not be converted to string",
                other.debug_type()
            ))
        }),
    }
}

/// `$string[$offset] = $value`: the string with one byte written, padded
/// with spaces past its end; unchanged for a negative offset before its
/// start (PHP warns).
fn write_string_offset(
    s: &PhpString,
    offset: &Value,
    value: &Value,
) -> Result<PhpString, Throwable> {
    let offset = match offset {
        Value::Int(i) => *i,
        Value::Null => 0,
        Value::Bool(b) => i64::from(*b),
        Value::Float(f) => php_value::numeric::float_key(*f),
        Value::String(o) => string_offset(o).ok_or_else(|| {
            type_error("Cannot access offset of type string on string".to_owned())
        })?,
        other => {
            return Err(type_error(format!(
                "Cannot access offset of type {} on string",
                other.debug_type()
            )));
        }
    };
    let len = i64::try_from(s.len()).unwrap_or(i64::MAX);
    if offset < 0 && -offset > len {
        return Ok(s.clone());
    }
    let value = offset_value(value)?;
    let Some(&byte) = value.as_bytes().first() else {
        return Err(error("Cannot assign an empty string to a string offset"));
    };
    let at = if offset < 0 { len + offset } else { offset };
    let at = usize::try_from(at).map_err(|_| error("String offset out of range"))?;
    let mut bytes = s.as_bytes().to_vec();
    if at >= bytes.len() {
        bytes.resize(at, b' ');
        bytes.push(byte);
    } else {
        bytes[at] = byte;
    }
    Ok(PhpString::from(bytes))
}

/// `$attributes[$attribute][$parameter] = $value`.
fn write_nested(
    attributes: &mut PhpArray,
    attribute: &Value,
    parameter: &Value,
    value: Value,
) -> Result<(), Throwable> {
    let outer = key(attribute)?;
    let target = attributes.get(&outer).cloned().unwrap_or(Value::Null);
    let written = match target {
        Value::Null | Value::Bool(false) => {
            let mut array = PhpArray::new();
            array.insert(key(parameter)?, value);
            Value::Array(array)
        }
        Value::Array(mut array) => {
            array.insert(key(parameter)?, value);
            Value::Array(array)
        }
        Value::String(s) => Value::String(write_string_offset(&s, parameter, &value)?),
        Value::Object(o) => {
            return Err(error(format!(
                "Cannot use object of type {} as array",
                o.class()
            )));
        }
        Value::Bool(true) | Value::Int(_) | Value::Float(_) => {
            return Err(error("Cannot use a scalar value as an array"));
        }
    };
    attributes.insert(outer, written);
    Ok(())
}

/// The attributes the route mapping gives the parameters (without
/// `_route_mapping`): a parameter is renamed, or grouped with others
/// under one attribute.
#[expect(clippy::if_not_else, reason = "upstream's branches, in its order")]
fn map_attributes(parameters: &PhpArray, mapping: &Value) -> Result<PhpArray, Throwable> {
    let mut mapped_attributes = PhpArray::new();
    let mut attributes = PhpArray::new();
    let empty = Value::string("");

    for (name, value) in parameters.iter() {
        let (attribute, parameter) = match mapping_entry(mapping, name) {
            None => (name.to_value(), name.to_value()),
            Some(Value::Array(pair_value)) => {
                let (attribute, parameter) = pair(&pair_value);
                mapped_attributes.insert(key(&attribute)?, empty.clone());
                (attribute, parameter)
            }
            Some(attribute) => (attribute, name.to_value()),
        };

        if !isset(&mapped_attributes, &attribute)? {
            let attribute_key = key(&attribute)?;
            attributes.insert(attribute_key.clone(), value.clone());
            mapped_attributes.insert(attribute_key, parameter);
        } else {
            let attribute_key = key(&attribute)?;
            let mapped = mapped_attributes
                .get(&attribute_key)
                .cloned()
                .unwrap_or(Value::Null);
            if mapped != empty {
                let previous = attributes
                    .get(&attribute_key)
                    .cloned()
                    .unwrap_or(Value::Null);
                let mut group = PhpArray::new();
                group.insert(key(&mapped)?, previous);
                group.insert(key(&parameter)?, value.clone());
                attributes.insert(attribute_key.clone(), Value::Array(group));
                mapped_attributes.insert(attribute_key, empty.clone());
            } else {
                write_nested(&mut attributes, &attribute, &parameter, value.clone())?;
            }
        }
    }

    Ok(attributes)
}

fn boxed(e: Throwable) -> symfony_event_dispatcher::Throwable {
    Box::new(e)
}

impl Callable for RouterListener {
    fn class(&self) -> &'static str {
        CLASS
    }

    fn call(
        &self,
        method: &str,
        event: &mut dyn EventObject,
        _event_name: &str,
        _dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), symfony_event_dispatcher::Throwable> {
        match method {
            "onKernelRequest" => {
                let event = event
                    .downcast_mut::<RequestEvent>()
                    .ok_or_else(|| type_mismatch("onKernelRequest", RequestEvent::CLASS))?;
                self.on_kernel_request(event).map_err(boxed)
            }
            "onKernelFinishRequest" => {
                let event = event.downcast_mut::<FinishRequestEvent>().ok_or_else(|| {
                    type_mismatch("onKernelFinishRequest", FinishRequestEvent::CLASS)
                })?;
                self.on_kernel_finish_request(Some(event)).map_err(boxed)
            }
            "onKernelException" => {
                let event = event
                    .downcast_mut::<ExceptionEvent>()
                    .ok_or_else(|| type_mismatch("onKernelException", ExceptionEvent::CLASS))?;
                self.on_kernel_exception(event).map_err(boxed)
            }
            other => Err(UndefinedMethod::error(CLASS, other)),
        }
    }
}

/// PHP's `\TypeError` for an event of another class.
fn type_mismatch(method: &str, class: &str) -> symfony_event_dispatcher::Throwable {
    boxed(type_error(format!(
        "{CLASS}::{method}(): Argument #1 ($event) must be of type {class}"
    )))
}

impl EventSubscriber for RouterListener {
    fn subscribed_events(&self) -> Vec<(&'static str, Subscription)> {
        Self::get_subscribed_events()
    }
}
