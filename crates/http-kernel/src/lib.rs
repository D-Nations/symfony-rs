//! Ports the supported lifecycle from Symfony HttpKernel/HttpKernel.php and
//! Event/{Request,Controller,Response,Exception}Event.php; listener ordering
//! follows EventDispatcher/EventDispatcher.php. See README.md for the subset.
//! Exception redirect classification follows HttpFoundation/Response.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel HttpKernel.php Event/RequestEvent.php Event/ControllerEvent.php Event/ResponseEvent.php Event/ExceptionEvent.php
//! Upstream: symfony/event-dispatcher EventDispatcher.php
//! Upstream: symfony/http-foundation Response.php

#![forbid(unsafe_code)]

pub mod argument_resolver;
pub mod controller;
pub mod controller_metadata;
pub mod event;
pub mod exception;
pub mod http_kernel;
pub mod router_listener;
pub mod value_resolver;

use std::{cmp::Reverse, future::Future, sync::Arc};

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode, request::Parts},
    response::Response,
};

/// An explicit application failure. Details are for server-side logging;
/// the kernel never writes them to an HTTP response on its own.
#[derive(Debug)]
pub struct Exception {
    pub status: StatusCode,
    pub headers: Box<HeaderMap>,
    pub detail: String,
    /// The application's own error when it is not an HTTP exception, as
    /// `ExceptionEvent::getThrowable()` returns it, for exception listeners
    /// to downcast. `None` for an HTTP exception (`HttpExceptionInterface`),
    /// which is `status`, `headers` and `detail` alone.
    pub throwable: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl Exception {
    /// An HTTP exception (`HttpException`): its status, message and headers.
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            headers: Box::default(),
            detail: detail.into(),
            throwable: None,
        }
    }

    /// Any other exception: `HttpKernel::handleThrowable` answers 500 unless
    /// a listener sets a response, and listeners can inspect the error.
    pub fn from_throwable(throwable: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            headers: Box::default(),
            detail: throwable.to_string(),
            throwable: Some(Box::new(throwable)),
        }
    }

    /// Whether this is an HTTP exception (`instanceof HttpExceptionInterface`).
    pub fn is_http(&self) -> bool {
        self.throwable.is_none()
    }
}

impl std::fmt::Display for Exception {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for Exception {}

/// Propagation belongs to one event, never to the entire request.
#[derive(Default)]
pub struct Propagation(bool);

impl Propagation {
    pub fn stop(&mut self) {
        self.0 = true;
    }
    pub fn is_stopped(&self) -> bool {
        self.0
    }
}

pub struct RequestEvent<'a> {
    pub request: &'a mut Request,
    pub propagation: Propagation,
    response: Option<Response>,
}

impl RequestEvent<'_> {
    /// Like Symfony's `RequestEvent::setResponse()`, stops this event.
    pub fn set_response(&mut self, response: Response) {
        self.response = Some(response);
        self.propagation.stop();
    }
}

pub struct ControllerEvent<'a, C> {
    pub request: &'a mut Request,
    /// An application-owned typed target; replacing it changes what runs.
    pub controller: &'a mut C,
    pub propagation: Propagation,
}

pub struct ResponseEvent<'a> {
    /// Snapshot after controller hooks, before the body is consumed.
    pub request: &'a Parts,
    pub response: &'a mut Response,
    pub propagation: Propagation,
}

pub struct ExceptionEvent<'a> {
    pub request: &'a Parts,
    /// Listeners may replace the error before a later listener handles it.
    pub exception: &'a mut Exception,
    pub propagation: Propagation,
    response: Option<Response>,
    allow_custom_response_code: bool,
}

impl ExceptionEvent<'_> {
    pub fn set_response(&mut self, response: Response) {
        self.response = Some(response);
        self.propagation.stop();
    }

    pub fn allow_custom_response_code(&mut self) {
        self.allow_custom_response_code = true;
    }
}

/// Hooks are synchronous; controllers remain asynchronous. Register at startup.
/// Register separate listeners when hooks need different priorities.
pub trait Listener<C>: Send + Sync {
    fn request(&self, _event: &mut RequestEvent<'_>) -> Result<(), Exception> {
        Ok(())
    }
    fn controller(&self, _event: &mut ControllerEvent<'_, C>) -> Result<(), Exception> {
        Ok(())
    }
    fn response(&self, _event: &mut ResponseEvent<'_>) -> Result<(), Exception> {
        Ok(())
    }
    fn exception(&self, _event: &mut ExceptionEvent<'_>) -> Result<(), Exception> {
        Ok(())
    }
}

