//! CookieTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Names and assertions are
//! PHPUnit's; a data provider's cases run in a loop. The class is
//! `time-sensitive`: each test runs on a mocked clock, as `ClockMock` gives
//! it, and `strtotime()` reads the same clock. `assertEquals()` on two
//! cookies compares every property, as derived `PartialEq` does.
//!
//! Upstream: symfony/http-foundation Tests/CookieTest.php

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

use php_value::{
    PhpString,
    clock::{self, MockClock, time},
    dateparse::strtotime,
    datetime::{DateTimeImmutable, TimeZone, gmdate},
};
use symfony_http_foundation::cookie::{Cookie, Expire, Options, SameSite};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const INVALID_ARGUMENT: &str = "InvalidArgumentException";

/// `ClockMock`'s clock for a `time-sensitive` test.
fn clock() -> MockClock {
    clock::mock(1_700_000_000)
}

fn s(text: &str) -> Option<PhpString> {
    Some(PhpString::from(text))
}

fn ts(text: &str) -> i64 {
    strtotime(text.as_bytes(), time()).unwrap().unwrap()
}

/// `Cookie::create($name, $value, $expire, $path, $domain, $secure,
/// $httpOnly, $raw, $sameSite)` with PHP's positional arguments.
#[allow(clippy::too_many_arguments)]
fn create(
    name: &str,
    value: Option<&str>,
    expire: impl Into<Expire>,
    path: Option<&str>,
    domain: Option<&str>,
    secure: Option<bool>,
    http_only: bool,
    raw: bool,
    same_site: Option<&str>,
) -> symfony_http_foundation::exception::Result<Cookie> {
    Cookie::create(
        name,
        Options {
            value: value.map(PhpString::from),
            expire: expire.into(),
            path: path.map(PhpString::from),
            domain: domain.map(PhpString::from),
            secure,
            http_only,
            raw,
            same_site: same_site.map(PhpString::from),
            partitioned: false,
        },
    )
}

fn create_name(name: &str) -> Cookie {
    Cookie::create(name, Options::default()).unwrap()
}

fn create_value(name: &str, value: &str) -> Cookie {
    Cookie::create(
        name,
        Options {
            value: s(value),
            ..Options::default()
        },
    )
    .unwrap()
}

fn create_expire(name: &str, value: Option<&str>, expire: impl Into<Expire>) -> Cookie {
    Cookie::create(
        name,
        Options {
            value: value.map(PhpString::from),
            expire: expire.into(),
            ..Options::default()
        },
    )
    .unwrap()
}

fn from_string(cookie: &str) -> Cookie {
    Cookie::from_string(cookie.as_bytes(), false).unwrap()
}

fn names_with_special_characters() -> [&'static str; 8] {
    [
        ",MyName",
        ";MyName",
        " MyName",
        "\tMyName",
        "\rMyName",
        "\nMyName",
        "\x0BMyName",
        "\x0CMyName",
    ]
}

#[test]
fn testInstantiationThrowsExceptionIfRawCookieNameContainsSpecialCharacters() {
    let _clock = clock();
    for name in names_with_special_characters() {
        let e = create(
            name,
            None,
            0,
            None,
            None,
            None,
            false,
            true,
            s("lax").as_ref().map(|_| "lax"),
        )
        .unwrap_err();
        assert!(e.is_a(INVALID_ARGUMENT), "{name:?}");
    }
}

#[test]
fn testWithRawThrowsExceptionIfCookieNameContainsSpecialCharacters() {
    let _clock = clock();
    for name in names_with_special_characters() {
        let e = create_name(name).with_raw(true).unwrap_err();
        assert!(e.is_a(INVALID_ARGUMENT), "{name:?}");
    }
}

#[test]
fn testInstantiationSucceedNonRawCookieNameContainsSpecialCharacters() {
    let _clock = clock();
    for name in names_with_special_characters() {
        assert!(Cookie::create(name, Options::default()).is_ok(), "{name:?}");
    }
}

#[test]
fn testInstantiationThrowsExceptionIfCookieNameIsEmpty() {
    let _clock = clock();
    let e = Cookie::create("", Options::default()).unwrap_err();
    assert!(e.is_a(INVALID_ARGUMENT));
}

#[test]
fn testInstantiationAcceptsZeroName() {
    let _clock = clock();
    let cookie = create_value("0", "value");

    assert_eq!(cookie.name().as_bytes(), b"0");
    assert!(cookie.to_string().starts_with("0=value;"));
}

