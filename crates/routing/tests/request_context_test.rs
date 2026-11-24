//! RequestContextTest.php at symfony/routing
//! f3fefc6cad3748d538be970ca81b71f06d7aa8b3. Names and assertions are
//! PHPUnit's; `#[TestWith]` cases run in a loop. `assertSame($context,
//! $context->setX())` (the fluent interface) compares the returned
//! reference with the context. `testPort` passes PHP's `'123'` and `'456'`
//! as the integers PHP's coercion makes of them before the call.
//! `testFromRequest` waits for HttpFoundation's Request, and
//! `testConstructParametersBcLayer` is a PHP subclass calling
//! `setParameters()` before the parent constructor (not applicable to a
//! Rust value); both are listed in the manifest.
//!
//! Upstream: symfony/routing Tests/RequestContextTest.php
#![allow(non_snake_case)]

use php_value::{PhpArray, PhpString, Value};
use symfony_routing::request_context::RequestContext;

fn s(text: &str) -> PhpString {
    PhpString::from(text)
}

fn from_uri(uri: &[u8]) -> RequestContext {
    RequestContext::from_uri(uri, b"localhost", b"http", 80, 443)
}

#[test]
fn testConstruct() {
    let parameters: PhpArray = [("foo", Value::string("bar"))].into_iter().collect();
    let request_context = RequestContext::new(
        "foo",
        "post",
        "foo.bar",
        "HTTPS",
        8080,
        444,
        "/baz",
        "bar=foobar",
        Some(parameters.clone()),
    );

    assert_eq!(s("foo"), *request_context.base_url());
    assert_eq!(s("POST"), *request_context.method());
    assert_eq!(s("foo.bar"), *request_context.host());
    assert_eq!(s("https"), *request_context.scheme());
    assert_eq!(8080, request_context.http_port());
    assert_eq!(444, request_context.https_port());
    assert_eq!(s("/baz"), *request_context.path_info());
    assert_eq!(s("bar=foobar"), *request_context.query_string());
    assert_eq!(parameters, *request_context.parameters());
}

#[test]
fn testFromUriWithBaseUrl() {
    let request_context = from_uri(b"https://test.com:444/index.php");

    assert_eq!(s("GET"), *request_context.method());
    assert_eq!(s("https"), *request_context.scheme());
    assert_eq!(s("test.com"), *request_context.host());
    assert_eq!(s("/index.php"), *request_context.base_url());
    assert_eq!(s("/"), *request_context.path_info());
    assert_eq!(80, request_context.http_port());
    assert_eq!(444, request_context.https_port());
}

#[test]
fn testFromUriWithTrailingSlash() {
    let request_context = from_uri(b"http://test.com:8080/");

    assert_eq!(s("http"), *request_context.scheme());
    assert_eq!(s("test.com"), *request_context.host());
    assert_eq!(8080, request_context.http_port());
    assert_eq!(443, request_context.https_port());
    assert_eq!(s(""), *request_context.base_url());
    assert_eq!(s("/"), *request_context.path_info());
}

#[test]
fn testFromUriWithoutTrailingSlash() {
    let request_context = from_uri(b"https://test.com");

    assert_eq!(s("https"), *request_context.scheme());
    assert_eq!(s("test.com"), *request_context.host());
    assert_eq!(s(""), *request_context.base_url());
    assert_eq!(s("/"), *request_context.path_info());
}

#[test]
fn testFromUriBeingEmpty() {
    let request_context = from_uri(b"");

    assert_eq!(s("http"), *request_context.scheme());
    assert_eq!(s("localhost"), *request_context.host());
    assert_eq!(s(""), *request_context.base_url());
    assert_eq!(s("/"), *request_context.path_info());
}

#[test]
fn testFromBadUri() {
    let cases: [&[u8]; 10] = [
        b"http://foo.com\\bar",
        b"\\\\foo.com/bar",
        b"a\rb",
        b"a\nb",
        b"a\tb",
        b"\0foo",
        b"foo\0",
        b" foo",
        b"foo ",
        b":",
    ];
    for uri in cases {
        let context = from_uri(uri);

        assert_eq!(s("http"), *context.scheme(), "{uri:?}");
        assert_eq!(s("localhost"), *context.host(), "{uri:?}");
        assert_eq!(s(""), *context.base_url(), "{uri:?}");
        assert_eq!(s("/"), *context.path_info(), "{uri:?}");
    }
}

#[test]
fn testGetParameters() {
    let mut request_context = RequestContext::default();
    assert_eq!(PhpArray::new(), *request_context.parameters());

    let parameters: PhpArray = [("foo", Value::string("bar"))].into_iter().collect();
    request_context.set_parameters(parameters.clone());
    assert_eq!(parameters, *request_context.parameters());
}

#[test]
fn testHasParameter() {
    let mut request_context = RequestContext::default();
    request_context.set_parameters([("foo", Value::string("bar"))].into_iter().collect());

    assert!(request_context.has_parameter(b"foo"));
    assert!(!request_context.has_parameter(b"baz"));
}

#[test]
fn testGetParameter() {
    let mut request_context = RequestContext::default();
    request_context.set_parameters([("foo", Value::string("bar"))].into_iter().collect());

    assert_eq!(
        Some(&Value::string("bar")),
        request_context.parameter(b"foo")
    );
    assert_eq!(None, request_context.parameter(b"baz"));
}

#[test]
fn testSetParameter() {
    let mut request_context = RequestContext::default();
    request_context.set_parameter(b"foo", Value::string("bar"));

    assert_eq!(
        Some(&Value::string("bar")),
        request_context.parameter(b"foo")
    );
}

#[test]
fn testMethod() {
    let mut request_context = RequestContext::default();
    request_context.set_method("post");

    assert_eq!(s("POST"), *request_context.method());
}

#[test]
fn testScheme() {
    let mut request_context = RequestContext::default();
    request_context.set_scheme("HTTPS");

    assert_eq!(s("https"), *request_context.scheme());
}

#[test]
fn testHost() {
    let mut request_context = RequestContext::default();
    request_context.set_host("eXampLe.com");

    assert_eq!(s("example.com"), *request_context.host());
}

#[test]
fn testQueryString() {
    let mut request_context = RequestContext::default();
    request_context.set_query_string(None::<&str>);

    assert_eq!(s(""), *request_context.query_string());
}

#[test]
fn testPort() {
    let mut request_context = RequestContext::default();
    request_context.set_http_port(123);
    request_context.set_https_port(456);

    assert_eq!(123, request_context.http_port());
    assert_eq!(456, request_context.https_port());
}

#[test]
fn testFluentInterface() {
    let mut request_context = RequestContext::default();
    let this: *const RequestContext = &request_context;

    assert!(std::ptr::eq(this, request_context.set_base_url("/app.php")));
    assert!(std::ptr::eq(this, request_context.set_path_info("/index")));
    assert!(std::ptr::eq(this, request_context.set_method("POST")));
    assert!(std::ptr::eq(this, request_context.set_scheme("https")));
    assert!(std::ptr::eq(this, request_context.set_host("example.com")));
    assert!(std::ptr::eq(
        this,
        request_context.set_query_string(Some("foo=bar"))
    ));
    assert!(std::ptr::eq(this, request_context.set_http_port(80)));
    assert!(std::ptr::eq(this, request_context.set_https_port(443)));
    assert!(std::ptr::eq(
        this,
        request_context.set_parameters(PhpArray::new())
    ));
    assert!(std::ptr::eq(
        this,
        request_context.set_parameter(b"foo", Value::string("bar"))
    ));
}
