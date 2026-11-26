//! RequestMatcher/AttributesRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/AttributesRequestMatcherTest.php
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

use php_value::object::Closure;
use symfony_http_foundation::request_matcher::attributes_request_matcher::AttributesRequestMatcher;

#[test]
fn test() -> TestResult {
    for (name, key, regexp, expected) in [
        ("0", "foo", "foo_.*", true),
        ("1", "foo", "foo", true),
        ("2", "foo", "^foo_bar$", true),
        ("3", "foo", "babar", false),
        ("with-closure", "_controller", "babar", false),
    ] {
        let matcher = AttributesRequestMatcher::new([(key, regexp)]);
        let mut request = Request::create("/admin/foo")?;
        request
            .attributes_mut()
            .set("foo", Value::string("foo_bar"));
        request
            .attributes_mut()
            .set("_controller", Closure::object(|_| Value::string("foo")));

        assert_eq!(expected, matches(&matcher, &mut request)?, "{name}");
    }
    Ok(())
}
