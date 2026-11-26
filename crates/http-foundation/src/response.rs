//! Port of Symfony\Component\HttpFoundation\Response.php: status, content,
//! protocol version and charset; the cache headers (`setCache()`,
//! freshness, validators, `Vary`); `prepare()` against a [`Request`];
//! `isNotModified()`; and `sendHeaders()` / `sendContent()` / `send()`.
//!
//! What PHP does to the process while sending (`header()`,
//! `header_remove()`, `headers_send()`, `echo`, the output buffers,
//! `fastcgi_finish_request()`, `ini_set('default_mimetype', '')`) is
//! recorded, in order, in this thread's [`crate::php::Output`]; the
//! HTTP adapter applies it. `headers_sent()`, `PHP_SAPI` and which
//! functions exist are read from there too.
//!
//! PHP types: a status code is an `int` (`i64`); content, texts and the
//! protocol version are byte strings; `setCache()` takes PHP's options
//! array and coerces its values as PHP does a call without
//! `strict_types` (`TypeError` messages without PHP's "called in"
//! suffix). `Response::$statusTexts` is a public static array in PHP that
//! a program may change; here it is the constant [`STATUS_TEXTS`]. A
//! subclass's `static::class` in the `headers` deprecation is not
//! reproduced (a Rust struct has no subclasses).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation Response.php

use php_pcre::Pcre;
use php_value::{
    Key, PhpArray, PhpString, Value,
    clock::time,
    compare::loose_equals,
    dateparse::strtotime,
    datetime::{DateTimeImmutable, TimeZone, gmdate},
    numeric::{int_argument, string_to_int},
};

use crate::{
    binary_file_response::BinaryFile,
    exception::{Class, Error, Result},
    header_bag::{HeaderValues, ResponseHeaderBag, Values},
    php::{
        FinishRequest, OutputCall, PHP_OUTPUT_HANDLER_CLEANABLE, PHP_OUTPUT_HANDLER_FLUSHABLE,
        PHP_OUTPUT_HANDLER_REMOVABLE, with_output,
    },
    request::Request,
    streamed_response::{Stream, Throwable},
};

/// `Response::class`.
pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\Response";

/// `HTTP_RESPONSE_CACHE_CONTROL_DIRECTIVES`: each `setCache()` option and
/// whether it carries a value.
const CACHE_CONTROL_DIRECTIVES: [(&str, bool); 14] = [
    ("must_revalidate", false),
    ("no_cache", false),
    ("no_store", false),
    ("no_transform", false),
    ("public", false),
    ("private", false),
    ("proxy_revalidate", false),
    ("max_age", true),
    ("s_maxage", true),
    ("stale_if_error", true),         // RFC5861
    ("stale_while_revalidate", true), // RFC5861
    ("immutable", false),
    ("last_modified", true),
    ("etag", true),
];

/// `Response::$statusTexts`: the status codes translation table, complete
/// according to the IANA HTTP Status Code Registry (last updated
/// 2021-10-01). Unless otherwise noted, the status code is defined in
/// RFC2616.
pub const STATUS_TEXTS: [(i64, &str); 62] = [
    (100, "Continue"),
    (101, "Switching Protocols"),
    (102, "Processing"), // RFC2518
    (103, "Early Hints"),
    (200, "OK"),
    (201, "Created"),
    (202, "Accepted"),
    (203, "Non-Authoritative Information"),
    (204, "No Content"),
    (205, "Reset Content"),
    (206, "Partial Content"),
    (207, "Multi-Status"),     // RFC4918
    (208, "Already Reported"), // RFC5842
    (226, "IM Used"),          // RFC3229
    (300, "Multiple Choices"),
    (301, "Moved Permanently"),
    (302, "Found"),
    (303, "See Other"),
    (304, "Not Modified"),
    (305, "Use Proxy"),
    (307, "Temporary Redirect"),
    (308, "Permanent Redirect"), // RFC7238
    (400, "Bad Request"),
    (401, "Unauthorized"),
    (402, "Payment Required"),
    (403, "Forbidden"),
    (404, "Not Found"),
    (405, "Method Not Allowed"),
    (406, "Not Acceptable"),
    (407, "Proxy Authentication Required"),
    (408, "Request Timeout"),
    (409, "Conflict"),
    (410, "Gone"),
    (411, "Length Required"),
    (412, "Precondition Failed"),
    (413, "Content Too Large"), // RFC-ietf-httpbis-semantics
    (414, "URI Too Long"),
    (415, "Unsupported Media Type"),
    (416, "Range Not Satisfiable"),
    (417, "Expectation Failed"),
    (418, "I'm a teapot"),                    // RFC2324
    (421, "Misdirected Request"),             // RFC7540
    (422, "Unprocessable Content"),           // RFC-ietf-httpbis-semantics
    (423, "Locked"),                          // RFC4918
    (424, "Failed Dependency"),               // RFC4918
    (425, "Too Early"),                       // RFC-ietf-httpbis-replay-04
    (426, "Upgrade Required"),                // RFC2817
    (428, "Precondition Required"),           // RFC6585
    (429, "Too Many Requests"),               // RFC6585
    (431, "Request Header Fields Too Large"), // RFC6585
    (451, "Unavailable For Legal Reasons"),   // RFC7725
    (500, "Internal Server Error"),
    (501, "Not Implemented"),
    (502, "Bad Gateway"),
    (503, "Service Unavailable"),
    (504, "Gateway Timeout"),
    (505, "HTTP Version Not Supported"),
    (506, "Variant Also Negotiates"),         // RFC2295
    (507, "Insufficient Storage"),            // RFC4918
    (508, "Loop Detected"),                   // RFC5842
    (510, "Not Extended"),                    // RFC2774
    (511, "Network Authentication Required"), // RFC6585
];

/// `Response::$statusTexts[$code]`.
pub fn status_text(code: i64) -> Option<&'static str> {
    STATUS_TEXTS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, text)| *text)
}

/// `Symfony\Component\HttpFoundation\Response`.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    headers: ResponseHeaderBag,
    content: PhpString,
    version: PhpString,
    status_code: i64,
    status_text: PhpString,
    charset: Option<PhpString>,
    /// `$this->sentHeaders`: headers already sent in informational
    /// responses.
    sent_headers: Vec<(Key, Values)>,
    /// The `StreamedResponse` state, for an instance of that subclass.
    stream: Option<Box<Stream>>,
    /// The `BinaryFileResponse` state, for an instance of that subclass.
    binary: Option<Box<BinaryFile>>,
}

