//! Port of Symfony\Component\Routing\RequestContext.php.
//!
//! `fromRequest()` reads HttpFoundation's [`Request`] (whose getters
//! cache, so it is borrowed mutably) and passes what it reads to the
//! setters as PHP does from a file without `strict_types`: the port to
//! `setHttpPort(int)` and `setHttpsPort(int)` and the `QUERY_STRING` server
//! value to `setQueryString(?string)` are coerced, or are PHP's
//! `\TypeError`. A Request exception propagates as is. The setters run
//! in upstream's order, so a failure leaves the earlier ones applied, as
//! in PHP.
//!
//! The constructor's `$parameters = null` keeps the parameters a PHP
//! subclass set before calling it; a Rust value is built whole, so `None`
//! is `[]`. Ports are integers: PHP's coercion of a numeric string
//! argument (`setHttpPort('123')`) happens at the call, before the method.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/routing RequestContext.php

use php_value::{Key, PhpArray, PhpString, Value, url::parse_url};
use symfony_http_foundation::request::Request;

use crate::{
    exception::Error,
    php::{int_argument, nullable_string_argument},
};

const SET_HTTP_PORT: &str = r"Symfony\Component\Routing\RequestContext::setHttpPort";
const SET_HTTPS_PORT: &str = r"Symfony\Component\Routing\RequestContext::setHttpsPort";
const SET_QUERY_STRING: &str = r"Symfony\Component\Routing\RequestContext::setQueryString";

/// `RequestContext`: what the matcher and generator know about the
/// current request.
#[derive(Debug, Clone, PartialEq)]
pub struct RequestContext {
    base_url: PhpString,
    path_info: PhpString,
    method: PhpString,
    host: PhpString,
    scheme: PhpString,
    http_port: i64,
    https_port: i64,
    query_string: PhpString,
    parameters: PhpArray,
}

impl Default for RequestContext {
    /// `new RequestContext()`.
    fn default() -> Self {
        Self::new("", "GET", "localhost", "http", 80, 443, "/", "", None)
    }
}

fn lower(s: &[u8]) -> PhpString {
    PhpString::from(s.to_ascii_lowercase())
}

