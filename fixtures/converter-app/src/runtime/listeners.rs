//! The two framework listeners the fixture's requests pass through:
//! `RouterListener` (`kernel.request`, priority 32) and `ResponseListener`
//! (`kernel.response`, priority 0). Not ported: `ErrorListener`, whose
//! logging is not compared and whose error page no recorded case reaches.
//!
//! Upstream: symfony/http-kernel EventListener/RouterListener.php EventListener/ResponseListener.php Exception/MethodNotAllowedHttpException.php
//! Upstream: symfony/routing RequestContext.php

use std::sync::Arc;

use axum::http::{HeaderValue, StatusCode, header};
use symfony_http_kernel::{Exception, Listener, RequestEvent, ResponseEvent};
use symfony_routing::{Match, RouteCollection};

use super::{
    framework_bundle::UrlRedirect,
    http_foundation::{RequestInfo, Server, prepare},
};
use crate::routes::Route;

/// What the router put into the request's attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Matched {
    Route(Route),
    Redirect(UrlRedirect),
}

/// The request as the router read it, kept for the controllers.
#[derive(Debug, Clone)]
pub struct Routed {
    pub info: RequestInfo,
    pub matched: Matched,
}

pub struct RouterListener {
    pub routes: Arc<RouteCollection<Route>>,
    pub server: Server,
}

impl<C> Listener<C> for RouterListener {
    fn request(&self, event: &mut RequestEvent<'_>) -> Result<(), Exception> {
        let request = &*event.request;
        // RequestContext::fromRequest() reads getHost() before getPort().
        let info = RequestInfo::from_parts(
            request.method(),
            request.uri(),
            request.headers(),
            &self.server,
        )?;
        let matched = match self.routes.match_request(&info.method, &info.path_info) {
            Match::Found(route) => Matched::Route(route),
            Match::TrailingSlashRedirect { path, .. } => Matched::Redirect(UrlRedirect {
                path,
                permanent: true,
                http_port: info.port,
            }),
            Match::NotFound => {
                let mut message = format!(
                    "No route found for \"{} {}\"",
                    info.method,
                    info.uri_for_path(&info.path_info)
                );
                if let Some(referer) = &info.referer {
                    message.push_str(&format!(" (from \"{referer}\")"));
                }
                return Err(Exception::new(StatusCode::NOT_FOUND, message));
            }
            Match::MethodNotAllowed { allow } => {
                let allow = allow.join(", ");
                let message = format!(
                    "No route found for \"{} {}\": Method Not Allowed (Allow: {allow})",
                    info.method,
                    info.uri_for_path(&info.path_info)
                );
                let mut exception = Exception::new(StatusCode::METHOD_NOT_ALLOWED, message);
                // MethodNotAllowedHttpException: strtoupper(implode(', ', $allow)).
                let value = HeaderValue::from_str(&allow.to_ascii_uppercase()).map_err(|_| {
                    Exception::new(StatusCode::INTERNAL_SERVER_ERROR, "invalid Allow header")
                })?;
                exception.headers.insert(header::ALLOW, value);
                return Err(exception);
            }
        };
        event
            .request
            .extensions_mut()
            .insert(Routed { info, matched });
        Ok(())
    }
}

/// `kernel.charset` for every response; none of the fixture's sets its own.
pub struct ResponseListener {
    pub charset: &'static str,
}

impl<C> Listener<C> for ResponseListener {
    fn response(&self, event: &mut ResponseEvent<'_>) -> Result<(), Exception> {
        prepare(event.response, event.request, self.charset);
        Ok(())
    }
}
