//! RedirectResponseTest.php at symfony/http-foundation v8.1.11. Names and
//! assertions are PHPUnit's; `expectException()` checks the class the
//! error carries.
//!
//! Upstream: symfony/http-foundation Tests/RedirectResponseTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_pcre::Pcre;
use php_value::PhpString;
use symfony_http_foundation::redirect_response::RedirectResponse;

const NO_HEADERS: [(&str, &str); 0] = [];

fn redirect(url: &str) -> RedirectResponse {
    RedirectResponse::new(url, 302, NO_HEADERS).unwrap()
}

#[test]
fn testGenerateMetaRedirect() {
    let response = redirect("foo.bar");

    let content = Pcre::new(br"/\s+/")
        .unwrap()
        .replace(response.get_content().as_bytes(), b" ")
        .unwrap();
    assert!(
        Pcre::new(br#"#<meta http-equiv="refresh" content="\d+;url='foo\.bar'" />#"#)
            .unwrap()
            .is_match(&content)
            .unwrap()
    );
}

#[test]
fn testRedirectResponseConstructorEmptyUrl() {
    let error = RedirectResponse::new("", 302, NO_HEADERS).unwrap_err();
    assert!(error.is_a("InvalidArgumentException"));
    assert_eq!(
        &PhpString::from("Cannot redirect to an empty URL."),
        error.message()
    );
}

#[test]
fn testRedirectResponseConstructorWrongStatusCode() {
    let error = RedirectResponse::new("foo.bar", 404, NO_HEADERS).unwrap_err();
    assert!(error.is_a("InvalidArgumentException"));
}

#[test]
fn testGenerateLocationHeader() {
    let response = redirect("foo.bar");

    assert!(response.headers().has("Location"));
    assert_eq!(
        Some(PhpString::from("foo.bar")),
        response.headers().get("Location", None)
    );
}

#[test]
fn testGenerateContentTypeHeader() {
    let response = redirect("foo.bar");

    assert_eq!(
        Some(PhpString::from("text/html; charset=utf-8")),
        response.headers().get("Content-Type", None)
    );
}

#[test]
fn testGetTargetUrl() {
    let response = redirect("foo.bar");

    assert_eq!(&PhpString::from("foo.bar"), response.get_target_url());
}

#[test]
fn testSetTargetUrl() {
    let mut response = redirect("foo.bar");
    response.set_target_url("baz.beep").unwrap();

    assert_eq!(&PhpString::from("baz.beep"), response.get_target_url());
}

#[test]
fn testCacheHeaders() {
    let response = RedirectResponse::new("foo.bar", 301, NO_HEADERS).unwrap();
    assert!(!response.headers().has_cache_control_directive("no-cache"));

    let response =
        RedirectResponse::new("foo.bar", 301, [("cache-control", "max-age=86400")]).unwrap();
    assert!(!response.headers().has_cache_control_directive("no-cache"));
    assert!(response.headers().has_cache_control_directive("max-age"));

    let response =
        RedirectResponse::new("foo.bar", 301, [("Cache-Control", "max-age=86400")]).unwrap();
    assert!(!response.headers().has_cache_control_directive("no-cache"));
    assert!(response.headers().has_cache_control_directive("max-age"));

    let response = RedirectResponse::new("foo.bar", 302, NO_HEADERS).unwrap();
    assert!(response.headers().has_cache_control_directive("no-cache"));
}
