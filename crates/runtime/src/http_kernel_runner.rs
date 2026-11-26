//! Port of Symfony\Component\Runtime\Runner\Symfony\HttpKernelRunner.php:
//! handles the request, sends the response without flushing, ends the
//! request (`fastcgi_finish_request()` where PHP-FPM gives it, unless in
//! debug mode; otherwise the output buffers are closed and flushed), then
//! terminates a terminable kernel.
//!
//! `\function_exists('fastcgi_finish_request')` and
//! `litespeed_finish_request` are this thread's `php::Output`
//! `finish_request`; calling them records the call and runs the hook the
//! HTTP adapter set ([`crate::server`]), which sends the response while
//! `terminate()` runs, as PHP-FPM does.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/runtime Runner/Symfony/HttpKernelRunner.php

use std::sync::Arc;

use symfony_http_foundation::{
    php::{FinishRequest, OutputCall, with_output},
    request_stack::RequestHandle,
    response::Response,
};
use symfony_http_kernel::{
    exception::Throwable,
    http_kernel::{HttpKernel, HttpKernelInterface, TerminableInterface},
};

/// A kernel as the runner holds it: `HttpKernelInterface`, and
/// `instanceof TerminableInterface`.
pub trait RunnableKernel: HttpKernelInterface {
    /// The kernel as a `TerminableInterface`, when it is one.
    fn as_terminable(&self) -> Option<&dyn TerminableInterface> {
        None
    }
}

impl RunnableKernel for HttpKernel {
    fn as_terminable(&self) -> Option<&dyn TerminableInterface> {
        Some(self)
    }
}

/// `HttpKernelRunner`.
pub struct HttpKernelRunner {
    kernel: Arc<dyn RunnableKernel>,
    request: RequestHandle,
    debug: bool,
}

impl HttpKernelRunner {
    /// `new HttpKernelRunner($kernel, $request, $debug)`.
    pub fn new(kernel: Arc<dyn RunnableKernel>, request: RequestHandle, debug: bool) -> Self {
        Self {
            kernel,
            request,
            debug,
        }
    }

    /// `run()`: 0 once the response was sent and the kernel terminated.
    pub fn run(&self) -> Result<i64, Throwable> {
        let response =
            self.kernel
                .handle(&self.request, <dyn HttpKernelInterface>::MAIN_REQUEST, true)?;

        response.with_mut(|r| r.send(false).map(|_| ()))?;

        let finish = with_output(|o| o.finish_request);
        if finish == FinishRequest::FastCgi && !self.debug {
            crate::finish_request(OutputCall::FastcgiFinishRequest);
        } else if finish == FinishRequest::LiteSpeed && !self.debug {
            crate::finish_request(OutputCall::LitespeedFinishRequest);
        } else {
            Response::close_output_buffers(0, true);
            symfony_http_foundation::php::flush();
        }

        if let Some(kernel) = self.kernel.as_terminable() {
            kernel.terminate(&self.request, &response)?;
        }

        Ok(0)
    }
}