#[test]
fn testFromStringAcceptsZeroName() {
    let _clock = clock();
    let cookie = from_string("0=value");

    assert_eq!(cookie.name().as_bytes(), b"0");
    assert!(cookie.to_string().starts_with("0=value;"));
}

fn paths_and_domains_with_special_characters() -> [&'static str; 10] {
    [
        "/p,q",
        "/p;q",
        "/p q",
        "/p\tq",
        "/p\rq",
        "/p\nq",
        "/p\x0Bq",
        "/p\x0Cq",
        "/p; SameSite=None; Secure",
        "victim.com; secure",
    ]
}

#[test]
fn testInstantiationThrowsExceptionIfPathContainsSpecialCharacters() {
    let _clock = clock();
    for path in paths_and_domains_with_special_characters() {
        let e = create(
            "MyCookie",
            Some("MyValue"),
            0,
            Some(path),
            None,
            None,
            true,
            false,
            Some("lax"),
        )
        .unwrap_err();
        assert!(e.is_a(INVALID_ARGUMENT), "{path:?}");
    }
}

#[test]
fn testInstantiationThrowsExceptionIfDomainContainsSpecialCharacters() {
    let _clock = clock();
    for domain in paths_and_domains_with_special_characters() {
        let e = create(
            "MyCookie",
            Some("MyValue"),
            0,
            Some("/"),
            Some(domain),
            None,
            true,
            false,
            Some("lax"),
        )
        .unwrap_err();
        assert!(e.is_a(INVALID_ARGUMENT), "{domain:?}");
    }
}

#[test]
fn testWithPathThrowsExceptionIfPathContainsSpecialCharacters() {
    let _clock = clock();
    for path in paths_and_domains_with_special_characters() {
        let e = create_name("MyCookie").with_path(path).unwrap_err();
        assert!(e.is_a(INVALID_ARGUMENT), "{path:?}");
    }
}

#[test]
fn testWithDomainThrowsExceptionIfDomainContainsSpecialCharacters() {
    let _clock = clock();
    for domain in paths_and_domains_with_special_characters() {
        let e = create_name("MyCookie").with_domain(s(domain)).unwrap_err();
        assert!(e.is_a(INVALID_ARGUMENT), "{domain:?}");
    }
}

#[test]
fn testFromStringEndsQuotedPathsAtTheNextSemicolon() {
    let _clock = clock();
    let cookie = from_string("foo=bar; path=\"/p; SameSite=None\"");

    assert_eq!(cookie.path().as_bytes(), b"\"/p");
    assert_eq!(cookie.same_site(), None);
}

#[test]
fn testOrdinaryPathsAndDomainsAreAccepted() -> TestResult {
    let _clock = clock();
    for value in [
        "/",
        "/foo",
        "/foo/bar",
        "/foo=bar",
        "/foo%20bar",
        ".myfoodomain.com",
        "myfoodomain.com",
    ] {
        let cookie = create(
            "MyCookie",
            Some("MyValue"),
            0,
            Some(value),
            Some(value),
            None,
            true,
            false,
            Some("lax"),
        )?;

        assert_eq!(cookie.path().as_bytes(), value.as_bytes());
        assert_eq!(cookie.domain(), s(value).as_ref());

        let cookie = create_name("MyCookie")
            .with_path(value)?
            .with_domain(s(value))?;

        assert_eq!(cookie.path().as_bytes(), value.as_bytes());
        assert_eq!(cookie.domain(), s(value).as_ref());
    }
    Ok(())
}

#[test]
fn testNullAndEmptyPathsAndDomainsAreAccepted() -> TestResult {
    let _clock = clock();
    let cookie = create(
        "MyCookie",
        Some("MyValue"),
        0,
        None,
        None,
        None,
        true,
        false,
        Some("lax"),
    )?;

    assert_eq!(cookie.path().as_bytes(), b"/");
    assert_eq!(cookie.domain(), None);

    let cookie = create(
        "MyCookie",
        Some("MyValue"),
        0,
        Some(""),
        None,
        None,
        true,
        false,
        Some("lax"),
    )?
    .with_domain(None)?;

    assert_eq!(cookie.path().as_bytes(), b"/");
    assert_eq!(cookie.domain(), None);
    Ok(())
}

#[test]
fn testInvalidExpiration() {
    let _clock = clock();
    let e = Cookie::create(
        "MyCookie",
        Options {
            value: s("foo"),
            expire: Expire::from("bar"),
            ..Options::default()
        },
    )
    .unwrap_err();
    assert!(e.is_a(INVALID_ARGUMENT));
}

