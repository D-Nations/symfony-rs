//! The HTTP adapter and `HttpKernelRunner`: the parameters nginx sends,
//! the response it makes of PHP-FPM's, and whole requests through
//! `FrontController` with a small kernel (the response sent before
//! `terminate()` ends, uploaded files removed, an uncaught throwable, the
//! paths the web server refuses).
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::{
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, channel},
    },
    time::Duration,
};

use axum::{
    body::{Body, HttpBody, to_bytes},
    http::{Request, StatusCode},
};
use php_value::{Handle, PhpString};
use symfony_http_foundation::{
    event_stream_response::{self, EventCallback, Events},
    header_bag::ResponseHeaderBag,
    php::{FinishRequest, OutputCall, set_globals, take_globals, with_output},
    request::Request as PhpRequest,
    request_stack::RequestHandle,
    response::Response,
    sapi::{CgiResponse, Ini},
    server_event::ServerEvent,
    streamed_response::{self, CallbackOrChunks},
};
use symfony_http_kernel::{
    controller::ResponseHandle,
    exception::{PhpClass, PhpException, Throwable},
    http_kernel::{HttpKernelInterface, TerminableInterface},
};
use symfony_runtime::{
    http_kernel_runner::{HttpKernelRunner, RunnableKernel},
    server::{FrontController, NGINX_BUFFER, ServerConfig, fastcgi_params, into_response},
};
use tower::ServiceExt;

