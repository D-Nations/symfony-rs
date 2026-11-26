//! RequestMatcher/HostRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/HostRequestMatcherTest.php
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

use symfony_http_foundation::request_matcher::host_request_matcher::HostRequestMatcher;

#[test]
fn test() -> TestResult {
    for (pattern, is_match) in [
        (r".*\.example\.com", true),
        (r"\.example\.com$", true),
        (r"^.*\.example\.com$", true),
        (r".*\.sensio\.com", false),
        (r".*\.example\.COM", true),
        (r"\.example\.COM$", true),
        (r"^.*\.example\.COM$", true),
        (r".*\.sensio\.COM", false),
    ] {
        let matcher = HostRequestMatcher::new(pattern);
        let mut request = create(
            "",
            "get",
            &[("HTTP_HOST", Value::string("foo.example.com"))],
            None,
        )?;
        assert_eq!(is_match, matches(&matcher, &mut request)?, "{pattern}");
    }
    Ok(())
}
