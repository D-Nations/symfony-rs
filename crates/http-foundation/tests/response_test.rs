//! ResponseTest.php and its base ResponseTestCase.php at
//! symfony/http-foundation v8.1.11. Names and assertions are PHPUnit's.
//! The class is `time-sensitive`: each test runs on a mocked clock.
//!
//! PHP objects are handles; a Rust value is owned. Where a test keeps a
//! handle to an object the response holds (the cookie of
//! `testPrepareSetsCookiesSecure`), the assertion reads the response's
//! own. `sendContent()`'s `echo` is read from the recorded output in
//! place of `ob_start()`/`ob_get_clean()`. The `DefaultResponse`
//! subclass is `Response` itself (it overrides nothing).
//!
//! Upstream: symfony/http-foundation Tests/ResponseTest.php Tests/ResponseTestCase.php
#![allow(
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use std::{path::Path, sync::Arc};

use php_value::{
    PhpArray, PhpString, Value,
    clock::{self, MockClock, time},
    datetime::{DateTimeImmutable, TimeZone},
    object::StringableObject,
};
use symfony_http_foundation::{
    cookie::{Cookie, Options},
    exception::Class,
    header_bag::{Cookies, HeaderValues, ResponseHeaderBag},
    php::{Globals, OutputCall, set_globals, with_output},
    request::{Create, Request},
    response::{Response, status_text},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The mocked clock, and a fresh PHP request for the output.
fn clock() -> MockClock {
    set_globals(Globals::default());
    clock::mock(1_700_000_000)
}

fn s(text: &str) -> Option<PhpString> {
    Some(PhpString::from(text))
}

/// `new Response($content, $status, $headers)`.
fn new_response<const N: usize>(
    content: &str,
    status: i64,
    headers: [(&str, &str); N],
) -> Response {
    Response::new(
        Some(content),
        status,
        ResponseHeaderBag::new(headers).unwrap(),
    )
    .unwrap()
}

fn content_type(response: &Response) -> Option<PhpString> {
    response.headers().get("Content-Type", None)
}

fn create(uri: &str, method: &str) -> Request {
    Request::create_with(
        uri,
        Create {
            method: s(method),
            ..Create::default()
        },
    )
    .unwrap()
}

fn set_header(request: &mut Request, name: &str, value: &str) {
    request
        .headers_mut()
        .set(name.as_bytes(), value, true)
        .unwrap();
}

fn set(response: &mut Response, name: &str, value: impl Into<HeaderValues>) {
    response
        .headers_mut()
        .set(name.as_bytes(), value, true)
        .unwrap();
}

/// `createDateTimeNow()`.
fn date_now() -> DateTimeImmutable {
    DateTimeImmutable::from_timestamp(time(), 0, TimeZone::Utc)
}

fn one_hour_ago() -> DateTimeImmutable {
    DateTimeImmutable::from_timestamp(time() - 3600, 0, TimeZone::Utc)
}

fn one_hour_later() -> DateTimeImmutable {
    DateTimeImmutable::from_timestamp(time() + 3600, 0, TimeZone::Utc)
}

/// `->format(\DATE_RFC2822)`.
fn rfc2822(date: &DateTimeImmutable) -> String {
    date.format("D, d M Y H:i:s O")
}

fn echoed() -> Vec<PhpString> {
    with_output(|o| {
        o.calls
            .iter()
            .filter_map(|call| match call {
                OutputCall::Echo(text) => Some(text.clone()),
                _ => None,
            })
            .collect()
    })
}

fn vary(response: &Response) -> Vec<String> {
    response
        .get_vary()
        .iter()
        .map(|v| v.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn testToString() {
    let _clock = clock();
    let response = Response::default();
    let text = response.to_php_string().to_string_lossy().into_owned();
    let response: Vec<&str> = text.split("\r\n").collect();
    assert_eq!("HTTP/1.0 200 OK", response[0]);
    assert_eq!("Cache-Control: no-cache, private", response[1]);
}

#[test]
fn testClone() {
    let _clock = clock();
    let response = Response::default();
    let response_clone = response.clone();
    assert_eq!(response, response_clone);
}

#[test]
fn testSendHeaders() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    let pointer: *const Response = &raw const response;
    let headers = response.send_headers(None)?;
    assert!(std::ptr::eq(pointer, headers));
    Ok(())
}

#[test]
fn testSendInformationalResponse() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.send_headers(Some(103))?;

    // Informational responses must not override the main status code
    assert_eq!(200, response.get_status_code());

    response.send_headers(None)?;
    Ok(())
}

#[test]
fn testSend() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    let pointer: *const Response = &raw const response;
    let response_send = response.send(true)?;
    assert!(std::ptr::eq(pointer, response_send));
    Ok(())
}

#[test]
fn testGetCharset() {
    let _clock = clock();
    let mut response = Response::default();
    let charset_origin = "utf-8";
    response.set_charset(charset_origin);
    let charset = response.get_charset();
    assert_eq!(s(charset_origin).as_ref(), charset);
}

#[test]
fn testIsCacheable() -> TestResult {
    let _clock = clock();
    let response = Response::default();
    assert!(!response.is_cacheable()?);
    Ok(())
}

#[test]
fn testIsCacheableWithErrorCode() -> TestResult {
    let _clock = clock();
    let response = new_response("", 500, []);
    assert!(!response.is_cacheable()?);
    Ok(())
}

#[test]
fn testIsCacheableWithNoStoreDirective() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    set(&mut response, "cache-control", "private");
    assert!(!response.is_cacheable()?);
    Ok(())
}

#[test]
fn testIsCacheableWithSetTtl() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_ttl(10)?;
    assert!(response.is_cacheable()?);
    Ok(())
}

#[test]
fn testMustRevalidate() {
    let _clock = clock();
    let response = Response::default();
    assert!(!response.must_revalidate());
}

#[test]
fn testMustRevalidateWithMustRevalidateCacheControlHeader() {
    let _clock = clock();
    let mut response = Response::default();
    set(&mut response, "cache-control", "must-revalidate");

    assert!(response.must_revalidate());
}

