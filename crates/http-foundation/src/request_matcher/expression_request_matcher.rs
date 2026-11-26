//! Port of Symfony\Component\HttpFoundation\RequestMatcher\ExpressionRequestMatcher.php:
//! an expression evaluated with `request`, `method`, `path` (URL-decoded
//! path info), `host`, `ip` (the client IP) and `attributes`.
//!
//! The matcher's result is PHP's `bool` return type in coercive mode: a
//! scalar is converted, while null, an array or an object is a
//! `\TypeError`.
//!
//! `request` is a `MethodObject` (expression-language's objects with
//! methods) over a copy of the request. It has:
//!
//! - the getters `getMethod()`, `getRealMethod()`, `getPathInfo()`,
//!   `getBasePath()`, `getBaseUrl()`, `getRequestUri()`, `getUri()`,
//!   `getHost()`, `getHttpHost()`, `getSchemeAndHttpHost()`, `getScheme()`,
//!   `getPort()`, `getClientIp()`, `getQueryString()`, `getLocale()`;
//! - the tests `isSecure()`, `isXmlHttpRequest()` and `isMethod($method)`;
//! - the bags `attributes`, `query`, `request`, `cookies`, `server` and
//!   `headers`, each with `all()`, `keys()`, `get($key, $default)`,
//!   `has($key)` and `count()`.
//!
//! Other methods are "Unable to call method" errors, as for an object
//! without them. A method that changes the request changes only the copy.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/ExpressionRequestMatcher.php

use std::sync::{Arc, Mutex, PoisonError};

use php_value::{Key, PhpArray, PhpString, Value, url::rawurldecode};
use symfony_expression_language::{
    Class as ExpressionClass, Error as ExpressionError, Expression, ExpressionInput,
    ExpressionLanguage, MethodObject,
};

use crate::{
    exception::{Class, Error},
    request::Request,
    request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `Request::class`.
const REQUEST_CLASS: &str = "Symfony\\Component\\HttpFoundation\\Request";

/// The expression: a source or a parsed `Expression`.
#[derive(Debug, Clone)]
pub enum ExpressionSource {
    Source(PhpString),
    Expression(Expression),
}

/// `new ExpressionRequestMatcher($language, $expression)`.
#[derive(Debug, Clone)]
pub struct ExpressionRequestMatcher {
    language: Arc<ExpressionLanguage>,
    expression: ExpressionSource,
}

impl ExpressionRequestMatcher {
    pub fn new(language: Arc<ExpressionLanguage>, expression: ExpressionSource) -> Self {
        Self {
            language,
            expression,
        }
    }
}

fn thrown(error: ExpressionError) -> Throwable {
    Arc::new(error)
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "used as `map_err`'s function"
)]
fn crate_error(error: Error) -> ExpressionError {
    ExpressionError::new(
        ExpressionClass::from_name(error.class().name()),
        error.message().clone(),
    )
}

type Shared = Arc<Mutex<Request>>;

fn with<R>(request: &Shared, f: impl FnOnce(&mut Request) -> R) -> R {
    f(&mut request.lock().unwrap_or_else(PoisonError::into_inner))
}

fn string(value: PhpString) -> Value {
    Value::String(value)
}

fn nullable(value: Option<PhpString>) -> Value {
    value.map_or(Value::Null, Value::String)
}

/// A bag as an object: its entries, and how `get()` and `has()` look a
/// key up (header names without case).
fn bag(class: &str, all: PhpArray, headers: bool) -> Value {
    let lookup = move |all: &PhpArray, key: &Value| -> Option<Value> {
        let key = key.to_php_string().unwrap_or_default();
        if headers {
            let wanted = key.as_bytes().to_ascii_lowercase().replace_underscores();
            all.iter()
                .find(|(k, _)| {
                    k.to_value().to_php_string().is_some_and(|k| {
                        k.as_bytes().to_ascii_lowercase().replace_underscores() == wanted
                    })
                })
                .map(|(_, v)| match v {
                    // HeaderBag::get(): the first value
                    Value::Array(values) => values.values().next().cloned().unwrap_or(Value::Null),
                    other => other.clone(),
                })
        } else {
            all.get(&Key::from_bytes(key.as_bytes())).cloned()
        }
    };
    let (for_all, for_keys, for_get, for_has, for_count) =
        (all.clone(), all.clone(), all.clone(), all.clone(), all);
    MethodObject::new(class)
        .method("all", move |_| Ok(Value::Array(for_all.clone())))
        .method("keys", move |_| {
            Ok(Value::Array(PhpArray::list(
                for_keys.keys().map(Key::to_value),
            )))
        })
        .method("get", move |arguments| {
            let key = arguments.first().cloned().unwrap_or(Value::Null);
            let default = arguments.get(1).cloned().unwrap_or(Value::Null);
            Ok(lookup(&for_get, &key).unwrap_or(default))
        })
        .method("has", move |arguments| {
            let key = arguments.first().cloned().unwrap_or(Value::Null);
            Ok(Value::Bool(lookup(&for_has, &key).is_some()))
        })
        .method("count", move |_| {
            Ok(Value::Int(
                i64::try_from(for_count.len()).unwrap_or(i64::MAX),
            ))
        })
        .into_value()
}

trait Underscores {
    fn replace_underscores(self) -> Vec<u8>;
}

