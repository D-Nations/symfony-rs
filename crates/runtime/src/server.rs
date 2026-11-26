//! The HTTP adapter: serves a ported Symfony kernel from Axum as nginx in
//! front of PHP-FPM serves a front controller `index.php` at the document
//! root, with Symfony's documented nginx location (`fastcgi_params`,
//! `SCRIPT_FILENAME` and `DOCUMENT_ROOT` from the document root, every
//! path answered by `/index.php`).
//!
//! The web server's part, written from nginx's documentation:
//!
//! - [`fastcgi_params`]: the parameters in `fastcgi_params`' order
//!   (`QUERY_STRING`, `REQUEST_METHOD`, `CONTENT_TYPE`, `CONTENT_LENGTH`,
//!   `SCRIPT_NAME`, `REQUEST_URI`, `DOCUMENT_URI`, `DOCUMENT_ROOT`,
//!   `SERVER_PROTOCOL`, `REQUEST_SCHEME`, `HTTPS` when on,
//!   `GATEWAY_INTERFACE`, `SERVER_SOFTWARE`, `REMOTE_ADDR`, `REMOTE_PORT`,
//!   `SERVER_ADDR`, `SERVER_PORT`, `SERVER_NAME`, `REDIRECT_STATUS`), then
//!   `SCRIPT_FILENAME`, then each request header as `HTTP_<NAME>` (a name
//!   with `_` is dropped, as nginx's `ignore_invalid_headers` does;
//!   `Proxy` is dropped, the "httpoxy" mitigation; repeated headers are
//!   joined with `, `, `Cookie` with `; `, as nginx 1.23 and later do). A
//!   body without `Content-Length` (chunked) is buffered and its length
//!   sent, as nginx does for `FastCGI`. A body larger than
//!   [`ServerConfig::client_max_body_size`] is answered 413, as nginx does.
//! - [`into_response`]: the CGI response as nginx passes it on: the
//!   `Status:` line gives the status (502 when it is not a code from 100
//!   to 999); other lines become headers, except the `X-Accel-*` ones
//!   nginx hides by default (`fastcgi_hide_header`). Of those, only
//!   `X-Accel-Buffering` is acted on; `X-Accel-Redirect`,
//!   `X-Accel-Expires`, `X-Accel-Limit-Rate` and `X-Accel-Charset` are
//!   hidden and logged as not applied.
//! - Delivery: nginx answers the client once PHP-FPM sent the headers
//!   (the first output, or `flush()`), and passes the body on as it
//!   comes: at each `flush()` with `X-Accel-Buffering: no`, otherwise
//!   (`fastcgi_buffering` on, nginx's default) in pieces of
//!   [`NGINX_BUFFER`] bytes and the rest at the end. A body without
//!   `Content-Length` goes to the client chunked. The bytes are PHP's;
//!   where nginx cuts the buffered pieces is approximated (PHP-FPM's own
//!   8 KiB write buffer between flushes is not modelled).
//! - The front controller's own path (`/index.php`, `/index.php/...`)
//!   and any other path ending in `.php` are answered 404 (empty) by the
//!   web server, as Symfony's location (`internal`, `location ~ \.php$ {
//!   return 404; }`) does. No static file is served (`try_files $uri`
//!   finds none). A line that is not a valid
//!   `Name: value` header for `http` is dropped and logged (nginx answers
//!   502 for some of them: not reproduced). A HEAD response has no body.
//!
//! The PHP part is `symfony_http_foundation::sapi` (recorded from
//! PHP-FPM). [`FrontController`] runs one PHP request per HTTP request on
//! a blocking thread of its own, as a PHP-FPM worker runs one at a time:
//! the superglobals are set (`php::set_globals`), the bootstrap runs,
//! `Request::createFromGlobals()` builds the request, a new kernel from
//! the factory handles it through [`HttpKernelRunner`], the output goes
//! to the client as it is flushed and ends at `fastcgi_finish_request()`
//! while `terminate()` runs, and then the request's state is cleared (superglobals, uploaded
//! files, `Request`'s statics). A throwable that escapes is PHP's uncaught
//! exception: logged, and a 500 if nothing was sent.
//!
//! Upstream: none