#[test]
fn testMustRevalidateWithProxyRevalidateCacheControlHeader() {
    let _clock = clock();
    let mut response = Response::default();
    set(&mut response, "cache-control", "proxy-revalidate");

    assert!(response.must_revalidate());
}

#[test]
fn testSetNotModified() {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    let pointer: *const Response = &raw const response;
    let modified = response.set_not_modified();
    assert!(std::ptr::eq(pointer, modified));
    assert_eq!(304, modified.get_status_code());

    modified.send_content();
    let string = echoed();
    assert_eq!(vec![PhpString::default()], string);
}

#[test]
fn testIsSuccessful() {
    let _clock = clock();
    let response = Response::default();
    assert!(response.is_successful());
}

#[test]
fn testIsNotModified() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    let modified = response.is_not_modified(&mut Request::default())?;
    assert!(!modified);
    Ok(())
}

#[test]
fn testIsNotModifiedNotSafe() -> TestResult {
    let _clock = clock();
    let mut request = create("/homepage", "POST");

    let mut response = Response::default();
    assert!(!response.is_not_modified(&mut request)?);
    Ok(())
}

#[test]
fn testIsNotModifiedLastModified() -> TestResult {
    let _clock = clock();
    let before = "Sun, 25 Aug 2013 18:32:31 GMT";
    let modified = "Sun, 25 Aug 2013 18:33:31 GMT";
    let after = "Sun, 25 Aug 2013 19:33:31 GMT";

    let mut request = Request::default();
    set_header(&mut request, "If-Modified-Since", modified);

    let mut response = Response::default();

    set(&mut response, "Last-Modified", modified);
    assert!(response.is_not_modified(&mut request)?);

    set(&mut response, "Last-Modified", before);
    assert!(response.is_not_modified(&mut request)?);

    set(&mut response, "Last-Modified", after);
    assert!(!response.is_not_modified(&mut request)?);

    set(&mut response, "Last-Modified", "");
    assert!(!response.is_not_modified(&mut request)?);
    Ok(())
}

#[test]
fn testIsNotModifiedEtag() -> TestResult {
    let _clock = clock();
    let etag_one = "randomly_generated_etag";
    let etag_two = "randomly_generated_etag_2";

    let mut request = Request::default();
    set_header(
        &mut request,
        "If-None-Match",
        &format!("{etag_one}, {etag_two}, etagThree"),
    );

    let mut response = Response::default();

    set(&mut response, "ETag", etag_one);
    assert!(response.is_not_modified(&mut request)?);

    set(&mut response, "ETag", etag_two);
    assert!(response.is_not_modified(&mut request)?);

    set(&mut response, "ETag", "");
    assert!(!response.is_not_modified(&mut request)?);

    // Test wildcard
    let mut request = Request::default();
    set_header(&mut request, "If-None-Match", "*");

    set(&mut response, "ETag", etag_one);
    assert!(response.is_not_modified(&mut request)?);
    Ok(())
}

#[test]
fn testIsNotModifiedWeakEtag() -> TestResult {
    let _clock = clock();
    let etag = "randomly_generated_etag";
    let weak_etag = "W/randomly_generated_etag";

    let mut request = Request::default();
    set_header(&mut request, "If-None-Match", etag);
    let mut response = Response::default();

    set(&mut response, "ETag", etag);
    assert!(response.is_not_modified(&mut request)?);

    set(&mut response, "ETag", weak_etag);
    assert!(response.is_not_modified(&mut request)?);

    set_header(&mut request, "If-None-Match", weak_etag);
    let mut response = Response::default();

    set(&mut response, "ETag", etag);
    assert!(response.is_not_modified(&mut request)?);

    set(&mut response, "ETag", weak_etag);
    assert!(response.is_not_modified(&mut request)?);
    Ok(())
}

#[test]
fn testIsNotModifiedLastModifiedAndEtag() -> TestResult {
    let _clock = clock();
    let before = "Sun, 25 Aug 2013 18:32:31 GMT";
    let modified = "Sun, 25 Aug 2013 18:33:31 GMT";
    let after = "Sun, 25 Aug 2013 19:33:31 GMT";
    let etag = "randomly_generated_etag";

    let mut request = Request::default();
    set_header(&mut request, "If-None-Match", &format!("{etag}, etagThree"));
    set_header(&mut request, "If-Modified-Since", modified);

    let mut response = Response::default();

    set(&mut response, "ETag", etag);
    set(&mut response, "Last-Modified", after);
    assert!(response.is_not_modified(&mut request)?);

    set(&mut response, "ETag", "non-existent-etag");
    set(&mut response, "Last-Modified", before);
    assert!(!response.is_not_modified(&mut request)?);

    set(&mut response, "ETag", etag);
    set(&mut response, "Last-Modified", modified);
    assert!(response.is_not_modified(&mut request)?);
    Ok(())
}

#[test]
fn testIsNotModifiedIfModifiedSinceAndEtagWithoutLastModified() -> TestResult {
    let _clock = clock();
    let modified = "Sun, 25 Aug 2013 18:33:31 GMT";
    let etag = "randomly_generated_etag";

    let mut request = Request::default();
    set_header(&mut request, "If-None-Match", &format!("{etag}, etagThree"));
    set_header(&mut request, "If-Modified-Since", modified);

    let mut response = Response::default();

    set(&mut response, "ETag", etag);
    assert!(response.is_not_modified(&mut request)?);

    set(&mut response, "ETag", "non-existent-etag");
    assert!(!response.is_not_modified(&mut request)?);
    Ok(())
}

#[test]
fn testIfNoneMatchWithoutETag() -> TestResult {
    let _clock = clock();
    let mut request = Request::default();
    set_header(&mut request, "If-None-Match", "randomly_generated_etag");

    assert!(!Response::default().is_not_modified(&mut request)?);

    // Test wildcard
    let mut request = Request::default();
    set_header(&mut request, "If-None-Match", "*");

    assert!(!Response::default().is_not_modified(&mut request)?);
    Ok(())
}

