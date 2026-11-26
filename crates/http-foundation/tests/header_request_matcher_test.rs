//! RequestMatcher/HeaderRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! A header given without a name (`['X-foo']`) is set under the key `0`.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/HeaderRequestMatcherTest.php
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

use symfony_http_foundation::request_matcher::header_request_matcher::HeaderRequestMatcher;

type Headers = &'static [(&'static str, &'static str)];

fn get_data_for_array() -> [(&'static str, Headers, bool); 7] {
    [
        (
            "Superfluous header",
            &[("X-Foo", "foo"), ("bar", "bar"), ("baz", "baz")],
            true,
        ),
        ("Exact match", &[("X-Foo", "foo"), ("bar", "bar")], true),
        (
            "Case insensitivity",
            &[("x-foo", "foo"), ("BAR", "bar")],
            true,
        ),
        (
            "Only one header matching",
            &[("bar", "bar"), ("baz", "baz")],
            false,
        ),
        ("Only one header", &[("X-foo", "foo")], false),
        ("Header name as a value", &[("0", "X-foo")], false),
        ("Empty headers", &[], false),
    ]
}

fn get_data_for_single_string() -> [(&'static str, Headers, bool); 5] {
    [
        (
            "Superfluous header",
            &[("X-Foo", "foo"), ("bar", "bar")],
            true,
        ),
        ("Exact match", &[("X-foo", "foo")], true),
        ("Case insensitivity", &[("x-foo", "foo")], true),
        ("Header name as a value", &[("0", "X-foo")], false),
        ("Empty headers", &[], false),
    ]
}

fn check(
    matcher: &HeaderRequestMatcher,
    headers: Headers,
    expected: bool,
    name: &str,
) -> TestResult {
    let mut request = Request::create("https://example.com")?;
    for (k, v) in headers {
        request.headers_mut().set(k.as_bytes(), *v, true)?;
    }
    assert_eq!(expected, matches(matcher, &mut request)?, "{name}");
    Ok(())
}

#[test]
fn testArray() -> TestResult {
    for (name, headers, expected) in get_data_for_array() {
        check(
            &HeaderRequestMatcher::new(["x-foo", "bar"]),
            headers,
            expected,
            name,
        )?;
    }
    Ok(())
}

#[test]
fn testCommaSeparatedString() -> TestResult {
    for (name, headers, expected) in get_data_for_array() {
        check(
            &HeaderRequestMatcher::new(["x-foo, bar"]),
            headers,
            expected,
            name,
        )?;
    }
    Ok(())
}

#[test]
fn testSingleString() -> TestResult {
    for (name, headers, expected) in get_data_for_single_string() {
        check(
            &HeaderRequestMatcher::new(["x-foo"]),
            headers,
            expected,
            name,
        )?;
    }
    Ok(())
}