impl Default for Response {
    /// `new Response()`.
    fn default() -> Self {
        Self {
            headers: ResponseHeaderBag::empty(),
            content: PhpString::default(),
            version: PhpString::from("1.0"),
            status_code: 200,
            status_text: PhpString::from("OK"),
            charset: None,
            sent_headers: Vec::new(),
            stream: None,
            binary: None,
        }
    }
}

/// `$a >= $b` for two `strtotime()` results (`int|false`): `false` is
/// compared as a bool with the other side.
fn time_at_least(a: Option<i64>, b: Option<i64>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a >= b,
        (None, Some(b)) => b == 0,
        (Some(_) | None, None) => true,
    }
}

/// `strtotime($text)` at the current time; a text it does not read is
/// refused.
fn strtotime_now(text: &[u8]) -> Result<Option<i64>> {
    strtotime(text, time()).map_err(|unsupported| Error::new(Class::NotPorted, unsupported.0))
}

/// `(int) $value` for a Cache-Control directive (`true` or a string).
fn int_cast(value: &Value) -> i64 {
    match value {
        Value::Null => 0,
        Value::Bool(b) => i64::from(*b),
        Value::Int(i) => *i,
        Value::String(s) => string_to_int(s.as_bytes()),
        other => other
            .to_php_string()
            .map_or(0, |s| string_to_int(s.as_bytes())),
    }
}

/// `stripos($haystack, $needle)` for an ASCII needle.
fn stripos(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window.eq_ignore_ascii_case(needle))
}

/// `preg_split('/[\s,]+/', $item)`: empty pieces kept.
fn split_vary(item: &[u8]) -> Vec<PhpString> {
    let separator = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b',');
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < item.len() {
        if separator(&item[at]) {
            pieces.push(PhpString::from(&item[start..at]));
            while at < item.len() && separator(&item[at]) {
                at += 1;
            }
            start = at;
        } else {
            at += 1;
        }
    }
    pieces.push(PhpString::from(&item[start..]));
    pieces
}

/// `array_diff($a, $b)`'s values: those of `a` whose string form is not
/// among `b`'s.
fn array_diff(a: &Values, b: &Values) -> Values {
    let text = |v: &Option<PhpString>| v.as_ref().map_or(&b""[..], PhpString::as_bytes).to_vec();
    let others: Vec<Vec<u8>> = b.iter().map(text).collect();
    a.iter()
        .filter(|v| !others.contains(&text(v)))
        .cloned()
        .collect()
}

fn key_text(key: &Key) -> PhpString {
    key.to_value().to_php_string().unwrap_or_default()
}

fn concat(parts: &[&[u8]]) -> PhpString {
    PhpString::from(parts.concat())
}

/// `header($line, $replace, $responseCode)`.
fn header(line: PhpString, replace: bool, response_code: i64) {
    with_output(|o| {
        o.calls.push(OutputCall::Header {
            line,
            replace,
            response_code,
        });
    });
}

fn not_ported(error: impl std::fmt::Display) -> Error {
    Error::new(Class::NotPorted, error.to_string())
}

/// The `TypeError` for a `setCache()` value passed to `method`.
fn type_error(method: &str, parameter: &str, expected: &str, given: &Value) -> Error {
    Error::new(
        Class::PhpTypeError,
        format!(
            "{CLASS}::{method}(): Argument #1 (${parameter}) must be of type {expected}, {} given",
            given.debug_type()
        ),
    )
}

/// A `setCache()` value passed to an `int` parameter.
fn int_option(method: &str, value: &Value) -> Result<i64> {
    const LIMIT: f64 = 9_223_372_036_854_775_808.0; // 2^63
    match value {
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::Int(i) => Ok(*i),
        #[expect(
            clippy::cast_possible_truncation,
            reason = "PHP truncates a float passed to an int parameter"
        )]
        Value::Float(f) if f.is_finite() && (-LIMIT..LIMIT).contains(f) => Ok(f.trunc() as i64),
        Value::String(s) => {
            int_argument(s.as_bytes()).ok_or_else(|| type_error(method, "value", "int", value))
        }
        _ => Err(type_error(method, "value", "int", value)),
    }
}