#[test]
fn testIsValidateable() {
    let _clock = clock();
    let response = new_response("", 200, [("Last-Modified", &rfc2822(&one_hour_ago()))]);
    assert!(
        response.is_validateable(),
        "->isValidateable() returns true if Last-Modified is present"
    );

    let response = new_response("", 200, [("ETag", "\"12345\"")]);
    assert!(
        response.is_validateable(),
        "->isValidateable() returns true if ETag is present"
    );

    let response = Response::default();
    assert!(
        !response.is_validateable(),
        "->isValidateable() returns false when no validator is present"
    );
}

#[test]
fn testGetDate() -> TestResult {
    let _clock = clock();
    let one_hour_ago = one_hour_ago();
    let response = new_response("", 200, [("Date", &rfc2822(&one_hour_ago))]);
    let date = response.get_date()?.unwrap();
    assert_eq!(
        one_hour_ago.timestamp(),
        date.timestamp(),
        "->getDate() returns the Date header if present"
    );

    let response = Response::default();
    let date = response.get_date()?.unwrap();
    assert_eq!(
        time(),
        date.timestamp(),
        "->getDate() returns the current Date if no Date header present"
    );

    let mut response = response_with_date();
    let now = date_now();
    set(&mut response, "Date", rfc2822(&now).as_str());
    let date = response.get_date()?.unwrap();
    assert_eq!(
        now.timestamp(),
        date.timestamp(),
        "->getDate() returns the date when the header has been modified"
    );

    let mut response = new_response("", 200, []);
    let now = date_now();
    response.headers_mut().remove("Date");
    let date = response.get_date()?.unwrap();
    assert_eq!(
        now.timestamp(),
        date.timestamp(),
        "->getDate() returns the current Date when the header has previously been removed"
    );
    Ok(())
}

fn response_with_date() -> Response {
    new_response("", 200, [("Date", &rfc2822(&one_hour_ago()))])
}

#[test]
fn testGetMaxAge() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    set(&mut response, "Cache-Control", "s-maxage=600, max-age=0");
    assert_eq!(
        Some(600),
        response.get_max_age()?,
        "->getMaxAge() uses s-maxage cache control directive when present"
    );

    let mut response = Response::default();
    set(&mut response, "Cache-Control", "max-age=600");
    assert_eq!(
        Some(600),
        response.get_max_age()?,
        "->getMaxAge() falls back to max-age when no s-maxage directive present"
    );

    let mut response = Response::default();
    set(&mut response, "Cache-Control", "must-revalidate");
    set(
        &mut response,
        "Expires",
        rfc2822(&one_hour_later()).as_str(),
    );
    assert_eq!(
        Some(3600),
        response.get_max_age()?,
        "->getMaxAge() falls back to Expires when no max-age or s-maxage directive present"
    );

    let mut response = Response::default();
    set(&mut response, "Expires", "-1");
    assert_eq!(Some(0), response.get_max_age()?);

    let response = Response::default();
    assert_eq!(
        None,
        response.get_max_age()?,
        "->getMaxAge() returns null if no freshness information available"
    );
    Ok(())
}

#[test]
fn testSetSharedMaxAge() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_shared_max_age(20)?;

    let cache_control = response.headers().get("Cache-Control", None);
    assert_eq!(s("public, s-maxage=20"), cache_control);
    Ok(())
}

#[test]
fn testSetStaleIfError() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_shared_max_age(20)?;
    response.set_stale_if_error(86400)?;

    let cache_control = response.headers().get("Cache-Control", None);
    assert_eq!(
        s("public, s-maxage=20, stale-if-error=86400"),
        cache_control
    );
    Ok(())
}

#[test]
fn testSetStaleWhileRevalidate() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_shared_max_age(20)?;
    response.set_stale_while_revalidate(300)?;

    let cache_control = response.headers().get("Cache-Control", None);
    assert_eq!(
        s("public, s-maxage=20, stale-while-revalidate=300"),
        cache_control
    );
    Ok(())
}

#[test]
fn testSetStaleIfErrorWithoutSharedMaxAge() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_stale_if_error(86400)?;

    let cache_control = response.headers().get("Cache-Control", None);
    assert_eq!(s("stale-if-error=86400, private"), cache_control);
    Ok(())
}

#[test]
fn testSetStaleWhileRevalidateWithoutSharedMaxAge() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_stale_while_revalidate(300)?;

    let cache_control = response.headers().get("Cache-Control", None);
    assert_eq!(s("stale-while-revalidate=300, private"), cache_control);
    Ok(())
}

#[test]
fn testIsPrivate() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    set(&mut response, "Cache-Control", "max-age=100");
    response.set_private()?;
    assert_eq!(
        Value::string("100"),
        response.headers().get_cache_control_directive("max-age"),
        "->isPrivate() adds the private Cache-Control directive when set to true"
    );
    assert!(
        matches!(
            response.headers().get_cache_control_directive("private"),
            Value::Bool(true)
        ),
        "->isPrivate() adds the private Cache-Control directive when set to true"
    );

    let mut response = Response::default();
    set(&mut response, "Cache-Control", "public, max-age=100");
    response.set_private()?;
    assert_eq!(
        Value::string("100"),
        response.headers().get_cache_control_directive("max-age"),
        "->isPrivate() adds the private Cache-Control directive when set to true"
    );
    assert!(
        matches!(
            response.headers().get_cache_control_directive("private"),
            Value::Bool(true)
        ),
        "->isPrivate() adds the private Cache-Control directive when set to true"
    );
    assert!(
        !response.headers().has_cache_control_directive("public"),
        "->isPrivate() removes the public Cache-Control directive"
    );
    Ok(())
}