impl Underscores for Vec<u8> {
    /// `HeaderBag` keys: `_` is `-`.
    fn replace_underscores(self) -> Vec<u8> {
        self.into_iter()
            .map(|b| if b == b'_' { b'-' } else { b })
            .collect()
    }
}

/// A request getter as a method.
type Getter = fn(&mut Request) -> crate::exception::Result<Value>;

/// The `request` variable.
fn request_object(request: &Request) -> Value {
    let shared: Shared = Arc::new(Mutex::new(request.clone()));
    let mut object = MethodObject::new(REQUEST_CLASS);

    let getters: [(&str, Getter); 15] = [
        ("getMethod", |r| r.get_method().map(string)),
        ("getRealMethod", |r| r.get_real_method().map(string)),
        ("getPathInfo", |r| r.get_path_info().map(string)),
        ("getBasePath", |r| r.get_base_path().map(string)),
        ("getBaseUrl", |r| r.get_base_url().map(string)),
        ("getRequestUri", |r| r.get_request_uri().map(string)),
        ("getUri", |r| r.get_uri().map(string)),
        ("getHost", |r| r.get_host().map(string)),
        ("getHttpHost", |r| r.get_http_host().map(string)),
        ("getSchemeAndHttpHost", |r| {
            r.get_scheme_and_http_host().map(string)
        }),
        ("getScheme", |r| r.get_scheme().map(Value::string)),
        ("getPort", Request::get_port),
        ("getClientIp", |r| r.get_client_ip().map(nullable)),
        ("getQueryString", |r| r.get_query_string().map(nullable)),
        ("isSecure", |r| r.is_secure().map(Value::Bool)),
    ];
    for (name, getter) in getters {
        let shared = Arc::clone(&shared);
        object = object.method(name, move |_| with(&shared, getter).map_err(crate_error));
    }
    let locale = Arc::clone(&shared);
    let ajax = Arc::clone(&shared);
    let is_method = Arc::clone(&shared);
    object = object
        .method("getLocale", move |_| {
            Ok(Value::String(with(&locale, |r| r.get_locale())))
        })
        .method("isXmlHttpRequest", move |_| {
            Ok(Value::Bool(with(&ajax, |r| r.is_xml_http_request())))
        })
        .method("isMethod", move |arguments| {
            let method = arguments
                .first()
                .and_then(Value::to_php_string)
                .unwrap_or_default();
            with(&is_method, |r| {
                r.is_method(&String::from_utf8_lossy(method.as_bytes()))
            })
            .map(Value::Bool)
            .map_err(crate_error)
        });

    let bags = [
        (
            "attributes",
            "Symfony\\Component\\HttpFoundation\\ParameterBag",
            request.attributes().all().clone(),
            false,
        ),
        (
            "query",
            "Symfony\\Component\\HttpFoundation\\InputBag",
            request.query().all().clone(),
            false,
        ),
        (
            "request",
            "Symfony\\Component\\HttpFoundation\\InputBag",
            request.request().all().clone(),
            false,
        ),
        (
            "cookies",
            "Symfony\\Component\\HttpFoundation\\InputBag",
            request.cookies().all().clone(),
            false,
        ),
        (
            "server",
            "Symfony\\Component\\HttpFoundation\\ServerBag",
            request.server().all().clone(),
            false,
        ),
        (
            "headers",
            "Symfony\\Component\\HttpFoundation\\HeaderBag",
            headers_array(request),
            true,
        ),
    ];
    for (name, class, all, headers) in bags {
        object = object.property(name, bag(class, all, headers));
    }
    object.into_value()
}

/// `$request->headers->all()`: lowercase names, lists of values.
fn headers_array(request: &Request) -> PhpArray {
    let mut out = PhpArray::new();
    for (name, values) in request.headers().all().iter() {
        out.insert(
            name.clone(),
            Value::Array(PhpArray::list(
                values
                    .iter()
                    .map(|v| v.clone().map_or(Value::Null, Value::String)),
            )),
        );
    }
    out
}

impl RequestMatcherInterface for ExpressionRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        let crate_thrown = |e: Error| -> Throwable { Arc::new(e) };
        let mut values = PhpArray::new();
        values.insert("request", request_object(request));
        values.insert(
            "method",
            Value::String(request.get_method().map_err(crate_thrown)?),
        );
        let path = request.get_path_info().map_err(crate_thrown)?;
        values.insert(
            "path",
            Value::String(PhpString::from(rawurldecode(path.as_bytes()))),
        );
        values.insert(
            "host",
            Value::String(request.get_host().map_err(crate_thrown)?),
        );
        values.insert(
            "ip",
            nullable(request.get_client_ip().map_err(crate_thrown)?),
        );
        values.insert(
            "attributes",
            Value::Array(request.attributes().all().clone()),
        );

        let expression: ExpressionInput = match &self.expression {
            ExpressionSource::Source(source) => source.clone().into(),
            ExpressionSource::Expression(expression) => expression.into(),
        };
        let result = self
            .language
            .evaluate(expression, &values)
            .map_err(thrown)?;
        match result {
            Value::Bool(b) => Ok(b),
            Value::Int(_) | Value::Float(_) | Value::String(_) => Ok(result.is_truthy()),
            other => Err(Arc::new(Error::new(
                Class::PhpTypeError,
                format!(
                    "Symfony\\Component\\HttpFoundation\\RequestMatcher\\ExpressionRequestMatcher::matches(): Return value must be of type bool, {} returned",
                    other.value_name()
                ),
            ))),
        }
    }
}
