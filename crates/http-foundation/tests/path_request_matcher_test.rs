//! RequestMatcher/PathRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/PathRequestMatcherTest.php
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

use symfony_http_foundation::request_matcher::path_request_matcher::PathRequestMatcher;

#[test]
fn test() -> TestResult {
    for (regexp, expected) in [
        ("/admin/.*", true),
        ("/admin", true),
        ("^/admin/.*$", true),
        ("/blog/.*", false),
    ] {
        let matcher = PathRequestMatcher::new(regexp);
        let mut request = Request::create("/admin/foo")?;
        assert_eq!(expected, matches(&matcher, &mut request)?, "{regexp}");
    }
    Ok(())
}

#[test]
fn testWithLocaleIsNotSupported() -> TestResult {
    let matcher = PathRequestMatcher::new("^/{_locale}/login$");
    let mut request = Request::create("/en/login")?;
    request.set_locale("en");

    assert!(!matches(&matcher, &mut request)?);
    Ok(())
}

#[test]
fn testWithEncodedCharacters() -> TestResult {
    let matcher = PathRequestMatcher::new("^/admin/fo o*$");
    let mut request = Request::create("/admin/fo%20o")?;
    assert!(matches(&matcher, &mut request)?);
    Ok(())
}

#[test]
fn testWithEncodedControlCharacters() -> TestResult {
    let mut request = Request::create("/admin/fo%0Ao")?;
    assert!(matches(&PathRequestMatcher::new("^.*$"), &mut request)?);
    assert!(matches(&PathRequestMatcher::new("^/.*$"), &mut request)?);
    assert!(matches(
        &PathRequestMatcher::new("^/admin/.*$"),
        &mut request
    )?);
    Ok(())
}