#[test]
fn testNegativeExpirationIsNotPossible() -> TestResult {
    let _clock = clock();
    let cookie = create_expire("foo", Some("bar"), -100);

    assert_eq!(0, cookie.expires_time());

    let cookie = create_value("foo", "bar").with_expires(-100)?;

    assert_eq!(0, cookie.expires_time());
    Ok(())
}

#[test]
fn testMinimalParameters() -> TestResult {
    let _clock = clock();
    let constructed_cookie = Cookie::new("foo", Options::default())?;

    let created_cookie = Cookie::create("foo", Options::default())?;

    let cookie = Cookie::new(
        "foo",
        Options {
            value: None,
            expire: Expire::Timestamp(0),
            path: s("/"),
            domain: None,
            secure: None,
            http_only: true,
            raw: false,
            same_site: s("lax"),
            partitioned: false,
        },
    )?;

    assert_eq!(constructed_cookie, cookie);

    assert_eq!(created_cookie, cookie);
    Ok(())
}

#[test]
fn testGetValue() {
    let _clock = clock();
    let value = "MyValue";
    let cookie = create_value("MyCookie", value);

    assert_eq!(
        cookie.value(),
        s(value).as_ref(),
        "->getValue() returns the proper value"
    );
}

#[test]
fn testGetPath() {
    let _clock = clock();
    let cookie = create_value("foo", "bar");

    assert_eq!(
        cookie.path().as_bytes(),
        b"/",
        "->getPath() returns / as the default path"
    );
}

#[test]
fn testGetExpiresTime() -> TestResult {
    let _clock = clock();
    let cookie = create_value("foo", "bar");

    assert_eq!(
        0,
        cookie.expires_time(),
        "->getExpiresTime() returns the default expire date"
    );

    let expire = time() + 3600;
    let cookie = create_expire("foo", Some("bar"), expire);

    assert_eq!(
        expire,
        cookie.expires_time(),
        "->getExpiresTime() returns the expire date"
    );

    let expire = time() + 3600;
    let cookie = create_name("foo").with_expires(expire)?;

    assert_eq!(
        expire,
        cookie.expires_time(),
        "->getExpiresTime() returns the expire date"
    );
    Ok(())
}

#[test]
fn testConstructorWithDateTime() -> TestResult {
    let _clock = clock();
    // `new \DateTime()`: now, in the default zone.
    let expire = DateTimeImmutable::from_timestamp(time(), 0, TimeZone::Utc);
    let cookie = create_expire("foo", Some("bar"), expire);

    assert_eq!(
        expire.format("U"),
        cookie.expires_time().to_string(),
        "->getExpiresTime() returns the expire date"
    );

    let cookie = create_name("foo").with_expires(expire)?;

    assert_eq!(
        expire.format("U"),
        cookie.expires_time().to_string(),
        "->getExpiresTime() returns the expire date"
    );
    Ok(())
}

#[test]
fn testConstructorWithDateTimeImmutable() -> TestResult {
    let _clock = clock();
    let expire = DateTimeImmutable::from_timestamp(time(), 0, TimeZone::Utc);
    let cookie = create_expire("foo", Some("bar"), expire);

    assert_eq!(
        expire.format("U"),
        cookie.expires_time().to_string(),
        "->getExpiresTime() returns the expire date"
    );

    let cookie = create_name("foo")
        .with_value(s("bar"))
        .with_expires(expire)?;

    assert_eq!(
        expire.format("U"),
        cookie.expires_time().to_string(),
        "->getExpiresTime() returns the expire date"
    );
    Ok(())
}

#[test]
fn testGetExpiresTimeWithStringValue() -> TestResult {
    let _clock = clock();
    let value = "+1 day";
    let cookie = create_expire("foo", Some("bar"), value);
    let expire = ts(value);

    assert!(
        (expire - cookie.expires_time()).abs() <= 1,
        "->getExpiresTime() returns the expire date"
    );

    let cookie = create_name("foo")
        .with_value(s("bar"))
        .with_expires(value)?;

    assert!(
        (expire - cookie.expires_time()).abs() <= 1,
        "->getExpiresTime() returns the expire date"
    );
    Ok(())
}

