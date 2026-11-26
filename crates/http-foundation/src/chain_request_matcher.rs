//! Port of Symfony\Component\HttpFoundation\ChainRequestMatcher.php: a
//! request matches when every matcher of the chain does; the first that
//! does not ends the check.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation ChainRequestMatcher.php

use std::sync::Arc;

use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new ChainRequestMatcher($matchers)`.
#[derive(Debug, Clone)]
pub struct ChainRequestMatcher {
    matchers: Vec<Arc<dyn RequestMatcherInterface>>,
}

impl ChainRequestMatcher {
    pub fn new(matchers: impl IntoIterator<Item = Arc<dyn RequestMatcherInterface>>) -> Self {
        Self {
            matchers: matchers.into_iter().collect(),
        }
    }
}

impl RequestMatcherInterface for ChainRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        for matcher in &self.matchers {
            if !matcher.matches(request)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
