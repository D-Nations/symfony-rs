//! ServerBagTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Names and assertions are
//! PHPUnit's: `assertEquals()` on two arrays ignores key order, so those
//! compare sorted pairs; `assertSame()` compares in order.
//!
//! Upstream: symfony/http-foundation Tests/ServerBagTest.php
#![allow(non_snake_case)]

use php_value::{Key, PhpArray, Value, base64};
use symfony_http_foundation::server_bag::ServerBag;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn array(pairs: &[(&str, &str)]) -> PhpArray {
    pairs
        .iter()
        .map(|(k, v)| (Key::from(*k), Value::from(*v)))
        .collect()
}

/// `assertEquals($expected, $actual)` on two string-keyed arrays.
fn assert_equals(expected: &[(&str, &str)], actual: &PhpArray) {
    let mut expected: Vec<(Key, Value)> = array(expected)
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut actual: Vec<(Key, Value)> =
        actual.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    expected.sort_by(|a, b| a.0.cmp(&b.0));
    actual.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(expected, actual);
}

fn basic(credentials: &str) -> String {
    format!("Basic {}", base64::encode(credentials.as_bytes()))
}

/// `md5('secret')`.
const SECRET_MD5: &str = "5ebe2294ecd0e0f08eab7690d2a6ee69";

fn digest(separator: &str) -> String {
    format!(
        "Digest{separator}username=\"foo\", realm=\"acme\", nonce=\"{SECRET_MD5}\", uri=\"/protected, qop=\"auth\""
    )
}

#[test]
fn testShouldExtractHeadersFromServerArray() -> TestResult {
    let server = array(&[
        ("SOME_SERVER_VARIABLE", "value"),
        ("SOME_SERVER_VARIABLE2", "value"),
        ("ROOT", "value"),
        ("HTTP_CONTENT_TYPE", "text/html"),
        ("HTTP_CONTENT_LENGTH", "0"),
        ("HTTP_ETAG", "asdf"),
        ("PHP_AUTH_USER", "foo"),
        ("PHP_AUTH_PW", "bar"),
    ]);
    let mut bag = ServerBag::new(server);
    assert_equals(
        &[
            ("CONTENT_TYPE", "text/html"),
            ("CONTENT_LENGTH", "0"),
            ("ETAG", "asdf"),
            ("AUTHORIZATION", &basic("foo:bar")),
            ("PHP_AUTH_USER", "foo"),
            ("PHP_AUTH_PW", "bar"),
        ],
        &bag.get_headers()?,
    );
    Ok(())
}

#[test]
fn testHttpPasswordIsOptional() -> TestResult {
    let mut bag = ServerBag::new(array(&[("PHP_AUTH_USER", "foo")]));

    assert_equals(
        &[
            ("AUTHORIZATION", &basic("foo:")),
            ("PHP_AUTH_USER", "foo"),
            ("PHP_AUTH_PW", ""),
        ],
        &bag.get_headers()?,
    );
    Ok(())
}

#[test]
fn testHttpPasswordIsOptionalWhenPassedWithHttpPrefix() -> TestResult {
    let mut bag = ServerBag::new(array(&[("HTTP_PHP_AUTH_USER", "foo")]));

    assert_equals(
        &[("AUTHORIZATION", &basic("foo:")), ("PHP_AUTH_USER", "foo")],
        &bag.get_headers()?,
    );
    Ok(())
}

#[test]
fn testHttpBasicAuthWithPhpCgi() -> TestResult {
    let mut bag = ServerBag::new(array(&[("HTTP_AUTHORIZATION", &basic("foo:bar"))]));

    assert_equals(
        &[
            ("AUTHORIZATION", &basic("foo:bar")),
            ("PHP_AUTH_USER", "foo"),
            ("PHP_AUTH_PW", "bar"),
        ],
        &bag.get_headers()?,
    );
    Ok(())
}

#[test]
fn testHttpBasicAuthWithPhpCgiBogus() -> TestResult {
    let header = format!("Basic_{}", base64::encode(b"foo:bar"));
    let mut bag = ServerBag::new(array(&[("HTTP_AUTHORIZATION", &header)]));

    // Username and passwords should not be set as the header is bogus
    let headers = bag.get_headers()?;
    assert!(!headers.contains_key(&Key::from("PHP_AUTH_USER")));
    assert!(!headers.contains_key(&Key::from("PHP_AUTH_PW")));
    Ok(())
}