#[test]
fn testGetDomain() -> TestResult {
    let _clock = clock();
    let cookie = create(
        "foo",
        Some("bar"),
        0,
        Some("/"),
        Some(".myfoodomain.com"),
        None,
        true,
        false,
        Some("lax"),
    )?;

    assert_eq!(
        cookie.domain(),
        s(".myfoodomain.com").as_ref(),
        "->getDomain() returns the domain name on which the cookie is valid"
    );

    let cookie = create_name("foo").with_domain(s(".mybardomain.com"))?;

    assert_eq!(
        cookie.domain(),
        s(".mybardomain.com").as_ref(),
        "->getDomain() returns the domain name on which the cookie is valid"
    );
    Ok(())
}

#[test]
fn testIsSecure() -> TestResult {
    let _clock = clock();
    let cookie = create(
        "foo",
        Some("bar"),
        0,
        Some("/"),
        Some(".myfoodomain.com"),
        Some(true),
        true,
        false,
        Some("lax"),
    )?;

    assert!(
        cookie.is_secure(),
        "->isSecure() returns whether the cookie is transmitted over HTTPS"
    );

    let cookie = create_name("foo").with_secure(true);

    assert!(
        cookie.is_secure(),
        "->isSecure() returns whether the cookie is transmitted over HTTPS"
    );
    Ok(())
}

#[test]
fn testIsHttpOnly() -> TestResult {
    let _clock = clock();
    let cookie = create(
        "foo",
        Some("bar"),
        0,
        Some("/"),
        Some(".myfoodomain.com"),
        Some(false),
        true,
        false,
        Some("lax"),
    )?;

    assert!(
        cookie.is_http_only(),
        "->isHttpOnly() returns whether the cookie is only transmitted over HTTP"
    );

    let cookie = create_name("foo").with_http_only(true);

    assert!(
        cookie.is_http_only(),
        "->isHttpOnly() returns whether the cookie is only transmitted over HTTP"
    );
    Ok(())
}

#[test]
fn testIsPartitioned() -> TestResult {
    let _clock = clock();
    let cookie = Cookie::new(
        "foo",
        Options {
            value: s("bar"),
            expire: Expire::Timestamp(0),
            path: s("/"),
            domain: s(".myfoodomain.com"),
            secure: Some(true),
            http_only: true,
            raw: false,
            same_site: s("Lax"),
            partitioned: true,
        },
    )?;

    assert!(cookie.is_partitioned());

    let cookie = create_name("foo").with_partitioned(true);

    assert!(cookie.is_partitioned());
    Ok(())
}

#[test]
fn testCookieIsNotCleared() -> TestResult {
    let _clock = clock();
    let cookie = create_expire("foo", Some("bar"), time() + 3600 * 24);

    assert!(
        !cookie.is_cleared(),
        "->isCleared() returns false if the cookie did not expire yet"
    );

    let cookie = create_name("foo").with_expires(time() + 3600 * 24)?;

    assert!(
        !cookie.is_cleared(),
        "->isCleared() returns false if the cookie did not expire yet"
    );
    Ok(())
}

#[test]
fn testCookieIsCleared() -> TestResult {
    let _clock = clock();
    let cookie = create_expire("foo", Some("bar"), time() - 20);

    assert!(
        cookie.is_cleared(),
        "->isCleared() returns true if the cookie has expired"
    );

    let cookie = create_name("foo").with_expires(time() - 20)?;

    assert!(
        cookie.is_cleared(),
        "->isCleared() returns true if the cookie has expired"
    );

    let cookie = create_value("foo", "bar");

    assert!(!cookie.is_cleared());

    let cookie = create_value("foo", "bar");

    assert!(!cookie.is_cleared());

    let cookie = create_expire("foo", Some("bar"), -1);

    assert!(!cookie.is_cleared());

    let cookie = create_name("foo").with_expires(-1)?;

    assert!(!cookie.is_cleared());
    Ok(())
}

