//! Running a ported Symfony kernel behind an HTTP server, as Symfony
//! Runtime runs it under PHP-FPM: [`http_kernel_runner`] ports
//! `HttpKernelRunner`, and [`server`] is the HTTP adapter that serves it
//! from Axum the way nginx and PHP-FPM serve a front controller.
//!
//! See README.md for what is mapped and what is not.
//!
//! Upstream: none

#![forbid(unsafe_code)]

pub mod http_kernel_runner;
pub mod server;

use std::cell::RefCell;

use symfony_http_foundation::php::{OutputCall, with_output};

type FinishHook = Box<dyn FnOnce()>;

thread_local! {
    /// What ends the response of the request this thread runs: the HTTP
    /// adapter sends what was output so far.
    static FINISH: RefCell<Option<FinishHook>> = const { RefCell::new(None) };
}

/// Sets what `fastcgi_finish_request()` does on this thread for the
/// request it runs (the adapter sends the response).
pub fn set_finish_hook(hook: Option<FinishHook>) {
    FINISH.with(|f| *f.borrow_mut() = hook);
}

/// `fastcgi_finish_request()` or `litespeed_finish_request()`: the call is
/// recorded, and the response is sent once.
pub fn finish_request(call: OutputCall) {
    with_output(|o| o.calls.push(call));
    if let Some(hook) = FINISH.with(|f| f.borrow_mut().take()) {
        hook();
    }
}
