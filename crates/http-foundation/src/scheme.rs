//! Request scheme resolution, as Symfony's `HttpFoundation` `Request.php`
//! does behind a trusted reverse proxy. This server listens over HTTP;
//! only an explicitly trusted TCP peer may report HTTPS termination.
//!
//! Upstream: symfony/http-foundation Request.php

use std::net::{IpAddr, SocketAddr};

use axum::{extract::ConnectInfo, http::request::Parts};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
}

impl Scheme {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https => "https",
        }
    }

    pub fn from_request(parts: &Parts, trusted_proxies: &[IpAddr]) -> Self {
        let trusted = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .is_some_and(|peer| trusted_proxies.contains(&peer.0.ip()));
        if trusted {
            let mut values = parts.headers.get_all("x-forwarded-proto").iter();
            if let (Some(value), None) = (values.next(), values.next())
                && value
                    .to_str()
                    .is_ok_and(|value| value.trim().eq_ignore_ascii_case("https"))
            {
                return Self::Https;
            }
        }
        Self::Http
    }
}