#[test]
fn testToString() -> TestResult {
    let _clock = clock();
    let expected = "foo=bar; expires=Fri, 20 May 2011 15:25:52 GMT; Max-Age=0; path=/; domain=.myfoodomain.com; secure; httponly";
    let expire = ts("Fri, 20 May 2011 15:25:52 GMT");
    let cookie = create(
        "foo",
        Some("bar"),
        expire,
        Some("/"),
        Some(".myfoodomain.com"),
        Some(true),
        true,
        false,
        None,
    )?;
    assert_eq!(
        expected,
        cookie.to_string(),
        "->__toString() returns string representation of the cookie"
    );

    let cookie = create_name("foo")
        .with_value(s("bar"))
        .with_expires(ts("Fri, 20 May 2011 15:25:52 GMT"))?
        .with_domain(s(".myfoodomain.com"))?
        .with_secure(true)
        .with_same_site(None)?;
    assert_eq!(
        expected,
        cookie.to_string(),
        "->__toString() returns string representation of the cookie"
    );

    let expected = "foo=bar%20with%20white%20spaces; expires=Fri, 20 May 2011 15:25:52 GMT; Max-Age=0; path=/; domain=.myfoodomain.com; secure; httponly";
    let cookie = create(
        "foo",
        Some("bar with white spaces"),
        ts("Fri, 20 May 2011 15:25:52 GMT"),
        Some("/"),
        Some(".myfoodomain.com"),
        Some(true),
        true,
        false,
        None,
    )?;
    assert_eq!(
        expected,
        cookie.to_string(),
        "->__toString() encodes the value of the cookie according to RFC 3986 (white space = %20)"
    );

    let cookie = create_name("foo")
        .with_value(s("bar with white spaces"))
        .with_expires(ts("Fri, 20 May 2011 15:25:52 GMT"))?
        .with_domain(s(".myfoodomain.com"))?
        .with_secure(true)
        .with_same_site(None)?;
    assert_eq!(
        expected,
        cookie.to_string(),
        "->__toString() encodes the value of the cookie according to RFC 3986 (white space = %20)"
    );

    let expected = format!(
        "foo=deleted; expires={}; Max-Age=0; path=/admin/; domain=.myfoodomain.com; httponly",
        gmdate("D, d M Y H:i:s T", time() - 31_536_001)
    );
    let cookie = create(
        "foo",
        None,
        1,
        Some("/admin/"),
        Some(".myfoodomain.com"),
        Some(false),
        true,
        false,
        None,
    )?;
    assert_eq!(
        expected,
        cookie.to_string(),
        "->__toString() returns string representation of a cleared cookie if value is NULL"
    );

    let cookie = create_name("foo")
        .with_expires(1)?
        .with_path("/admin/")?
        .with_domain(s(".myfoodomain.com"))?
        .with_same_site(None)?;
    assert_eq!(
        expected,
        cookie.to_string(),
        "->__toString() returns string representation of a cleared cookie if value is NULL"
    );

    let expected = format!(
        "foo=deleted; expires={}; Max-Age=0; path=/admin/; domain=.myfoodomain.com; secure; httponly; samesite=none; partitioned",
        gmdate("D, d M Y H:i:s T", time() - 31_536_001)
    );
    let cookie = Cookie::new(
        "foo",
        Options {
            value: None,
            expire: Expire::Timestamp(1),
            path: s("/admin/"),
            domain: s(".myfoodomain.com"),
            secure: Some(true),
            http_only: true,
            raw: false,
            same_site: s("none"),
            partitioned: true,
        },
    )?;
    assert_eq!(
        expected,
        cookie.to_string(),
        "->__toString() returns string representation of a cleared cookie if value is NULL"
    );

    let cookie = create_name("foo")
        .with_expires(1)?
        .with_path("/admin/")?
        .with_domain(s(".myfoodomain.com"))?
        .with_secure(true)
        .with_http_only(true)
        .with_same_site(Some("none"))?
        .with_partitioned(true);
    assert_eq!(
        expected,
        cookie.to_string(),
        "->__toString() returns string representation of a cleared cookie if value is NULL"
    );

    let expected = "foo=bar; path=/; httponly; samesite=lax";
    let cookie = create_value("foo", "bar");
    assert_eq!(expected, cookie.to_string());

    let cookie = create_name("foo").with_value(s("bar"));
    assert_eq!(expected, cookie.to_string());
    Ok(())
}

#[test]
fn testRawCookie() -> TestResult {
    let _clock = clock();
    let cookie = create(
        "foo",
        Some("b a r"),
        0,
        Some("/"),
        None,
        Some(false),
        false,
        false,
        None,
    )?;
    assert!(!cookie.is_raw());
    assert_eq!("foo=b%20a%20r; path=/", cookie.to_string());

    let cookie = create_name("test")
        .with_value(s("t e s t"))
        .with_http_only(false)
        .with_same_site(None)?;
    assert!(!cookie.is_raw());
    assert_eq!("test=t%20e%20s%20t; path=/", cookie.to_string());

    let cookie = create(
        "foo",
        Some("b+a+r"),
        0,
        Some("/"),
        None,
        Some(false),
        false,
        true,
        None,
    )?;
    assert!(cookie.is_raw());
    assert_eq!("foo=b+a+r; path=/", cookie.to_string());

    let cookie = create_name("foo")
        .with_value(s("t+e+s+t"))
        .with_http_only(false)
        .with_raw(true)?
        .with_same_site(None)?;
    assert!(cookie.is_raw());
    assert_eq!("foo=t+e+s+t; path=/", cookie.to_string());
    Ok(())
}

