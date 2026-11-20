//! `RedirectController::urlRedirectAction()`, the controller
//! `RedirectableCompiledUrlMatcher::redirect()` names for a trailing-slash
//! redirect: `permanent: true`, no scheme, `httpPort` from the request.
//!
//! Upstream: symfony/framework-bundle Controller/RedirectController.php Routing/RedirectableCompiledUrlMatcher.php

use axum::{http::StatusCode, response::Response};
use symfony_http_kernel::Exception;

use super::{
    NotPorted,
    http_foundation::{RequestInfo, redirect_response},
};

/// The arguments the matcher passes (`keepRequestMethod` is false).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlRedirect {
    pub path: String,
    pub permanent: bool,
    pub http_port: i64,
}

pub fn url_redirect_action(
    request: &RequestInfo,
    redirect: &UrlRedirect,
) -> Result<Response, Exception> {
    let mut path = redirect.path.clone();
    if path.is_empty() {
        let status = if redirect.permanent {
            StatusCode::GONE
        } else {
            StatusCode::NOT_FOUND
        };
        // new HttpException($status): an empty message.
        return Err(Exception::new(status, ""));
    }
    let status = if redirect.permanent {
        StatusCode::MOVED_PERMANENTLY
    } else {
        StatusCode::FOUND
    };
    let scheme = request.scheme();
    if path.starts_with("//") {
        path = format!("{scheme}:{path}");
        // parse_url() finds the scheme just added: redirect to it as is.
        return redirect_response(&path, status);
    }
    if !path.starts_with('/') {
        // parse_url($path, PHP_URL_SCHEME) would need porting.
        return Err(NotPorted("urlRedirectAction() with a path not starting with '/'").into());
    }
    // `$request->server->get('QUERY_STRING') ?: $request->getQueryString()`:
    // the raw query string when it is truthy. When it is '0', PHP falls back
    // to the normalized form, also '0'.
    let qs = match request.query_string.as_str() {
        "" => String::new(),
        raw if path.contains('?') => format!("&{raw}"),
        raw => format!("?{raw}"),
    };
    let port = if redirect.http_port != 80 {
        format!(":{}", redirect.http_port)
    } else {
        String::new()
    };
    let url = format!("{scheme}://{}{port}{path}{qs}", request.host);
    redirect_response(&url, status)
}
