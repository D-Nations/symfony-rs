//! Selected HttpKernelTest.php assertions at installed Symfony commit
//! 924974dbdcdf4cc40d7bb2c65ae83bb6dc67cff1. Names are kept from PHPUnit.
//! Additional tests pin priority, propagation and explicit Rust error boundaries.
//!
//! Upstream: symfony/http-kernel Tests/HttpKernelTest.php
#![allow(non_snake_case)]

use axum::{
    body::{Body, to_bytes},
    extract::Request,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::sync::{Arc, Mutex};
use symfony_http_kernel::{
    ControllerEvent, Exception, ExceptionEvent, Kernel, Listener, RequestEvent, ResponseEvent,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn request() -> Request {
    Request::new(Body::from("unconsumed"))
}
fn failure() -> Exception {
    Exception::new(StatusCode::INTERNAL_SERVER_ERROR, "foo")
}
async fn text(response: Response) -> Result<String, Box<dyn std::error::Error>> {
    Ok(String::from_utf8(
        to_bytes(response.into_body(), 1024).await?.to_vec(),
    )?)
}

struct ErrorPage {
    status: StatusCode,
    custom: bool,
}
impl Listener<()> for ErrorPage {
    fn exception(&self, event: &mut ExceptionEvent<'_>) -> Result<(), Exception> {
        if self.custom {
            event.allow_custom_response_code();
        }
        event.set_response((self.status, event.exception.detail.clone()).into_response());
        Ok(())
    }
}

#[tokio::test]
async fn testHandleWhenControllerThrowsAnExceptionAndCatchIsTrue() {
    let result = Kernel::default()
        .handle(request(), |_| Ok(()), |_, ()| async { Err(failure()) })
        .await;
    assert_eq!(result.err().map(|e| e.detail).as_deref(), Some("foo"));
}

#[tokio::test]
async fn testHandleWhenControllerThrowsAnExceptionAndCatchIsTrueWithAHandlingListener() -> TestResult
{
    let mut kernel = Kernel::default();
    kernel.add_listener(
        0,
        ErrorPage {
            status: StatusCode::OK,
            custom: false,
        },
    );
    let response = kernel
        .handle(request(), |_| Ok(()), |_, ()| async { Err(failure()) })
        .await?;
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(text(response).await?, "foo");
    Ok(())
}

#[tokio::test]
async fn testHandleWhenControllerThrowsAnExceptionAndCatchIsTrueWithANonHandlingListener() {
    struct NonHandling;
    impl Listener<()> for NonHandling {}
    let mut kernel = Kernel::default();
    kernel.add_listener(0, NonHandling);
    let original = failure();
    let headers_identity = std::ptr::from_ref(original.headers.as_ref());
    let error = kernel
        .handle(request(), |_| Ok(()), |_, ()| async { Err(original) })
        .await
        .err();
    assert!(error.is_some_and(|error| std::ptr::eq(error.headers.as_ref(), headers_identity)));
}

#[tokio::test]
async fn testHandleExceptionWithARedirectionResponse() -> TestResult {
    struct Redirect;
    impl Listener<()> for Redirect {
        fn exception(&self, event: &mut ExceptionEvent<'_>) -> Result<(), Exception> {
            event.set_response(
                (
                    StatusCode::MOVED_PERMANENTLY,
                    [(header::LOCATION, "/login")],
                )
                    .into_response(),
            );
            Ok(())
        }
    }
    let mut kernel = Kernel::default();
    kernel.add_listener(0, Redirect);
    let response = kernel
        .handle(
            request(),
            |_| Ok(()),
            |_, ()| async { Err(Exception::new(StatusCode::FORBIDDEN, "")) },
        )
        .await?;
    assert_eq!(response.status(), StatusCode::MOVED_PERMANENTLY);
    assert_eq!(response.headers()[header::LOCATION], "/login");
    Ok(())
}

#[tokio::test]
async fn testHandleHttpException() -> TestResult {
    let mut kernel = Kernel::default();
    kernel.add_listener(
        0,
        ErrorPage {
            status: StatusCode::OK,
            custom: false,
        },
    );
    let response = kernel
        .handle(
            request(),
            |_| Ok(()),
            |_, ()| async {
                let mut error = Exception::new(StatusCode::METHOD_NOT_ALLOWED, "");
                error
                    .headers
                    .insert(header::ALLOW, HeaderValue::from_static("POST"));
                Err(error)
            },
        )
        .await?;
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(response.headers()[header::ALLOW], "POST");
    Ok(())
}

#[tokio::test]
async fn testHandleWhenAnExceptionIsHandledWithASpecificStatusCode() -> TestResult {
    for status in [StatusCode::OK, StatusCode::FOUND, StatusCode::FORBIDDEN] {
        let mut kernel = Kernel::default();
        kernel.add_listener(
            0,
            ErrorPage {
                status,
                custom: true,
            },
        );
        let response = kernel
            .handle(request(), |_| Ok(()), |_, ()| async { Err(failure()) })
            .await?;
        assert_eq!(response.status(), status);
    }
    Ok(())
}

#[tokio::test]
async fn exception_redirect_classification_keeps_symfonys_201_and_rejects_304() -> TestResult {
    for code in [
        200, 201, 204, 300, 301, 302, 303, 304, 305, 306, 307, 308, 400, 500,
    ] {
        let status = StatusCode::from_u16(code)?;
        let mut kernel = Kernel::default();
        kernel.add_listener(
            0,
            ErrorPage {
                status,
                custom: false,
            },
        );
        let response = kernel
            .handle(request(), |_| Ok(()), |_, ()| async { Err(failure()) })
            .await?;
        let retained = matches!(code, 201 | 301 | 302 | 303 | 307 | 308 | 400 | 500);
        assert_eq!(
            response.status(),
            if retained {
                status
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            },
            "{code}"
        );
    }
    Ok(())
}

struct EarlyResponse;
impl Listener<()> for EarlyResponse {
    fn request(&self, event: &mut RequestEvent<'_>) -> Result<(), Exception> {
        event.set_response("hello".into_response());
        Ok(())
    }
}

#[tokio::test]
async fn testHandleWhenAListenerReturnsAResponse() -> TestResult {
    let mut kernel = Kernel::default();
    kernel.add_listener(0, EarlyResponse);
    let response = kernel
        .handle(
            request(),
            |_| panic!("resolver must not run"),
            |_, ()| async { panic!("controller must not run") },
        )
        .await?;
    assert_eq!(text(response).await?, "hello");
    Ok(())
}

#[tokio::test]
async fn testHandleWhenNoControllerIsFound() {
    let result = Kernel::default()
        .handle(
            request(),
            |_| Err(Exception::new(StatusCode::NOT_FOUND, "")),
            |_, ()| async { panic!("controller must not run") },
        )
        .await;
    assert_eq!(result.err().map(|e| e.status), Some(StatusCode::NOT_FOUND));
}

#[tokio::test]
async fn testHandleWhenTheControllerIsAClosure() -> TestResult {
    let identity = Arc::new(());
    let mut original = "foo".into_response();
    original.extensions_mut().insert(Arc::clone(&identity));
    let response = Kernel::default()
        .handle(request(), |_| Ok(()), |_, ()| async { Ok(original) })
        .await?;
    assert!(
        response
            .extensions()
            .get::<Arc<()>>()
            .is_some_and(|marker| Arc::ptr_eq(marker, &identity))
    );
    assert_eq!(text(response).await?, "foo");
    Ok(())
}

struct ReplaceResponse;
impl Listener<()> for ReplaceResponse {
    fn response(&self, event: &mut ResponseEvent<'_>) -> Result<(), Exception> {
        *event.response = "foo".into_response();
        Ok(())
    }
}

#[tokio::test]
async fn testHandleWithAResponseListener() -> TestResult {
    let mut kernel = Kernel::default();
    kernel.add_listener(0, ReplaceResponse);
    let response = kernel
        .handle(
            request(),
            |_| Ok(()),
            |_, ()| async { Ok("".into_response()) },
        )
        .await?;
    assert_eq!(text(response).await?, "foo");
    Ok(())
}

#[derive(Clone, Copy, Debug)]
enum Stage {
    Request,
    Resolve,
    Controller,
    Invoke,
    Response,
    Exception,
}

struct Trace {
    id: &'static str,
    trace: Arc<Mutex<Vec<String>>>,
    stop: Option<Stage>,
}
impl Trace {
    fn record(&self, stage: Stage) {
        self.trace
            .lock()
            .unwrap()
            .push(format!("{stage:?}:{}", self.id));
    }
}
impl Listener<String> for Trace {
    fn request(&self, event: &mut RequestEvent<'_>) -> Result<(), Exception> {
        self.record(Stage::Request);
        // Prove that resolution and invocation see request listener changes.
        event
            .request
            .headers_mut()
            .insert("x-listener", HeaderValue::from_static("present"));
        if matches!(self.stop, Some(Stage::Request)) {
            event.propagation.stop();
        }
        Ok(())
    }
    fn controller(&self, event: &mut ControllerEvent<'_, String>) -> Result<(), Exception> {
        self.record(Stage::Controller);
        *event.controller = "replacement".to_owned();
        if matches!(self.stop, Some(Stage::Controller)) {
            event.propagation.stop();
        }
        Ok(())
    }
    fn response(&self, event: &mut ResponseEvent<'_>) -> Result<(), Exception> {
        self.record(Stage::Response);
        assert_eq!(event.request.headers["x-listener"], "present");
        if matches!(self.stop, Some(Stage::Response)) {
            event.propagation.stop();
        }
        Ok(())
    }
}

#[tokio::test]
async fn priority_ties_keep_registration_order_and_controller_replacement_runs() -> TestResult {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let mut kernel = Kernel::default();
    for (priority, id) in [(0, "low"), (20, "first"), (20, "second")] {
        kernel.add_listener(
            priority,
            Trace {
                id,
                trace: Arc::clone(&trace),
                stop: None,
            },
        );
    }
    let response = kernel
        .handle(
            request(),
            |request| {
                assert_eq!(request.headers()["x-listener"], "present");
                trace.lock().unwrap().push("resolve".to_owned());
                Ok("original".to_owned())
            },
            |request, controller| async {
                trace.lock().unwrap().push("invoke".to_owned());
                assert_eq!(
                    to_bytes(request.into_body(), 1024)
                        .await
                        .map_err(|_| failure())?
                        .as_ref(),
                    b"unconsumed"
                );
                Ok(controller.into_response())
            },
        )
        .await?;
    assert_eq!(text(response).await?, "replacement");
    assert_eq!(
        *trace.lock().unwrap(),
        [
            "Request:first",
            "Request:second",
            "Request:low",
            "resolve",
            "Controller:first",
            "Controller:second",
            "Controller:low",
            "invoke",
            "Response:first",
            "Response:second",
            "Response:low",
        ]
    );
    Ok(())
}

#[tokio::test]
async fn stopping_propagation_only_stops_the_current_event() -> TestResult {
    for stage in [Stage::Request, Stage::Controller, Stage::Response] {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let mut kernel = Kernel::default();
        kernel.add_listener(
            10,
            Trace {
                id: "first",
                trace: Arc::clone(&trace),
                stop: Some(stage),
            },
        );
        kernel.add_listener(
            0,
            Trace {
                id: "second",
                trace: Arc::clone(&trace),
                stop: None,
            },
        );
        kernel
            .handle(
                request(),
                |_| Ok(String::new()),
                |_, _| async { Ok("ok".into_response()) },
            )
            .await?;
        let trace = trace.lock().unwrap();
        assert!(!trace.contains(&format!("{stage:?}:second")));
        for other in [Stage::Request, Stage::Controller, Stage::Response] {
            if std::mem::discriminant(&stage) != std::mem::discriminant(&other) {
                assert!(trace.contains(&format!("{other:?}:second")));
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn early_and_error_responses_still_run_response_hooks() -> TestResult {
    let mut early = Kernel::default();
    early.add_listener(10, EarlyResponse);
    early.add_listener(0, ReplaceResponse);
    let response = early
        .handle(
            request(),
            |_| panic!("resolution skipped"),
            |_, ()| async { panic!("invocation skipped") },
        )
        .await?;
    assert_eq!(text(response).await?, "foo");
    let mut error = Kernel::default();
    error.add_listener(
        10,
        ErrorPage {
            status: StatusCode::OK,
            custom: false,
        },
    );
    error.add_listener(0, ReplaceResponse);
    let response = error
        .handle(request(), |_| Ok(()), |_, ()| async { Err(failure()) })
        .await?;
    assert_eq!(text(response).await?, "foo");
    Ok(())
}

#[tokio::test]
async fn failures_at_each_stage_are_handled_without_recursive_error_dispatch() -> TestResult {
    struct FailAt(Stage);
    impl Listener<()> for FailAt {
        fn request(&self, _: &mut RequestEvent<'_>) -> Result<(), Exception> {
            if matches!(self.0, Stage::Request) {
                Err(failure())
            } else {
                Ok(())
            }
        }
        fn controller(&self, _: &mut ControllerEvent<'_, ()>) -> Result<(), Exception> {
            if matches!(self.0, Stage::Controller) {
                Err(failure())
            } else {
                Ok(())
            }
        }
        fn response(&self, _: &mut ResponseEvent<'_>) -> Result<(), Exception> {
            if matches!(self.0, Stage::Response) {
                Err(failure())
            } else {
                Ok(())
            }
        }
        fn exception(&self, _: &mut ExceptionEvent<'_>) -> Result<(), Exception> {
            if matches!(self.0, Stage::Exception) {
                Err(Exception::new(StatusCode::BAD_GATEWAY, "listener failed"))
            } else {
                Ok(())
            }
        }
    }
    for stage in [
        Stage::Request,
        Stage::Resolve,
        Stage::Controller,
        Stage::Invoke,
        Stage::Response,
        Stage::Exception,
    ] {
        let mut kernel = Kernel::default();
        kernel.add_listener(10, FailAt(stage));
        kernel.add_listener(
            0,
            ErrorPage {
                status: StatusCode::OK,
                custom: false,
            },
        );
        let result = kernel
            .handle(
                request(),
                |_| {
                    if matches!(stage, Stage::Resolve) {
                        Err(failure())
                    } else {
                        Ok(())
                    }
                },
                |_, ()| async {
                    if matches!(stage, Stage::Invoke | Stage::Exception) {
                        Err(failure())
                    } else {
                        Ok("ok".into_response())
                    }
                },
            )
            .await;
        if matches!(stage, Stage::Exception) {
            assert_eq!(
                result.err().map(|e| e.status),
                Some(StatusCode::BAD_GATEWAY)
            );
        } else {
            let response = result?;
            assert_eq!(
                response.status(),
                StatusCode::INTERNAL_SERVER_ERROR,
                "{stage:?}"
            );
            assert_eq!(text(response).await?, "foo");
        }
    }
    Ok(())
}

#[tokio::test]
async fn exception_replacement_and_handling_stop_lower_priority_listeners() -> TestResult {
    struct Replace;
    impl Listener<()> for Replace {
        fn exception(&self, event: &mut ExceptionEvent<'_>) -> Result<(), Exception> {
            *event.exception = Exception::new(StatusCode::BAD_REQUEST, "replacement");
            Ok(())
        }
    }
    struct MustNotRun;
    impl Listener<()> for MustNotRun {
        fn exception(&self, _: &mut ExceptionEvent<'_>) -> Result<(), Exception> {
            panic!("exception propagation must stop")
        }
    }
    let mut kernel = Kernel::default();
    kernel.add_listener(20, Replace);
    kernel.add_listener(
        10,
        ErrorPage {
            status: StatusCode::OK,
            custom: false,
        },
    );
    kernel.add_listener(0, MustNotRun);
    let response = kernel
        .handle(request(), |_| Ok(()), |_, ()| async { Err(failure()) })
        .await?;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(text(response).await?, "replacement");
    Ok(())
}

#[tokio::test]
async fn exception_listeners_can_inspect_an_application_throwable() -> TestResult {
    #[derive(Debug)]
    struct Domain;
    impl std::fmt::Display for Domain {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("domain failure")
        }
    }
    impl std::error::Error for Domain {}
    struct Unprocessable;
    impl Listener<()> for Unprocessable {
        fn exception(&self, event: &mut ExceptionEvent<'_>) -> Result<(), Exception> {
            let exception = &*event.exception;
            if !exception.is_http()
                && exception
                    .throwable
                    .as_ref()
                    .is_some_and(|t| t.downcast_ref::<Domain>().is_some())
            {
                let body = exception.detail.clone();
                event.set_response((StatusCode::UNPROCESSABLE_ENTITY, body).into_response());
            }
            Ok(())
        }
    }
    let mut kernel = Kernel::default();
    kernel.add_listener(0, Unprocessable);
    let response = kernel
        .handle(
            request(),
            |_| Ok(()),
            |_, ()| async { Err(Exception::from_throwable(Domain)) },
        )
        .await?;
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(text(response).await?, "domain failure");
    assert!(failure().is_http());
    let unhandled = Kernel::<()>::default()
        .handle(
            request(),
            |_| Ok(()),
            |_, ()| async { Err(Exception::from_throwable(Domain)) },
        )
        .await;
    assert_eq!(
        unhandled.err().map(|e| e.status),
        Some(StatusCode::INTERNAL_SERVER_ERROR)
    );
    Ok(())
}