#[test]
fn testGetMaxAge() {
    let _clock = clock();
    let cookie = create_value("foo", "bar");
    assert_eq!(0, cookie.max_age());

    let expire = time() + 100;
    let cookie = create_expire("foo", Some("bar"), expire);
    assert_eq!(expire - time(), cookie.max_age());

    let expire = time() - 100;
    let cookie = create_expire("foo", Some("bar"), expire);
    assert_eq!(0, cookie.max_age());
}

#[test]
fn testFromString() -> TestResult {
    let _clock = clock();
    let cookie = from_string(
        "foo=bar; expires=Fri, 20 May 2011 15:25:52 GMT; path=/; domain=.myfoodomain.com; secure; httponly",
    );
    assert_eq!(
        create(
            "foo",
            Some("bar"),
            ts("Fri, 20 May 2011 15:25:52 GMT"),
            Some("/"),
            Some(".myfoodomain.com"),
            Some(true),
            true,
            true,
            None
        )?,
        cookie
    );

    let cookie = Cookie::from_string(b"foo=bar", true)?;
    assert_eq!(
        create(
            "foo",
            Some("bar"),
            0,
            Some("/"),
            None,
            Some(false),
            false,
            false,
            None
        )?,
        cookie
    );

    let cookie = Cookie::from_string(b"foo=bar=", true)?;
    assert_eq!(
        create(
            "foo",
            Some("bar="),
            0,
            Some("/"),
            None,
            Some(false),
            false,
            false,
            None
        )?,
        cookie
    );

    let cookie = Cookie::from_string(b"foo", true)?;
    assert_eq!(
        create(
            "foo",
            None,
            0,
            Some("/"),
            None,
            Some(false),
            false,
            false,
            None
        )?,
        cookie
    );

    let cookie =
        from_string("foo_cookie=foo=1&bar=2&baz=3; expires=Tue, 22 Sep 2020 06:27:09 GMT; path=/");
    assert_eq!(
        create(
            "foo_cookie",
            Some("foo=1&bar=2&baz=3"),
            ts("Tue, 22 Sep 2020 06:27:09 GMT"),
            Some("/"),
            None,
            Some(false),
            false,
            true,
            None
        )?,
        cookie
    );

    let cookie = from_string("foo_cookie=foo==; expires=Tue, 22 Sep 2020 06:27:09 GMT; path=/");
    assert_eq!(
        create(
            "foo_cookie",
            Some("foo=="),
            ts("Tue, 22 Sep 2020 06:27:09 GMT"),
            Some("/"),
            None,
            Some(false),
            false,
            true,
            None
        )?,
        cookie
    );

    let cookie = from_string(
        "foo_cookie=foo==; expires=Tue, 22 Sep 2020 06:27:09 GMT; path=/; secure; httponly; samesite=none; partitioned",
    );
    assert_eq!(
        Cookie::new(
            "foo_cookie",
            Options {
                value: s("foo=="),
                expire: Expire::Timestamp(ts("Tue, 22 Sep 2020 06:27:09 GMT")),
                path: s("/"),
                domain: None,
                secure: Some(true),
                http_only: true,
                raw: true,
                same_site: s("none"),
                partitioned: true,
            }
        )?,
        cookie
    );
    Ok(())
}

#[test]
fn testFromStringWithHttpOnly() {
    let _clock = clock();
    let cookie = from_string(
        "foo=bar; expires=Fri, 20 May 2011 15:25:52 GMT; path=/; domain=.myfoodomain.com; secure; httponly",
    );
    assert!(cookie.is_http_only());

    let cookie = from_string(
        "foo=bar; expires=Fri, 20 May 2011 15:25:52 GMT; path=/; domain=.myfoodomain.com; secure",
    );
    assert!(!cookie.is_http_only());
}

#[test]
fn testFromStringTreatsSecureAsFlag() {
    let _clock = clock();
    for value in ["0", "false"] {
        assert!(from_string(&format!("foo=bar; Secure={value}")).is_secure());
    }
}