impl Response {
    pub const HTTP_CONTINUE: i64 = 100;
    pub const HTTP_SWITCHING_PROTOCOLS: i64 = 101;
    pub const HTTP_PROCESSING: i64 = 102; // RFC2518
    pub const HTTP_EARLY_HINTS: i64 = 103; // RFC8297
    pub const HTTP_OK: i64 = 200;
    pub const HTTP_CREATED: i64 = 201;
    pub const HTTP_ACCEPTED: i64 = 202;
    pub const HTTP_NON_AUTHORITATIVE_INFORMATION: i64 = 203;
    pub const HTTP_NO_CONTENT: i64 = 204;
    pub const HTTP_RESET_CONTENT: i64 = 205;
    pub const HTTP_PARTIAL_CONTENT: i64 = 206;
    pub const HTTP_MULTI_STATUS: i64 = 207; // RFC4918
    pub const HTTP_ALREADY_REPORTED: i64 = 208; // RFC5842
    pub const HTTP_IM_USED: i64 = 226; // RFC3229
    pub const HTTP_MULTIPLE_CHOICES: i64 = 300;
    pub const HTTP_MOVED_PERMANENTLY: i64 = 301;
    pub const HTTP_FOUND: i64 = 302;
    pub const HTTP_SEE_OTHER: i64 = 303;
    pub const HTTP_NOT_MODIFIED: i64 = 304;
    pub const HTTP_USE_PROXY: i64 = 305;
    pub const HTTP_RESERVED: i64 = 306;
    pub const HTTP_TEMPORARY_REDIRECT: i64 = 307;
    pub const HTTP_PERMANENTLY_REDIRECT: i64 = 308; // RFC7238
    pub const HTTP_BAD_REQUEST: i64 = 400;
    pub const HTTP_UNAUTHORIZED: i64 = 401;
    pub const HTTP_PAYMENT_REQUIRED: i64 = 402;
    pub const HTTP_FORBIDDEN: i64 = 403;
    pub const HTTP_NOT_FOUND: i64 = 404;
    pub const HTTP_METHOD_NOT_ALLOWED: i64 = 405;
    pub const HTTP_NOT_ACCEPTABLE: i64 = 406;
    pub const HTTP_PROXY_AUTHENTICATION_REQUIRED: i64 = 407;
    pub const HTTP_REQUEST_TIMEOUT: i64 = 408;
    pub const HTTP_CONFLICT: i64 = 409;
    pub const HTTP_GONE: i64 = 410;
    pub const HTTP_LENGTH_REQUIRED: i64 = 411;
    pub const HTTP_PRECONDITION_FAILED: i64 = 412;
    pub const HTTP_REQUEST_ENTITY_TOO_LARGE: i64 = 413;
    pub const HTTP_REQUEST_URI_TOO_LONG: i64 = 414;
    pub const HTTP_UNSUPPORTED_MEDIA_TYPE: i64 = 415;
    pub const HTTP_REQUESTED_RANGE_NOT_SATISFIABLE: i64 = 416;
    pub const HTTP_EXPECTATION_FAILED: i64 = 417;
    pub const HTTP_I_AM_A_TEAPOT: i64 = 418; // RFC2324
    pub const HTTP_MISDIRECTED_REQUEST: i64 = 421; // RFC7540
    pub const HTTP_UNPROCESSABLE_ENTITY: i64 = 422; // RFC4918
    pub const HTTP_LOCKED: i64 = 423; // RFC4918
    pub const HTTP_FAILED_DEPENDENCY: i64 = 424; // RFC4918
    pub const HTTP_TOO_EARLY: i64 = 425; // RFC-ietf-httpbis-replay-04
    pub const HTTP_UPGRADE_REQUIRED: i64 = 426; // RFC2817
    pub const HTTP_PRECONDITION_REQUIRED: i64 = 428; // RFC6585
    pub const HTTP_TOO_MANY_REQUESTS: i64 = 429; // RFC6585
    pub const HTTP_REQUEST_HEADER_FIELDS_TOO_LARGE: i64 = 431; // RFC6585
    pub const HTTP_UNAVAILABLE_FOR_LEGAL_REASONS: i64 = 451; // RFC7725
    pub const HTTP_INTERNAL_SERVER_ERROR: i64 = 500;
    pub const HTTP_NOT_IMPLEMENTED: i64 = 501;
    pub const HTTP_BAD_GATEWAY: i64 = 502;
    pub const HTTP_SERVICE_UNAVAILABLE: i64 = 503;
    pub const HTTP_GATEWAY_TIMEOUT: i64 = 504;
    pub const HTTP_VERSION_NOT_SUPPORTED: i64 = 505;
    pub const HTTP_VARIANT_ALSO_NEGOTIATES_EXPERIMENTAL: i64 = 506; // RFC2295
    pub const HTTP_INSUFFICIENT_STORAGE: i64 = 507; // RFC4918
    pub const HTTP_LOOP_DETECTED: i64 = 508; // RFC5842
    pub const HTTP_NOT_EXTENDED: i64 = 510; // RFC2774
    pub const HTTP_NETWORK_AUTHENTICATION_REQUIRED: i64 = 511; // RFC6585

    /// `new Response($content, $status, $headers)`.
    ///
    /// Fails with `\InvalidArgumentException` when the status code is not
    /// valid.
    pub fn new(
        content: Option<impl Into<PhpString>>,
        status: i64,
        headers: ResponseHeaderBag,
    ) -> Result<Self> {
        let mut response = Self {
            headers,
            ..Self::default()
        };
        response.set_content(content.map(Into::into))?;
        response.set_status_code(status, None)?;
        response.set_protocol_version("1.0");
        Ok(response)
    }

    /// `new StreamedResponse(...)`'s `parent::__construct(null, $status,
    /// $headers)`, with the subclass's state (see
    /// [`crate::streamed_response::new`]).
    pub(crate) fn with_stream(
        status: i64,
        headers: ResponseHeaderBag,
        stream: Stream,
    ) -> Result<Self> {
        let mut response = Self {
            headers,
            stream: Some(Box::new(stream)),
            ..Self::default()
        };
        response.set_status_code(status, None)?;
        response.set_protocol_version("1.0");
        Ok(response)
    }

    /// The `BinaryFileResponse` state: `Some` for `$response instanceof
    /// BinaryFileResponse`.
    pub fn binary_file(&self) -> Option<&BinaryFile> {
        self.binary.as_deref()
    }

    /// The `BinaryFileResponse` state, to change it.
    pub fn binary_file_mut(&mut self) -> Option<&mut BinaryFile> {
        self.binary.as_deref_mut()
    }

    /// `new BinaryFileResponse(...)`'s `parent::__construct(null,
    /// $status, $headers)`, with the subclass's state.
    pub(crate) fn with_binary_file(
        status: i64,
        headers: ResponseHeaderBag,
        binary: BinaryFile,
    ) -> Result<Self> {
        let mut response = Self::new(None::<PhpString>, status, headers)?;
        response.binary = Some(Box::new(binary));
        Ok(response)
    }

    /// The `StreamedResponse` state: `Some` for `$response instanceof
    /// StreamedResponse`.
    pub fn stream(&self) -> Option<&Stream> {
        self.stream.as_deref()
    }

    /// The `StreamedResponse` state, to set its callback or chunks.
    pub fn stream_mut(&mut self) -> Option<&mut Stream> {
        self.stream.as_deref_mut()
    }

