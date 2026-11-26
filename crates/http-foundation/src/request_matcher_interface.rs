//! Port of Symfony\Component\HttpFoundation\RequestMatcherInterface.php:
//! what decides whether a rule applies to a request.
//!
//! `matches()` returns PHP's `bool`, or what the request's methods threw
//! (an untrusted host's `SuspiciousOperationException`, for one): request
//! getters cache what they compute, so they take the request mutably.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcherInterface.php

use std::fmt;

use crate::{request::Request, streamed_response::Throwable};

/// `RequestMatcherInterface`.
pub trait RequestMatcherInterface: fmt::Debug + Send + Sync {
    /// `matches($request)`: whether the rule matches the request.
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable>;
}
