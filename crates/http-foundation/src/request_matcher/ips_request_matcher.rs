//! Port of Symfony\Component\HttpFoundation\RequestMatcher\IpsRequestMatcher.php:
//! the client IP (`''` when there is none) is one of the addresses or
//! subnets (`IpUtils::checkIp()`); a list given as text is split on
//! commas.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RequestMatcher/IpsRequestMatcher.php

use std::sync::Arc;

use super::split_all;
use crate::{
    ip_utils, request::Request, request_matcher_interface::RequestMatcherInterface,
    streamed_response::Throwable,
};

/// `new IpsRequestMatcher($ips)`; a string is a list of one.
#[derive(Debug, Clone)]
pub struct IpsRequestMatcher {
    ips: Vec<String>,
}

impl IpsRequestMatcher {
    pub fn new<T: AsRef<[u8]>>(ips: impl IntoIterator<Item = T>) -> Self {
        Self {
            ips: split_all(ips, <[u8]>::to_vec)
                .into_iter()
                .map(|ip| String::from_utf8_lossy(&ip).into_owned())
                .collect(),
        }
    }
}

impl RequestMatcherInterface for IpsRequestMatcher {
    fn matches(&self, request: &mut Request) -> Result<bool, Throwable> {
        if self.ips.is_empty() {
            return Ok(true);
        }
        let ip = request
            .get_client_ip()
            .map_err(|e| -> Throwable { Arc::new(e) })?
            .unwrap_or_default();
        let ips: Vec<&str> = self.ips.iter().map(String::as_str).collect();
        ip_utils::check_ip(&String::from_utf8_lossy(ip.as_bytes()), &ips)
            .map_err(|e| -> Throwable { Arc::new(e) })
    }
}
