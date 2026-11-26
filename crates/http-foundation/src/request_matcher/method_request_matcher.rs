//! Port of Symfony\Component\HttpFoundation\RequestMatcher\MethodRequestMatcher.php:
//! the method is one of the methods (uppercased; a list given as text is
//! split on commas). GET brings HEAD, as the router serves HEAD with the
//! GET route.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/MethodRequestMatcher.php

use std::sync::Arc;

use super::split_all;
use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new MethodRequestMatcher($methods)`; a string is a list of one.
#[derive(Debug, Clone)]
pub struct MethodRequestMatcher {
    methods: Vec<Vec<u8>>,
}

impl MethodRequestMatcher {
    pub fn new<T: AsRef<[u8]>>(methods: impl IntoIterator<Item = T>) -> Self {
        let mut methods = split_all(methods, <[u8]>::to_ascii_uppercase);
        // HEAD requests are served by the GET handler, as the router does
        let has = |m: &[Vec<u8>], name: &[u8]| m.iter().any(|x| x == name);
        if has(&methods, b"GET") && !has(&methods, b"HEAD") {
            methods.push(b"HEAD".to_vec());
        }
        Self { methods }
    }
}

impl RequestMatcherInterface for MethodRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        if self.methods.is_empty() {
            return Ok(true);
        }
        let value = request
            .get_method()
            .map_err(|e| -> Throwable { Arc::new(e) })?;
        Ok(self
            .methods
            .iter()
            .any(|m| m.as_slice() == value.as_bytes()))
    }
}
