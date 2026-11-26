//! RequestMatcher/PortRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/PortRequestMatcherTest.php
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

use symfony_http_foundation::request_matcher::port_request_matcher::PortRequestMatcher;

#[test]
fn test() -> TestResult {
    for (port, expected) in [(8000, true), (9000, false)] {
        let matcher = PortRequestMatcher::new(port);
        let mut request = create(
            "",
            "get",
            &[
                ("HTTP_HOST", Value::Null),
                ("SERVER_PORT", Value::Int(8000)),
            ],
            None,
        )?;
        assert_eq!(expected, matches(&matcher, &mut request)?, "{port}");
    }
    Ok(())
}
