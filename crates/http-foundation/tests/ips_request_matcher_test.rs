//! RequestMatcher/IpsRequestMatcherTest.php at symfony/http-foundation v8.1.11. Names
//! and assertions are PHPUnit's; each data set of a provider is a case
//! of the same test.
//!
//! A string argument is a list of one.
//!
//! Upstream: symfony/http-foundation Tests/RequestMatcher/IpsRequestMatcherTest.php
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

use symfony_http_foundation::request_matcher::ips_request_matcher::IpsRequestMatcher;

#[test]
fn test() -> TestResult {
    let cases: [(&[&str], bool); 12] = [
        (&[], true),
        (&["127.0.0.1"], true),
        (&["192.168.0.1"], false),
        (&["127.0.0.1"], true),
        (&["127.0.0.1, ::1"], true),
        (&["192.168.0.1, ::1"], false),
        (&["127.0.0.1", "::1"], true),
        (&["192.168.0.1", "::1"], false),
        (&["1.1.1.1", "2.2.2.2", "127.0.0.1, ::1"], true),
        (&["1.1.1.1", "2.2.2.2", "192.168.0.1, ::1"], false),
        (&["192.168.1.0/24"], false),
        (&["127.0.0.1/32"], true),
    ];
    for (ips, expected) in cases {
        let matcher = IpsRequestMatcher::new(ips.iter().copied());
        let mut request = create(
            "",
            "GET",
            &[("REMOTE_ADDR", Value::string("127.0.0.1"))],
            None,
        )?;
        assert_eq!(expected, matches(&matcher, &mut request)?, "{ips:?}");
    }
    Ok(())
}