    /// `$response::class`.
    pub fn class(&self) -> &'static str {
        if self.binary.is_some() {
            return crate::binary_file_response::CLASS;
        }
        match self.stream.as_deref() {
            Some(stream) if stream.event_stream().is_some() => crate::event_stream_response::CLASS,
            Some(stream) if stream.is_json() => crate::streamed_json_response::CLASS,
            Some(_) => crate::streamed_response::CLASS,
            None => CLASS,
        }
    }

    /// `$response->headers`.
    pub fn headers(&self) -> &ResponseHeaderBag {
        &self.headers
    }

    /// `$response->headers`, to change the bag.
    pub fn headers_mut(&mut self) -> &mut ResponseHeaderBag {
        &mut self.headers
    }

    /// `$response->headers = $bag`, deprecated since 8.1.
    pub fn set_headers(&mut self, headers: ResponseHeaderBag) {
        symfony_deprecation_contracts::trigger_deprecation(
            b"symfony/http-foundation",
            b"8.1",
            format!(
                "Directly setting property \"headers\" of \"{CLASS}\" is deprecated; pass the header bag as a constructor argument instead."
            )
            .as_bytes(),
        );
        self.headers = headers;
    }

    /// `__toString()`: the response as an HTTP string. It is what will be
    /// sent only after `prepare()`.
    pub fn to_php_string(&self) -> PhpString {
        let mut text = self.status_line(self.status_code).as_bytes().to_vec();
        text.extend_from_slice(b"\r\n");
        text.extend_from_slice(self.headers.to_php_string().as_bytes());
        text.extend_from_slice(b"\r\n");
        // `getContent()`; a streamed response's `false` is `''`.
        if let Some(content) = self.get_content() {
            text.extend_from_slice(content.as_bytes());
        }
        PhpString::from(text)
    }

    /// `sprintf('HTTP/%s %s %s', $this->version, $code, $this->statusText)`.
    fn status_line(&self, code: i64) -> PhpString {
        concat(&[
            b"HTTP/",
            self.version.as_bytes(),
            b" ",
            code.to_string().as_bytes(),
            b" ",
            self.status_text.as_bytes(),
        ])
    }

    /// `prepare($request)`: makes the response compliant with RFC 2616
    /// for the request it answers. A `BinaryFileResponse` also sets its
    /// file's headers and the range to send.
    pub fn prepare(&mut self, request: &mut Request) -> Result<&mut Self> {
        if self.binary.is_some() {
            crate::binary_file_response::prepare(self, request)?;
            return Ok(self);
        }
        self.prepare_parent(request)
    }

    /// `Response::prepare($request)`, which a subclass calls as
    /// `parent::prepare()`.
    pub(crate) fn prepare_parent(&mut self, request: &mut Request) -> Result<&mut Self> {
        if self.is_informational() || self.is_empty() {
            self.set_content(None)?;
            self.headers.remove("Content-Type");
            self.headers.remove("Content-Length");
            // prevent PHP from sending the Content-Type header based on default_mimetype
            with_output(|o| o.default_mimetype = PhpString::default());
        } else {
            // Content-type based on the Request
            if !self.headers.has("Content-Type")
                && let Some(format) = request.get_request_format(None)?
                && let Some(mime_type) = request.get_mime_type(format.as_bytes())
                && Value::String(mime_type.clone()).is_truthy()
            {
                self.headers.set(b"Content-Type", mime_type, true)?;
            }

            // Fix Content-Type
            let charset = match &self.charset {
                Some(charset) if Value::String(charset.clone()).is_truthy() => charset.clone(),
                _ => PhpString::from("utf-8"),
            };
            if self.headers.has("Content-Type") {
                let content_type = self.headers.get("Content-Type", None).unwrap_or_default();
                if stripos(content_type.as_bytes(), b"text/") == Some(0)
                    && stripos(content_type.as_bytes(), b"charset").is_none()
                {
                    // add the charset
                    let value =
                        concat(&[content_type.as_bytes(), b"; charset=", charset.as_bytes()]);
                    self.headers.set(b"Content-Type", value, true)?;
                }
            } else {
                let value = concat(&[b"text/html; charset=", charset.as_bytes()]);
                self.headers.set(b"Content-Type", value, true)?;
            }

            // Fix Content-Length
            if self.headers.has("Transfer-Encoding") {
                self.headers.remove("Content-Length");
            }

            if request.is_method("HEAD")? {
                // cf. RFC2616 14.13
                let length = self.headers.get("Content-Length", None);
                self.set_content(None)?;
                if let Some(length) = length
                    && Value::String(length.clone()).is_truthy()
                {
                    self.headers.set(b"Content-Length", length, true)?;
                }
            }
        }

        // Fix protocol
        let protocol = request.server().get("SERVER_PROTOCOL", Value::Null);
        if !loose_equals(&Value::string("HTTP/1.0"), &protocol) {
            self.set_protocol_version("1.1");
        }

        // Check if we need to send extra expire info headers
        let cache_control = self
            .headers
            .get("Cache-Control", Some(""))
            .unwrap_or_default();
        if loose_equals(&Value::string("1.0"), &Value::String(self.version.clone()))
            && cache_control
                .as_bytes()
                .windows(8)
                .any(|w| w == b"no-cache")
        {
            self.headers.set(b"pragma", "no-cache", true)?;
            self.headers.set(b"expires", "-1", true)?;
        }

        self.ensure_ie_over_ssl_compatibility(request)?;

        if request.is_secure()? {
            for cookie in self.headers.cookies_mut() {
                cookie.set_secure_default(true);
            }
        }

        Ok(self)
    }

    /// `sendHeaders($statusCode)`: the headers, then the cookies, then the
    /// status line. A 1xx `status_code` sends an informational response
    /// (`headers_send()`) where the SAPI has it, without changing the
    /// response's own status.
    ///
    /// A `StreamedResponse` sends them once: after a call that was not
    /// informational, later calls do nothing.
    pub fn send_headers(&mut self, status_code: Option<i64>) -> Result<&mut Self> {
        if let Some(stream) = &mut self.stream {
            if stream.headers_sent {
                return Ok(self);
            }
            // `$statusCode < 100 || $statusCode >= 200`; null is below 100.
            if !status_code.is_some_and(|code| (100..200).contains(&code)) {
                stream.headers_sent = true;
            }
        }
        let (headers_sent, sapi, headers_send_exists) =
            with_output(|o| (o.headers_sent, o.sapi.clone(), o.headers_send_exists));
        // headers have already been sent by the developer
        if headers_sent {
            if !matches!(sapi.as_bytes(), b"cli" | b"phpdbg" | b"embed") {
                let code = status_code.unwrap_or(self.status_code);
                header(self.status_line(code), true, code);
            }
            return Ok(self);
        }

        let informational = status_code.is_some_and(|code| (100..200).contains(&code));
        if informational && !headers_send_exists {
            // skip informational responses if not supported by the SAPI
            return Ok(self);
        }

        // headers
        for (name, values) in self.headers.all_preserve_case_without_cookies().iter() {
            // As recommended by RFC 8297, PHP automatically copies headers
            // from previous 103 responses, we need to deal with that if
            // headers changed
            let mut previous = self
                .sent_headers
                .iter()
                .find(|(sent, _)| sent == name)
                .map(|(_, values)| values.clone());
            if previous.as_ref() == Some(values) {
                // Header already sent in a previous response, it will be
                // automatically copied in this response by PHP
                continue;
            }

            let text = key_text(name);
            let replace = text.as_bytes().eq_ignore_ascii_case(b"Content-Type");

            if let Some(sent) = &previous
                && !array_diff(sent, values).is_empty()
            {
                with_output(|o| o.calls.push(OutputCall::HeaderRemove(text.clone())));
                previous = None;
            }

            let new_values = match &previous {
                None => values.clone(),
                Some(sent) => array_diff(values, sent),
            };

            for value in new_values {
                let value = value.unwrap_or_default();
                header(
                    concat(&[text.as_bytes(), b": ", value.as_bytes()]),
                    replace,
                    self.status_code,
                );
            }

            if informational {
                match self.sent_headers.iter_mut().find(|(sent, _)| sent == name) {
                    Some((_, sent)) => sent.clone_from(values),
                    None => self.sent_headers.push((name.clone(), values.clone())),
                }
            }
        }

        // cookies
        if let crate::header_bag::Cookies::Flat(cookies) =
            self.headers.get_cookies(ResponseHeaderBag::COOKIES_FLAT)?
        {
            for cookie in cookies {
                header(
                    concat(&[b"Set-Cookie: ", cookie.to_php_string().as_bytes()]),
                    false,
                    self.status_code,
                );
            }
        }

        if let (true, Some(code)) = (informational, status_code) {
            with_output(|o| o.calls.push(OutputCall::HeadersSend(code)));
            return Ok(self);
        }

        let code = status_code.unwrap_or(self.status_code);

        // status
        header(self.status_line(code), true, code);

        Ok(self)
    }

    /// `sendContent()`: `echo $this->content`. A `StreamedResponse` runs
    /// its callback instead, once; without one it throws
    /// `\LogicException`, and what the callback throws is passed on.
    pub fn send_content(&mut self) -> std::result::Result<&mut Self, Throwable> {
        if self.binary.is_some() {
            crate::binary_file_response::send_content(self)?;
            return Ok(self);
        }
        if let Some(stream) = &mut self.stream {
            stream.send_content()?;
            return Ok(self);
        }
        let content = self.content.clone();
        with_output(|o| o.calls.push(OutputCall::Echo(content)));
        Ok(self)
    }

    /// `send($flush)`: headers and content, then the request is finished
    /// (`fastcgi_finish_request()`, `litespeed_finish_request()`, or the
    /// output buffers flushed outside the CLI).
    pub fn send(&mut self, flush: bool) -> std::result::Result<&mut Self, Throwable> {
        self.send_headers(None)
            .map_err(|e| std::sync::Arc::new(e) as Throwable)?;
        self.send_content()?;

        if !flush {
            return Ok(self);
        }

        let (finish, sapi) = with_output(|o| (o.finish_request, o.sapi.clone()));
        match finish {
            FinishRequest::FastCgi => {
                with_output(|o| o.calls.push(OutputCall::FastcgiFinishRequest));
            }
            FinishRequest::LiteSpeed => {
                with_output(|o| o.calls.push(OutputCall::LitespeedFinishRequest));
            }
            FinishRequest::None => {
                if !matches!(sapi.as_bytes(), b"cli" | b"phpdbg" | b"embed") {
                    Self::close_output_buffers(0, true);
                    crate::php::flush();
                }
            }
        }

        Ok(self)
    }

    /// `setContent($content)`; null is `''`. A `StreamedResponse` takes
    /// only null, which marks its content as sent; other content throws
    /// `\LogicException`.
    pub fn set_content(&mut self, content: Option<PhpString>) -> Result<&mut Self> {
        if self.binary.is_some() {
            if content.is_some() {
                return Err(Error::new(
                    Class::PhpLogic,
                    "The content cannot be set on a BinaryFileResponse instance.",
                ));
            }
            return Ok(self);
        }
        if let Some(stream) = &mut self.stream {
            if content.is_some() {
                return Err(Error::new(
                    Class::PhpLogic,
                    "The content cannot be set on a StreamedResponse instance.",
                ));
            }
            stream.streamed = true;
            return Ok(self);
        }
        self.content = content.unwrap_or_default();
        Ok(self)
    }

    /// `$this->content`, for the subclasses whose `getContent()` is it.
    pub(crate) fn content(&self) -> &PhpString {
        &self.content
    }

    /// `getContent()`: `None` is the `false` of a `StreamedResponse` and
    /// a `BinaryFileResponse`.
    pub fn get_content(&self) -> Option<&PhpString> {
        if self.stream.is_some() || self.binary.is_some() {
            return None;
        }
        Some(&self.content)
    }

    /// `setProtocolVersion($version)`.
    pub fn set_protocol_version(&mut self, version: impl Into<PhpString>) -> &mut Self {
        self.version = version.into();
        self
    }

    /// `getProtocolVersion()`.
    pub fn get_protocol_version(&self) -> &PhpString {
        &self.version
    }

    /// `setStatusCode($code, $text)`: a null text is the code's from
    /// [`STATUS_TEXTS`], else `unknown status`.
    ///
    /// Fails with `\InvalidArgumentException` when the code is not valid;
    /// the code is set all the same, as in PHP.
    pub fn set_status_code(&mut self, code: i64, text: Option<PhpString>) -> Result<&mut Self> {
        self.status_code = code;
        if self.is_invalid() {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                format!("The HTTP status code \"{code}\" is not valid."),
            ));
        }

        self.status_text =
            text.unwrap_or_else(|| PhpString::from(status_text(code).unwrap_or("unknown status")));

        Ok(self)
    }

    /// `getStatusCode()`.
    pub fn get_status_code(&self) -> i64 {
        self.status_code
    }

    /// The protected `$statusText`.
    pub fn status_text(&self) -> &PhpString {
        &self.status_text
    }

    /// `setCharset($charset)`.
    pub fn set_charset(&mut self, charset: impl Into<PhpString>) -> &mut Self {
        self.charset = Some(charset.into());
        self
    }

    /// `getCharset()`.
    pub fn get_charset(&self) -> Option<&PhpString> {
        self.charset.as_ref()
    }

    /// `isCacheable()`: whether a shared cache may keep the response.
    pub fn is_cacheable(&self) -> Result<bool> {
        if ![200, 203, 300, 301, 302, 404, 410].contains(&self.status_code) {
            return Ok(false);
        }

        if self.headers.has_cache_control_directive("no-store")
            || self
                .headers
                .get_cache_control_directive("private")
                .is_truthy()
        {
            return Ok(false);
        }

        Ok(self.is_validateable() || self.is_fresh()?)
    }

    /// `isFresh()`: the TTL is positive.
    pub fn is_fresh(&self) -> Result<bool> {
        Ok(self.get_ttl()?.is_some_and(|ttl| ttl > 0))
    }

    /// `isValidateable()`: a Last-Modified or `ETag` header is present.
    pub fn is_validateable(&self) -> bool {
        self.headers.has("Last-Modified") || self.headers.has("ETag")
    }

    /// `setPrivate()`.
    pub fn set_private(&mut self) -> Result<&mut Self> {
        self.headers.remove_cache_control_directive("public")?;
        self.headers.add_cache_control_directive("private", true)?;
        Ok(self)
    }

    /// `setPublic()`.
    pub fn set_public(&mut self) -> Result<&mut Self> {
        self.headers.add_cache_control_directive("public", true)?;
        self.headers.remove_cache_control_directive("private")?;
        Ok(self)
    }

    /// `setImmutable($immutable)`.
    pub fn set_immutable(&mut self, immutable: bool) -> Result<&mut Self> {
        if immutable {
            self.headers
                .add_cache_control_directive("immutable", true)?;
        } else {
            self.headers.remove_cache_control_directive("immutable")?;
        }
        Ok(self)
    }

    /// `isImmutable()`.
    pub fn is_immutable(&self) -> bool {
        self.headers.has_cache_control_directive("immutable")
    }

    /// `mustRevalidate()`.
    pub fn must_revalidate(&self) -> bool {
        self.headers.has_cache_control_directive("must-revalidate")
            || self.headers.has_cache_control_directive("proxy-revalidate")
    }

    /// `getDate()`: the Date header; `\RuntimeException` when it is not
    /// parseable.
    pub fn get_date(&self) -> Result<Option<DateTimeImmutable>> {
        self.headers.get_date("Date", None)
    }

    /// `(int) $this->getDate()->format('U')`; PHP's `\Error` when there
    /// is no date.
    fn date_timestamp(&self) -> Result<i64> {
        self.get_date()?
            .map(|date| date.timestamp())
            .ok_or_else(|| {
                Error::new(
                    Class::PhpError,
                    "Call to a member function format() on null",
                )
            })
    }

    /// `$date->format('D, d M Y H:i:s').' GMT'` in UTC.
    fn http_date(date: &DateTimeImmutable) -> String {
        let mut text = gmdate("D, d M Y H:i:s", date.timestamp());
        text.push_str(" GMT");
        text
    }

    /// `setDate($date)`.
    pub fn set_date(&mut self, date: &DateTimeImmutable) -> Result<&mut Self> {
        self.headers
            .set(b"Date", Self::http_date(date).as_str(), true)?;
        Ok(self)
    }

    /// `getAge()`: the Age header, else the seconds since Date.
    pub fn get_age(&self) -> Result<i64> {
        if let Some(age) = self.headers.get("Age", None) {
            return Ok(string_to_int(age.as_bytes()));
        }

        Ok(time().saturating_sub(self.date_timestamp()?).max(0))
    }

    /// `expire()`: a fresh response gets an Age equal to its max age.
    pub fn expire(&mut self) -> Result<&mut Self> {
        if self.is_fresh()? {
            let max_age = self.get_max_age()?.unwrap_or_default();
            self.headers
                .set(b"Age", max_age.to_string().as_str(), true)?;
            self.headers.remove("Expires");
        }
        Ok(self)
    }

    /// `getExpires()`: the Expires header; an invalid date (`0`, `-1`)
    /// is two days ago, as RFC 2616 wants it in the past.
    pub fn get_expires(&self) -> Result<Option<DateTimeImmutable>> {
        match self.headers.get_date("Expires", None) {
            Err(error) if error.class() == Class::PhpRuntime => {
                // according to RFC 2616 invalid date formats (e.g. "0" and "-1") must be treated as in the past
                Ok(Some(DateTimeImmutable::from_timestamp(
                    time().saturating_sub(172_800),
                    0,
                    TimeZone::Offset(0),
                )))
            }
            other => other,
        }
    }

    /// `setExpires($date)`; null removes the header.
    pub fn set_expires(&mut self, date: Option<&DateTimeImmutable>) -> Result<&mut Self> {
        match date {
            None => self.headers.remove("Expires"),
            Some(date) => self
                .headers
                .set(b"Expires", Self::http_date(date).as_str(), true)?,
        }
        Ok(self)
    }

    /// `getMaxAge()`: s-maxage, else max-age, else Expires less Date;
    /// null when none is present.
    pub fn get_max_age(&self) -> Result<Option<i64>> {
        if self.headers.has_cache_control_directive("s-maxage") {
            return Ok(Some(int_cast(
                &self.headers.get_cache_control_directive("s-maxage"),
            )));
        }

        if self.headers.has_cache_control_directive("max-age") {
            return Ok(Some(int_cast(
                &self.headers.get_cache_control_directive("max-age"),
            )));
        }

        if let Some(expires) = self.get_expires()? {
            let max_age = expires.timestamp().saturating_sub(self.date_timestamp()?);
            return Ok(Some(max_age.max(0)));
        }

        Ok(None)
    }

    /// `setMaxAge($value)`.
    pub fn set_max_age(&mut self, value: i64) -> Result<&mut Self> {
        self.headers.add_cache_control_directive("max-age", value)?;
        Ok(self)
    }

    /// `setStaleIfError($value)`.
    pub fn set_stale_if_error(&mut self, value: i64) -> Result<&mut Self> {
        self.headers
            .add_cache_control_directive("stale-if-error", value)?;
        Ok(self)
    }

    /// `setStaleWhileRevalidate($value)`.
    pub fn set_stale_while_revalidate(&mut self, value: i64) -> Result<&mut Self> {
        self.headers
            .add_cache_control_directive("stale-while-revalidate", value)?;
        Ok(self)
    }

    /// `setSharedMaxAge($value)`: also public.
    pub fn set_shared_max_age(&mut self, value: i64) -> Result<&mut Self> {
        self.set_public()?;
        self.headers
            .add_cache_control_directive("s-maxage", value)?;
        Ok(self)
    }

    /// `getTtl()`: max age less age, at least 0; null without freshness
    /// information.
    pub fn get_ttl(&self) -> Result<Option<i64>> {
        match self.get_max_age()? {
            Some(max_age) => Ok(Some(max_age.saturating_sub(self.get_age()?).max(0))),
            None => Ok(None),
        }
    }

    /// `$this->getAge() + $seconds`; PHP's sum would become a float past
    /// the int range, which is refused.
    fn age_plus(&self, seconds: i64) -> Result<i64> {
        self.get_age()?
            .checked_add(seconds)
            .ok_or_else(|| not_ported("an age past the int range"))
    }

    /// `setTtl($seconds)`: s-maxage of age plus `seconds`.
    pub fn set_ttl(&mut self, seconds: i64) -> Result<&mut Self> {
        let value = self.age_plus(seconds)?;
        self.set_shared_max_age(value)
    }

    /// `setClientTtl($seconds)`: max-age of age plus `seconds`.
    pub fn set_client_ttl(&mut self, seconds: i64) -> Result<&mut Self> {
        let value = self.age_plus(seconds)?;
        self.set_max_age(value)
    }

    /// `getLastModified()`; `\RuntimeException` when it is not parseable.
    pub fn get_last_modified(&self) -> Result<Option<DateTimeImmutable>> {
        self.headers.get_date("Last-Modified", None)
    }

    /// `setLastModified($date)`; null removes the header.
    pub fn set_last_modified(&mut self, date: Option<&DateTimeImmutable>) -> Result<&mut Self> {
        match date {
            None => self.headers.remove("Last-Modified"),
            Some(date) => {
                self.headers
                    .set(b"Last-Modified", Self::http_date(date).as_str(), true)?;
            }
        }
        Ok(self)
    }

    /// `getEtag()`.
    pub fn get_etag(&self) -> Option<PhpString> {
        self.headers.get("ETag", None)
    }

    /// `setEtag($etag, $weak)`: quoted unless it starts with `"`; null
    /// removes the header.
    pub fn set_etag(&mut self, etag: Option<&[u8]>, weak: bool) -> Result<&mut Self> {
        match etag {
            None => self.headers.remove("Etag"),
            Some(etag) => {
                let quoted = if etag.starts_with(b"\"") {
                    etag.to_vec()
                } else {
                    [&b"\""[..], etag, b"\""].concat()
                };
                let prefix: &[u8] = if weak { b"W/" } else { b"" };
                self.headers
                    .set(b"ETag", concat(&[prefix, &quoted]), true)?;
            }
        }
        Ok(self)
    }

    /// `setCache($options)`: `must_revalidate`, `no_cache`, `no_store`,
    /// `no_transform`, public, private, `proxy_revalidate`, `max_age`,
    /// `s_maxage`, `stale_if_error`, `stale_while_revalidate`, immutable,
    /// `last_modified` and etag. Any other key is an
    /// `\InvalidArgumentException`; a value of the wrong type a
    /// `\TypeError`.
    pub fn set_cache(&mut self, options: &PhpArray) -> Result<&mut Self> {
        let diff: Vec<PhpString> = options
            .keys()
            .map(key_text)
            .filter(|key| {
                !CACHE_CONTROL_DIRECTIVES
                    .iter()
                    .any(|(name, _)| name.as_bytes() == key.as_bytes())
            })
            .collect();
        if !diff.is_empty() {
            let joined: Vec<&[u8]> = diff.iter().map(PhpString::as_bytes).collect();
            let mut message = b"Response does not support the following options: \"".to_vec();
            message.extend_from_slice(&joined.join(&b"\", \""[..]));
            message.extend_from_slice(b"\".");
            return Err(Error::new(Class::PhpInvalidArgument, message));
        }

        let option = |name: &str| -> Option<&Value> {
            options
                .get(&Key::from_bytes(name.as_bytes()))
                .filter(|value| !value.is_null())
        };

        if let Some(etag) = option("etag") {
            let etag = match etag {
                Value::Array(_) => None,
                Value::Object(object) => object.to_php_string(),
                scalar => scalar.to_php_string(),
            }
            .ok_or_else(|| type_error("setEtag", "etag", "?string", etag))?;
            self.set_etag(Some(etag.as_bytes()), false)?;
        }

        if let Some(date) = option("last_modified") {
            let parsed = date
                .as_object()
                .and_then(|object| object.downcast_ref::<DateTimeImmutable>())
                .copied()
                .ok_or_else(|| type_error("setLastModified", "date", "?DateTimeInterface", date))?;
            self.set_last_modified(Some(&parsed))?;
        }

        if let Some(value) = option("max_age") {
            let value = int_option("setMaxAge", value)?;
            self.set_max_age(value)?;
        }

        if let Some(value) = option("s_maxage") {
            let value = int_option("setSharedMaxAge", value)?;
            self.set_shared_max_age(value)?;
        }

        if let Some(value) = option("stale_while_revalidate") {
            let value = int_option("setStaleWhileRevalidate", value)?;
            self.set_stale_while_revalidate(value)?;
        }

        if let Some(value) = option("stale_if_error") {
            let value = int_option("setStaleIfError", value)?;
            self.set_stale_if_error(value)?;
        }

        for (directive, has_value) in CACHE_CONTROL_DIRECTIVES {
            if !has_value && let Some(value) = option(directive) {
                let name = directive.replace('_', "-");
                if value.is_truthy() {
                    self.headers.add_cache_control_directive(&name, true)?;
                } else {
                    self.headers.remove_cache_control_directive(&name)?;
                }
            }
        }

        if let Some(public) = option("public") {
            if public.is_truthy() {
                self.set_public()?;
            } else {
                self.set_private()?;
            }
        }

        if let Some(private) = option("private") {
            if private.is_truthy() {
                self.set_private()?;
            } else {
                self.set_public()?;
            }
        }

        Ok(self)
    }

    /// `setNotModified()`: status 304, no body, and none of the headers a
    /// 304 must not include.
    pub fn set_not_modified(&mut self) -> &mut Self {
        // 304 is a valid code.
        let _ = self.set_status_code(304, None);
        // Null content is always accepted.
        let _ = self.set_content(None);

        // remove headers that MUST NOT be included with 304 Not Modified responses
        for header in [
            "Allow",
            "Content-Encoding",
            "Content-Language",
            "Content-Length",
            "Content-MD5",
            "Content-Type",
            "Last-Modified",
        ] {
            self.headers.remove(header);
        }

        self
    }

    /// `hasVary()`.
    pub fn has_vary(&self) -> bool {
        self.headers.get("Vary", None).is_some()
    }

    /// `getVary()`: the header names in every Vary value.
    pub fn get_vary(&self) -> Vec<PhpString> {
        self.headers
            .all_of("Vary")
            .iter()
            .flat_map(|item| split_vary(item.as_ref().map_or(&b""[..], PhpString::as_bytes)))
            .collect()
    }

    /// `setVary($headers, $replace)`.
    pub fn set_vary(
        &mut self,
        headers: impl Into<HeaderValues>,
        replace: bool,
    ) -> Result<&mut Self> {
        self.headers.set(b"Vary", headers, replace)?;
        Ok(self)
    }

    /// `isNotModified($request)`: whether the response's validators match
    /// the request's conditions; if they do, the response becomes a 304
    /// (`setNotModified()`).
    pub fn is_not_modified(&mut self, request: &mut Request) -> Result<bool> {
        if !request.is_method_cacheable()? {
            return Ok(false);
        }

        let mut not_modified = false;
        let last_modified = self.headers.get("Last-Modified", None);
        let modified_since = request.headers().get("If-Modified-Since", None);
        let if_none_match = request.get_etags();
        let etag = self.get_etag();
        let truthy = |s: &Option<PhpString>| {
            s.as_ref()
                .is_some_and(|s| Value::String(s.clone()).is_truthy())
        };

        if let (false, Some(etag)) = (if_none_match.is_empty(), &etag) {
            let etag = etag.as_bytes();
            let etag = etag.strip_prefix(b"W/").unwrap_or(etag);

            // Use weak comparison as per https://tools.ietf.org/html/rfc7232#section-3.2.
            for candidate in &if_none_match {
                let candidate = candidate.as_bytes();
                let candidate = candidate.strip_prefix(b"W/").unwrap_or(candidate);
                if candidate == etag || candidate == b"*" {
                    not_modified = true;
                    break;
                }
            }
        }
        // Only do If-Modified-Since date comparison when If-None-Match is not present as per https://tools.ietf.org/html/rfc7232#section-3.3.
        else if truthy(&modified_since) && truthy(&last_modified) {
            let since = strtotime_now(modified_since.unwrap_or_default().as_bytes())?;
            let last = strtotime_now(last_modified.unwrap_or_default().as_bytes())?;
            not_modified = time_at_least(since, last);
        }

        if not_modified {
            self.set_not_modified();
        }

        Ok(not_modified)
    }

    /// `isInvalid()`.
    pub fn is_invalid(&self) -> bool {
        self.status_code < 100 || self.status_code >= 600
    }

    /// `isInformational()`.
    pub fn is_informational(&self) -> bool {
        (100..200).contains(&self.status_code)
    }

    /// `isSuccessful()`.
    pub fn is_successful(&self) -> bool {
        (200..300).contains(&self.status_code)
    }

    /// `isRedirection()`.
    pub fn is_redirection(&self) -> bool {
        (300..400).contains(&self.status_code)
    }

    /// `isClientError()`.
    pub fn is_client_error(&self) -> bool {
        (400..500).contains(&self.status_code)
    }

    /// `isServerError()`.
    pub fn is_server_error(&self) -> bool {
        (500..600).contains(&self.status_code)
    }

    /// `isOk()`.
    pub fn is_ok(&self) -> bool {
        self.status_code == 200
    }

    /// `isForbidden()`.
    pub fn is_forbidden(&self) -> bool {
        self.status_code == 403
    }

    /// `isNotFound()`.
    pub fn is_not_found(&self) -> bool {
        self.status_code == 404
    }

    /// `isRedirect($location)`: a redirect status, and the Location
    /// header loosely equal to `location` when one is given.
    pub fn is_redirect(&self, location: Option<&[u8]>) -> bool {
        [201, 301, 302, 303, 307, 308].contains(&self.status_code)
            && location.is_none_or(|location| {
                let header = self
                    .headers
                    .get("Location", None)
                    .map_or(Value::Null, Value::String);
                loose_equals(&Value::string(location), &header)
            })
    }

    /// `isEmpty()`.
    pub fn is_empty(&self) -> bool {
        [204, 304].contains(&self.status_code)
    }

    /// `closeOutputBuffers($targetLevel, $flush)`: flushes or cleans the
    /// output buffers down to `target_level`, stopping at one that cannot
    /// be removed.
    pub fn close_output_buffers(target_level: i64, flush: bool) {
        with_output(|o| {
            let status = o.buffers.clone();
            let mut level = i64::try_from(status.len()).unwrap_or(i64::MAX);
            let flags = PHP_OUTPUT_HANDLER_REMOVABLE
                | if flush {
                    PHP_OUTPUT_HANDLER_FLUSHABLE
                } else {
                    PHP_OUTPUT_HANDLER_CLEANABLE
                };

            loop {
                let current = level;
                level -= 1;
                if current <= target_level {
                    break;
                }
                let Some(s) = usize::try_from(level).ok().and_then(|at| status.get(at)) else {
                    break;
                };
                let removable = match s.del {
                    None => s.flags.is_none_or(|f| f & flags == flags),
                    Some(del) => del,
                };
                if !removable {
                    break;
                }
                o.buffers.pop();
                o.calls.push(if flush {
                    OutputCall::ObEndFlush
                } else {
                    OutputCall::ObEndClean
                });
            }
        });
    }

    /// `setContentSafe($safe)`: marks the response as safe (RFC 8674).
    pub fn set_content_safe(&mut self, safe: bool) -> Result<()> {
        if safe {
            self.headers.set(b"Preference-Applied", "safe", true)?;
        } else if self
            .headers
            .get("Preference-Applied", None)
            .is_some_and(|v| v.as_bytes() == b"safe")
        {
            self.headers.remove("Preference-Applied");
        }

        self.set_vary("Prefer", false)?;
        Ok(())
    }

    /// `ensureIEOverSSLCompatibility($request)`: no Cache-Control on an
    /// attachment sent over HTTPS to Internet Explorer before 9.
    fn ensure_ie_over_ssl_compatibility(&mut self, request: &mut Request) -> Result<()> {
        let disposition = self
            .headers
            .get("Content-Disposition", None)
            .unwrap_or_default();
        if stripos(disposition.as_bytes(), b"attachment").is_none() {
            return Ok(());
        }
        let agent = match request.server().get("HTTP_USER_AGENT", Value::Null) {
            Value::Null => PhpString::default(),
            other => other.to_php_string_or_stringable().ok_or_else(|| {
                Error::new(
                    Class::PhpTypeError,
                    format!(
                        "preg_match(): Argument #2 ($subject) must be of type string, {} given",
                        other.debug_type()
                    ),
                )
            })?,
        };
        let msie = Pcre::new(b"/MSIE (.*?);/i").map_err(not_ported)?;
        let Some(groups) = msie
            .captures_from(agent.as_bytes(), 0)
            .map_err(not_ported)?
        else {
            return Ok(());
        };
        let Some(Some((start, end))) = groups.first().copied() else {
            return Ok(());
        };
        if !request.is_secure()? {
            return Ok(());
        }
        let version = Pcre::new(b"/(MSIE )(.*?);/")
            .map_err(not_ported)?
            .replace(&agent.as_bytes()[start..end], b"$2")
            .map_err(not_ported)?;
        if string_to_int(&version) < 9 {
            self.headers.remove("Cache-Control");
        }
        Ok(())
    }
}

impl std::fmt::Display for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_php_string().to_string_lossy())
    }
}
