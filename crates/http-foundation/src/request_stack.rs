//! Port of Symfony\Component\HttpFoundation\RequestStack.php: the requests
//! being handled, the main request first.
//!
//! Requests are PHP objects that the stack, the kernel and listeners
//! share: the stack holds [`RequestHandle`]s, and [`RequestStack`] itself
//! is shared the same way (cloning it gives the same stack).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestStack.php

use std::sync::Arc;

use php_value::Handle;

use crate::{
    exception::{Class, Error, Result},
    request::Request,
    session::session_interface::SessionInterface,
};

/// A PHP `Request` object handle.
pub type RequestHandle = Handle<Request>;

/// `Symfony\Component\HttpFoundation\RequestStack`. Clones are the same
/// stack, as a PHP object passed to several services is.
#[derive(Debug, Clone)]
pub struct RequestStack(Handle<Vec<RequestHandle>>);

impl Default for RequestStack {
    /// `new RequestStack()`.
    fn default() -> Self {
        Self(Handle::new(Vec::new()))
    }
}

impl RequestStack {
    /// `new RequestStack($requests)`.
    pub fn new(requests: impl IntoIterator<Item = RequestHandle>) -> Self {
        let stack = Self::default();
        for request in requests {
            stack.push(request);
        }
        stack
    }

    /// `$a === $b`.
    pub fn same(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }

    /// `push($request)`. This method should generally not be called
    /// directly as the stack management should be taken care of by the
    /// application itself.
    pub fn push(&self, request: RequestHandle) {
        self.0.with_mut(|requests| requests.push(request));
    }

    /// `pop()`: the current request, removed from the stack.
    pub fn pop(&self) -> Option<RequestHandle> {
        self.0.with_mut(Vec::pop)
    }

    /// `getCurrentRequest()`.
    pub fn get_current_request(&self) -> Option<RequestHandle> {
        self.0.with(|requests| requests.last().cloned())
    }

    /// `getMainRequest()`.
    pub fn get_main_request(&self) -> Option<RequestHandle> {
        self.0.with(|requests| requests.first().cloned())
    }

    /// `getParentRequest()`: the request before the current one; `None`
    /// when the current request is the main request.
    pub fn get_parent_request(&self) -> Option<RequestHandle> {
        self.0.with(|requests| {
            requests
                .len()
                .checked_sub(2)
                .and_then(|at| requests.get(at).cloned())
        })
    }

    /// `getSession()`: the current request's session, when it has one
    /// (or a factory, which this calls).
    pub fn get_session(&self) -> Result<Arc<dyn SessionInterface>> {
        if let Some(request) = self.get_current_request()
            && request.with(|r| r.has_session(false))
        {
            return request.with_mut(Request::get_session);
        }
        Err(Error::new(
            Class::SessionNotFound,
            "There is currently no session available.",
        ))
    }

    /// `resetRequestFormats()`: `Request::$formats` back to the defaults.
    pub fn reset_request_formats(&self) {
        Request::reset_formats();
    }
}
