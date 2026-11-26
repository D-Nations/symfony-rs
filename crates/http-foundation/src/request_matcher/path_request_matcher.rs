//! Port of Symfony\Component\HttpFoundation\RequestMatcher\PathRequestMatcher.php:
//! the URL-decoded path info matches `'{'.$regexp.'}s'`.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/PathRequestMatcher.php

use std::sync::Arc;

use php_value::url::rawurldecode;

use super::Pattern;
use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new PathRequestMatcher($regexp)`.
#[derive(Debug)]
pub struct PathRequestMatcher {
    regexp: Pattern,
}

impl PathRequestMatcher {
    pub fn new(regexp: impl AsRef<[u8]>) -> Self {
        Self {
            regexp: Pattern::new(regexp.as_ref(), b"s"),
        }
    }
}

impl RequestMatcherInterface for PathRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        let value = request
            .get_path_info()
            .map_err(|e| -> Throwable { Arc::new(e) })?;
        Ok(self.regexp.is_match(&rawurldecode(value.as_bytes())))
    }
}