fn texts(params: &[(PhpString, PhpString)]) -> Vec<(String, String)> {
    params
        .iter()
        .map(|(n, v)| {
            (
                n.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

#[test]
fn sends_nginx_fastcgi_params() {
    let (parts, ()) = Request::builder()
        .method("POST")
        .uri("/inspect?a=1&b")
        .header("Host", "lab.example")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Cookie", "a=1")
        .header("Cookie", "b=2")
        .header("Accept", "text/html")
        .header("Accept", "*/*")
        .header("X_Underscore", "dropped")
        .header("Proxy", "http://evil")
        .body(())
        .unwrap()
        .into_parts();
    let config = ServerConfig {
        server_name: PhpString::from("lab.example"),
        ..ServerConfig::default()
    };
    let params = fastcgi_params(
        &parts,
        3,
        &config,
        SocketAddr::from(([192, 0, 2, 10], 50432)),
    );
    let expected: Vec<(String, String)> = [
        ("QUERY_STRING", "a=1&b"),
        ("REQUEST_METHOD", "POST"),
        ("CONTENT_TYPE", "application/x-www-form-urlencoded"),
        ("CONTENT_LENGTH", "3"),
        ("SCRIPT_NAME", "/index.php"),
        ("REQUEST_URI", "/inspect?a=1&b"),
        ("DOCUMENT_URI", "/index.php"),
        ("DOCUMENT_ROOT", "/srv/app/public"),
        ("SERVER_PROTOCOL", "HTTP/1.1"),
        ("REQUEST_SCHEME", "http"),
        ("GATEWAY_INTERFACE", "CGI/1.1"),
        ("SERVER_SOFTWARE", "nginx"),
        ("REMOTE_ADDR", "192.0.2.10"),
        ("REMOTE_PORT", "50432"),
        ("SERVER_ADDR", "127.0.0.1"),
        ("SERVER_PORT", "80"),
        ("SERVER_NAME", "lab.example"),
        ("REDIRECT_STATUS", "200"),
        ("SCRIPT_FILENAME", "/srv/app/public/index.php"),
        ("HTTP_HOST", "lab.example"),
        ("HTTP_CONTENT_TYPE", "application/x-www-form-urlencoded"),
        ("HTTP_COOKIE", "a=1; b=2"),
        ("HTTP_ACCEPT", "text/html, */*"),
    ]
    .iter()
    .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
    .collect();
    assert_eq!(expected, texts(&params));

    let https = ServerConfig {
        https: true,
        server_addr: SocketAddr::from(([127, 0, 0, 1], 443)),
        ..ServerConfig::default()
    };
    let (parts, ()) = Request::builder().uri("/").body(()).unwrap().into_parts();
    let params = texts(&fastcgi_params(&parts, 0, &https, https.remote_addr));
    assert!(params.contains(&("HTTPS".to_owned(), "on".to_owned())));
    assert!(params.contains(&("REQUEST_SCHEME".to_owned(), "https".to_owned())));
    assert!(params.contains(&("CONTENT_LENGTH".to_owned(), String::new())));
}

#[tokio::test]
async fn passes_php_fpm_responses_as_nginx_does() {
    let lines = |lines: &[&str]| CgiResponse {
        headers: lines.iter().map(|l| PhpString::from(*l)).collect(),
        body: b"body".to_vec(),
    };
    let response = into_response(
        &lines(&[
            "Status: 404 Not Found",
            "X-A: 1",
            "x-a: 2",
            "NoColon",
            "Bad Name: v",
            "Content-type: text/html; charset=UTF-8",
        ]),
        false,
    );
    assert_eq!(StatusCode::NOT_FOUND, response.status());
    let values: Vec<_> = response.headers().get_all("x-a").iter().collect();
    assert_eq!(values, ["1", "2"]);
    assert!(response.headers().get("nocolon").is_none());
    assert_eq!(
        response.headers()["content-type"],
        "text/html; charset=UTF-8"
    );
    assert_eq!(
        &to_bytes(response.into_body(), 64).await.unwrap()[..],
        b"body"
    );

    for (status, expected) in [
        ("Status: 299", 299),
        ("Status: 99 Low", 502),
        ("Status: abc", 502),
        ("Status: 4040 Long", 404),
    ] {
        let response = into_response(&lines(&[status]), true);
        assert_eq!(expected, response.status().as_u16(), "{status}");
        assert!(to_bytes(response.into_body(), 64).await.unwrap().is_empty());
    }
}

/// A kernel that answers with a fixed response and reports `terminate()`.
struct FakeKernel {
    content: &'static str,
    fail: bool,
    terminated: Mutex<std::sync::mpsc::Sender<&'static str>>,
}

impl HttpKernelInterface for FakeKernel {
    fn handle(
        &self,
        request: &RequestHandle,
        _: i64,
        _: bool,
    ) -> Result<ResponseHandle, Throwable> {
        if self.fail {
            return Err(PhpException::new(PhpClass::LogicException, "broken").throwable());
        }
        let uri = request.with_mut(PhpRequest::get_request_uri).unwrap();
        let headers = ResponseHeaderBag::new([("X-Uri", uri)]).unwrap();
        Ok(Handle::new(
            Response::new(Some(self.content), 201, headers).unwrap(),
        ))
    }
}

impl TerminableInterface for FakeKernel {
    fn terminate(&self, _: &RequestHandle, _: &ResponseHandle) -> Result<(), Throwable> {
        // terminate() runs after the client has the response.
        std::thread::sleep(Duration::from_millis(50));
        let _ = self.terminated.lock().unwrap().send("terminated");
        Ok(())
    }
}

impl RunnableKernel for FakeKernel {
    fn as_terminable(&self) -> Option<&dyn TerminableInterface> {
        Some(self)
    }
}

fn front(fail: bool) -> (axum::Router, Receiver<&'static str>) {
    let (sender, receiver) = channel();
    let sender = Arc::new(Mutex::new(sender));
    let kernel = Arc::new(move || {
        let kernel: Arc<dyn RunnableKernel> = Arc::new(FakeKernel {
            content: "made",
            fail,
            terminated: Mutex::new(sender.lock().unwrap().clone()),
        });
        Ok(kernel)
    });
    let front = FrontController::new(ServerConfig::default(), Ini::default(), kernel);
    (Arc::new(front).router(), receiver)
}

#[tokio::test(flavor = "multi_thread")]
async fn serves_the_kernel_and_terminates_after_the_response() {
    let (router, terminated) = front(false);
    let response = router
        .oneshot(
            Request::builder()
                .uri("/x?y=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(StatusCode::CREATED, response.status());
    assert_eq!(response.headers()["x-uri"], "/x?y=1");
    assert_eq!(response.headers()["cache-control"], "no-cache, private");
    // The response came back while terminate() was still running.
    assert!(terminated.try_recv().is_err());
    assert_eq!(
        &to_bytes(response.into_body(), 64).await.unwrap()[..],
        b"made"
    );
    assert_eq!(
        Ok("terminated"),
        terminated.recv_timeout(Duration::from_secs(5))
    );
}

/// A kernel that answers with a `StreamedResponse` of chunks.
struct StreamingKernel;

impl HttpKernelInterface for StreamingKernel {
    fn handle(&self, _: &RequestHandle, _: i64, _: bool) -> Result<ResponseHandle, Throwable> {
        let chunks = ["first ", "second ", "third"].map(PhpString::from).to_vec();
        let response = streamed_response::new(
            CallbackOrChunks::Chunks(chunks),
            200,
            ResponseHeaderBag::new([("Content-Type", "text/plain")]).unwrap(),
        )
        .unwrap();
        Ok(Handle::new(response))
    }
}

impl RunnableKernel for StreamingKernel {}

#[tokio::test(flavor = "multi_thread")]
async fn a_streamed_response_is_sent_by_its_callback() {
    let kernel = Arc::new(|| {
        let kernel: Arc<dyn RunnableKernel> = Arc::new(StreamingKernel);
        Ok(kernel)
    });
    let front = FrontController::new(ServerConfig::default(), Ini::default(), kernel);
    let response = Arc::new(front)
        .router()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(StatusCode::OK, response.status());
    // PHP-FPM adds `default_charset` to a text type set by `header()`.
    assert_eq!(
        response.headers()["content-type"],
        "text/plain;charset=UTF-8"
    );
    assert_eq!(
        &to_bytes(response.into_body(), 64).await.unwrap()[..],
        b"first second third"
    );
}

/// A kernel whose streamed response writes `first`, flushes, waits for
/// the test, then writes `rest`; with the headers it is given.
struct PausingKernel {
    first: Vec<u8>,
    rest: Vec<u8>,
    headers: Vec<(&'static str, &'static str)>,
    go: Arc<Mutex<Receiver<()>>>,
}

impl HttpKernelInterface for PausingKernel {
    fn handle(&self, _: &RequestHandle, _: i64, _: bool) -> Result<ResponseHandle, Throwable> {
        let (first, rest, go) = (self.first.clone(), self.rest.clone(), self.go.clone());
        let callback = Arc::new(move || {
            streamed_response::echo(PhpString::from(first.clone()));
            streamed_response::flush();
            // A closed channel (the test gave up) ends the wait too.
            let _ = go.lock().unwrap().recv_timeout(Duration::from_secs(5));
            streamed_response::echo(PhpString::from(rest.clone()));
            Ok(())
        });
        let headers = ResponseHeaderBag::new(self.headers.iter().copied()).unwrap();
        let response =
            streamed_response::new(CallbackOrChunks::Callback(callback), 200, headers).unwrap();
        Ok(Handle::new(response))
    }
}

impl RunnableKernel for PausingKernel {}

/// Serves a `PausingKernel`; the sender lets its script go on.
async fn pausing(
    first: &[u8],
    rest: &[u8],
    headers: Vec<(&'static str, &'static str)>,
) -> (axum::response::Response, std::sync::mpsc::Sender<()>) {
    let (go, wait) = channel();
    let wait = Arc::new(Mutex::new(wait));
    let (first, rest) = (first.to_vec(), rest.to_vec());
    let kernel = Arc::new(move || {
        let kernel: Arc<dyn RunnableKernel> = Arc::new(PausingKernel {
            first: first.clone(),
            rest: rest.clone(),
            headers: headers.clone(),
            go: wait.clone(),
        });
        Ok(kernel)
    });
    let front = FrontController::new(ServerConfig::default(), Ini::default(), kernel);
    let response = Arc::new(front)
        .router()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    (response, go)
}

/// The next piece of a body, or `None` when it has ended; `Err` when none
/// came within the wait.
async fn next_piece(body: &mut Body, wait: Duration) -> Result<Option<Vec<u8>>, ()> {
    let frame = std::future::poll_fn(|cx| std::pin::Pin::new(&mut *body).poll_frame(cx));
    match tokio::time::timeout(wait, frame).await {
        Err(_) => Err(()),
        Ok(None) => Ok(None),
        Ok(Some(frame)) => Ok(Some(
            frame
                .unwrap()
                .into_data()
                .map(|d| d.to_vec())
                .unwrap_or_default(),
        )),
    }
}

/// Every remaining byte of a body.
async fn rest_of(body: &mut Body) -> Vec<u8> {
    let mut all = Vec::new();
    while let Ok(Some(piece)) = next_piece(body, Duration::from_secs(5)).await {
        all.extend_from_slice(&piece);
    }
    all
}

#[tokio::test(flavor = "multi_thread")]
async fn x_accel_buffering_no_passes_each_flush_on() {
    let (response, go) = pausing(b"first", b" rest", vec![("X-Accel-Buffering", "no")]).await;
    assert_eq!(StatusCode::OK, response.status());
    // nginx hides the header from the client, and sends no length.
    assert!(response.headers().get("x-accel-buffering").is_none());
    assert!(response.headers().get("content-length").is_none());
    let mut body = response.into_body();
    // The flushed part arrives while the script still waits.
    assert_eq!(
        Ok(Some(b"first".to_vec())),
        next_piece(&mut body, Duration::from_secs(5)).await
    );
    go.send(()).unwrap();
    assert_eq!(b" rest".to_vec(), rest_of(&mut body).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn buffering_holds_a_small_flush_until_the_end() {
    let (response, go) = pausing(b"first", b" rest", Vec::new()).await;
    // The head comes at the first flush; the body waits in nginx's buffer.
    assert_eq!(StatusCode::OK, response.status());
    let mut body = response.into_body();
    assert_eq!(
        Err(()),
        next_piece(&mut body, Duration::from_millis(300)).await
    );
    go.send(()).unwrap();
    assert_eq!(b"first rest".to_vec(), rest_of(&mut body).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn buffering_passes_a_full_buffer_on() {
    let first = vec![b'a'; NGINX_BUFFER];
    let (response, go) = pausing(&first, b"end", Vec::new()).await;
    let mut body = response.into_body();
    assert_eq!(
        Ok(Some(first.clone())),
        next_piece(&mut body, Duration::from_secs(5)).await
    );
    go.send(()).unwrap();
    assert_eq!(b"end".to_vec(), rest_of(&mut body).await);
}

#[tokio::test]
async fn nginx_hides_the_x_accel_headers() {
    let response = into_response(
        &CgiResponse {
            headers: [
                "X-Accel-Redirect: /protected/file",
                "x-accel-expires: 10",
                "X-Accel-Limit-Rate: 100",
                "X-Accel-Buffering: no",
                "X-Accel-Charset: utf-8",
                "X-Accel-Other: kept",
                "Content-Type: text/plain",
            ]
            .map(PhpString::from)
            .to_vec(),
            body: b"x".to_vec(),
        },
        false,
    );
    let names: Vec<&str> = response
        .headers()
        .keys()
        .map(axum::http::HeaderName::as_str)
        .collect();
    assert_eq!(vec!["x-accel-other", "content-type"], names);
}

/// A kernel answering with an `EventStreamResponse` whose generator
/// waits for the test before each event after the first, and counts the
/// events it yields.
struct EventKernel {
    go: Arc<Mutex<Receiver<()>>>,
    yielded: Arc<std::sync::atomic::AtomicUsize>,
}

impl HttpKernelInterface for EventKernel {
    fn handle(&self, _: &RequestHandle, _: i64, _: bool) -> Result<ResponseHandle, Throwable> {
        let (go, yielded) = (self.go.clone(), self.yielded.clone());
        let callback: EventCallback = Arc::new(move |_| {
            let (go, yielded) = (go.clone(), yielded.clone());
            let events: Events = Box::new((0..5).map(move |i| {
                if i > 0 {
                    let _ = go.lock().unwrap().recv_timeout(Duration::from_secs(5));
                }
                yielded.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(ServerEvent::data(format!("event {i}").as_str()))
            }));
            Ok(Some(events))
        });
        let response =
            event_stream_response::new(Some(callback), 200, [("", ""); 0], None).unwrap();
        Ok(Handle::new(response))
    }
}

impl RunnableKernel for EventKernel {}

async fn events() -> (
    axum::response::Response,
    std::sync::mpsc::Sender<()>,
    Arc<std::sync::atomic::AtomicUsize>,
) {
    let (go, wait) = channel();
    let wait = Arc::new(Mutex::new(wait));
    let yielded = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = yielded.clone();
    let kernel = Arc::new(move || {
        let kernel: Arc<dyn RunnableKernel> = Arc::new(EventKernel {
            go: wait.clone(),
            yielded: counter.clone(),
        });
        Ok(kernel)
    });
    let front = FrontController::new(ServerConfig::default(), Ini::default(), kernel);
    let response = Arc::new(front)
        .router()
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    (response, go, yielded)
}

#[tokio::test(flavor = "multi_thread")]
async fn server_events_reach_the_client_as_they_are_sent() {
    let (response, go, _) = events().await;
    // PHP-FPM adds `default_charset` to a text type set by `header()`.
    assert_eq!(
        response.headers()["content-type"],
        "text/event-stream;charset=UTF-8"
    );
    assert!(response.headers().get("x-accel-buffering").is_none());
    let mut body = response.into_body();
    let mut seen = Vec::new();
    // Each event is passed on before the generator is let go on.
    for i in 0..5 {
        while !String::from_utf8_lossy(&seen).contains(&format!("data: event {i}\n\n")) {
            let piece = next_piece(&mut body, Duration::from_secs(5)).await;
            seen.extend(piece.unwrap().unwrap());
        }
        let _ = go.send(());
    }
    assert_eq!(
        "data: event 0\n\ndata: event 1\n\ndata: event 2\n\ndata: event 3\n\ndata: event 4\n\n",
        String::from_utf8_lossy(&seen)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_client_gone_ends_the_event_loop() {
    let (response, go, yielded) = events().await;
    let mut body = response.into_body();
    let first = next_piece(&mut body, Duration::from_secs(5)).await;
    assert!(first.unwrap().is_some());
    drop(body);
    // The next event's flush finds the client gone; connection_aborted()
    // then ends the loop before the event after it.
    for _ in 0..4 {
        let _ = go.send(());
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    let count = yielded.load(std::sync::atomic::Ordering::SeqCst);
    assert!(count < 5, "{count} events yielded");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_uncaught_throwable_is_a_500() {
    let (router, _) = front(true);
    let response = router
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(StatusCode::INTERNAL_SERVER_ERROR, response.status());
    assert!(to_bytes(response.into_body(), 64).await.unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_web_server_refuses_php_paths_and_large_bodies() {
    for path in ["/index.php", "/index.php/x", "/other.php"] {
        let (router, _) = front(false);
        let response = router
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(StatusCode::NOT_FOUND, response.status(), "{path}");
    }
    let (router, _) = front(false);
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .body(Body::from(vec![b'x'; 1024 * 1024 + 1]))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(StatusCode::PAYLOAD_TOO_LARGE, response.status());
}

/// A kernel whose controller sees the uploaded file.
struct UploadKernel(Arc<Mutex<Vec<String>>>);

impl HttpKernelInterface for UploadKernel {
    fn handle(
        &self,
        request: &RequestHandle,
        _: i64,
        _: bool,
    ) -> Result<ResponseHandle, Throwable> {
        let files = request.with(|r| r.files().all().clone());
        for (_, value) in files.iter() {
            if let Some(file) = symfony_http_foundation::file_bag::uploaded_file(value) {
                let path = String::from_utf8_lossy(file.file().get_pathname()).into_owned();
                assert!(std::path::Path::new(&path).exists());
                self.0.lock().unwrap().push(path);
            }
        }
        Ok(Handle::new(Response::default()))
    }
}

impl RunnableKernel for UploadKernel {}

#[tokio::test(flavor = "multi_thread")]
async fn uploaded_files_are_removed_after_the_request() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let kernel_seen = seen.clone();
    let kernel = Arc::new(move || {
        let kernel: Arc<dyn RunnableKernel> = Arc::new(UploadKernel(kernel_seen.clone()));
        Ok(kernel)
    });
    let router = Arc::new(FrontController::new(
        ServerConfig::default(),
        Ini::default(),
        kernel,
    ))
    .router();
    let body = "--b\r\nContent-Disposition: form-data; name=\"f\"; filename=\"a.txt\"\r\n\r\ndata\r\n--b--\r\n";
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("Content-Type", "multipart/form-data; boundary=b")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(StatusCode::OK, response.status());
    let path = seen
        .lock()
        .unwrap()
        .pop()
        .expect("the controller saw the file");
    let mut gone = false;
    for _ in 0..100 {
        if !std::path::Path::new(&path).exists() {
            gone = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(gone, "{path} was not removed");
}

/// A kernel that only records whether it was terminated.
struct Recorder(Mutex<bool>);

impl HttpKernelInterface for Recorder {
    fn handle(&self, _: &RequestHandle, _: i64, _: bool) -> Result<ResponseHandle, Throwable> {
        Ok(Handle::new(Response::default()))
    }
}

impl TerminableInterface for Recorder {
    fn terminate(&self, _: &RequestHandle, _: &ResponseHandle) -> Result<(), Throwable> {
        *self.0.lock().unwrap() = true;
        Ok(())
    }
}

impl RunnableKernel for Recorder {
    fn as_terminable(&self) -> Option<&dyn TerminableInterface> {
        Some(self)
    }
}

fn finishes(finish: FinishRequest, debug: bool) -> (Vec<OutputCall>, bool) {
    set_globals(symfony_http_foundation::php::Globals::default());
    with_output(|o| {
        o.finish_request = finish;
        o.sapi = PhpString::from("fpm-fcgi");
    });
    let kernel = Arc::new(Recorder(Mutex::new(false)));
    let request = Handle::new(PhpRequest::create("/").unwrap());
    let code = HttpKernelRunner::new(kernel.clone(), request, debug)
        .run()
        .unwrap();
    assert_eq!(0, code);
    let calls = take_globals().output.calls;
    let terminated = *kernel.0.lock().unwrap();
    (calls, terminated)
}

#[test]
fn the_runner_finishes_the_request_as_the_sapi_allows() {
    let last = |calls: &[OutputCall]| calls.last().cloned();
    let (calls, terminated) = finishes(FinishRequest::FastCgi, false);
    assert_eq!(Some(OutputCall::FastcgiFinishRequest), last(&calls));
    assert!(terminated);
    // send(false) does not finish the request itself.
    assert_eq!(
        1,
        calls
            .iter()
            .filter(|c| **c == OutputCall::FastcgiFinishRequest)
            .count()
    );
    let (calls, _) = finishes(FinishRequest::LiteSpeed, false);
    assert_eq!(Some(OutputCall::LitespeedFinishRequest), last(&calls));
    for (finish, debug) in [(FinishRequest::FastCgi, true), (FinishRequest::None, false)] {
        let (calls, terminated) = finishes(finish, debug);
        assert_eq!(Some(OutputCall::Flush), last(&calls));
        assert!(terminated);
    }
}
