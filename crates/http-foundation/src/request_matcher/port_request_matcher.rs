//! Port of Symfony\Component\HttpFoundation\RequestMatcher\PortRequestMatcher.php:
//! `$request->getPort() === $port`, so a port read as a string from
//! `SERVER_PORT` never matches.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/PortRequestMatcher.php

use std::sync::Arc;

use php_value::Value;

use crate::{
    request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new PortRequestMatcher($port)`.
#[derive(Debug, Clone)]
pub struct PortRequestMatcher {
    port: i64,
}

impl PortRequestMatcher {
    pub fn new(port: i64) -> Self {
        Self { port }
    }
}

impl RequestMatcherInterface for PortRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        let port = request
            .get_port()
            .map_err(|e| -> Throwable { Arc::new(e) })?;
        Ok(port.identical(&Value::Int(self.port)))
    }
}