#[test]
fn testExpire() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    set(&mut response, "Cache-Control", "max-age=100");
    response.expire()?;
    assert_eq!(
        s("100"),
        response.headers().get("Age", None),
        "->expire() sets the Age to max-age when present"
    );

    let mut response = Response::default();
    set(&mut response, "Cache-Control", "max-age=100, s-maxage=500");
    response.expire()?;
    assert_eq!(
        s("500"),
        response.headers().get("Age", None),
        "->expire() sets the Age to s-maxage when both max-age and s-maxage are present"
    );

    let mut response = Response::default();
    set(&mut response, "Cache-Control", "max-age=5, s-maxage=500");
    set(&mut response, "Age", "1000");
    response.expire()?;
    assert_eq!(
        s("1000"),
        response.headers().get("Age", None),
        "->expire() does nothing when the response is already stale/expired"
    );

    let mut response = Response::default();
    response.expire()?;
    assert!(
        !response.headers().has("Age"),
        "->expire() does nothing when the response does not include freshness information"
    );

    let mut response = Response::default();
    set(&mut response, "Expires", "-1");
    response.expire()?;
    assert_eq!(
        None,
        response.headers().get("Age", None),
        "->expire() does not set the Age when the response is expired"
    );

    let mut response = Response::default();
    set(
        &mut response,
        "Expires",
        rfc2822(&DateTimeImmutable::from_timestamp(
            time() + 600,
            0,
            TimeZone::Utc,
        ))
        .as_str(),
    );
    response.expire()?;
    assert_eq!(
        None,
        response.headers().get("Expires", None),
        "->expire() removes the Expires header when the response is fresh"
    );
    Ok(())
}

#[test]
fn testNullExpireHeader() -> TestResult {
    let _clock = clock();
    let response = Response::new(
        None::<PhpString>,
        200,
        ResponseHeaderBag::new([("Expires", None::<&str>)])?,
    )?;
    assert!(response.get_expires()?.is_none());
    Ok(())
}

#[test]
fn testGetTtl() -> TestResult {
    let _clock = clock();
    let response = Response::default();
    assert_eq!(
        None,
        response.get_ttl()?,
        "->getTtl() returns null when no Expires or Cache-Control headers are present"
    );

    let mut response = Response::default();
    set(
        &mut response,
        "Expires",
        rfc2822(&one_hour_later()).as_str(),
    );
    assert_eq!(
        Some(3600),
        response.get_ttl()?,
        "->getTtl() uses the Expires header when no max-age is present"
    );

    let mut response = Response::default();
    set(&mut response, "Expires", rfc2822(&one_hour_ago()).as_str());
    assert_eq!(
        Some(0),
        response.get_ttl()?,
        "->getTtl() returns zero when Expires is in past"
    );

    let mut response = Response::default();
    let date = response.get_date()?.unwrap();
    set(&mut response, "Expires", rfc2822(&date).as_str());
    set(&mut response, "Age", "0");
    assert_eq!(
        Some(0),
        response.get_ttl()?,
        "->getTtl() correctly handles zero"
    );

    let mut response = Response::default();
    set(&mut response, "Cache-Control", "max-age=60");
    assert_eq!(
        Some(60),
        response.get_ttl()?,
        "->getTtl() uses Cache-Control max-age when present"
    );
    Ok(())
}

#[test]
fn testSetClientTtl() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_client_ttl(10)?;

    assert_eq!(response.get_max_age()?, Some(response.get_age()? + 10));
    Ok(())
}

#[test]
fn testGetSetProtocolVersion() {
    let _clock = clock();
    let mut response = Response::default();

    assert_eq!(&PhpString::from("1.0"), response.get_protocol_version());

    response.set_protocol_version("1.1");

    assert_eq!(&PhpString::from("1.1"), response.get_protocol_version());
}

#[test]
fn testGetVary() {
    let _clock = clock();
    let response = Response::default();
    assert!(
        vary(&response).is_empty(),
        "->getVary() returns an empty array if no Vary header is present"
    );

    let mut response = Response::default();
    set(&mut response, "Vary", "Accept-Language");
    assert_eq!(
        vec!["Accept-Language"],
        vary(&response),
        "->getVary() parses a single header name value"
    );

    let mut response = Response::default();
    set(&mut response, "Vary", "Accept-Language User-Agent    X-Foo");
    assert_eq!(
        vec!["Accept-Language", "User-Agent", "X-Foo"],
        vary(&response),
        "->getVary() parses multiple header name values separated by spaces"
    );

    let mut response = Response::default();
    set(
        &mut response,
        "Vary",
        "Accept-Language,User-Agent,    X-Foo",
    );
    assert_eq!(
        vec!["Accept-Language", "User-Agent", "X-Foo"],
        vary(&response),
        "->getVary() parses multiple header name values separated by commas"
    );

    let expected = vec!["Accept-Language", "User-Agent", "X-foo"];

    let mut response = Response::default();
    set(&mut response, "Vary", expected.clone());
    assert_eq!(
        expected,
        vary(&response),
        "->getVary() parses multiple header name values in arrays"
    );

    let mut response = Response::default();
    set(&mut response, "Vary", "Accept-Language, User-Agent, X-foo");
    assert_eq!(
        expected,
        vary(&response),
        "->getVary() parses multiple header name values in arrays"
    );
}

#[test]
fn testSetVary() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_vary("Accept-Language", true)?;
    assert_eq!(vec!["Accept-Language"], vary(&response));

    response.set_vary("Accept-Language, User-Agent", true)?;
    assert_eq!(
        vec!["Accept-Language", "User-Agent"],
        vary(&response),
        "->setVary() replace the vary header by default"
    );

    response.set_vary("X-Foo", false)?;
    assert_eq!(
        vec!["Accept-Language", "User-Agent", "X-Foo"],
        vary(&response),
        "->setVary() doesn't wipe out earlier Vary headers if replace is set to false"
    );
    Ok(())
}

#[test]
fn testDefaultContentType() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    response.prepare(&mut Request::default())?;

    assert_eq!(s("text/html; charset=utf-8"), content_type(&response));
    Ok(())
}

