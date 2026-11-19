//! What the controllers read from a request, in the form Symfony's
//! `Request` gives it to the original (`vendor/symfony/http-foundation`).
//!
//! This is a request compatibility subset, not the complete component.
//! No demo state, routes, templates or database types belong here.
//!
//! Upstream: symfony/http-foundation Request.php

#![forbid(unsafe_code)]

pub mod accept_language;
pub mod cookies;
pub mod multipart;
pub mod query;
pub mod scheme;

/// PHP's default `post_max_size`. HTTP adapters must bound body reads to
/// this size and pass an empty body on read failure, as the demo does.
pub const POST_MAX_SIZE: usize = 8 * 1024 * 1024;

/// `Request::createFromGlobals()` form fields. POST retains fields read
/// before a multipart warning; PUT, DELETE, PATCH and QUERY discard the
/// whole result on a parser failure (`request_parse_body()`).
pub fn form_fields(method: &str, content_type: &[u8], body: &[u8]) -> query::Query {
    let strict = match method {
        "POST" => false,
        "PUT" | "DELETE" | "PATCH" | "QUERY" => true,
        _ => return query::Query::default(),
    };
    match BodyFormat::of(content_type) {
        Some(BodyFormat::UrlEncoded) => query::Query::parse(&String::from_utf8_lossy(body)),
        Some(BodyFormat::Multipart) => {
            let parsed = multipart::parse(content_type, body);
            if strict && parsed.failed {
                query::Query::default()
            } else {
                query::Query::from_fields(&parsed.fields)
            }
        }
        None => query::Query::default(),
    }
}

/// The body formats PHP reads into form fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyFormat {
    UrlEncoded,
    Multipart,
}

impl BodyFormat {
    /// `sapi_read_post_data()`: the content type up to the first `;`, `,`
    /// or space, in any case, must be one of the two exactly.
    pub fn of(content_type: &[u8]) -> Option<Self> {
        let end = content_type
            .iter()
            .position(|&b| matches!(b, b';' | b',' | b' '))
            .unwrap_or(content_type.len());
        let media = content_type[..end].to_ascii_lowercase();
        match media.as_slice() {
            b"application/x-www-form-urlencoded" => Some(Self::UrlEncoded),
            b"multipart/form-data" => Some(Self::Multipart),
            _ => None,
        }
    }
}