use std::{
    cell::RefCell,
    net::SocketAddr,
    pin::Pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    Router,
    body::{Body, Bytes, HttpBody, to_bytes},
    extract::{ConnectInfo, Request},
    http::{HeaderName, HeaderValue, StatusCode, Version, request::Parts},
    response::{IntoResponse, Response},
};
use php_value::{Handle, PhpString, clock::microtime};
use symfony_http_foundation::{
    php::{set_flush_hook, set_globals, take_globals, with_output},
    request::Request as PhpRequest,
    sapi::{self, CgiRequest, CgiResponse, Ini, Sapi},
};
use symfony_http_kernel::exception::Throwable;

use crate::http_kernel_runner::{HttpKernelRunner, RunnableKernel};

/// The web server's configuration: the `server` block and the parts of
/// the connection PHP sees.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// `server_name` (the first name): `SERVER_NAME`.
    pub server_name: PhpString,
    /// The address the server listens on: `SERVER_ADDR`, `SERVER_PORT`.
    pub server_addr: SocketAddr,
    /// Whether the server terminates TLS (`HTTPS=on`, scheme `https`).
    pub https: bool,
    /// `root`, resolved (`$realpath_root`): `DOCUMENT_ROOT`.
    pub document_root: PhpString,
    /// The front controller, `/index.php`: `SCRIPT_NAME`, `DOCUMENT_URI`,
    /// and with the document root `SCRIPT_FILENAME`.
    pub script_name: PhpString,
    /// `nginx/$nginx_version`: `SERVER_SOFTWARE`.
    pub server_software: PhpString,
    /// The client address when the connection gives none (a request not
    /// from a socket, as in tests).
    pub remote_addr: SocketAddr,
    /// The environment PHP-FPM passes on to `$_SERVER`, in order (none:
    /// PHP-FPM's default `clear_env`).
    pub environment: Vec<(PhpString, PhpString)>,
    /// `client_max_body_size`, in bytes (nginx's default, 1 MiB).
    pub client_max_body_size: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server_name: PhpString::from("localhost"),
            server_addr: SocketAddr::from(([127, 0, 0, 1], 80)),
            https: false,
            document_root: PhpString::from("/srv/app/public"),
            script_name: PhpString::from("/index.php"),
            server_software: PhpString::from("nginx"),
            remote_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            environment: Vec::new(),
            client_max_body_size: 1024 * 1024,
        }
    }
}

fn param(name: &str, value: impl Into<PhpString>) -> (PhpString, PhpString) {
    (PhpString::from(name), value.into())
}

/// The `FastCGI` parameters nginx sends for `parts` with a body of
/// `body_len` bytes, from the client at `remote`.
pub fn fastcgi_params(
    parts: &Parts,
    body_len: usize,
    config: &ServerConfig,
    remote: SocketAddr,
) -> Vec<(PhpString, PhpString)> {
    let header = |name: &str| {
        let values: Vec<&[u8]> = parts
            .headers
            .get_all(name)
            .iter()
            .map(HeaderValue::as_bytes)
            .collect();
        PhpString::from(values.join(&b", "[..]))
    };
    let mut content_length = header("content-length");
    if content_length.as_bytes().is_empty() && body_len > 0 {
        content_length = PhpString::from(body_len.to_string());
    }
    let request_uri = parts
        .uri
        .path_and_query()
        .map_or("/", axum::http::uri::PathAndQuery::as_str);
    let protocol = match parts.version {
        Version::HTTP_09 => "HTTP/0.9",
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_2 => "HTTP/2.0",
        Version::HTTP_3 => "HTTP/3.0",
        _ => "HTTP/1.1",
    };
    let mut script_filename = config.document_root.as_bytes().to_vec();
    script_filename.extend_from_slice(config.script_name.as_bytes());
    let mut params = vec![
        param("QUERY_STRING", parts.uri.query().unwrap_or_default()),
        param("REQUEST_METHOD", parts.method.as_str()),
        param("CONTENT_TYPE", header("content-type")),
        param("CONTENT_LENGTH", content_length),
        param("SCRIPT_NAME", config.script_name.clone()),
        param("REQUEST_URI", request_uri),
        param("DOCUMENT_URI", config.script_name.clone()),
        param("DOCUMENT_ROOT", config.document_root.clone()),
        param("SERVER_PROTOCOL", protocol),
        param(
            "REQUEST_SCHEME",
            if config.https { "https" } else { "http" },
        ),
    ];
    if config.https {
        params.push(param("HTTPS", "on"));
    }
    params.extend([
        param("GATEWAY_INTERFACE", "CGI/1.1"),
        param("SERVER_SOFTWARE", config.server_software.clone()),
        param("REMOTE_ADDR", remote.ip().to_string()),
        param("REMOTE_PORT", remote.port().to_string()),
        param("SERVER_ADDR", config.server_addr.ip().to_string()),
        param("SERVER_PORT", config.server_addr.port().to_string()),
        param("SERVER_NAME", config.server_name.clone()),
        param("REDIRECT_STATUS", "200"),
        param("SCRIPT_FILENAME", script_filename),
    ]);
    let mut seen: Vec<&HeaderName> = Vec::new();
    for name in parts.headers.keys() {
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let text = name.as_str();
        if text.contains('_') || text == "proxy" {
            continue;
        }
        let separator: &[u8] = if text == "cookie" { b"; " } else { b", " };
        let values: Vec<&[u8]> = parts
            .headers
            .get_all(name)
            .iter()
            .map(HeaderValue::as_bytes)
            .collect();
        let mut php_name = b"HTTP_".to_vec();
        php_name.extend(text.bytes().map(|b| {
            if b == b'-' {
                b'_'
            } else {
                b.to_ascii_uppercase()
            }
        }));
        params.push((
            PhpString::from(php_name),
            PhpString::from(values.join(separator)),
        ));
    }
    params
}

