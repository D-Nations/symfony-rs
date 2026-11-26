//! RequestMatcher/ExpressionRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/ExpressionRequestMatcherTest.php
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

use std::sync::Arc;

use symfony_expression_language::ExpressionLanguage;
use symfony_http_foundation::request_matcher::expression_request_matcher::{
    ExpressionRequestMatcher, ExpressionSource,
};

#[test]
fn testMatchesWhenParentMatchesIsTrue() -> TestResult {
    for (expression, expected) in [
        ("request.getMethod() == method", true),
        ("request.getPathInfo() == path", true),
        ("request.getHost() == host", true),
        ("request.getClientIp() == ip", true),
        ("request.attributes.all() == attributes", true),
        (
            "request.getMethod() == method && request.getPathInfo() == path && request.getHost() == host && request.getClientIp() == ip &&  request.attributes.all() == attributes",
            true,
        ),
        ("request.getMethod() != method", false),
        (
            "request.getMethod() != method && request.getPathInfo() == path && request.getHost() == host && request.getClientIp() == ip &&  request.attributes.all() == attributes",
            false,
        ),
    ] {
        let mut request = Request::create("/foo")?;
        let expression_request_matcher = ExpressionRequestMatcher::new(
            Arc::new(ExpressionLanguage::new()),
            ExpressionSource::Source(PhpString::from(expression)),
        );
        assert_eq!(
            expected,
            matches(&expression_request_matcher, &mut request)?,
            "{expression}"
        );
    }
    Ok(())
}
