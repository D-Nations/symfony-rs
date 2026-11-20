//! What the fixture needs from HttpFoundation beyond `symfony-http-foundation`:
//! `Request::getHost()`, `getPort()`, `getHttpHost()`, `getUriForPath()`
//! and `isHostValid()`; `InputBag::get()` / `getString()`;
//! `Response::__construct()` with `ResponseHeaderBag`'s default
//! `Cache-Control`; `RedirectResponse`; and `Response::prepare()`.
//!
//! Fixed by this fixture's deployment, as under `php -S` without a proxy:
//! no trusted proxies or trusted hosts, scheme `http` (no `HTTPS`), base URL
//! `''` (front controller at the document root), no `_format` attribute,
//! and `kernel.charset` `UTF-8`. Not ported: the `Date` header (the HTTP
//! server adds one), IPv4/IPv6 literal hosts, caller-supplied
//! `Cache-Control`, cookies.
//!
//! Upstream: symfony/http-foundation Request.php InputBag.php Response.php ResponseHeaderBag.php RedirectResponse.php

use axum::{
    body::Body,
    extract::Request,
    http::{
        HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Version, header, request::Parts,
    },
    response::Response,
};
use php_value::{PhpString, numeric::string_to_int};
use symfony_http_foundation::query::Query;
use symfony_http_kernel::Exception;

use super::{
    NotPorted,
    php::{htmlspecialchars_quotes, strtolower, trim},
};

/// `$_SERVER` values the deployment supplies, used when a request has no
/// `Host` header.
#[derive(Debug, Clone)]
pub struct Server {
    pub name: String,
    pub port: i64,
}

/// The parts of `Request` that the router listener and RedirectController read.
#[derive(Debug, Clone)]
pub struct RequestInfo {
    /// `getMethod()`: `REQUEST_METHOD`, uppercased (no method override).
    pub method: String,
    /// `getPathInfo()`: the raw path, without the query string.
    pub path_info: String,
    /// `$request->server->get('QUERY_STRING')`, as sent.
    pub query_string: String,
    /// `getHost()`, validated.
    pub host: String,
    /// `getPort()`.
    pub port: i64,
    /// `$request->headers->get('referer')`.
    pub referer: Option<String>,
}

impl RequestInfo {
    pub fn from_parts(
        method: &Method,
        uri: &axum::http::Uri,
        headers: &HeaderMap,
        server: &Server,
    ) -> Result<Self, Exception> {
        // `!$host = $this->headers->get('HOST')`: '' and '0' count as absent.
        let host_header = header_text(headers, header::HOST).filter(|h| !h.is_empty() && h != "0");
        let host = host(host_header.as_deref(), server)?;
        let port = match &host_header {
            Some(raw) => port(raw)?,
            None => server.port,
        };
        Ok(Self {
            method: method.as_str().to_ascii_uppercase(),
            path_info: uri.path().to_owned(),
            query_string: uri.query().unwrap_or("").to_owned(),
            host,
            port,
            referer: header_text(headers, header::REFERER).filter(|r| !r.is_empty()),
        })
    }

    /// `getScheme()`: always `http` in this deployment.
    pub fn scheme(&self) -> &'static str {
        "http"
    }

    /// `getHttpHost()`.
    pub fn http_host(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// `getUriForPath($path)`: scheme and HTTP host, base URL `''`, path.
    pub fn uri_for_path(&self, path: &str) -> String {
        format!("{}://{}{}", self.scheme(), self.http_host(), path)
    }
}