/// The headers nginx does not pass from PHP-FPM to the client
/// (`fastcgi_hide_header`'s default, besides `Status`).
const HIDDEN: [&str; 5] = [
    "X-Accel-Expires",
    "X-Accel-Redirect",
    "X-Accel-Limit-Rate",
    "X-Accel-Buffering",
    "X-Accel-Charset",
];

/// nginx's `fastcgi_buffer_size` and each of its `fastcgi_buffers` by
/// default: one memory page (4 KiB on Linux x86-64). With buffering on, a
/// piece of the body is passed on once this much is waiting.
pub const NGINX_BUFFER: usize = 4096;

/// `X-Accel-Buffering: no` (nginx compares the value without case).
fn unbuffered(cgi: &CgiResponse) -> bool {
    cgi.headers.iter().any(|line| {
        let line = line.as_bytes();
        let name = line_name(line);
        name.eq_ignore_ascii_case(b"x-accel-buffering")
            && line[name.len()..]
                .strip_prefix(b":")
                .map(<[u8]>::trim_ascii)
                .is_some_and(|value| value.eq_ignore_ascii_case(b"no"))
    })
}

/// A header line's name: the bytes before `:`, without trailing blanks.
fn line_name(line: &[u8]) -> &[u8] {
    let end = line.iter().position(|&b| b == b':').unwrap_or(line.len());
    line[..end].trim_ascii_end()
}

/// The HTTP response nginx makes of PHP-FPM's CGI response.
pub fn into_response(cgi: &CgiResponse, head: bool) -> Response {
    let mut status = StatusCode::OK;
    let mut response = Response::new(Body::empty());
    for line in &cgi.headers {
        let line = line.as_bytes();
        let Some(colon) = line.iter().position(|&b| b == b':') else {
            eprintln!(
                "symfony-runtime: dropped a header line without a colon: {:?}",
                String::from_utf8_lossy(line)
            );
            continue;
        };
        let (name, value) = (&line[..colon], &line[colon + 1..]);
        let start = value
            .iter()
            .position(|b| !matches!(b, b' ' | b'\t'))
            .unwrap_or(value.len());
        let value = &value[start..];
        if name.eq_ignore_ascii_case(b"status") {
            let code = value
                .get(..3)
                .and_then(|digits| std::str::from_utf8(digits).ok())
                .and_then(|digits| digits.parse::<u16>().ok())
                .filter(|code| (100..=999).contains(code))
                .and_then(|code| StatusCode::from_u16(code).ok());
            status = code.unwrap_or(StatusCode::BAD_GATEWAY);
            continue;
        }
        if let Some(hidden) = HIDDEN
            .iter()
            .find(|h| name.eq_ignore_ascii_case(h.as_bytes()))
        {
            if *hidden != "X-Accel-Buffering" {
                eprintln!("symfony-runtime: {hidden} is hidden from the client but not applied");
            }
            continue;
        }
        match (HeaderName::from_bytes(name), HeaderValue::from_bytes(value)) {
            (Ok(name), Ok(value)) => {
                response.headers_mut().append(name, value);
            }
            _ => eprintln!(
                "symfony-runtime: dropped an invalid header line: {:?}",
                String::from_utf8_lossy(line)
            ),
        }
    }
    *response.status_mut() = status;
    if !head {
        *response.body_mut() = Body::from(cgi.body.clone());
    }
    response
}