#[test]
fn testFromStringTreatsHttpOnlyAsFlag() {
    let _clock = clock();
    for value in ["0", "false"] {
        assert!(from_string(&format!("foo=bar; HttpOnly={value}")).is_http_only());
    }
}

#[test]
fn testFromStringTreatsPartitionedAsFlag() {
    let _clock = clock();
    for value in ["0", "false"] {
        assert!(from_string(&format!("foo=bar; Partitioned={value}")).is_partitioned());
    }
}

#[test]
fn testFromStringPreservesQuotes() -> TestResult {
    let _clock = clock();
    for (header, decode, value) in [
        ("foo=\"bar\"; path=/", false, "\"bar\""),
        ("foo=\"bar=baz\"; path=/", false, "\"bar=baz\""),
        (" foo = \"bar\" ; path=/", false, "\"bar\""),
        ("foo=\"bar%20baz\"; path=/", true, "\"bar baz\""),
    ] {
        let cookie = Cookie::from_string(header.as_bytes(), decode)?;

        assert_eq!(cookie.value(), s(value).as_ref(), "{header:?}");
    }
    Ok(())
}

#[test]
fn testFromStringIgnoresQuotesWhenSplittingAttributes() {
    let _clock = clock();
    let cookie = from_string("foo=\"a;b\"; path=/x; secure; httponly");

    assert_eq!(cookie.value(), s("\"a").as_ref());
    assert_eq!(cookie.path().as_bytes(), b"/x");
    assert!(cookie.is_secure());
    assert!(cookie.is_http_only());
}

#[test]
fn testFromStringKeepsQuotesInAttributeValues() {
    let _clock = clock();
    let cookie = from_string(
        "foo=bar; Path=\"/x\"; Domain=\"example.com\"; SameSite=\"Lax\"; Max-Age=\"10\"",
    );

    assert_eq!(cookie.path().as_bytes(), b"\"/x\"");
    assert_eq!(cookie.domain(), s("\"example.com\"").as_ref());
    assert_eq!(cookie.same_site(), None);
    assert_eq!(0, cookie.expires_time());
}

#[test]
fn testFromStringIgnoresQuotesInExpires() {
    let _clock = clock();
    assert_eq!(
        ts("Fri, 20 May 2011 15:25:52 GMT"),
        from_string("foo=bar; Expires=\"Fri, 20 May 2011 15:25:52 GMT\"").expires_time()
    );
    assert_eq!(
        1_234_567_890,
        from_string("foo=bar; Expires=\"1234567890\"").expires_time()
    );
}

#[test]
fn testFromStringUsesTheDefaultPathForAPathWithoutValue() {
    let _clock = clock();
    assert_eq!(from_string("foo=bar; Path; Secure").path().as_bytes(), b"/");
}

#[test]
fn testFromStringIgnoresInvalidExpires() {
    let _clock = clock();
    let cookie = from_string("foo=bar; Expires=invalid; Secure; HttpOnly");

    assert_eq!(0, cookie.expires_time());
    assert!(cookie.is_secure());
    assert!(cookie.is_http_only());
}

#[test]
fn testFromStringReadsTimestampExpires() {
    let _clock = clock();
    let expire = time() + 3600;

    assert_eq!(
        expire,
        from_string(&format!("foo=bar; Expires={expire}")).expires_time()
    );
    assert_eq!(
        1_234_567_890,
        from_string("foo=bar; Expires=1234567890").expires_time()
    );
    assert_eq!(
        2_000_000_000,
        from_string("foo=bar; Expires=2000000000").expires_time()
    );
    assert_eq!(0, from_string("foo=bar; Expires=0").expires_time());
}

#[test]
fn testFromStringIgnoresInvalidSameSite() {
    let _clock = clock();
    let cookie = from_string("foo=bar; SameSite=future; Secure; HttpOnly");

    assert_eq!(cookie.same_site(), None);
    assert!(cookie.is_secure());
    assert!(cookie.is_http_only());
}

#[test]
fn testFromStringIgnoresRawAttribute() -> TestResult {
    let _clock = clock();
    for (decode, raw) in [(false, "0"), (true, "1")] {
        let cookie = Cookie::from_string(format!("foo=bar%20baz; raw={raw}").as_bytes(), decode)?;

        assert_eq!(!decode, cookie.is_raw());
        assert!(cookie.to_string().starts_with("foo=bar%20baz;"));
    }
    Ok(())
}

