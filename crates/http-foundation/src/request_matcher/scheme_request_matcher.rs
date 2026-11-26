//! Port of Symfony\Component\HttpFoundation\RequestMatcher\SchemeRequestMatcher.php:
//! the scheme is one of the schemes (lowercased; a list given as text is
//! split on commas).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/SchemeRequestMatcher.php

use std::sync::Arc;

use super::split_all;
use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new SchemeRequestMatcher($schemes)`; a string is a list of one.
#[derive(Debug, Clone)]
pub struct SchemeRequestMatcher {
    schemes: Vec<Vec<u8>>,
}

impl SchemeRequestMatcher {
    pub fn new<T: AsRef<[u8]>>(schemes: impl IntoIterator<Item = T>) -> Self {
        let schemes = split_all(schemes, <[u8]>::to_ascii_lowercase);
        Self { schemes }
    }
}

impl RequestMatcherInterface for SchemeRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        if self.schemes.is_empty() {
            return Ok(true);
        }
        let value = request
            .get_scheme()
            .map_err(|e| -> Throwable { Arc::new(e) })?;
        Ok(self
            .schemes
            .iter()
            .any(|m| m.as_slice() == value.as_bytes()))
    }
}