/// Builds the kernel for one request, as a front controller does.
pub type KernelFactory = Arc<dyn Fn() -> Result<Arc<dyn RunnableKernel>, Throwable> + Send + Sync>;

/// What runs before `createFromGlobals()` (trusted proxies and hosts,
/// method override).
pub type Bootstrap = Arc<dyn Fn() + Send + Sync>;

/// A front controller `index.php` served by nginx and PHP-FPM.
pub struct FrontController {
    server: ServerConfig,
    ini: Ini,
    kernel: KernelFactory,
    bootstrap: Option<Bootstrap>,
    debug: bool,
}

impl FrontController {
    pub fn new(server: ServerConfig, ini: Ini, kernel: KernelFactory) -> Self {
        Self {
            server,
            ini,
            kernel,
            bootstrap: None,
            debug: false,
        }
    }

    #[must_use]
    pub fn with_bootstrap(mut self, bootstrap: Bootstrap) -> Self {
        self.bootstrap = Some(bootstrap);
        self
    }

    /// `HttpKernelRunner`'s `$debug`.
    #[must_use]
    pub fn with_debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    /// An Axum router that answers every request with this front
    /// controller.
    pub fn router(self: Arc<Self>) -> Router {
        Router::new().fallback(move |request: Request| {
            let front = self.clone();
            async move { front.handle(request).await }
        })
    }

    /// Serves one HTTP request.
    pub async fn handle(self: Arc<Self>, request: Request) -> Response {
        let remote = request
            .extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map_or(self.server.remote_addr, |info| info.0);
        let (parts, body) = request.into_parts();
        if self.denies(parts.uri.path()) {
            return StatusCode::NOT_FOUND.into_response();
        }
        let Ok(body) = to_bytes(body, self.server.client_max_body_size).await else {
            return StatusCode::PAYLOAD_TOO_LARGE.into_response();
        };
        let head = parts.method == axum::http::Method::HEAD;
        let cgi = CgiRequest {
            environment: self.server.environment.clone(),
            params: fastcgi_params(&parts, body.len(), &self.server, remote),
            stdin: body.to_vec(),
            time: microtime(),
        };
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let front = self.clone();
        let worker = tokio::task::spawn_blocking(move || front.run(&cgi, head, sender));
        if let Ok(start) = receiver.await {
            // terminate() may still run: the worker finishes on its own.
            drop(worker);
            let mut response = into_response(&start.response, head);
            if let (Some(rest), false) = (start.rest, head) {
                *response.body_mut() = Body::new(ChannelBody(rest));
            }
            response
        } else {
            eprintln!("symfony-runtime: the PHP request ended without a response");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }

    /// Symfony's nginx location: the front controller is `internal` and
    /// any other `.php` path is answered 404 by the web server.
    fn denies(&self, path: &str) -> bool {
        let script = self.server.script_name.as_bytes();
        let path = path.as_bytes();
        path.ends_with(b".php")
            || path
                .strip_prefix(script)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(b"/"))
    }

    /// One PHP request on this (blocking) thread.
    fn run(&self, cgi: &CgiRequest, head: bool, sender: tokio::sync::oneshot::Sender<Start>) {
        let startup = match sapi::start(cgi, &self.ini) {
            Ok(startup) => startup,
            Err(error) => {
                eprintln!("symfony-runtime: cannot write an uploaded file: {error}");
                let mut sapi = Sapi::new(&self.ini, head);
                sapi.fatal_error();
                let _ = sender.send(Start {
                    response: sapi.end(),
                    rest: None,
                });
                return;
            }
        };
        for warning in &startup.warnings {
            eprintln!("symfony-runtime: PHP Warning: {warning}");
        }
        set_globals(startup.globals);
        let delivery = Rc::new(RefCell::new(Delivery::new(&self.ini, head, sender)));
        let on_finish = delivery.clone();
        crate::set_finish_hook(Some(Box::new(move || {
            on_finish.borrow_mut().advance(Point::Finish);
        })));
        let on_flush = delivery.clone();
        set_flush_hook(Some(Box::new(move || {
            on_flush.borrow_mut().advance(Point::Flush);
        })));

        let outcome = self.run_script();
        crate::set_finish_hook(None);
        set_flush_hook(None);
        let failed = outcome.is_err();
        if let Err(error) = outcome {
            eprintln!("symfony-runtime: PHP Fatal error: Uncaught {error}");
        }
        delivery.borrow_mut().advance(Point::End { fatal: failed });

        sapi::finish(&startup.uploaded);
        take_globals();
        PhpRequest::reset_statics();
    }

