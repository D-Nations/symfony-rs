//! Port of Symfony\Component\HttpFoundation\RequestMatcher\IsJsonRequestMatcher.php:
//! the body is valid JSON (`json_validate()`: what `json_decode()` reads
//! at the default depth, 512).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/IsJsonRequestMatcher.php

use std::sync::Arc;

use php_value::json::decode_assoc;

use crate::{
    request::{ContentValue, Request},
    request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new IsJsonRequestMatcher()`.
#[derive(Debug, Clone, Default)]
pub struct IsJsonRequestMatcher;

impl RequestMatcherInterface for IsJsonRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        let content = request
            .get_content(false)
            .map_err(|e| -> Throwable { Arc::new(e) })?;
        let ContentValue::String(content) = content else {
            return Ok(false);
        };
        Ok(decode_assoc(content.as_bytes(), 512, false).is_ok())
    }
}