/// `$request->headers->get($name)`: the first value. PHP keeps the bytes;
/// a value that is not UTF-8 is read lossily here.
fn header_text(headers: &HeaderMap, name: HeaderName) -> Option<String> {
    headers
        .get(name)
        .map(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
}

/// `getHost()`: the Host header (else `SERVER_NAME`), trimmed, a trailing
/// `:digits` removed, lowercased, then `isHostValid()`. An invalid host
/// throws SuspiciousOperationException, a request exception, which
/// `HttpKernel::handle` turns into a 400 with the same message.
fn host(host_header: Option<&str>, server: &Server) -> Result<String, Exception> {
    let raw = host_header.unwrap_or(&server.name);
    let trimmed = trim(raw);
    let without_port = match trimmed.rfind(':') {
        Some(at)
            if at + 1 < trimmed.len() && trimmed[at + 1..].bytes().all(|b| b.is_ascii_digit()) =>
        {
            &trimmed[..at]
        }
        _ => trimmed,
    };
    let host = strtolower(without_port);
    if !host.is_empty() && !is_host_valid(&host)? {
        return Err(Exception::new(
            StatusCode::BAD_REQUEST,
            format!("Invalid Host \"{host}\"."),
        ));
    }
    Ok(host)
}

/// `isHostValid()`. Only the host-name branch is ported: `[...]` IPv6
/// literals and names ending in `.digits` (IPv4 via `filter_var`) are refused.
fn is_host_valid(host: &str) -> Result<bool, NotPorted> {
    if host.starts_with('[') {
        return Err(NotPorted("IPv6 literal Host validation (filter_var)"));
    }
    let bytes = host.as_bytes();
    // preg_match('/\.[0-9]++\.?$/D', $host)
    let core = bytes.strip_suffix(b".").unwrap_or(bytes);
    let digits = core.iter().rev().take_while(|b| b.is_ascii_digit()).count();
    if digits > 0 && core.len() > digits && core[core.len() - digits - 1] == b'.' {
        return Err(NotPorted("IPv4-shaped Host validation (filter_var)"));
    }
    // '' === preg_replace('/[-a-zA-Z0-9_]++\.?/', '', $host): runs of name
    // characters, each followed by at most one dot.
    let name = |b: &u8| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_';
    let mut at = 0;
    while at < bytes.len() {
        let run = bytes[at..].iter().take_while(|b| name(b)).count();
        if run == 0 {
            return Ok(false);
        }
        at += run;
        if bytes.get(at) == Some(&b'.') {
            at += 1;
        }
    }
    Ok(true)
}

/// `getPort()` from a Host header: the text after the last `:` (after `]`
/// for IPv6), `(int)`-cast; 80 when there is none or it is falsy (`''`, `'0'`).
fn port(host_header: &str) -> Result<i64, NotPorted> {
    if host_header.starts_with('[') {
        return Err(NotPorted("IPv6 literal Host port"));
    }
    Ok(
        match host_header.rfind(':').map(|at| &host_header[at + 1..]) {
            Some(text) if !text.is_empty() && text != "0" => string_to_int(text.as_bytes()),
            _ => 80,
        },
    )
}

/// `$request->query` (an `InputBag`), parsed by `symfony-http-foundation`.
pub struct InputBag(Query);

impl InputBag {
    pub fn query(request: &Request) -> Self {
        Self(Query::parse(request.uri().query().unwrap_or("")))
    }

    /// `getString($key, $default)`: `(string) $this->get($key, $default)`.
    /// A non-scalar value throws BadRequestException, which
    /// `HttpKernel::handle` turns into a 400 with the same message.
    pub fn get_string(&self, key: &str, default: &str) -> Result<PhpString, Exception> {
        match self.0.get(key) {
            Ok(value) => Ok(PhpString::from(value.unwrap_or(default))),
            Err(_) => Err(Exception::new(
                StatusCode::BAD_REQUEST,
                format!("Input value \"{key}\" contains a non-scalar value."),
            )),
        }
    }
}

/// `new Response($content, $status, $headers)`: headers set in order (a
/// later name replaces an earlier one, case-insensitively), then
/// `ResponseHeaderBag` computes `Cache-Control`.
pub fn new_response(
    content: PhpString,
    status: StatusCode,
    headers: &[(&str, &str)],
) -> Result<Response, Exception> {
    let mut response = Response::new(Body::from(content.to_vec()));
    *response.status_mut() = status;
    for (name, value) in headers {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| NotPorted("a header name http cannot represent"))?;
        let value = HeaderValue::from_bytes(value.as_bytes())
            .map_err(|_| NotPorted("a header value http cannot represent"))?;
        response.headers_mut().insert(name, value);
    }
    if response.headers().contains_key(header::CACHE_CONTROL) {
        return Err(NotPorted("caller-supplied Cache-Control (computeCacheControlValue)").into());
    }
    // computeCacheControlValue() with no directives.
    let value = if response.headers().contains_key(header::LAST_MODIFIED)
        || response.headers().contains_key(header::EXPIRES)
    {
        "private, must-revalidate"
    } else {
        "no-cache, private"
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(value));
    Ok(response)
}