/// Immutable during requests and safe to share between concurrent connections.
pub struct Kernel<C> {
    listeners: Vec<(i32, Arc<dyn Listener<C>>)>,
}

impl<C> Default for Kernel<C> {
    fn default() -> Self {
        Self {
            listeners: Vec::new(),
        }
    }
}

impl<C> Kernel<C> {
    /// Larger priorities run first. Equal priorities keep registration order.
    /// Sorting occurs here, never while handling a request.
    pub fn add_listener(&mut self, priority: i32, listener: impl Listener<C> + 'static) {
        self.listeners.push((priority, Arc::new(listener)));
        self.listeners
            .sort_by_key(|(priority, _)| Reverse(*priority));
    }

    pub async fn handle<R, I, F>(
        &self,
        mut request: Request,
        resolve: R,
        invoke: I,
    ) -> Result<Response, Exception>
    where
        R: FnOnce(&mut Request) -> Result<C, Exception>,
        I: FnOnce(Request, C) -> F,
        F: Future<Output = Result<Response, Exception>>,
    {
        let mut event = RequestEvent {
            request: &mut request,
            propagation: Propagation::default(),
            response: None,
        };
        for (_, listener) in &self.listeners {
            if event.propagation.is_stopped() {
                break;
            }
            if let Err(error) = listener.request(&mut event) {
                return self.handle_exception(&head(&request), error);
            }
        }
        if let Some(response) = event.response {
            return self.filter_response(&head(&request), response);
        }
        let mut controller = match resolve(&mut request) {
            Ok(controller) => controller,
            Err(error) => return self.handle_exception(&head(&request), error),
        };
        let mut event = ControllerEvent {
            request: &mut request,
            controller: &mut controller,
            propagation: Propagation::default(),
        };
        for (_, listener) in &self.listeners {
            if event.propagation.is_stopped() {
                break;
            }
            if let Err(error) = listener.controller(&mut event) {
                return self.handle_exception(&head(&request), error);
            }
        }
        let parts = head(&request);
        match invoke(request, controller).await {
            Ok(response) => self.filter_response(&parts, response),
            Err(error) => self.handle_exception(&parts, error),
        }
    }

    fn filter_response(
        &self,
        request: &Parts,
        mut response: Response,
    ) -> Result<Response, Exception> {
        if let Err(error) = self.response_listeners(request, &mut response) {
            return self.handle_exception(request, error);
        }
        Ok(response)
    }

    fn response_listeners(
        &self,
        request: &Parts,
        response: &mut Response,
    ) -> Result<(), Exception> {
        let mut event = ResponseEvent {
            request,
            response,
            propagation: Propagation::default(),
        };
        for (_, listener) in &self.listeners {
            if event.propagation.is_stopped() {
                break;
            }
            listener.response(&mut event)?;
        }
        Ok(())
    }

    fn handle_exception(
        &self,
        request: &Parts,
        mut error: Exception,
    ) -> Result<Response, Exception> {
        let mut event = ExceptionEvent {
            request,
            exception: &mut error,
            propagation: Propagation::default(),
            response: None,
            allow_custom_response_code: false,
        };
        for (_, listener) in &self.listeners {
            if event.propagation.is_stopped() {
                break;
            }
            listener.exception(&mut event)?;
        }
        let Some(mut response) = event.response else {
            return Err(error);
        };
        let status = response.status();
        if !event.allow_custom_response_code
            && !status.is_client_error()
            && !status.is_server_error()
            // HttpFoundation Response::isRedirect(), not isRedirection():
            // Symfony includes 201, but excludes 300, 304, 305 and 306.
            && !matches!(status.as_u16(), 201 | 301 | 302 | 303 | 307 | 308)
        {
            *response.status_mut() = error.status;
            response.headers_mut().extend(*error.headers);
        }
        // HttpKernel::handleThrowable keeps the handled response if filtering
        // it fails. Do not recurse into exception listeners indefinitely.
        let _ = self.response_listeners(request, &mut response);
        Ok(response)
    }
}

fn head(request: &Request) -> Parts {
    // Clone metadata only; never buffer or clone the request body.
    let mut copy = Request::new(());
    *copy.method_mut() = request.method().clone();
    *copy.uri_mut() = request.uri().clone();
    *copy.version_mut() = request.version();
    *copy.headers_mut() = request.headers().clone();
    *copy.extensions_mut() = request.extensions().clone();
    copy.into_parts().0
}
