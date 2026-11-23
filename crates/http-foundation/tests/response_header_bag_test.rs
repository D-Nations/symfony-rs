//! ResponseHeaderBagTest.php at symfony/http-foundation
//! 79325f87c3a2acc7564ddce9f457046e980e7255. Names and assertions are
//! PHPUnit's. The class is `time-sensitive`: each test runs on a mocked
//! clock. `assertSetCookieHeader()`'s pattern `^Set-Cookie:\s+...$` (`m`)
//! is checked line by line.
//!
//! Upstream: symfony/http-foundation Tests/ResponseHeaderBagTest.php
#![allow(non_snake_case)]

use php_value::{
    Key, PhpString,
    clock::{self, MockClock, time},
    datetime::gmdate,
};
use symfony_http_foundation::{
    cookie::{Cookie, Options},
    header_bag::{ClearCookie, Cookies, HeaderValues, ResponseHeaderBag},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn clock() -> MockClock {
    clock::mock(1_700_000_000)
}

fn s(text: &str) -> Option<PhpString> {
    Some(PhpString::from(text))
}

fn bag<const N: usize>(headers: [(&str, &str); N]) -> ResponseHeaderBag {
    ResponseHeaderBag::new(headers).unwrap()
}

/// `Cookie::create($name, $value, 0, $path, $domain)`.
fn cookie(name: &str, value: &str, path: &str, domain: Option<&str>) -> Cookie {
    Cookie::create(
        name,
        Options {
            value: s(value),
            path: s(path),
            domain: domain.map(PhpString::from),
            ..Options::default()
        },
    )
    .unwrap()
}

/// `Cookie::create($name, $value, 0, '/', null, false, false, true, null)`.
fn raw_cookie(name: &str, value: &str) -> Cookie {
    Cookie::create(
        name,
        Options {
            value: s(value),
            secure: Some(false),
            http_only: false,
            raw: true,
            same_site: None,
            ..Options::default()
        },
    )
    .unwrap()
}

fn assert_set_cookie_header(expected: &str, actual: &ResponseHeaderBag) {
    let text = actual.to_string().replace("\r\n", "\n");
    let found = text.lines().any(|line| {
        line.strip_prefix("Set-Cookie:")
            .filter(|rest| rest.starts_with(char::is_whitespace))
            .is_some_and(|rest| rest.trim_start() == expected)
    });
    assert!(found, "no Set-Cookie: {expected} in\n{text}");
}

fn flat(bag: &ResponseHeaderBag) -> Vec<Cookie> {
    match bag.get_cookies(ResponseHeaderBag::COOKIES_FLAT) {
        Ok(Cookies::Flat(cookies)) => cookies,
        other => panic!("{other:?}"),
    }
}

fn tree(bag: &ResponseHeaderBag) -> symfony_http_foundation::header_bag::CookieTree {
    match bag.get_cookies(ResponseHeaderBag::COOKIES_ARRAY) {
        Ok(Cookies::Array(cookies)) => cookies,
        other => panic!("{other:?}"),
    }
}

fn k(text: &str) -> Key {
    Key::from_bytes(text.as_bytes())
}

#[test]
fn testAllPreserveCase() {
    let _clock = clock();
    let headers = [
        ("fOo", "BAR"),
        ("ETag", "xyzzy"),
        ("Content-MD5", "Q2hlY2sgSW50ZWdyaXR5IQ=="),
        ("P3P", "CP=\"CAO PSA OUR\""),
        ("WWW-Authenticate", "Basic realm=\"WallyWorld\""),
        ("X-UA-Compatible", "IE=edge,chrome=1"),
        ("X-XSS-Protection", "1; mode=block"),
    ];

    let bag = bag(headers);
    let all_preserved_case = bag.all_preserve_case();

    for (header_name, _) in headers {
        assert!(
            all_preserved_case.contains_key(&k(header_name)),
            "->allPreserveCase() gets all input keys in original case"
        );
    }
}

#[test]
fn testCacheControlHeader() -> TestResult {
    let _clock = clock();
    let get = |bag: &ResponseHeaderBag| bag.get("Cache-Control", None);

    let b = bag([]);
    assert_eq!(s("no-cache, private"), get(&b));
    assert!(b.has_cache_control_directive("no-cache"));

    let b = bag([("Cache-Control", "public")]);
    assert_eq!(s("public"), get(&b));
    assert!(b.has_cache_control_directive("public"));

    let b = bag([("ETag", "abcde")]);
    assert_eq!(s("no-cache, private"), get(&b));
    assert!(b.has_cache_control_directive("private"));
    assert!(b.has_cache_control_directive("no-cache"));
    assert!(!b.has_cache_control_directive("max-age"));

    let b = bag([("Expires", "Wed, 16 Feb 2011 14:17:43 GMT")]);
    assert_eq!(s("private, must-revalidate"), get(&b));

    let b = bag([
        ("Expires", "Wed, 16 Feb 2011 14:17:43 GMT"),
        ("Cache-Control", "max-age=3600"),
    ]);
    assert_eq!(s("max-age=3600, private"), get(&b));

    let b = bag([("Last-Modified", "abcde")]);
    assert_eq!(s("private, must-revalidate"), get(&b));

    let b = bag([("Etag", "abcde"), ("Last-Modified", "abcde")]);
    assert_eq!(s("private, must-revalidate"), get(&b));

    let b = bag([("cache-control", "max-age=100")]);
    assert_eq!(s("max-age=100, private"), get(&b));

    let b = bag([("cache-control", "s-maxage=100")]);
    assert_eq!(s("s-maxage=100"), get(&b));

    let b = bag([("cache-control", "private, max-age=100")]);
    assert_eq!(s("max-age=100, private"), get(&b));

    let b = bag([("cache-control", "public, max-age=100")]);
    assert_eq!(s("max-age=100, public"), get(&b));

    let mut b = ResponseHeaderBag::empty();
    b.set(b"Last-Modified", "abcde", true)?;
    assert_eq!(s("private, must-revalidate"), get(&b));

    let mut b = ResponseHeaderBag::empty();
    b.set(b"Cache-Control", vec!["public", "must-revalidate"], true)?;
    assert_eq!(1, b.all_of("Cache-Control").len());
    assert_eq!(s("must-revalidate, public"), get(&b));

    let mut b = ResponseHeaderBag::empty();
    b.set(b"Cache-Control", "public", true)?;
    b.set(b"Cache-Control", "must-revalidate", false)?;
    assert_eq!(1, b.all_of("Cache-Control").len());
    assert_eq!(s("must-revalidate, public"), get(&b));
    Ok(())
}

#[test]
fn testCacheControlClone() -> TestResult {
    let _clock = clock();
    let bag1 = bag([("foo", "bar")]);
    let bag2 = ResponseHeaderBag::new(bag1.all_preserve_case().iter().map(|(k, v)| {
        (
            k.to_value()
                .to_php_string()
                .unwrap_or_default()
                .as_bytes()
                .to_vec(),
            HeaderValues::Many(v.clone()),
        )
    }))?;
    assert_eq!(bag1.all_preserve_case(), bag2.all_preserve_case());
    Ok(())
}

#[test]
fn testToStringIncludesCookieHeaders() -> TestResult {
    let _clock = clock();
    let mut bag = bag([]);
    bag.set_cookie(Cookie::create(
        "foo",
        Options {
            value: s("bar"),
            ..Options::default()
        },
    )?);

    assert_set_cookie_header("foo=bar; path=/; httponly; samesite=lax", &bag);

    bag.clear_cookie("foo", Some("/"), None, ClearCookie::default())?;

    assert_set_cookie_header(
        &format!(
            "foo=deleted; expires={}; Max-Age=0; path=/; httponly",
            gmdate("D, d M Y H:i:s T", time() - 31_536_001)
        ),
        &bag,
    );
    Ok(())
}

#[test]
fn testClearCookieSecureNotHttpOnly() -> TestResult {
    let _clock = clock();
    let mut bag = bag([]);

    bag.clear_cookie(
        "foo",
        Some("/"),
        None,
        ClearCookie {
            secure: true,
            http_only: false,
            ..ClearCookie::default()
        },
    )?;

    assert_set_cookie_header(
        &format!(
            "foo=deleted; expires={}; Max-Age=0; path=/; secure",
            gmdate("D, d M Y H:i:s T", time() - 31_536_001)
        ),
        &bag,
    );
    Ok(())
}

#[test]
fn testClearCookieSamesite() -> TestResult {
    let _clock = clock();
    let mut bag = bag([]);

    bag.clear_cookie(
        "foo",
        Some("/"),
        None,
        ClearCookie {
            secure: true,
            http_only: false,
            same_site: s("none"),
            partitioned: false,
        },
    )?;
    assert_set_cookie_header(
        &format!(
            "foo=deleted; expires={}; Max-Age=0; path=/; secure; samesite=none",
            gmdate("D, d M Y H:i:s T", time() - 31_536_001)
        ),
        &bag,
    );
    Ok(())
}

#[test]
fn testClearCookiePartitioned() -> TestResult {
    let _clock = clock();
    let mut bag = bag([]);

    bag.clear_cookie(
        "foo",
        Some("/"),
        None,
        ClearCookie {
            secure: true,
            http_only: false,
            same_site: s("none"),
            partitioned: true,
        },
    )?;
    assert_set_cookie_header(
        &format!(
            "foo=deleted; expires={}; Max-Age=0; path=/; secure; samesite=none; partitioned",
            gmdate("D, d M Y H:i:s T", time() - 31_536_001)
        ),
        &bag,
    );
    Ok(())
}

#[test]
fn testReplace() -> TestResult {
    let _clock = clock();
    let mut bag = bag([]);
    assert_eq!(s("no-cache, private"), bag.get("Cache-Control", None));
    assert!(bag.has_cache_control_directive("no-cache"));

    bag.replace([("Cache-Control", "public")])?;
    assert_eq!(s("public"), bag.get("Cache-Control", None));
    assert!(bag.has_cache_control_directive("public"));
    Ok(())
}

#[test]
fn testReplaceWithRemove() -> TestResult {
    let _clock = clock();
    let mut bag = bag([]);
    assert_eq!(s("no-cache, private"), bag.get("Cache-Control", None));
    assert!(bag.has_cache_control_directive("no-cache"));

    bag.remove("Cache-Control");
    bag.replace::<&str, &str>([])?;
    assert_eq!(s("no-cache, private"), bag.get("Cache-Control", None));
    assert!(bag.has_cache_control_directive("no-cache"));
    Ok(())
}

#[test]
fn testCookiesWithSameNames() {
    let _clock = clock();
    let mut bag = ResponseHeaderBag::empty();
    bag.set_cookie(cookie("foo", "bar", "/path/foo", Some("foo.bar")));
    bag.set_cookie(cookie("foo", "bar", "/path/bar", Some("foo.bar")));
    bag.set_cookie(cookie("foo", "bar", "/path/bar", Some("bar.foo")));
    bag.set_cookie(cookie("foo", "bar", "/", None));

    assert_eq!(4, flat(&bag).len());
    assert_eq!(
        s("foo=bar; path=/path/foo; domain=foo.bar; httponly; samesite=lax"),
        bag.get("set-cookie", None)
    );
    assert_eq!(
        vec![
            s("foo=bar; path=/path/foo; domain=foo.bar; httponly; samesite=lax"),
            s("foo=bar; path=/path/bar; domain=foo.bar; httponly; samesite=lax"),
            s("foo=bar; path=/path/bar; domain=bar.foo; httponly; samesite=lax"),
            s("foo=bar; path=/; httponly; samesite=lax"),
        ],
        bag.all_of("set-cookie")
    );

    assert_set_cookie_header(
        "foo=bar; path=/path/foo; domain=foo.bar; httponly; samesite=lax",
        &bag,
    );
    assert_set_cookie_header(
        "foo=bar; path=/path/bar; domain=foo.bar; httponly; samesite=lax",
        &bag,
    );
    assert_set_cookie_header(
        "foo=bar; path=/path/bar; domain=bar.foo; httponly; samesite=lax",
        &bag,
    );
    assert_set_cookie_header("foo=bar; path=/; httponly; samesite=lax", &bag);

    let cookies = tree(&bag);

    let has = |domain: &str, path: &str, name: &str| {
        cookies
            .get(&k(domain))
            .and_then(|paths| paths.get(&k(path)))
            .is_some_and(|names| names.contains_key(&k(name)))
    };
    assert!(has("foo.bar", "/path/foo", "foo"));
    assert!(has("foo.bar", "/path/bar", "foo"));
    assert!(has("bar.foo", "/path/bar", "foo"));
    assert!(has("", "/", "foo"));
}

#[test]
fn testRemoveCookie() {
    let _clock = clock();
    let mut bag = ResponseHeaderBag::empty();
    assert!(!bag.has("set-cookie"));

    bag.set_cookie(cookie("foo", "bar", "/path/foo", Some("foo.bar")));
    bag.set_cookie(cookie("bar", "foo", "/path/bar", Some("foo.bar")));
    assert!(bag.has("set-cookie"));

    let cookies = tree(&bag);
    assert!(
        cookies
            .get(&k("foo.bar"))
            .is_some_and(|p| p.contains_key(&k("/path/foo")))
    );

    bag.remove_cookie("foo", Some("/path/foo"), Some("foo.bar"));
    assert!(bag.has("set-cookie"));

    let cookies = tree(&bag);
    assert!(
        cookies
            .get(&k("foo.bar"))
            .is_some_and(|p| !p.contains_key(&k("/path/foo")))
    );

    bag.remove_cookie("bar", Some("/path/bar"), Some("foo.bar"));
    assert!(!bag.has("set-cookie"));

    let cookies = tree(&bag);
    assert!(!cookies.contains_key(&k("foo.bar")));
}

#[test]
fn testRemoveCookieWithNullRemove() {
    let _clock = clock();
    let mut bag = ResponseHeaderBag::empty();
    bag.set_cookie(cookie("foo", "bar", "/", None));
    bag.set_cookie(cookie("bar", "foo", "/", None));

    let cookies = tree(&bag);
    assert!(cookies.get(&k("")).is_some_and(|p| p.contains_key(&k("/"))));

    bag.remove_cookie("foo", None, None);
    let cookies = tree(&bag);
    assert!(
        cookies
            .get(&k(""))
            .and_then(|p| p.get(&k("/")))
            .is_some_and(|n| !n.contains_key(&k("foo")))
    );

    bag.remove_cookie("bar", None, None);
    let cookies = tree(&bag);
    assert!(
        !cookies
            .get(&k(""))
            .and_then(|p| p.get(&k("/")))
            .is_some_and(|n| n.contains_key(&k("bar")))
    );
}

#[test]
fn testSetCookieHeader() -> TestResult {
    let _clock = clock();
    let mut bag = ResponseHeaderBag::empty();
    bag.set(b"set-cookie", "foo=bar", true)?;
    assert_eq!(vec![raw_cookie("foo", "bar")], flat(&bag));

    bag.set(b"set-cookie", "foo2=bar2", false)?;
    assert_eq!(
        vec![raw_cookie("foo", "bar"), raw_cookie("foo2", "bar2")],
        flat(&bag)
    );

    bag.remove("set-cookie");
    assert_eq!(Vec::<Cookie>::new(), flat(&bag));
    Ok(())
}

#[test]
fn testGetCookiesWithInvalidArgument() {
    let _clock = clock();
    let bag = ResponseHeaderBag::empty();

    let e = bag.get_cookies("invalid_argument").unwrap_err();
    assert!(e.is_a("InvalidArgumentException"));
}

#[test]
fn testToStringDoesntMessUpHeaders() -> TestResult {
    let _clock = clock();
    let mut headers = ResponseHeaderBag::empty();

    headers.set(b"Location", "http://www.symfony.com", true)?;
    headers.set(b"Content-type", "text/html", true)?;

    let _ = headers.to_string();

    let all_headers = headers.all_preserve_case();
    assert_eq!(
        Some(&vec![s("http://www.symfony.com")]),
        all_headers.get(&k("Location"))
    );
    assert_eq!(
        Some(&vec![s("text/html")]),
        all_headers.get(&k("Content-type"))
    );
    Ok(())
}

#[test]
fn testDateHeaderAddedOnCreation() -> TestResult {
    let _clock = clock();
    let now = time();

    let bag = ResponseHeaderBag::empty();
    assert!(bag.has("Date"));

    assert_eq!(
        Some(now),
        bag.get_date("Date", None)?.map(|date| date.timestamp())
    );
    Ok(())
}

#[test]
fn testDateHeaderCanBeSetOnCreation() {
    let _clock = clock();
    let some_date = "Thu, 23 Mar 2017 09:15:12 GMT";
    let bag = bag([("Date", some_date)]);

    assert_eq!(s(some_date), bag.get("Date", None));
}

#[test]
fn testDateHeaderWillBeRecreatedWhenRemoved() {
    let _clock = clock();
    let some_date = "Thu, 23 Mar 2017 09:15:12 GMT";
    let mut bag = bag([("Date", some_date)]);
    bag.remove("Date");

    // a (new) Date header is still present
    assert!(bag.has("Date"));
    assert_ne!(s(some_date), bag.get("Date", None));
}

#[test]
fn testDateHeaderWillBeRecreatedWhenHeadersAreReplaced() -> TestResult {
    let _clock = clock();
    let mut bag = ResponseHeaderBag::empty();
    bag.replace::<&str, &str>([])?;

    assert!(bag.has("Date"));
    Ok(())
}
