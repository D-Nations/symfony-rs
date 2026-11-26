//! Port of Symfony\Component\HttpFoundation\RequestMatcher\HeaderRequestMatcher.php:
//! every header named must be present (names compared without case); a
//! list given as text is split on commas.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/HeaderRequestMatcher.php

use super::split_all;
use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new HeaderRequestMatcher($headers)`; a string is a list of one.
#[derive(Debug, Clone)]
pub struct HeaderRequestMatcher {
    headers: Vec<Vec<u8>>,
}

impl HeaderRequestMatcher {
    pub fn new<T: AsRef<[u8]>>(headers: impl IntoIterator<Item = T>) -> Self {
        Self {
            headers: split_all(headers, <[u8]>::to_vec),
        }
    }
}

impl RequestMatcherInterface for HeaderRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        if self.headers.is_empty() {
            return Ok(true);
        }
        Ok(self
            .headers
            .iter()
            .all(|header| request.headers().has(&String::from_utf8_lossy(header))))
    }
}
