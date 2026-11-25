//! HeaderUtilsTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Names and assertions are
//! PHPUnit's; a provider's cases run in a loop. `http_build_query()` is
//! `php_value::url`'s.
//!
//! Upstream: symfony/http-foundation Tests/HeaderUtilsTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::needless_pass_by_value,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

use php_value::{PhpArray, PhpString, Value, php_array, url};
use symfony_http_foundation::header_utils::{
    combine, make_disposition, parse_query, quote, split, to_string, unquote,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// A PHP list of strings or nested lists, written as the provider's
/// array literals.
#[derive(Clone)]
enum E {
    S(&'static str),
    L(Vec<E>),
}

use E::{L, S};

fn value(e: &E) -> Value {
    match e {
        S(s) => Value::from(*s),
        L(items) => Value::Array(PhpArray::list(items.iter().map(value))),
    }
}

fn list(items: Vec<E>) -> PhpArray {
    PhpArray::list(items.iter().map(value))
}

fn provide_header_to_split() -> Vec<(PhpArray, &'static str, &'static str)> {
    let cookie = |v: &'static str| {
        L(vec![
            L(vec![S("foo_cookie"), S(v)]),
            L(vec![S("expires"), S("Tue, 22-Sep-2020 06:27:09 GMT")]),
            L(vec![S("path"), S("/")]),
        ])
    };
    let rows: Vec<(E, &'static str, &'static str)> = vec![
        (L(vec![S("foo=123"), S("bar")]), "foo=123,bar", ","),
        (L(vec![S("foo=123"), S("bar")]), "foo=123, bar", ","),
        (
            L(vec![L(vec![S("foo=123"), S("bar")])]),
            "foo=123; bar",
            ",;",
        ),
        (
            L(vec![L(vec![S("foo=123")]), L(vec![S("bar")])]),
            "foo=123, bar",
            ",;",
        ),
        (L(vec![S("foo"), S("123, bar")]), "foo=123, bar", "="),
        (L(vec![S("foo"), S("123, bar")]), " foo = 123, bar ", "="),
        (
            L(vec![L(vec![S("foo"), S("123")]), L(vec![S("bar")])]),
            "foo=123, bar",
            ",=",
        ),
        (
            L(vec![
                L(vec![L(vec![S("foo"), S("123")])]),
                L(vec![L(vec![S("bar")]), L(vec![S("foo"), S("456")])]),
            ]),
            "foo=123, bar;; foo=456",
            ",;=",
        ),
        (
            L(vec![L(vec![L(vec![S("foo"), S("a,b;c=d")])])]),
            "foo=\"a,b;c=d\"",
            ",;=",
        ),
        (L(vec![S("foo"), S("bar")]), "foo,,,, bar", ","),
        (L(vec![S("foo"), S("bar")]), ",foo, bar,", ","),
        (L(vec![S("foo"), S("bar")]), " , foo, bar, ", ","),
        (L(vec![S("foo bar")]), "foo \"bar\"", ","),
        (L(vec![S("foo bar")]), "\"foo\" bar", ","),
        (L(vec![S("foo bar")]), "\"foo\" \"bar\"", ","),
        (L(vec![S("foo"), S("bar")]), "foo bar", " "),
        (L(vec![S("foo"), S("bar")]), "foo   bar", " "),
        (
            L(vec![L(vec![S("foo"), S("bar")]), L(vec![S("baz")])]),
            "foo bar, baz",
            ", ",
        ),
        (L(vec![S("foo bar"), S("baz")]), "foo bar\tbaz", "\t"),
        (
            cookie("foo=1&bar=2&baz=3"),
            "foo_cookie=foo=1&bar=2&baz=3; expires=Tue, 22-Sep-2020 06:27:09 GMT; path=/",
            ";=",
        ),
        (
            cookie("foo=="),
            "foo_cookie=foo==; expires=Tue, 22-Sep-2020 06:27:09 GMT; path=/",
            ";=",
        ),
        (
            cookie("foo="),
            "foo_cookie=foo=; expires=Tue, 22-Sep-2020 06:27:09 GMT; path=/",
            ";=",
        ),
        (
            cookie("foo=a=b"),
            "foo_cookie=foo=\"a=b\"; expires=Tue, 22-Sep-2020 06:27:09 GMT; path=/",
            ";=",
        ),
        // These are not a valid header values. We test that they parse anyway,
        // and that both the valid and invalid parts are returned.
        (L(vec![]), "", ","),
        (L(vec![]), ",,,", ","),
        (
            L(vec![L(vec![S(""), S("foo")]), L(vec![S("bar"), S("")])]),
            "=foo,bar=",
            ",=",
        ),
        (
            L(vec![S("foo"), S("foobar"), S("baz")]),
            "foo, foo\"bar\", \"baz",
            ",",
        ),
        (L(vec![S("foo"), S("bar, baz")]), "foo, \"bar, baz", ","),
        (L(vec![S("foo"), S("bar, baz\\")]), "foo, \"bar, baz\\", ","),
        (
            L(vec![S("foo"), S("bar, baz\\")]),
            "foo, \"bar, baz\\\\",
            ",",
        ),
    ];
    rows.into_iter()
        .map(|(expected, header, separators)| {
            let L(items) = expected else { unreachable!() };
            (list(items), header, separators)
        })
        .collect()
}

#[test]
fn testSplit() -> TestResult {
    for (expected, header, separator) in provide_header_to_split() {
        assert_eq!(
            split(header.as_bytes(), separator.as_bytes())?,
            expected,
            "{header:?} {separator:?}"
        );
    }
    Ok(())
}

#[test]
fn testCombine() -> TestResult {
    let mut foo_true = PhpArray::new();
    foo_true.insert("foo", Value::Bool(true));
    assert_eq!(
        combine(&list(vec![L(vec![S("foo"), S("123")])]))?,
        php_array!["foo" => "123"]
    );
    assert_eq!(combine(&list(vec![L(vec![S("foo")])]))?, foo_true);
    assert_eq!(combine(&list(vec![L(vec![S("Foo")])]))?, foo_true);
    assert_eq!(
        combine(&list(vec![L(vec![S("foo"), S("123")]), L(vec![S("bar")])]))?,
        php_array!["foo" => "123", "bar" => true]
    );
    Ok(())
}

#[test]
fn testToString() -> TestResult {
    let p = |s: &str| PhpString::from(s);
    assert_eq!(to_string(&php_array!["foo" => true], b",")?, p("foo"));
    assert_eq!(
        to_string(&php_array!["foo" => true, "bar" => true], b";")?,
        p("foo; bar")
    );
    assert_eq!(to_string(&php_array!["foo" => "123"], b",")?, p("foo=123"));
    assert_eq!(
        to_string(&php_array!["foo" => "1 2 3"], b",")?,
        p("foo=\"1 2 3\"")
    );
    assert_eq!(
        to_string(&php_array!["foo" => "1 2 3", "bar" => true], b",")?,
        p("foo=\"1 2 3\", bar")
    );
    Ok(())
}

#[test]
fn testQuote() {
    let p = |s: &str| PhpString::from(s);
    assert_eq!(quote(b"foo"), p("foo"));
    assert_eq!(quote(b"az09!#$%&'*.^_`|~-"), p("az09!#$%&'*.^_`|~-"));
    assert_eq!(quote(b"foo bar"), p("\"foo bar\""));
    assert_eq!(quote(b"foo [bar]"), p("\"foo [bar]\""));
    assert_eq!(quote(b"foo \"bar\""), p("\"foo \\\"bar\\\"\""));
    assert_eq!(quote(b"foo \\ bar"), p("\"foo \\\\ bar\""));
}

#[test]
fn testUnquote() {
    let p = |s: &str| PhpString::from(s);
    assert_eq!(unquote(b"foo"), p("foo"));
    assert_eq!(unquote(b"az09!#$%&'*.^_`|~-"), p("az09!#$%&'*.^_`|~-"));
    assert_eq!(unquote(b"\"foo bar\""), p("foo bar"));
    assert_eq!(unquote(b"\"foo [bar]\""), p("foo [bar]"));
    assert_eq!(unquote(b"\"foo \\\"bar\\\"\""), p("foo \"bar\""));
    assert_eq!(unquote(b"\"foo \\\"\\b\\a\\r\\\"\""), p("foo \"bar\""));
    assert_eq!(unquote(b"\"foo \\\\ bar\""), p("foo \\ bar"));
}

/// GHSA-r925-mgvr-h9cc (CVE-2026-108082), fixed upstream in 8.1.7: a run
/// of whitespace before a separator made PCRE backtrack. The tokenizer
/// scans each byte a bounded number of times; this pins the fixed
/// results.
#[test]
fn testSplitIsNotAmbiguousAboutWhitespaceBeforeSeparators() -> TestResult {
    let spaces = " ".repeat(25);
    assert_eq!(
        split(format!("a{spaces},b").as_bytes(), b",=")?,
        list(vec![L(vec![S("a")]), L(vec![S("b")])])
    );
    assert_eq!(
        split(format!("a{spaces},b").as_bytes(), b",;=")?,
        list(vec![L(vec![L(vec![S("a")])]), L(vec![L(vec![S("b")])])])
    );
    assert_eq!(
        split(format!("x=1{spaces};y=2").as_bytes(), b";=")?,
        list(vec![L(vec![S("x"), S("1")]), L(vec![S("y"), S("2")])])
    );

    let header = format!("a{spaces},").repeat(200);
    assert_eq!(split(header.as_bytes(), b",=")?.len(), 200);

    // whitespace inside a value is still part of it
    assert_eq!(split(b"a b  c", b",=")?, list(vec![L(vec![S("a b  c")])]));
    Ok(())
}

/// Not upstream: whitespace that `trim()` keeps (`\f`) and no separator,
/// in a long run, is skipped in one step (the split stays linear).
#[test]
fn long_whitespace_runs_split_in_linear_time() -> TestResult {
    let header = format!("a{}", "\x0c".repeat(200_000));
    let started = std::time::Instant::now();
    assert_eq!(
        split(header.as_bytes(), b",=")?,
        list(vec![L(vec![S("a")])])
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(5));
    Ok(())
}

#[test]
fn testMakeDispositionInvalidDisposition() {
    let e = make_disposition(b"invalid", b"foo.html", b"").unwrap_err();
    assert!(e.is_a("InvalidArgumentException"));
}

fn provide_make_disposition() -> Vec<(&'static str, &'static str, &'static str, &'static str)> {
    vec![
        (
            "attachment",
            "foo.html",
            "foo.html",
            "attachment; filename=foo.html",
        ),
        (
            "attachment",
            "foo.html",
            "",
            "attachment; filename=foo.html",
        ),
        (
            "attachment",
            "foo bar.html",
            "",
            "attachment; filename=\"foo bar.html\"",
        ),
        (
            "attachment",
            "foo \"bar\".html",
            "",
            "attachment; filename=\"foo \\\"bar\\\".html\"",
        ),
        (
            "attachment",
            "foo%20bar.html",
            "foo bar.html",
            "attachment; filename=\"foo bar.html\"; filename*=utf-8''foo%2520bar.html",
        ),
        (
            "attachment",
            "föö.html",
            "foo.html",
            "attachment; filename=foo.html; filename*=utf-8''f%C3%B6%C3%B6.html",
        ),
    ]
}

#[test]
fn testMakeDisposition() -> TestResult {
    for (disposition, filename, filename_fallback, expected) in provide_make_disposition() {
        assert_eq!(
            PhpString::from(expected),
            make_disposition(
                disposition.as_bytes(),
                filename.as_bytes(),
                filename_fallback.as_bytes()
            )?
        );
    }
    Ok(())
}

fn provide_make_disposition_fail() -> Vec<(&'static str, &'static str)> {
    vec![
        ("attachment", "foo%20bar.html"),
        ("attachment", "foo/bar.html"),
        ("attachment", "/foo.html"),
        ("attachment", "foo\\bar.html"),
        ("attachment", "\\foo.html"),
        ("attachment", "föö.html"),
    ]
}

#[test]
fn testMakeDispositionFail() {
    for (disposition, filename) in provide_make_disposition_fail() {
        let e = make_disposition(disposition.as_bytes(), filename.as_bytes(), b"").unwrap_err();
        assert!(e.is_a("InvalidArgumentException"), "{filename}");
    }
}

/// Not upstream here: v8.1.11's `/D` anchor, the fix that
/// `BinaryFileResponseTest`'s `testSetContentDispositionRejectsTrailingNewlineInFallback`
/// pins through `BinaryFileResponse` (not ported). v8.1.10 accepted a
/// fallback ending in one newline and put it into the header.
#[test]
fn make_disposition_rejects_a_fallback_ending_in_a_newline() {
    for fallback in [&b"foo\n"[..], b"foo\r\n", b"\n"] {
        let e = make_disposition(b"attachment", b"foo", fallback).unwrap_err();
        assert!(e.is_a("InvalidArgumentException"));
        assert_eq!(
            "The filename fallback must only contain ASCII characters.",
            e.message().to_string_lossy()
        );
    }
    // A filename ending in a newline is its own fallback, so it fails too.
    assert!(make_disposition(b"attachment", b"foo\n", b"").is_err());
}

fn provide_parse_query() -> Vec<(&'static [u8], Option<&'static str>)> {
    vec![
        (b"a=b&c=d", None),
        (b"a.b=c", None),
        (b"a+b=c", None),
        (b"a\0b=c", Some("a=")),
        (b"a%00b=c", Some("a=c")),
        (b"a[b=c", Some("a%5Bb=c")),
        (b"a]b=c", Some("a%5Db=c")),
        (b"a[b]=c", Some("a%5Bb%5D=c")),
        (b"a[b][c.d]=c", Some("a%5Bb%5D%5Bc.d%5D=c")),
        (b"a%5Bb%5D=c", None),
        (
            b"f[%2525][%26][%3D][p.c]=d",
            Some("f%5B%2525%5D%5B%26%5D%5B%3D%5D%5Bp.c%5D=d"),
        ),
    ]
}

/// `http_build_query($query, '', $separator)`.
fn build(query: &PhpArray, separator: &str) -> Result<String, Box<dyn std::error::Error>> {
    let built = url::http_build_query(query, separator.as_bytes()).ok_or("http_build_query")?;
    Ok(String::from_utf8(built)?)
}

#[test]
fn testParseQuery() -> TestResult {
    for (query, expected) in provide_parse_query() {
        let expected = match expected {
            Some(expected) => expected.to_owned(),
            None => String::from_utf8(query.to_vec())?,
        };
        assert_eq!(expected, build(&parse_query(query, false, b"&")?, "&")?);
    }
    Ok(())
}

#[test]
fn testParseCookie() -> TestResult {
    let query = "a.b=c; def%5Bg%5D=h";
    assert_eq!(
        query,
        build(&parse_query(query.as_bytes(), false, b";")?, "; ")?
    );
    Ok(())
}

#[test]
fn testParseQueryIgnoreBrackets() -> TestResult {
    assert_eq!(
        php_array!["a.b" => php_array!["A", "B"]],
        parse_query(b"a.b=A&a.b=B", true, b"&")?
    );
    assert_eq!(
        php_array!["a.b[]" => php_array!["A"]],
        parse_query(b"a.b[]=A", true, b"&")?
    );
    assert_eq!(
        php_array!["a.b[]" => php_array!["A"]],
        parse_query(b"a.b%5B%5D=A", true, b"&")?
    );
    Ok(())
}

/// PHP's warning is not reproduced at all (`php_value::vars::parse_str()`),
/// so only the result is compared.
#[test]
fn testParseQueryDoesNotWarnAboutKeysNestedBeyondMaxInputNestingLevel() -> TestResult {
    let deep_key = format!(
        "a{}",
        "[b]".repeat(php_value::vars::MAX_INPUT_NESTING_LEVEL + 1)
    );
    let query = parse_query(format!("{deep_key}=1&x=2").as_bytes(), false, b"&")?;
    assert_eq!(php_array!["x" => "2"], query);
    Ok(())
}