#[test]
fn testHttpBasicAuthWithPhpCgiRedirect() -> TestResult {
    let mut bag = ServerBag::new(array(&[(
        "REDIRECT_HTTP_AUTHORIZATION",
        &basic("username:pass:word"),
    )]));

    assert_equals(
        &[
            ("AUTHORIZATION", &basic("username:pass:word")),
            ("PHP_AUTH_USER", "username"),
            ("PHP_AUTH_PW", "pass:word"),
        ],
        &bag.get_headers()?,
    );
    Ok(())
}

#[test]
fn testHttpBasicAuthWithPhpCgiEmptyPassword() -> TestResult {
    let mut bag = ServerBag::new(array(&[("HTTP_AUTHORIZATION", &basic("foo:"))]));

    assert_equals(
        &[
            ("AUTHORIZATION", &basic("foo:")),
            ("PHP_AUTH_USER", "foo"),
            ("PHP_AUTH_PW", ""),
        ],
        &bag.get_headers()?,
    );
    Ok(())
}

#[test]
fn testHttpDigestAuthWithPhpCgi() -> TestResult {
    let digest = digest(" ");
    let mut bag = ServerBag::new(array(&[("HTTP_AUTHORIZATION", &digest)]));

    assert_equals(
        &[("AUTHORIZATION", &digest), ("PHP_AUTH_DIGEST", &digest)],
        &bag.get_headers()?,
    );
    Ok(())
}

#[test]
fn testHttpDigestAuthWithPhpCgiBogus() -> TestResult {
    let digest = digest("_");
    let mut bag = ServerBag::new(array(&[("HTTP_AUTHORIZATION", &digest)]));

    // Username and passwords should not be set as the header is bogus
    let headers = bag.get_headers()?;
    assert!(!headers.contains_key(&Key::from("PHP_AUTH_USER")));
    assert!(!headers.contains_key(&Key::from("PHP_AUTH_PW")));
    Ok(())
}

#[test]
fn testHttpDigestAuthWithPhpCgiRedirect() -> TestResult {
    let digest = digest(" ");
    let mut bag = ServerBag::new(array(&[("REDIRECT_HTTP_AUTHORIZATION", &digest)]));

    assert_equals(
        &[("AUTHORIZATION", &digest), ("PHP_AUTH_DIGEST", &digest)],
        &bag.get_headers()?,
    );
    Ok(())
}

#[test]
fn testOAuthBearerAuth() -> TestResult {
    let header_content = "Bearer L-yLEOr9zhmUYRkzN1jwwxwQ-PBNiKDc8dgfB4hTfvo";
    let mut bag = ServerBag::new(array(&[("HTTP_AUTHORIZATION", header_content)]));

    assert_equals(&[("AUTHORIZATION", header_content)], &bag.get_headers()?);
    Ok(())
}

#[test]
fn testOAuthBearerAuthWithRedirect() -> TestResult {
    let header_content = "Bearer L-yLEOr9zhmUYRkzN1jwwxwQ-PBNiKDc8dgfB4hTfvo";
    let mut bag = ServerBag::new(array(&[("REDIRECT_HTTP_AUTHORIZATION", header_content)]));

    assert_equals(&[("AUTHORIZATION", header_content)], &bag.get_headers()?);
    Ok(())
}

#[test]
fn testItDoesNotOverwriteTheAuthorizationHeaderIfItIsAlreadySet() -> TestResult {
    let header_content = "Bearer L-yLEOr9zhmUYRkzN1jwwxwQ-PBNiKDc8dgfB4hTfvo";
    let mut bag = ServerBag::new(array(&[
        ("PHP_AUTH_USER", "foo"),
        ("HTTP_AUTHORIZATION", header_content),
    ]));

    assert_equals(
        &[
            ("AUTHORIZATION", header_content),
            ("PHP_AUTH_USER", "foo"),
            ("PHP_AUTH_PW", ""),
        ],
        &bag.get_headers()?,
    );
    Ok(())
}

/// An HTTP request without content-type and content-length will result in
/// the variables $_SERVER['`CONTENT_TYPE`'] and $_SERVER['`CONTENT_LENGTH`']
/// containing an empty string in PHP.
#[test]
fn testRequestWithoutContentTypeAndContentLength() -> TestResult {
    let mut bag = ServerBag::new(array(&[
        ("CONTENT_TYPE", ""),
        ("CONTENT_LENGTH", ""),
        ("HTTP_USER_AGENT", "foo"),
    ]));

    assert_eq!(array(&[("USER_AGENT", "foo")]), bag.get_headers()?);
    Ok(())
}