impl RequestContext {
    /// `new RequestContext($baseUrl, $method, $host, $scheme, $httpPort,
    /// $httpsPort, $path, $queryString, $parameters)`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_url: impl AsRef<[u8]>,
        method: impl AsRef<[u8]>,
        host: impl AsRef<[u8]>,
        scheme: impl AsRef<[u8]>,
        http_port: i64,
        https_port: i64,
        path: impl AsRef<[u8]>,
        query_string: impl AsRef<[u8]>,
        parameters: Option<PhpArray>,
    ) -> Self {
        let mut context = Self {
            base_url: PhpString::default(),
            path_info: PhpString::default(),
            method: PhpString::default(),
            host: PhpString::default(),
            scheme: PhpString::default(),
            http_port,
            https_port,
            query_string: PhpString::default(),
            parameters: PhpArray::new(),
        };
        context
            .set_base_url(base_url)
            .set_method(method)
            .set_host(host)
            .set_scheme(scheme)
            .set_http_port(http_port)
            .set_https_port(https_port)
            .set_path_info(path)
            .set_query_string(Some(query_string));
        if let Some(parameters) = parameters {
            context.parameters = parameters;
        }
        context
    }

    /// `RequestContext::fromUri($uri, $host, $scheme, $httpPort,
    /// $httpsPort)`: a URI with a backslash before its query or fragment,
    /// or with a control byte or space at either end, or with a CR, LF
    /// or tab anywhere, is read as `''`. The URI's path becomes the base
    /// URL; a port goes to the scheme it belongs to (compared before
    /// lowercasing, as upstream does).
    pub fn from_uri(
        uri: &[u8],
        host: &[u8],
        scheme: &[u8],
        http_port: i64,
        https_port: i64,
    ) -> Self {
        let mut uri = uri;
        let query_or_fragment = uri
            .iter()
            .position(|&c| c == b'?' || c == b'#')
            .unwrap_or(uri.len());
        if uri
            .iter()
            .position(|&c| c == b'\\')
            .is_some_and(|i| i < query_or_fragment)
        {
            uri = b"";
        }
        if let (Some(&first), Some(&last)) = (uri.first(), uri.last())
            && (first <= 32
                || last <= 32
                || uri.iter().any(|&c| matches!(c, b'\r' | b'\n' | b'\t')))
        {
            uri = b"";
        }

        let parsed = parse_url(uri).unwrap_or_default();
        let scheme = parsed.scheme.unwrap_or_else(|| scheme.to_vec());
        let host = parsed.host.unwrap_or_else(|| host.to_vec());
        let (mut http_port, mut https_port) = (http_port, https_port);
        if let Some(port) = parsed.port {
            if scheme == b"http" {
                http_port = port;
            } else if scheme == b"https" {
                https_port = port;
            }
        }
        Self::new(
            parsed.path.unwrap_or_default(),
            "GET",
            host,
            scheme,
            http_port,
            https_port,
            "/",
            "",
            None,
        )
    }

    /// `fromRequest($request)`: the context of a request.
    pub fn from_request(&mut self, request: &mut Request) -> Result<&mut Self, Error> {
        self.set_base_url(request.get_base_url()?);
        self.set_path_info(request.get_path_info()?);
        self.set_method(request.get_method()?);
        self.set_host(request.get_host()?);
        self.set_scheme(request.get_scheme()?);

        // `$request->isSecure() || null === $request->getPort() ?
        // $this->httpPort : $request->getPort()`
        let http_port = if request.is_secure()? {
            self.http_port
        } else {
            match request.get_port()? {
                Value::Null => self.http_port,
                _ => int_argument(&request.get_port()?, SET_HTTP_PORT, 1, "httpPort")?,
            }
        };
        self.set_http_port(http_port);

        // `$request->isSecure() && null !== $request->getPort() ?
        // $request->getPort() : $this->httpsPort`
        let https_port = if request.is_secure()? {
            match request.get_port()? {
                Value::Null => self.https_port,
                _ => int_argument(&request.get_port()?, SET_HTTPS_PORT, 1, "httpsPort")?,
            }
        } else {
            self.https_port
        };
        self.set_https_port(https_port);

        let query_string = request.server().get("QUERY_STRING", Value::string(""));
        let query_string =
            nullable_string_argument(&query_string, SET_QUERY_STRING, 1, "queryString")?;
        self.set_query_string(query_string);
        Ok(self)
    }

    /// `getBaseUrl()`.
    pub fn base_url(&self) -> &PhpString {
        &self.base_url
    }

    /// `setBaseUrl()`: without trailing slashes.
    pub fn set_base_url(&mut self, base_url: impl AsRef<[u8]>) -> &mut Self {
        let b = base_url.as_ref();
        let end = b.iter().rposition(|&c| c != b'/').map_or(0, |e| e + 1);
        self.base_url = PhpString::from(&b[..end]);
        self
    }

    /// `getPathInfo()`.
    pub fn path_info(&self) -> &PhpString {
        &self.path_info
    }

    /// `setPathInfo()`.
    pub fn set_path_info(&mut self, path_info: impl AsRef<[u8]>) -> &mut Self {
        self.path_info = PhpString::from(path_info.as_ref());
        self
    }

    /// `getMethod()`: always uppercased.
    pub fn method(&self) -> &PhpString {
        &self.method
    }

    /// `setMethod()`.
    pub fn set_method(&mut self, method: impl AsRef<[u8]>) -> &mut Self {
        self.method = PhpString::from(method.as_ref().to_ascii_uppercase());
        self
    }

    /// `getHost()`: always lowercased, because it must be treated
    /// case-insensitive.
    pub fn host(&self) -> &PhpString {
        &self.host
    }

    /// `setHost()`.
    pub fn set_host(&mut self, host: impl AsRef<[u8]>) -> &mut Self {
        self.host = lower(host.as_ref());
        self
    }

    /// `getScheme()`.
    pub fn scheme(&self) -> &PhpString {
        &self.scheme
    }

    /// `setScheme()`: lowercased.
    pub fn set_scheme(&mut self, scheme: impl AsRef<[u8]>) -> &mut Self {
        self.scheme = lower(scheme.as_ref());
        self
    }

    /// `getHttpPort()`.
    pub fn http_port(&self) -> i64 {
        self.http_port
    }

    /// `setHttpPort()`.
    pub fn set_http_port(&mut self, http_port: i64) -> &mut Self {
        self.http_port = http_port;
        self
    }

    /// `getHttpsPort()`.
    pub fn https_port(&self) -> i64 {
        self.https_port
    }

    /// `setHttpsPort()`.
    pub fn set_https_port(&mut self, https_port: i64) -> &mut Self {
        self.https_port = https_port;
        self
    }

    /// `getQueryString()`: without the `?`.
    pub fn query_string(&self) -> &PhpString {
        &self.query_string
    }

    /// `setQueryString()`; null is `''`.
    pub fn set_query_string(&mut self, query_string: Option<impl AsRef<[u8]>>) -> &mut Self {
        self.query_string =
            query_string.map_or_else(PhpString::default, |q| PhpString::from(q.as_ref()));
        self
    }

    /// `getParameters()`.
    pub fn parameters(&self) -> &PhpArray {
        &self.parameters
    }

    /// `setParameters()`.
    pub fn set_parameters(&mut self, parameters: PhpArray) -> &mut Self {
        self.parameters = parameters;
        self
    }

    /// `getParameter()`: the value, or `None` when not set (or null).
    pub fn parameter(&self, name: &[u8]) -> Option<&Value> {
        self.parameters
            .get(&Key::from_bytes(name))
            .filter(|v| !v.is_null())
    }

    /// `hasParameter()`.
    pub fn has_parameter(&self, name: &[u8]) -> bool {
        self.parameters.contains_key(&Key::from_bytes(name))
    }

    /// `setParameter()`.
    pub fn set_parameter(&mut self, name: &[u8], parameter: Value) -> &mut Self {
        self.parameters.insert(Key::from_bytes(name), parameter);
        self
    }

    /// `isSecure()`.
    pub fn is_secure(&self) -> bool {
        self.scheme.as_bytes() == b"https"
    }
}
