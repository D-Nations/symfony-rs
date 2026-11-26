//! What the controllers read from a request, in the form Symfony's
//! `Request` gives it to the original (`vendor/symfony/http-foundation`).
//!
//! This is a request compatibility subset, not the complete component.
//! No demo state, routes, templates or database types belong here.
//!
//! Upstream: symfony/http-foundation Request.php

#![forbid(unsafe_code)]

pub mod accept_header;
pub mod accept_language;
pub mod binary_file_response;
pub mod chain_request_matcher;
pub mod cookie;
pub mod cookies;
pub mod event_stream_response;
pub mod exception;
pub mod file;
pub mod file_bag;
pub mod file_stream;
pub mod generator;
pub mod header_bag;
pub mod header_utils;
pub mod input_bag;
pub mod ip_utils;
pub mod json_response;
pub mod multipart;
pub mod parameter_bag;
pub mod php;
pub mod query;
pub mod redirect_response;
pub mod request;
pub mod request_matcher;
pub mod request_matcher_interface;
pub mod request_stack;
pub mod response;
pub mod sapi;
pub mod scheme;
pub mod server_bag;
pub mod server_event;
pub mod session;
pub mod streamed_json_response;
pub mod streamed_response;
pub mod uploaded_file;

/// PHP's default `post_max_size`. HTTP adapters must bound body reads to
/// this size and pass an empty body on read failure, as the demo does.
pub const POST_MAX_SIZE: usize = 8 * 1024 * 1024;

/// `Request::createFromGlobals()` form fields. POST retains fields read
/// before a multipart warning; PUT, DELETE, PATCH and QUERY discard the
/// whole result on a parser failure (`request_parse_body()`). Spaces and
/// tabs around the content type are ignored, as the HTTP server in front
/// of PHP drops them.
pub fn form_fields(method: &str, content_type: &[u8], body: &[u8]) -> query::Query {
    let strict = match method {
        "POST" => false,
        "PUT" | "DELETE" | "PATCH" | "QUERY" => true,
        _ => return query::Query::default(),
    };
    let content_type = trim_ows(content_type);
    match BodyFormat::of(content_type) {
        Some(BodyFormat::UrlEncoded) => query::Query::parse_body(body),
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

/// A header value without the spaces and tabs around it (HTTP's optional
/// whitespace).
pub fn trim_ows(value: &[u8]) -> &[u8] {
    let ows = |b: &u8| matches!(b, b' ' | b'\t');
    let start = value.iter().position(|b| !ows(b)).unwrap_or(value.len());
    let end = value
        .iter()
        .rposition(|b| !ows(b))
        .map_or(start, |end| end + 1);
    &value[start..end]
}

/// The body formats PHP reads into form fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyFormat {
    UrlEncoded,
    Multipart,
}

impl BodyFormat {
    /// The content type up to the first `;`, `,` or space, in any case,
    /// must be one of the two exactly (recorded:
    /// `tests/data/content_type_vectors.json`, and the multipart vectors'
    /// `not-multipart` and `boundary-param-*`).
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