#[test]
fn testFromStringKeepsValidAttributesBeforeInvalidDuplicates() {
    let _clock = clock();
    let cookie = from_string(
        "foo=bar; Expires=Fri, 20 May 2011 15:25:52 GMT; Expires=invalid; SameSite=Lax; SameSite=future; Max-Age=123abc",
    );

    assert_eq!(ts("Fri, 20 May 2011 15:25:52 GMT"), cookie.expires_time());
    assert_eq!(Some(SameSite::Lax), cookie.same_site());
}

#[test]
fn testFromStringHandlesOverflowingMaxAge() {
    let _clock = clock();
    let cookie = from_string("foo=bar; Max-Age=9999999999999999999999999999999999999999");

    assert_eq!(i64::MAX, cookie.expires_time());
}

#[test]
fn testFromStringExpiresNonPositiveMaxAge() {
    let _clock = clock();
    for max_age in ["0", "-1", "-9999999999999999999999999999999999999999"] {
        let cookie = from_string(&format!("foo=bar; Max-Age={max_age}"));

        assert!(cookie.is_cleared(), "{max_age}");
        assert_eq!(0, cookie.max_age(), "{max_age}");
        assert!(cookie.to_string().contains("; Max-Age=0"), "{max_age}");
    }
}

#[test]
fn testFromStringIgnoresInvalidMaxAge() {
    let _clock = clock();
    for max_age in ["123abc", "1.5", "+123", "invalid", "", "--1"] {
        let cookie = from_string(&format!("foo=bar; Max-Age={max_age}"));

        assert_eq!(0, cookie.expires_time(), "{max_age:?}");
    }
}

#[test]
fn testSameSiteAttribute() -> TestResult {
    let _clock = clock();
    let cookie = create(
        "foo",
        Some("bar"),
        0,
        Some("/"),
        None,
        Some(false),
        true,
        false,
        Some("Lax"),
    )?;
    assert_eq!(Some(SameSite::Lax), cookie.same_site());

    let cookie = create(
        "foo",
        Some("bar"),
        0,
        Some("/"),
        None,
        Some(false),
        true,
        false,
        Some(""),
    )?;
    assert_eq!(None, cookie.same_site());

    let cookie = create_name("foo").with_same_site(Some("Lax"))?;
    assert_eq!(Some(SameSite::Lax), cookie.same_site());
    Ok(())
}

#[test]
fn testSetSecureDefault() {
    let _clock = clock();
    let mut cookie = create_value("foo", "bar");

    assert!(!cookie.is_secure());

    cookie.set_secure_default(true);

    assert!(cookie.is_secure());

    cookie.set_secure_default(false);

    assert!(!cookie.is_secure());
}

#[test]
fn testMaxAge() {
    let _clock = clock();
    let future_date_one_hour = gmdate("D, d M Y H:i:s T", time() + 3600);

    let cookie = from_string("foo=bar; Max-Age=3600; path=/");
    assert_eq!(
        format!("foo=bar; expires={future_date_one_hour}; Max-Age=3600; path=/"),
        cookie.to_string()
    );

    let cookie = from_string(&format!(
        "foo=bar; expires={future_date_one_hour}; Max-Age=3600; path=/"
    ));
    assert_eq!(
        format!("foo=bar; expires={future_date_one_hour}; Max-Age=3600; path=/"),
        cookie.to_string()
    );

    let future_date_half_hour = gmdate("D, d M Y H:i:s T", time() + 1800);

    // Max-Age value takes precedence before expires
    let cookie = from_string(&format!(
        "foo=bar; expires={future_date_half_hour}; Max-Age=3600; path=/"
    ));
    assert_eq!(
        format!("foo=bar; expires={future_date_one_hour}; Max-Age=3600; path=/"),
        cookie.to_string()
    );
}

#[test]
fn testExpiredWithMaxAge() {
    let _clock = clock();
    let cookie = from_string("foo=bar; expires=Fri, 20 May 2011 15:25:52 GMT; Max-Age=0; path=/");
    assert_eq!(
        "foo=bar; expires=Fri, 20 May 2011 15:25:52 GMT; Max-Age=0; path=/",
        cookie.to_string()
    );

    let future_date = gmdate("D, d-M-Y H:i:s T", time() + 864_000);

    let cookie = from_string(&format!(
        "foo=bar; expires={future_date}; Max-Age=0; path=/"
    ));
    assert_eq!(time(), cookie.expires_time());
    assert_eq!(
        format!(
            "foo=bar; expires={}; Max-Age=0; path=/",
            gmdate("D, d M Y H:i:s T", cookie.expires_time())
        ),
        cookie.to_string()
    );
}
