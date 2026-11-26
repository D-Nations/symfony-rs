//! RequestMatcher/QueryParameterRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/QueryParameterRequestMatcherTest.php
#![allow(non_snake_case)]

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

use symfony_http_foundation::request_matcher::query_parameter_request_matcher::QueryParameterRequestMatcher;

fn get_data_for_array() -> [(&'static str, bool); 5] {
    [
        ("https://example.com?foo=&bar=", true),
        ("https://example.com?foo=foo1&bar=bar1", true),
        ("https://example.com?foo=foo1&bar=bar1&baz=baz1", true),
        ("https://example.com?foo=", false),
        ("https://example.com", false),
    ]
}

fn get_data_for_single_string() -> [(&'static str, bool); 5] {
    [
        ("https://example.com?foo=&bar=", true),
        ("https://example.com?foo=foo1", true),
        ("https://example.com?foo=", true),
        ("https://example.com?bar=bar1&baz=baz1", false),
        ("https://example.com", false),
    ]
}

#[test]
fn testArray() -> TestResult {
    for (uri, expected) in get_data_for_array() {
        let matcher = QueryParameterRequestMatcher::new(["foo", "bar"]);
        let mut request = Request::create(uri)?;
        assert_eq!(expected, matches(&matcher, &mut request)?, "{uri}");
    }
    Ok(())
}

#[test]
fn testCommaSeparatedString() -> TestResult {
    for (uri, expected) in get_data_for_array() {
        let matcher = QueryParameterRequestMatcher::new(["foo, bar"]);
        let mut request = Request::create(uri)?;
        assert_eq!(expected, matches(&matcher, &mut request)?, "{uri}");
    }
    Ok(())
}

#[test]
fn testSingleString() -> TestResult {
    for (uri, expected) in get_data_for_single_string() {
        let matcher = QueryParameterRequestMatcher::new(["foo"]);
        let mut request = Request::create(uri)?;
        assert_eq!(expected, matches(&matcher, &mut request)?, "{uri}");
    }
    Ok(())
}