#[test]
fn testContentTypeCharset() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    set(&mut response, "Content-Type", "text/css");

    // force fixContentType() to be called
    response.prepare(&mut Request::default())?;

    assert_eq!(s("text/css; charset=utf-8"), content_type(&response));
    Ok(())
}

#[test]
fn testContentTypeIsNull() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    set(&mut response, "Content-Type", None::<&str>);

    response.prepare(&mut Request::default())?;
    Ok(())
}

#[test]
fn testPrepareDoesNothingIfContentTypeIsSet() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    set(&mut response, "Content-Type", "text/plain");

    response.prepare(&mut Request::default())?;

    assert_eq!(
        s("text/plain; charset=utf-8"),
        response.headers().get("content-type", None)
    );
    Ok(())
}

#[test]
fn testPrepareDoesNothingIfRequestFormatIsNotDefined() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);

    response.prepare(&mut Request::default())?;

    assert_eq!(
        s("text/html; charset=utf-8"),
        response.headers().get("content-type", None)
    );
    Ok(())
}

/// Same URL cannot produce different Content-Type based on the value of
/// the Accept header, unless explicitly stated in the response object.
#[test]
fn testPrepareDoesNotSetContentTypeBasedOnRequestAcceptHeader() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    let mut request = Request::create("/")?;
    set_header(&mut request, "Accept", "application/json");
    response.prepare(&mut request)?;

    assert_eq!(
        s("text/html; charset=utf-8"),
        response.headers().get("content-type", None)
    );
    Ok(())
}

#[test]
fn testPrepareSetContentType() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    let mut request = Request::create("/")?;
    request.set_request_format(s("json"));

    response.prepare(&mut request)?;

    assert_eq!(
        s("application/json"),
        response.headers().get("content-type", None)
    );
    Ok(())
}

#[test]
fn testPrepareRemovesContentForHeadRequests() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    let mut request = create("/", "HEAD");

    let length = "12345";
    set(&mut response, "Content-Length", length);
    response.prepare(&mut request)?;

    assert_eq!(&PhpString::default(), response.get_content());
    assert_eq!(
        s(length),
        response.headers().get("Content-Length", None),
        "Content-Length should be as if it was GET; see RFC2616 14.13"
    );
    Ok(())
}

#[test]
fn testPrepareRemovesContentForInformationalResponse() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    let mut request = Request::create("/")?;

    response.set_content(s("content"));
    response.set_status_code(101, None)?;
    response.prepare(&mut request)?;
    assert_eq!(&PhpString::default(), response.get_content());
    assert!(!response.headers().has("Content-Type"));

    response.set_content(s("content"));
    response.set_status_code(304, None)?;
    response.prepare(&mut request)?;
    assert_eq!(&PhpString::default(), response.get_content());
    assert!(!response.headers().has("Content-Type"));
    assert!(!response.headers().has("Content-Length"));
    Ok(())
}

#[test]
fn testPrepareRemovesContentLength() -> TestResult {
    let _clock = clock();
    let mut response = new_response("foo", 200, []);
    let mut request = Request::create("/")?;

    set(&mut response, "Content-Length", "12345");
    response.prepare(&mut request)?;
    assert_eq!(s("12345"), response.headers().get("Content-Length", None));

    set(&mut response, "Transfer-Encoding", "chunked");
    response.prepare(&mut request)?;
    assert!(!response.headers().has("Content-Length"));
    Ok(())
}

#[test]
fn testPrepareSetsPragmaOnHttp10Only() -> TestResult {
    let _clock = clock();
    let mut request = create("/", "GET");
    request
        .server_mut()
        .set("SERVER_PROTOCOL", Value::string("HTTP/1.0"));

    let mut response = new_response("foo", 200, []);
    response.prepare(&mut request)?;
    assert_eq!(s("no-cache"), response.headers().get("pragma", None));
    assert_eq!(s("-1"), response.headers().get("expires", None));

    let mut response = new_response("foo", 200, []);
    response.headers_mut().remove("cache-control");
    response.prepare(&mut request)?;
    assert!(!response.headers().has("pragma"));
    assert!(!response.headers().has("expires"));

    request
        .server_mut()
        .set("SERVER_PROTOCOL", Value::string("HTTP/1.1"));
    let mut response = new_response("foo", 200, []);
    response.prepare(&mut request)?;
    assert!(!response.headers().has("pragma"));
    assert!(!response.headers().has("expires"));
    Ok(())
}

/// The response's cookie (the one `setCookie()` stored), read back from
/// its header bag.
fn the_cookie(response: &Response) -> Cookie {
    match response
        .headers()
        .get_cookies(ResponseHeaderBag::COOKIES_FLAT)
        .unwrap()
    {
        Cookies::Flat(cookies) => cookies.into_iter().next().unwrap(),
        Cookies::Array(_) => unreachable!("flat"),
    }
}

#[test]
fn testPrepareSetsCookiesSecure() -> TestResult {
    let _clock = clock();
    let cookie = Cookie::create(
        "foo",
        Options {
            value: s("bar"),
            ..Options::default()
        },
    )?;

    let mut response = new_response("foo", 200, []);
    response.headers_mut().set_cookie(cookie);

    let mut request = create("/", "GET");
    response.prepare(&mut request)?;

    assert!(!the_cookie(&response).is_secure());

    let mut request = create("https://localhost/", "GET");
    response.prepare(&mut request)?;

    assert!(the_cookie(&response).is_secure());
    Ok(())
}

fn options(pairs: Vec<(&str, Value)>) -> PhpArray {
    let mut options = PhpArray::new();
    for (key, value) in pairs {
        options.insert(key, value);
    }
    options
}

