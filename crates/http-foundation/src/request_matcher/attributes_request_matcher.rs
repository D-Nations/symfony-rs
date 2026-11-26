//! Port of Symfony\Component\HttpFoundation\RequestMatcher\AttributesRequestMatcher.php:
//! each attribute must be a string matching its pattern
//! (`preg_match('{'.$regexp.'}', $attribute)`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/AttributesRequestMatcher.php

use php_value::Value;

use super::Pattern;
use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new AttributesRequestMatcher($regexps)`: attribute names and
/// patterns, in order.
#[derive(Debug)]
pub struct AttributesRequestMatcher {
    regexps: Vec<(String, Pattern)>,
}

impl AttributesRequestMatcher {
    pub fn new<K: Into<String>, R: AsRef<[u8]>>(regexps: impl IntoIterator<Item = (K, R)>) -> Self {
        Self {
            regexps: regexps
                .into_iter()
                .map(|(key, regexp)| (key.into(), Pattern::new(regexp.as_ref(), b"")))
                .collect(),
        }
    }
}

impl RequestMatcherInterface for AttributesRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        for (key, regexp) in &self.regexps {
            let Value::String(attribute) = request.attributes().get(key, Value::Null) else {
                return Ok(false);
            };
            if !regexp.is_match(attribute.as_bytes()) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
