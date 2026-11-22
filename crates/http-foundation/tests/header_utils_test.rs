//! HeaderUtilsTest.php at symfony/http-foundation
//! 79325f87c3a2acc7564ddce9f457046e980e7255. Names and assertions are
//! PHPUnit's; a provider's cases run in a loop. makeDisposition and
//! parseQuery are not ported (header_utils's module doc), so their tests
//! stay pending.
//!
//! Upstream: symfony/http-foundation Tests/HeaderUtilsTest.php
#![allow(non_snake_case)]

use php_value::{PhpArray, PhpString, Value, php_array};
use symfony_http_foundation::header_utils::{combine, quote, split, to_string, unquote};

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
