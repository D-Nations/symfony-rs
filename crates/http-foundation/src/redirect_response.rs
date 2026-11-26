//! Port of Symfony\Component\HttpFoundation\RedirectResponse.php: a
//! redirect status, the Location header, and an HTML page with a meta
//! refresh to the target (`htmlspecialchars($url, ENT_QUOTES, 'UTF-8')`,
//! empty for a URL that is not UTF-8).
//!
//! A [`RedirectResponse`] is a [`Response`] (it dereferences to one), as
//! the PHP class extends it. Its headers are given as PHP's array, a list
//! of name and values pairs, so that a 301's default Cache-Control is
//! removed only when the array has no `cache-control` key in any case.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation RedirectResponse.php

use std::ops::{Deref, DerefMut};

use php_value::{PhpString, html::htmlspecialchars};

use crate::{
    exception::{Class, Error, Result},
    header_bag::{HeaderValues, ResponseHeaderBag},
    response::Response,
};

/// `Symfony\Component\HttpFoundation\RedirectResponse`.
#[derive(Debug, Clone, PartialEq)]
pub struct RedirectResponse {
    response: Response,
    target_url: PhpString,
}

impl Deref for RedirectResponse {
    type Target = Response;

    fn deref(&self) -> &Response {
        &self.response
    }
}

impl DerefMut for RedirectResponse {
    fn deref_mut(&mut self) -> &mut Response {
        &mut self.response
    }
}

impl From<RedirectResponse> for Response {
    fn from(response: RedirectResponse) -> Self {
        response.response
    }
}

impl RedirectResponse {
    /// `getContent()`: always a string, as the class is not a
    /// `StreamedResponse`.
    pub fn get_content(&self) -> &PhpString {
        self.response.content()
    }

    /// `new RedirectResponse($url, $status, $headers)`: a redirect so
    /// that it conforms to the rules defined for a redirect status code
    /// (RFC 2616 section 10.3). The URL should be a full URL, with schema
    /// etc., but practically every browser redirects on paths only as
    /// well; the Location header is always set to it.
    ///
    /// Fails with `\InvalidArgumentException` for a status code that is
    /// not valid, an empty URL, or a status that is not a redirect.
    pub fn new<K: AsRef<[u8]>, V: Into<HeaderValues>>(
        url: impl Into<PhpString>,
        status: i64,
        headers: impl IntoIterator<Item = (K, V)>,
    ) -> Result<Self> {
        let headers: Vec<(K, V)> = headers.into_iter().collect();
        // array_key_exists('cache-control', array_change_key_case($headers, CASE_LOWER))
        let has_cache_control = headers
            .iter()
            .any(|(name, _)| name.as_ref().eq_ignore_ascii_case(b"cache-control"));
        let bag = ResponseHeaderBag::new(headers)?;
        let mut response = Self {
            response: Response::new(Some(""), status, bag)?,
            target_url: PhpString::default(),
        };

        response.set_target_url(url)?;

        if !response.is_redirect(None) {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                format!("The HTTP status code is not a redirect (\"{status}\" given)."),
            ));
        }

        if 301 == status && !has_cache_control {
            response.headers_mut().remove("cache-control");
        }

        Ok(response)
    }

    /// `getTargetUrl()`.
    pub fn get_target_url(&self) -> &PhpString {
        &self.target_url
    }

    /// `setTargetUrl($url)`: the redirect target, the HTML page for it,
    /// Location and Content-Type.
    ///
    /// Fails with `\InvalidArgumentException` for an empty URL.
    pub fn set_target_url(&mut self, url: impl Into<PhpString>) -> Result<&mut Self> {
        let url = url.into();
        if url.is_empty() {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                "Cannot redirect to an empty URL.",
            ));
        }

        self.target_url = url.clone();

        let escaped = htmlspecialchars(url.as_bytes());
        let content = [
            &b"<!DOCTYPE html>\n<html>\n    <head>\n        <meta charset=\"UTF-8\" />\n        <meta http-equiv=\"refresh\" content=\"0;url='"[..],
            &escaped,
            b"'\" />\n\n        <title>Redirecting to ",
            &escaped,
            b"</title>\n    </head>\n    <body>\n        Redirecting to <a href=\"",
            &escaped,
            b"\">",
            &escaped,
            b"</a>.\n    </body>\n</html>",
        ]
        .concat();
        self.response.set_content(Some(PhpString::from(content)))?;

        self.response.headers_mut().set(b"Location", url, true)?;
        self.response
            .headers_mut()
            .set(b"Content-Type", "text/html; charset=utf-8", true)?;

        Ok(self)
    }
}