#[test]
fn testSetCache() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    // ['etag', 'last_modified', 'max_age', 's_maxage', 'private', 'public']
    match response.set_cache(&options(vec![("wrong option", Value::string("value"))])) {
        Ok(_) => {
            panic!("->setCache() throws an InvalidArgumentException if an option is not supported")
        }
        Err(e) => {
            assert_eq!(
                Class::PhpInvalidArgument,
                e.class(),
                "->setCache() throws an InvalidArgumentException if an option is not supported"
            );
            assert!(e.message().to_string_lossy().contains("\"wrong option\""));
        }
    }

    let options_etag = options(vec![("etag", Value::string("\"whatever\""))]);
    response.set_cache(&options_etag)?;
    assert_eq!(s("\"whatever\""), response.get_etag());

    let now = date_now();
    response.set_cache(&options(vec![(
        "last_modified",
        Value::Object(Arc::new(now)),
    )]))?;
    assert_eq!(
        now.timestamp(),
        response.get_last_modified()?.unwrap().timestamp()
    );

    response.set_cache(&options(vec![("max_age", Value::Int(100))]))?;
    assert_eq!(Some(100), response.get_max_age()?);

    response.set_cache(&options(vec![("s_maxage", Value::Int(200))]))?;
    assert_eq!(Some(200), response.get_max_age()?);

    assert!(response.headers().has_cache_control_directive("public"));
    assert!(!response.headers().has_cache_control_directive("private"));

    response.set_cache(&options(vec![("public", Value::Bool(true))]))?;
    assert!(response.headers().has_cache_control_directive("public"));
    assert!(!response.headers().has_cache_control_directive("private"));

    response.set_cache(&options(vec![("public", Value::Bool(false))]))?;
    assert!(!response.headers().has_cache_control_directive("public"));
    assert!(response.headers().has_cache_control_directive("private"));

    response.set_cache(&options(vec![("private", Value::Bool(true))]))?;
    assert!(!response.headers().has_cache_control_directive("public"));
    assert!(response.headers().has_cache_control_directive("private"));

    response.set_cache(&options(vec![("private", Value::Bool(false))]))?;
    assert!(response.headers().has_cache_control_directive("public"));
    assert!(!response.headers().has_cache_control_directive("private"));

    response.set_cache(&options(vec![("immutable", Value::Bool(true))]))?;
    assert!(response.headers().has_cache_control_directive("immutable"));

    response.set_cache(&options(vec![("immutable", Value::Bool(false))]))?;
    assert!(!response.headers().has_cache_control_directive("immutable"));

    let directives = [
        "proxy_revalidate",
        "must_revalidate",
        "no_cache",
        "no_store",
        "no_transform",
    ];
    for directive in directives {
        response.set_cache(&options(vec![(directive, Value::Bool(true))]))?;

        assert!(
            response
                .headers()
                .has_cache_control_directive(&directive.replace('_', "-"))
        );
    }

    for directive in directives {
        response.set_cache(&options(vec![(directive, Value::Bool(false))]))?;

        assert!(
            !response
                .headers()
                .has_cache_control_directive(&directive.replace('_', "-"))
        );
    }

    // new DefaultResponse()
    let mut response = Response::default();

    response.set_cache(&options_etag)?;
    assert_eq!(response.get_etag(), s("\"whatever\""));
    Ok(())
}

#[test]
fn testSendContent() {
    let _clock = clock();
    let mut response = new_response("test response rendering", 200, []);

    response.send_content();
    let string = echoed().concat();
    assert!(string.windows(23).any(|w| w == b"test response rendering"));
}

#[test]
fn testSetPublic() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_public()?;

    assert!(response.headers().has_cache_control_directive("public"));
    assert!(!response.headers().has_cache_control_directive("private"));
    Ok(())
}

#[test]
fn testSetImmutable() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_immutable(true)?;

    assert!(response.headers().has_cache_control_directive("immutable"));
    Ok(())
}

#[test]
fn testIsImmutable() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_immutable(true)?;

    assert!(response.is_immutable());
    Ok(())
}

/// `\DateTime::createFromFormat(\DateTimeInterface::ATOM,
/// '2013-01-26T09:21:56+0100', new \DateTimeZone('Europe/Berlin'))`: the
/// offset in the text wins over the zone argument.
fn berlin() -> DateTimeImmutable {
    DateTimeImmutable::from_fields(
        php_value::datetime::Fields {
            year: 2013,
            month: 1,
            day: 26,
            hour: 9,
            minute: 21,
            second: 56,
            microsecond: 0,
        },
        TimeZone::Offset(3600),
    )
}

#[test]
fn testSetDate() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_date(&berlin())?;

    assert_eq!(
        "2013-01-26T08:21:56+00:00",
        response.get_date()?.unwrap().format("Y-m-d\\TH:i:sP")
    );
    Ok(())
}

#[test]
fn testSetDateWithImmutable() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_date(&berlin())?;

    assert_eq!(
        "2013-01-26T08:21:56+00:00",
        response.get_date()?.unwrap().format("Y-m-d\\TH:i:sP")
    );
    Ok(())
}

#[test]
fn testSetExpires() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_expires(None)?;

    assert!(
        response.get_expires()?.is_none(),
        "->setExpires() remove the header when passed null"
    );

    let now = date_now();
    response.set_expires(Some(&now))?;

    assert_eq!(
        response.get_expires()?.unwrap().timestamp(),
        now.timestamp()
    );
    Ok(())
}

#[test]
fn testSetExpiresWithImmutable() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();

    let now = date_now();
    response.set_expires(Some(&now))?;

    assert_eq!(
        response.get_expires()?.unwrap().timestamp(),
        now.timestamp()
    );
    Ok(())
}

#[test]
fn testSetLastModified() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_last_modified(Some(&date_now()))?;
    assert!(response.get_last_modified()?.is_some());

    response.set_last_modified(None)?;
    assert!(response.get_last_modified()?.is_none());
    Ok(())
}

#[test]
fn testSetLastModifiedWithImmutable() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    response.set_last_modified(Some(&date_now()))?;
    assert!(response.get_last_modified()?.is_some());

    response.set_last_modified(None)?;
    assert!(response.get_last_modified()?.is_none());
    Ok(())
}

