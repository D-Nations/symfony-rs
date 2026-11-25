//! HeaderBagTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Names and assertions are
//! PHPUnit's. `addCacheControlDirective('max-age', 10)` passes the int as
//! PHP's `bool|string` parameter receives it, `"10"`.
//!
//! Upstream: symfony/http-foundation Tests/HeaderBagTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

use php_value::{Key, PhpString, Value};
use symfony_http_foundation::header_bag::{HeaderBag, HeaderValues, Values};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn s(text: &str) -> Option<PhpString> {
    Some(PhpString::from(text))
}

fn list(values: &[&str]) -> Values {
    values.iter().map(|v| s(v)).collect()
}

/// `$bag->all()` as `[name => values]` pairs.
fn all(bag: &HeaderBag) -> Vec<(Key, Values)> {
    bag.all()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

fn entry(name: &str, values: &[&str]) -> (Key, Values) {
    (Key::from_bytes(name.as_bytes()), list(values))
}

#[test]
fn testConstructor() {
    let bag = HeaderBag::new([("foo", "bar")]);
    assert!(bag.has("foo"));
}

#[test]
fn testToStringNull() {
    let bag = HeaderBag::new::<&str, &str>([]);
    assert_eq!("", bag.to_string());
}

#[test]
fn testToStringNotNull() {
    let bag = HeaderBag::new([("foo", "bar")]);
    assert_eq!("Foo: bar\r\n", bag.to_string());
}

#[test]
fn testKeys() {
    let bag = HeaderBag::new([("foo", "bar")]);
    let keys = bag.keys();
    assert_eq!(Key::from_bytes(b"foo"), keys[0]);
}

#[test]
fn testGetDate() -> TestResult {
    let bag = HeaderBag::new([("foo", "Tue, 4 Sep 2012 20:00:00 +0200")]);
    let header_date = bag.get_date("foo", None)?;
    assert!(header_date.is_some());
    Ok(())
}

#[test]
fn testGetDateNull() -> TestResult {
    let bag = HeaderBag::new([("foo", None::<&str>)]);
    let header_date = bag.get_date("foo", None)?;
    assert!(header_date.is_none());
    Ok(())
}

#[test]
fn testGetDateException() {
    let bag = HeaderBag::new([("foo", "Tue")]);
    let e = bag.get_date("foo", None).unwrap_err();
    assert!(e.is_a("RuntimeException"));
}

#[test]
fn testGetCacheControlHeader() -> TestResult {
    let mut bag = HeaderBag::new::<&str, &str>([]);
    bag.add_cache_control_directive("public", "#a")?;
    assert!(bag.has_cache_control_directive("public"));
    assert_eq!(Value::from("#a"), bag.get_cache_control_directive("public"));
    Ok(())
}

#[test]
fn testAll() {
    let bag = HeaderBag::new([("foo", "bar")]);
    assert_eq!(
        vec![entry("foo", &["bar"])],
        all(&bag),
        "->all() gets all the input"
    );

    let bag = HeaderBag::new([("FOO", "BAR")]);
    assert_eq!(
        vec![entry("foo", &["BAR"])],
        all(&bag),
        "->all() gets all the input key are lower case"
    );
}

#[test]
fn testReplace() -> TestResult {
    let mut bag = HeaderBag::new([("foo", "bar")]);

    bag.replace([("NOPE", "BAR")])?;
    assert_eq!(
        vec![entry("nope", &["BAR"])],
        all(&bag),
        "->replace() replaces the input with the argument"
    );
    assert!(
        !bag.has("foo"),
        "->replace() overrides previously set the input"
    );
    Ok(())
}

#[test]
fn testGet() -> TestResult {
    let mut bag = HeaderBag::new([("foo", "bar"), ("fuzz", "bizz")]);
    assert_eq!(s("bar"), bag.get("foo", None), "->get return current value");
    assert_eq!(
        s("bar"),
        bag.get("FoO", None),
        "->get key in case-insensitive"
    );
    assert_eq!(
        list(&["bar"]),
        bag.all_of("foo"),
        "->get return the value as array"
    );

    // defaults
    assert_eq!(
        None,
        bag.get("none", None),
        "->get unknown values returns null"
    );
    assert_eq!(
        s("default"),
        bag.get("none", Some("default")),
        "->get unknown values returns default"
    );
    assert_eq!(
        Values::new(),
        bag.all_of("none"),
        "->get unknown values returns an empty array"
    );

    bag.set(b"foo", "bor", false)?;
    assert_eq!(s("bar"), bag.get("foo", None), "->get return first value");
    assert_eq!(
        list(&["bar", "bor"]),
        bag.all_of("foo"),
        "->get return all values as array"
    );

    bag.set(b"baz", HeaderValues::One(None), true)?;
    assert_eq!(
        None,
        bag.get("baz", Some("nope")),
        "->get return null although different default value is given"
    );
    Ok(())
}

#[test]
fn testSetAssociativeArray() -> TestResult {
    let mut bag = HeaderBag::new::<&str, &str>([]);
    // ['bad-assoc-index' => 'value']: the keys are ignored
    bag.set(b"foo", vec!["value"], true)?;
    assert_eq!(s("value"), bag.get("foo", None));
    assert_eq!(
        list(&["value"]),
        bag.all_of("foo"),
        "assoc indices of multi-valued headers are ignored"
    );
    Ok(())
}

#[test]
fn testContains() -> TestResult {
    let mut bag = HeaderBag::new([("foo", "bar"), ("fuzz", "bizz")]);
    assert!(bag.contains("foo", "bar"), "->contains first value");
    assert!(bag.contains("fuzz", "bizz"), "->contains second value");
    assert!(!bag.contains("nope", "nope"), "->contains unknown value");
    assert!(!bag.contains("foo", "nope"), "->contains unknown value");

    // Multiple values
    bag.set(b"foo", "bor", false)?;
    assert!(bag.contains("foo", "bar"), "->contains first value");
    assert!(bag.contains("foo", "bor"), "->contains second value");
    assert!(!bag.contains("foo", "nope"), "->contains unknown value");
    Ok(())
}

#[test]
fn testCacheControlDirectiveAccessors() -> TestResult {
    let mut bag = HeaderBag::new::<&str, &str>([]);
    bag.add_cache_control_directive("public", true)?;

    assert!(bag.has_cache_control_directive("public"));
    assert_eq!(Value::Bool(true), bag.get_cache_control_directive("public"));
    assert_eq!(s("public"), bag.get("cache-control", None));

    bag.add_cache_control_directive("max-age", "10")?;
    assert!(bag.has_cache_control_directive("max-age"));
    assert_eq!(
        Value::from("10"),
        bag.get_cache_control_directive("max-age")
    );
    assert_eq!(s("max-age=10, public"), bag.get("cache-control", None));

    bag.remove_cache_control_directive("max-age")?;
    assert!(!bag.has_cache_control_directive("max-age"));
    Ok(())
}

#[test]
fn testCacheControlDirectiveParsing() -> TestResult {
    let mut bag = HeaderBag::new([("cache-control", "public, max-age=10")]);
    assert!(bag.has_cache_control_directive("public"));
    assert_eq!(Value::Bool(true), bag.get_cache_control_directive("public"));

    assert!(bag.has_cache_control_directive("max-age"));
    assert_eq!(
        Value::from("10"),
        bag.get_cache_control_directive("max-age")
    );

    bag.add_cache_control_directive("s-maxage", "100")?;
    assert_eq!(
        s("max-age=10, public, s-maxage=100"),
        bag.get("cache-control", None)
    );
    Ok(())
}

#[test]
fn testCacheControlDirectiveParsingQuotedZero() {
    let bag = HeaderBag::new([("cache-control", "max-age=\"0\"")]);
    assert!(bag.has_cache_control_directive("max-age"));
    assert_eq!(Value::from("0"), bag.get_cache_control_directive("max-age"));
}

#[test]
fn testCacheControlDirectiveOverrideWithReplace() -> TestResult {
    let mut bag = HeaderBag::new([("cache-control", "private, max-age=100")]);
    bag.replace([("cache-control", "public, max-age=10")])?;
    assert!(bag.has_cache_control_directive("public"));
    assert_eq!(Value::Bool(true), bag.get_cache_control_directive("public"));

    assert!(bag.has_cache_control_directive("max-age"));
    assert_eq!(
        Value::from("10"),
        bag.get_cache_control_directive("max-age")
    );
    Ok(())
}

#[test]
fn testCacheControlClone() {
    let headers = [("foo", "bar")];
    let bag1 = HeaderBag::new(headers);
    let bag2 = HeaderBag::new(bag1.all().iter().map(|(k, v)| {
        (
            k.to_value()
                .to_php_string()
                .unwrap_or_default()
                .as_bytes()
                .to_vec(),
            HeaderValues::Many(v.clone()),
        )
    }));

    assert_eq!(all(&bag1), all(&bag2));
}

#[test]
fn testGetIterator() {
    let headers = [("foo", "bar"), ("hello", "world"), ("third", "charm")];
    let header_bag = HeaderBag::new(headers);

    let mut i = 0;
    for (key, val) in header_bag.iter() {
        i += 1;
        let expected = headers
            .iter()
            .find(|(name, _)| Key::from_bytes(name.as_bytes()) == *key)
            .map(|(_, value)| list(&[value]));
        assert_eq!(expected.as_ref(), Some(val));
    }

    assert_eq!(headers.len(), i);
}

#[test]
fn testCount() {
    let headers = [("foo", "bar"), ("HELLO", "WORLD")];
    let header_bag = HeaderBag::new(headers);

    assert_eq!(headers.len(), header_bag.count());
}
