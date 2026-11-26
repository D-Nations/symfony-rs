//! RequestMatcher/SchemeRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! A string argument is a list of one.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/SchemeRequestMatcherTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::similar_names,
    reason = "upstream's $httpRequest and $httpsRequest"
)]

use php_value::{PhpArray, PhpString, Value};
use symfony_http_foundation::{
    request::{Content, Create, Request},
    request_matcher_interface::RequestMatcherInterface,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `Request::create($uri, $method, [], [], [], $server, $content)`.
#[allow(dead_code)]
fn create(
    uri: &str,
    method: &str,
    server: &[(&str, Value)],
    content: Option<&str>,
) -> Result<Request, Box<dyn std::error::Error>> {
    let mut array = PhpArray::new();
    for (key, value) in server {
        array.insert(*key, value.clone());
    }
    Ok(Request::create_with(
        uri,
        Create {
            method: Some(PhpString::from(method)),
            server: array,
            content: content.map_or(Content::Null, |c| Content::String(PhpString::from(c))),
            ..Create::default()
        },
    )?)
}

fn matches(
    matcher: &dyn RequestMatcherInterface,
    request: &mut Request,
) -> Result<bool, Box<dyn std::error::Error>> {
    matcher
        .matches(request)
        .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })
}

use symfony_http_foundation::request_matcher::scheme_request_matcher::SchemeRequestMatcher;

#[test]
fn test() -> TestResult {
    let cases: [(&str, &[&str], bool); 8] = [
        ("http", &["http"], true),
        ("http", &["HTTP"], true),
        ("https", &["https"], true),
        ("http", &["ftp"], false),
        ("http", &["ftp, http"], true),
        ("http", &["FTP, HTTP"], true),
        ("http", &["http", "ftp"], true),
        ("http", &["http,ftp"], true),
    ];
    for (request_scheme, matcher_scheme, is_match) in cases {
        let mut http_request = Request::create("")?;
        let mut https_request = create("", "get", &[("HTTPS", Value::string("on"))], None)?;
        let matcher = SchemeRequestMatcher::new(matcher_scheme.iter().copied());
        let case = format!("{request_scheme} {matcher_scheme:?}");

        if is_match {
            if request_scheme == "https" {
                assert!(!matches(&matcher, &mut http_request)?, "{case}");
                assert!(matches(&matcher, &mut https_request)?, "{case}");
            } else {
                assert!(!matches(&matcher, &mut https_request)?, "{case}");
                assert!(matches(&matcher, &mut http_request)?, "{case}");
            }
        } else {
            assert!(!matches(&matcher, &mut http_request)?, "{case}");
            assert!(!matches(&matcher, &mut https_request)?, "{case}");
        }
    }
    Ok(())
}

#[test]
fn testAlwaysMatchesOnParamsHeaders() -> TestResult {
    let matcher = SchemeRequestMatcher::new([] as [&str; 0]);
    let mut request = Request::create("sftp://example.com")?;
    assert!(matches(&matcher, &mut request)?);
    Ok(())
}