#[test]
fn testIsInvalid() {
    let _clock = clock();
    let mut response = Response::default();

    match response.set_status_code(99, None) {
        Ok(_) => panic!("fail"),
        Err(e) => {
            assert_eq!(Class::PhpInvalidArgument, e.class());
            assert!(response.is_invalid());
        }
    }

    match response.set_status_code(650, None) {
        Ok(_) => panic!("fail"),
        Err(e) => {
            assert_eq!(Class::PhpInvalidArgument, e.class());
            assert!(response.is_invalid());
        }
    }

    let response = new_response("", 200, []);
    assert!(!response.is_invalid());
}

/// `getStatusCodeFixtures()`; the codes are PHP strings passed to an
/// `int` parameter.
#[test]
fn testSetStatusCode() -> TestResult {
    let _clock = clock();
    let fixtures: [(i64, Option<&str>, &str); 6] = [
        (200, None, "OK"),
        (200, Some(""), ""),
        (200, Some("foo"), "foo"),
        (199, None, "unknown status"),
        (199, Some(""), ""),
        (199, Some("foo"), "foo"),
    ];
    for (code, text, expected_text) in fixtures {
        let mut response = Response::default();

        response.set_status_code(code, text.map(PhpString::from))?;

        let status_text = response.status_text();

        assert_eq!(&PhpString::from(expected_text), status_text);
    }
    Ok(())
}

#[test]
fn testIsInformational() {
    let _clock = clock();
    let response = new_response("", 100, []);
    assert!(response.is_informational());

    let response = new_response("", 200, []);
    assert!(!response.is_informational());
}

#[test]
fn testIsRedirectRedirection() {
    let _clock = clock();
    for code in [301, 302, 303, 307] {
        let response = new_response("", code, []);
        assert!(response.is_redirection());
        assert!(response.is_redirect(None));
    }

    let response = new_response("", 304, []);
    assert!(response.is_redirection());
    assert!(!response.is_redirect(None));

    let response = new_response("", 200, []);
    assert!(!response.is_redirection());
    assert!(!response.is_redirect(None));

    let response = new_response("", 404, []);
    assert!(!response.is_redirection());
    assert!(!response.is_redirect(None));

    let response = new_response("", 301, [("Location", "/good-uri")]);
    assert!(!response.is_redirect(Some(b"/bad-uri")));
    assert!(response.is_redirect(Some(b"/good-uri")));
}

#[test]
fn testIsNotFound() {
    let _clock = clock();
    let response = new_response("", 404, []);
    assert!(response.is_not_found());

    let response = new_response("", 200, []);
    assert!(!response.is_not_found());
}

#[test]
fn testIsEmpty() {
    let _clock = clock();
    for code in [204, 304] {
        let response = new_response("", code, []);
        assert!(response.is_empty());
    }

    let response = new_response("", 200, []);
    assert!(!response.is_empty());
}

#[test]
fn testIsForbidden() {
    let _clock = clock();
    let response = new_response("", 403, []);
    assert!(response.is_forbidden());

    let response = new_response("", 200, []);
    assert!(!response.is_forbidden());
}

#[test]
fn testIsOk() {
    let _clock = clock();
    let response = new_response("", 200, []);
    assert!(response.is_ok());

    let response = new_response("", 404, []);
    assert!(!response.is_ok());
}

#[test]
fn testIsServerOrClientError() {
    let _clock = clock();
    let response = new_response("", 404, []);
    assert!(response.is_client_error());
    assert!(!response.is_server_error());

    let response = new_response("", 500, []);
    assert!(!response.is_client_error());
    assert!(response.is_server_error());
}

#[test]
fn testHasVary() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    assert!(!response.has_vary());

    response.set_vary("User-Agent", true)?;
    assert!(response.has_vary());
    Ok(())
}

#[test]
fn testSetEtag() -> TestResult {
    let _clock = clock();
    let mut response = new_response("", 200, [("ETag", "\"12345\"")]);
    response.set_etag(None, false)?;

    assert_eq!(
        None,
        response.headers().get("Etag", None),
        "->setEtag() removes Etags when call with null"
    );
    Ok(())
}

/// `validContentProvider()`: PHP coerces the object and the int to the
/// `?string` parameter (no `strict_types`); here the caller does.
#[test]
fn testSetContent() {
    let _clock = clock();
    let rows = [
        (
            "obj",
            StringableObject::object(
                "Symfony\\Component\\HttpFoundation\\Tests\\StringableObject",
                "Foo",
            ),
        ),
        ("string", Value::string("Foo")),
        ("int", Value::Int(2)),
    ];
    for (name, content) in rows {
        let mut response = Response::default();
        response.set_content(content.to_php_string_or_stringable());
        assert_eq!(
            &content.to_php_string_or_stringable().unwrap(),
            response.get_content(),
            "{name}"
        );
    }
}

#[test]
fn testSettersAreChainable() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();
    let pointer: *const Response = &raw const response;
    let same = |r: &Response| std::ptr::eq(pointer, r);

    assert!(same(response.set_protocol_version("1.0")));
    assert!(same(response.set_charset("utf-8")));
    assert!(same(response.set_public()?));
    assert!(same(response.set_private()?));
    assert!(same(response.set_date(&date_now())?));
    assert!(same(response.expire()?));
    assert!(same(response.set_max_age(1)?));
    assert!(same(response.set_shared_max_age(1)?));
    assert!(same(response.set_ttl(1)?));
    assert!(same(response.set_client_ttl(1)?));
    Ok(())
}

/// Without the `DefaultResponse` subclass (a Rust struct has none):
/// `new Response()` triggers no deprecation.
#[test]
fn testNoDeprecationsAreTriggered() {
    let _clock = clock();
    let ((), deprecations) = symfony_deprecation_contracts::collect(|| {
        let _ = Response::default();
        let _ = Response::new(Some(""), 200, ResponseHeaderBag::empty());
    });

    // we just need to ensure that subclasses of Response can be created without any deprecations
    // being triggered if the subclass does not override any final methods
    assert_eq!(
        deprecations,
        [] as [symfony_deprecation_contracts::Deprecation; 0]
    );
}

