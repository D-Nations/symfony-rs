//! RequestMatcher/MethodRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! A string argument is a list of one.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/MethodRequestMatcherTest.php
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

use symfony_http_foundation::request_matcher::method_request_matcher::MethodRequestMatcher;

#[test]
fn test() -> TestResult {
    let cases: [(&str, &[&str], bool); 18] = [
        ("get", &["get"], true),
        ("get", &["post,get"], true),
        ("get", &["post, get"], true),
        ("get", &["post,GET"], true),
        ("get", &["get", "post"], true),
        ("get", &["post"], false),
        ("get", &["GET"], true),
        ("get", &["GET", "POST"], true),
        ("get", &["POST"], false),
        // HEAD requests are served by the GET handler
        ("head", &["get"], true),
        ("head", &["get", "post"], true),
        ("head", &["post,get"], true),
        ("head", &["GET"], true),
        ("head", &["head"], true),
        ("head", &["post", "head"], true),
        ("head", &["post"], false),
        ("get", &["head"], false),
        ("post", &["get"], false),
    ];
    for (request_method, matcher_method, is_match) in cases {
        let matcher = MethodRequestMatcher::new(matcher_method.iter().copied());
        let mut request = create("", request_method, &[], None)?;
        assert_eq!(
            is_match,
            matches(&matcher, &mut request)?,
            "{request_method} {matcher_method:?}"
        );
    }
    Ok(())
}

#[test]
fn testAlwaysMatchesOnEmptyMethod() -> TestResult {
    let matcher = MethodRequestMatcher::new([] as [&str; 0]);
    let mut request = create("https://example.com", "POST", &[], None)?;
    assert!(matches(&matcher, &mut request)?);
    Ok(())
}
