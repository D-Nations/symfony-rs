//! AcceptHeaderItemTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Names and assertions are
//! PHPUnit's; a provider's cases run in a loop. PHP's coercions of the
//! arguments (`setValue(1)`, `setAttribute('q', 0.75)`) are written as the
//! strings PHP makes of them.
//!
//! Upstream: symfony/http-foundation Tests/AcceptHeaderItemTest.php
#![allow(non_snake_case)]

use php_value::{PhpArray, Value, php_array};
use symfony_http_foundation::accept_header::AcceptHeaderItem;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn provide_from_string_data() -> Vec<(&'static str, &'static str, PhpArray)> {
    vec![
        ("text/html", "text/html", php_array![]),
        (
            "\"this;should,not=matter\"",
            "this;should,not=matter",
            php_array![],
        ),
        (
            "text/plain; charset=utf-8;param=\"this;should,not=matter\";\tfootnotes=true",
            "text/plain",
            php_array!["charset" => "utf-8", "param" => "this;should,not=matter", "footnotes" => "true"],
        ),
        (
            "\"this;should,not=matter\";charset=utf-8",
            "this;should,not=matter",
            php_array!["charset" => "utf-8"],
        ),
    ]
}

#[test]
fn testFromString() -> TestResult {
    for (string, value, attributes) in provide_from_string_data() {
        let item = AcceptHeaderItem::from_string(Some(string.as_bytes()))?;
        assert_eq!(value.as_bytes(), item.value());
        assert_eq!(&attributes, item.attributes());
    }
    Ok(())
}

fn provide_to_string_data() -> Vec<(&'static str, PhpArray, &'static str)> {
    vec![
        ("text/html", php_array![], "text/html"),
        (
            "text/plain",
            php_array!["charset" => "utf-8", "param" => "this;should,not=matter", "footnotes" => "true"],
            "text/plain; charset=utf-8; param=\"this;should,not=matter\"; footnotes=true",
        ),
    ]
}

#[test]
fn testToString() -> TestResult {
    for (value, attributes, string) in provide_to_string_data() {
        let item = AcceptHeaderItem::new(value, &attributes)?;
        assert_eq!(string.as_bytes(), item.to_php_string()?.as_bytes());
    }
    Ok(())
}

#[test]
fn testValue() -> TestResult {
    let mut item = AcceptHeaderItem::new("value", &php_array![])?;
    assert_eq!(b"value", item.value());

    item.set_value("new value");
    assert_eq!(b"new value", item.value());

    // setValue(1): PHP coerces the int to "1"
    item.set_value("1");
    assert_eq!(b"1", item.value());
    Ok(())
}

#[test]
fn testQuality() -> TestResult {
    let mut item = AcceptHeaderItem::new("value", &php_array![])?;
    assert_eq!(1.0, item.quality());

    item.set_quality(0.5);
    assert_eq!(0.5, item.quality());

    // setAttribute('q', 0.75): PHP coerces the float to "0.75"
    item.set_attribute(b"q", b"0.75");
    assert_eq!(0.75, item.quality());
    assert!(!item.has_attribute(b"q"));
    Ok(())
}

#[test]
fn testAttribute() -> TestResult {
    let mut item = AcceptHeaderItem::new("value", &php_array![])?;
    assert_eq!(&php_array![], item.attributes());
    assert!(!item.has_attribute(b"test"));
    assert_eq!(None, item.attribute(b"test"));
    assert_eq!(
        Value::from("default"),
        item.attribute(b"test")
            .cloned()
            .unwrap_or(Value::from("default"))
    );

    item.set_attribute(b"test", b"value");
    assert_eq!(&php_array!["test" => "value"], item.attributes());
    assert!(item.has_attribute(b"test"));
    assert_eq!(Some(&Value::from("value")), item.attribute(b"test"));
    assert_eq!(
        Value::from("value"),
        item.attribute(b"test")
            .cloned()
            .unwrap_or(Value::from("default"))
    );
    Ok(())
}
