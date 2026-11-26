//! Port of Symfony\Component\HttpFoundation\RequestMatcher\QueryParameterRequestMatcher.php:
//! `!array_diff_assoc($this->parameters, $request->query->keys())`.
//!
//! The parameters are lowercased (a list given as text is split on
//! commas) and the query's keys are not; `array_diff_assoc()` compares
//! positions as well as names, so the request matches only when its first
//! query keys are the parameters, in order.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/QueryParameterRequestMatcher.php

use super::split_all;
use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new QueryParameterRequestMatcher($parameters)`; a string is a list
/// of one.
#[derive(Debug, Clone)]
pub struct QueryParameterRequestMatcher {
    parameters: Vec<Vec<u8>>,
}

impl QueryParameterRequestMatcher {
    pub fn new<T: AsRef<[u8]>>(parameters: impl IntoIterator<Item = T>) -> Self {
        Self {
            parameters: split_all(parameters, <[u8]>::to_ascii_lowercase),
        }
    }
}

impl RequestMatcherInterface for QueryParameterRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        if self.parameters.is_empty() {
            return Ok(true);
        }
        let keys = request.query().keys();
        // The key at each parameter's position must have its name
        // ((string) comparison, as array_diff_assoc() makes).
        Ok(self.parameters.iter().enumerate().all(|(i, parameter)| {
            keys.get(i).is_some_and(|key| {
                key.to_value()
                    .to_php_string()
                    .is_some_and(|k| k.as_bytes() == parameter.as_slice())
            })
        }))
    }
}
