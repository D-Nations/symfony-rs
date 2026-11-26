//! Port of Symfony\Component\HttpFoundation\RequestMatcher\HostRequestMatcher.php:
//! the host matches `'{'.$regexp.'}i'`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/HostRequestMatcher.php

use std::sync::Arc;

use super::Pattern;
use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new HostRequestMatcher($regexp)`.
#[derive(Debug)]
pub struct HostRequestMatcher {
    regexp: Pattern,
}

impl HostRequestMatcher {
    pub fn new(regexp: impl AsRef<[u8]>) -> Self {
        Self {
            regexp: Pattern::new(regexp.as_ref(), b"i"),
        }
    }
}

impl RequestMatcherInterface for HostRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        let value = request
            .get_host()
            .map_err(|e| -> Throwable { Arc::new(e) })?;
        Ok(self.regexp.is_match(value.as_bytes()))
    }
}