/// `new RedirectResponse($url, $status)`: the refresh page as content,
/// `Location` and `Content-Type: text/html; charset=utf-8`; a 301 without
/// a caller-supplied Cache-Control drops the default one.
pub fn redirect_response(url: &str, status: StatusCode) -> Result<Response, Exception> {
    if url.is_empty() {
        return Err(
            NotPorted("RedirectResponse to an empty URL (InvalidArgumentException)").into(),
        );
    }
    if !matches!(status.as_u16(), 201 | 301 | 302 | 303 | 307 | 308) {
        return Err(NotPorted(
            "RedirectResponse with a non-redirect status (InvalidArgumentException)",
        )
        .into());
    }
    let escaped = htmlspecialchars_quotes(url);
    let content = format!(
        "<!DOCTYPE html>\n<html>\n    <head>\n        <meta charset=\"UTF-8\" />\n        <meta http-equiv=\"refresh\" content=\"0;url='{escaped}'\" />\n\n        <title>Redirecting to {escaped}</title>\n    </head>\n    <body>\n        Redirecting to <a href=\"{escaped}\">{escaped}</a>.\n    </body>\n</html>"
    );
    let mut response = new_response(
        PhpString::from(content),
        status,
        &[
            ("Location", url),
            ("Content-Type", "text/html; charset=utf-8"),
        ],
    )?;
    if status == StatusCode::MOVED_PERMANENTLY {
        response.headers_mut().remove(header::CACHE_CONTROL);
    }
    Ok(response)
}

/// `Response::prepare($request)` for a response with no charset of its own
/// (ResponseListener gives it `kernel.charset`).
pub fn prepare(response: &mut Response, request: &Parts, charset: &str) {
    let status = response.status();
    let headers = response.headers_mut();
    if status.is_informational() || matches!(status.as_u16(), 204 | 304) {
        headers.remove(header::CONTENT_TYPE);
        headers.remove(header::CONTENT_LENGTH);
        *response.body_mut() = Body::empty();
    } else {
        match headers
            .get(header::CONTENT_TYPE)
            .map(|v| v.as_bytes().to_vec())
        {
            None => {
                if let Ok(value) = HeaderValue::from_str(&format!("text/html; charset={charset}")) {
                    headers.insert(header::CONTENT_TYPE, value);
                }
            }
            Some(content_type) => {
                let lower = content_type.to_ascii_lowercase();
                if lower.starts_with(b"text/") && !lower.windows(7).any(|w| w == b"charset") {
                    let mut value = content_type;
                    value.extend_from_slice(format!("; charset={charset}").as_bytes());
                    if let Ok(value) = HeaderValue::from_bytes(&value) {
                        headers.insert(header::CONTENT_TYPE, value);
                    }
                }
            }
        }
        if headers.contains_key(header::TRANSFER_ENCODING) {
            headers.remove(header::CONTENT_LENGTH);
        }
        if request.method == Method::HEAD {
            // Content-Length, if set, is kept.
            *response.body_mut() = Body::empty();
        }
    }
    // The response starts as HTTP/1.0; any other request protocol makes it 1.1.
    if request.version == Version::HTTP_10 {
        *response.version_mut() = Version::HTTP_10;
        let no_cache = response
            .headers()
            .get(header::CACHE_CONTROL)
            .is_some_and(|v| v.as_bytes().windows(8).any(|w| w == b"no-cache"));
        if no_cache {
            response
                .headers_mut()
                .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
            response
                .headers_mut()
                .insert(header::EXPIRES, HeaderValue::from_static("-1"));
        }
    } else {
        *response.version_mut() = Version::HTTP_11;
    }
}