/// `ianaCodesReasonPhrasesProvider()`: the records of upstream's
/// `Tests/Fixtures/xml/http-status-codes.xml`, ranges expanded, without
/// `Unassigned` and `(Unused)`. The RELAX NG validation is not repeated.
fn iana_codes_reason_phrases() -> Option<Vec<(i64, String)>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let path =
        root.join("upstream/symfony/http-foundation/Tests/Fixtures/xml/http-status-codes.xml");
    let xml = std::fs::read_to_string(path).ok()?;
    let element = |record: &str, name: &str| -> String {
        let open = format!("<{name}>");
        let start = record.find(&open).expect("element") + open.len();
        let end = record[start..].find(&format!("</{name}>")).expect("end") + start;
        let text = &record[start..end];
        assert!(!text.contains('&'), "entities are not expected: {text}");
        text.to_owned()
    };
    let mut phrases = Vec::new();
    for record in xml.split("<record").skip(1) {
        let record = &record[..record.find("</record>").expect("record end")];
        let value = element(record, "value");
        let description = element(record, "description");

        if ["Unassigned", "(Unused)"].contains(&description.as_str()) {
            continue;
        }

        if let Some((from, to)) = value.split_once('-') {
            let (from, to): (i64, i64) = (from.trim().parse().ok()?, to.trim().parse().ok()?);
            for value in from..=to {
                phrases.push((value, description.clone()));
            }
        } else {
            phrases.push((value.trim().parse().ok()?, description));
        }
    }
    Some(phrases)
}

#[test]
fn testReasonPhraseDefaultsAgainstIana() {
    let Some(phrases) = iana_codes_reason_phrases() else {
        eprintln!("skipped: the IANA fixture (run scripts/fetch-upstream.sh)");
        return;
    };
    assert_ne!(phrases, [] as [(i64, std::string::String); 0]);
    for (code, reason_phrase) in phrases {
        assert_eq!(Some(reason_phrase.as_str()), status_text(code), "{code}");
    }
}

#[test]
fn testSetContentSafe() -> TestResult {
    let _clock = clock();
    let mut response = Response::default();

    assert!(!response.headers().has("Preference-Applied"));
    assert!(!response.headers().has("Vary"));

    response.set_content_safe(true)?;

    assert_eq!(
        s("safe"),
        response.headers().get("Preference-Applied", None)
    );
    assert_eq!(s("Prefer"), response.headers().get("Vary", None));

    response.set_content_safe(false)?;

    assert!(!response.headers().has("Preference-Applied"));
    assert_eq!(s("Prefer"), response.headers().get("Vary", None));
    Ok(())
}

#[test]
fn testDirectHeadersWriteIsDeprecated() {
    let _clock = clock();
    let mut response = Response::default();

    let ((), deprecations) = symfony_deprecation_contracts::collect(|| {
        response.set_headers(ResponseHeaderBag::empty());
    });

    assert_eq!(
        vec![b"Since symfony/http-foundation 8.1: Directly setting property \"headers\" of \"Symfony\\Component\\HttpFoundation\\Response\" is deprecated; pass the header bag as a constructor argument instead.".to_vec()],
        deprecations.into_iter().map(|d| d.message).collect::<Vec<_>>()
    );
}

/// `ResponseTestCase::testNoCacheControlHeaderOnAttachmentUsingHTTPSAndMSIE()`
/// with `provideResponse()` giving `new Response()`.
#[test]
fn testNoCacheControlHeaderOnAttachmentUsingHTTPSAndMSIE() -> TestResult {
    let _clock = clock();
    let attachment = |request: &mut Request| -> Result<Response, Box<dyn std::error::Error>> {
        let mut response = Response::default();
        set(
            &mut response,
            "Content-Disposition",
            "attachment; filename=\"fname.ext\"",
        );
        response.prepare(request)?;
        Ok(response)
    };

    // Check for HTTPS and IE 8
    let mut request = Request::default();
    request.server_mut().set("HTTPS", Value::Bool(true));
    request.server_mut().set(
        "HTTP_USER_AGENT",
        Value::string("Mozilla/4.0 (compatible; MSIE 8.0; Windows NT 6.0; Trident/4.0)"),
    );

    let response = attachment(&mut request)?;
    assert!(!response.headers().has("Cache-Control"));

    // Check for IE 10 and HTTPS
    request.server_mut().set(
        "HTTP_USER_AGENT",
        Value::string("Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1; WOW64; Trident/6.0)"),
    );

    let response = attachment(&mut request)?;
    assert!(response.headers().has("Cache-Control"));

    // Check for IE 9 and HTTPS
    request.server_mut().set(
        "HTTP_USER_AGENT",
        Value::string("Mozilla/5.0 (compatible; MSIE 9.0; Windows NT 7.1; Trident/5.0)"),
    );

    let response = attachment(&mut request)?;
    assert!(response.headers().has("Cache-Control"));

    // Check for IE 9 and HTTP
    request.server_mut().set("HTTPS", Value::Bool(false));

    let response = attachment(&mut request)?;
    assert!(response.headers().has("Cache-Control"));

    // Check for IE 8 and HTTP
    request.server_mut().set(
        "HTTP_USER_AGENT",
        Value::string("Mozilla/4.0 (compatible; MSIE 8.0; Windows NT 6.0; Trident/4.0)"),
    );

    let response = attachment(&mut request)?;
    assert!(response.headers().has("Cache-Control"));

    // Check for non-IE and HTTPS
    request.server_mut().set("HTTPS", Value::Bool(true));
    request.server_mut().set(
        "HTTP_USER_AGENT",
        Value::string("Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/537.17 (KHTML, like Gecko) Chrome/24.0.1312.60 Safari/537.17"),
    );

    let response = attachment(&mut request)?;
    assert!(response.headers().has("Cache-Control"));

    // Check for non-IE and HTTP
    request.server_mut().set("HTTPS", Value::Bool(false));

    let response = attachment(&mut request)?;
    assert!(response.headers().has("Cache-Control"));
    Ok(())
}