    fn run_script(&self) -> Result<i64, Throwable> {
        if let Some(bootstrap) = &self.bootstrap {
            bootstrap();
        }
        let request = PhpRequest::create_from_globals().map_err(|e| Arc::new(e) as Throwable)?;
        let kernel = (self.kernel)()?;
        HttpKernelRunner::new(kernel, Handle::new(request), self.debug).run()
    }
}

/// What the web server answers with first: the head (and, when the
/// script ended before, the whole body), then the rest as it comes.
struct Start {
    response: CgiResponse,
    rest: Option<tokio::sync::mpsc::Receiver<Bytes>>,
}

/// Where the script is when the output is looked at.
#[derive(Clone, Copy)]
enum Point {
    /// `flush()`.
    Flush,
    /// `fastcgi_finish_request()`.
    Finish,
    /// The end of the script; `fatal` for an uncaught throwable.
    End { fatal: bool },
}

/// The SAPI and the web server for one request: the calls the script made
/// are applied as they come, and what PHP-FPM sent is passed on as nginx
/// passes it.
struct Delivery {
    sapi: Sapi,
    head: bool,
    start: Option<tokio::sync::oneshot::Sender<Start>>,
    body: Option<tokio::sync::mpsc::Sender<Bytes>>,
    buffering: bool,
    pending: Vec<u8>,
    warned: usize,
}

impl Delivery {
    fn new(ini: &Ini, head: bool, start: tokio::sync::oneshot::Sender<Start>) -> Self {
        Self {
            sapi: Sapi::new(ini, head),
            head,
            start: Some(start),
            body: None,
            buffering: true,
            pending: Vec::new(),
            warned: 0,
        }
    }

    fn advance(&mut self, point: Point) {
        let (calls, mimetype) =
            with_output(|o| (std::mem::take(&mut o.calls), o.default_mimetype.clone()));
        self.sapi.set_default_mimetype(mimetype);
        for call in &calls {
            self.sapi.apply(call);
        }
        if matches!(point, Point::End { fatal: true }) {
            self.sapi.fatal_error();
        }
        let ending = !matches!(point, Point::Flush) || self.sapi.is_finished();
        for warning in &self.sapi.warnings()[self.warned..] {
            eprintln!("symfony-runtime: PHP Warning: {warning}");
        }
        self.warned = self.sapi.warnings().len();

        if let Some(start) = self.start.take() {
            if ending {
                let _ = start.send(Start {
                    response: self.sapi.end(),
                    rest: None,
                });
                return;
            }
            if !self.sapi.headers_sent() {
                self.start = Some(start);
                return;
            }
            let mut response = self.sapi.response();
            self.sapi.take_body();
            self.pending = std::mem::take(&mut response.body);
            self.buffering = !unbuffered(&response);
            let (sender, receiver) = tokio::sync::mpsc::channel(16);
            if start
                .send(Start {
                    response,
                    rest: Some(receiver),
                })
                .is_ok()
            {
                self.body = Some(sender);
            }
        }
        let Some(body) = &self.body else {
            return;
        };
        if ending {
            self.sapi.end();
        }
        let written = self.sapi.take_body();
        if !self.head {
            self.pending.extend_from_slice(&written);
        }
        let pass = ending || !self.buffering || self.pending.len() >= NGINX_BUFFER;
        if pass && !self.pending.is_empty() {
            let piece = Bytes::from(std::mem::take(&mut self.pending));
            // The client may be gone; PHP-FPM is not told either.
            if body.blocking_send(piece).is_err() {
                self.body = None;
            }
        }
        if ending {
            self.body = None;
        }
    }
}

/// The rest of a body, as the PHP thread passes it on.
struct ChannelBody(tokio::sync::mpsc::Receiver<Bytes>);

impl HttpBody for ChannelBody {
    type Data = Bytes;
    type Error = std::convert::Infallible;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<http_body::Frame<Bytes>, Self::Error>>> {
        self.0
            .poll_recv(cx)
            .map(|piece| piece.map(|bytes| Ok(http_body::Frame::data(bytes))))
    }
}
