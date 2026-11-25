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
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use php_value::{Handle, PhpString};
use symfony_http_foundation::{
    header_bag::ResponseHeaderBag,
    php::{FinishRequest, OutputCall, set_globals, take_globals, with_output},
    request::Request as PhpRequest,
    request_stack::RequestHandle,
    response::Response,
    sapi::{CgiResponse, Ini},
};
use symfony_http_kernel::{
    controller::ResponseHandle,
    exception::{PhpClass, PhpException, Throwable},
    http_kernel::{HttpKernelInterface, TerminableInterface},
};
use symfony_runtime::{
    http_kernel_runner::{HttpKernelRunner, RunnableKernel},
    server::{FrontController, ServerConfig, fastcgi_params, into_response},
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
        let uri = request.with_mut(|r| r.get_request_uri()).unwrap();
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
