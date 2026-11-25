//! AcceptHeaderTest.php at symfony/http-foundation
//! 30fa3efe6e736b0d06f5331cc6abe813582c0a26. Names and assertions are
//! PHPUnit's; a provider's cases run in a loop, every row kept.
//!
//! Upstream: symfony/http-foundation Tests/AcceptHeaderTest.php
#![allow(non_snake_case)]
#![allow(
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    reason = "test code ports PHPUnit tests and helpers as they are"
)]

use php_value::{Key, php_array};
use symfony_http_foundation::accept_header::{AcceptHeader, AcceptHeaderItem};

type TestResult = Result<(), Box<dyn std::error::Error>>;
type Provided<T> = Result<Vec<T>, Box<dyn std::error::Error>>;

fn item(value: &str) -> Result<AcceptHeaderItem, Box<dyn std::error::Error>> {
    Ok(AcceptHeaderItem::new(value, &php_array![])?)
}

fn keys(header: &mut AcceptHeader) -> Vec<Key> {
    header.all().iter().map(|(key, _)| key.clone()).collect()
}

fn key_list(values: &[&str]) -> Vec<Key> {
    values
        .iter()
        .map(|v| Key::from_bytes(v.as_bytes()))
        .collect()
}

#[test]
fn testFirst() -> TestResult {
    let mut header = AcceptHeader::from_string(Some(
        b"text/plain; q=0.5, text/html, text/x-dvi; q=0.8, text/x-c",
    ));
    assert_eq!(
        Some(&b"text/html"[..]),
        header.first().map(AcceptHeaderItem::value)
    );
    Ok(())
}

fn provide_from_string_data() -> Provided<(&'static str, Vec<AcceptHeaderItem>)> {
    Ok(vec![
        ("", vec![]),
        (";;;", vec![]),
        ("0", vec![item("0")?]),
        ("gzip", vec![item("gzip")?]),
        (
            "gzip,deflate,sdch",
            vec![item("gzip")?, item("deflate")?, item("sdch")?],
        ),
        (
            "gzip, deflate\t,sdch",
            vec![item("gzip")?, item("deflate")?, item("sdch")?],
        ),
        (
            "\"this;should,not=matter\"",
            vec![item("this;should,not=matter")?],
        ),
    ])
}

#[test]
fn testFromString() -> TestResult {
    for (string, items) in provide_from_string_data()? {
        let mut header = AcceptHeader::from_string(Some(string.as_bytes()));
        let mut parsed: Vec<AcceptHeaderItem> =
            header.all().iter().map(|(_, item)| item.clone()).collect();
        // reset index since the fixtures don't have them set
        for item in &mut parsed {
            item.set_index(0);
        }
        assert_eq!(items, parsed, "{string:?}");
    }
    Ok(())
}

fn provide_to_string_data() -> Provided<(Vec<AcceptHeaderItem>, &'static str)> {
    Ok(vec![
        (vec![], ""),
        (vec![item("gzip")?], "gzip"),
        (
            vec![item("gzip")?, item("deflate")?, item("sdch")?],
            "gzip,deflate,sdch",
        ),
        (
            vec![item("this;should,not=matter")?],
            "this;should,not=matter",
        ),
    ])
}

#[test]
fn testToString() -> TestResult {
    for (items, string) in provide_to_string_data()? {
        let header = AcceptHeader::new(items);
        assert_eq!(string.as_bytes(), header.to_php_string()?.as_bytes());
    }
    Ok(())
}

fn provide_filter_data() -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    vec![(
        "fr-FR,fr;q=0.8,en-US;q=0.6,en;q=0.4",
        "/fr.*/",
        vec!["fr-FR", "fr"],
    )]
}

#[test]
fn testFilter() -> TestResult {
    for (string, filter, values) in provide_filter_data() {
        let mut header =
            AcceptHeader::from_string(Some(string.as_bytes())).filter(filter.as_bytes())?;
        assert_eq!(key_list(&values), keys(&mut header));
    }
    Ok(())
}

fn provide_sorting_data() -> Vec<(&'static str, &'static str, Vec<&'static str>)> {
    vec![
        (
            "quality has priority",
            "*;q=0.3,ISO-8859-1,utf-8;q=0.7",
            vec!["ISO-8859-1", "utf-8", "*"],
        ),
        (
            "order matters when q is equal",
            "*;q=0.3,ISO-8859-1;q=0.7,utf-8;q=0.7",
            vec!["ISO-8859-1", "utf-8", "*"],
        ),
        (
            "order matters when q is equal2",
            "*;q=0.3,utf-8;q=0.7,ISO-8859-1;q=0.7",
            vec!["utf-8", "ISO-8859-1", "*"],
        ),
        (
            "additional attributes like \"format\" should be handled according RFC 9110",
            "text/*;q=0.3, text/plain;q=0.7, text/plain;format=flowed, text/plain;format=fixed;q=0.4, */*;q=0.5",
            vec![
                "text/plain;format=flowed",
                "text/plain",
                "*/*",
                "text/plain;format=fixed",
                "text/*",
            ],
        ),
        (
            "additional attributes like \"format\" should be handled according obsoleted RFC 7231 as well",
            "text/*;q=0.3, text/html;q=0.7, text/html;level=1, text/html;level=2;q=0.4, */*;q=0.5",
            vec![
                "text/html;level=1",
                "text/html",
                "*/*",
                "text/html;level=2",
                "text/*",
            ],
        ),
    ]
}

#[test]
fn testSorting() -> TestResult {
    for (name, string, values) in provide_sorting_data() {
        let mut header = AcceptHeader::from_string(Some(string.as_bytes()));
        assert_eq!(key_list(&values), keys(&mut header), "{name}");
    }
    Ok(())
}

fn provide_default_value_data() -> Vec<(&'static str, &'static str, Option<f64>)> {
    vec![
        (
            "text/plain;q=0.5, text/html, text/x-dvi;q=0.8, *;q=0.3",
            "text/xml",
            Some(0.3),
        ),
        (
            "text/plain;q=0.5, text/html, text/x-dvi;q=0.8, */*;q=0.3",
            "text/xml",
            Some(0.3),
        ),
        (
            "text/plain;q=0.5, text/html, text/x-dvi;q=0.8, */*;q=0.3",
            "text/html",
            Some(1.0),
        ),
        (
            "text/plain;q=0.5, text/html, text/x-dvi;q=0.8, */*;q=0.3",
            "text/plain",
            Some(0.5),
        ),
        (
            "text/plain;q=0.5, text/html, text/x-dvi;q=0.8, */*;q=0.3",
            "*",
            Some(0.3),
        ),
        (
            "text/plain;q=0.5, text/html, text/x-dvi;q=0.8, */*",
            "*",
            Some(1.0),
        ),
        (
            "text/plain;q=0.5, text/html, text/x-dvi;q=0.8, */*",
            "text/xml",
            Some(1.0),
        ),
        (
            "text/plain;q=0.5, text/html, text/x-dvi;q=0.8, */*",
            "text/*",
            Some(1.0),
        ),
        (
            "text/plain;q=0.5, text/html, text/*;q=0.8, */*",
            "text/*",
            Some(0.8),
        ),
        (
            "text/plain;q=0.5, text/html, text/*;q=0.8, */*",
            "text/html",
            Some(1.0),
        ),
        (
            "text/plain;q=0.5, text/html, text/*;q=0.8, */*",
            "text/x-dvi",
            Some(0.8),
        ),
        ("*;q=0.3, ISO-8859-1;q=0.7, utf-8;q=0.7", "*", Some(0.3)),
        ("*;q=0.3, ISO-8859-1;q=0.7, utf-8;q=0.7", "utf-8", Some(0.7)),
        (
            "*;q=0.3, ISO-8859-1;q=0.7, utf-8;q=0.7",
            "SHIFT_JIS",
            Some(0.3),
        ),
        // additional attributes like "format" should be handled according RFC 9110
        (
            "text/*;q=0.3, text/plain;q=0.7, text/plain;format=flowed, text/plain;format=fixed;q=0.4, */*;q=0.5",
            "text/plain;format=flowed",
            Some(1.0),
        ),
        // additional attributes like "format" should be handled according obsoleted RFC 7231 as well
        (
            "text/*;q=0.3, text/html;q=0.7, text/html;level=1, text/html;level=2;q=0.4, */*;q=0.5",
            "text/html;level=1",
            Some(1.0),
        ),
        // Edge cases for case-insensitivity
        (
            "text/plain;format=flowed;q=0.8, text/plain;Format=fixed",
            "text/plain;format=fixed",
            Some(1.0),
        ),
        (
            "text/plain;Charset=utf-8",
            "text/plain;charset=utf-8",
            Some(1.0),
        ),
        // Quoted values and specials
        (
            "text/plain;param=\"value with space\"",
            "text/plain;param=\"value with space\"",
            Some(1.0),
        ),
        (
            "text/plain;param=\"value\\with\\backslash\"",
            "text/plain;param=\"value\\with\\backslash\"",
            Some(1.0),
        ),
        (
            "text/plain;param=\"value with space\"",
            "text/plain;param=value with space",
            Some(1.0),
        ),
        // Flag params or empty
        ("text/plain;flowed;q=0.9", "text/plain;flowed", Some(0.9)),
        ("text/plain;param=", "text/plain;param=\"\"", Some(1.0)),
        ("text/plain;flowed", "text/plain", None),
        // Extra params in query
        (
            "text/plain;format=flowed",
            "text/plain;format=flowed;charset=utf-8",
            Some(1.0),
        ),
        ("text/plain;format=flowed", "text/plain;charset=utf-8", None),
        (
            "text/*;format=flowed",
            "text/plain;format=flowed",
            Some(1.0),
        ),
        ("text/*;format=flowed", "text/plain", None),
        // Wildcards and specificity
        (
            "*/*;q=0.1, text/*;format=flowed;q=0.5, text/plain;q=0.8",
            "text/plain;format=flowed",
            Some(0.8),
        ),
        ("*/*;param=test", "text/plain;param=test", Some(1.0)),
        ("*/*;param=test", "text/plain", None),
        // Non-media types
        ("utf-8;q=0.9, *;q=0.5", "iso-8859-1", Some(0.5)),
        ("*;q=0.5", "en-US", Some(0.5)),
        ("*/*;q=0.5", "utf-8", Some(0.5)),
        // Ties and duplicates
        (
            "text/plain;format=flowed;q=0.8, text/plain;format=flowed;q=0.8",
            "text/plain;format=flowed",
            Some(0.8),
        ),
        (
            "text/plain;q=0.5, text/plain;format=flowed;q=0.5",
            "text/plain;format=flowed;extra=foo",
            Some(0.5),
        ),
        // Invalid/malformed
        ("text", "text", Some(1.0)),
        ("text/", "text/plain", None),
        ("", "text/plain", None),
        // Mixed case types
        (
            "Text/Plain;Format=flowed",
            "text/plain;format=flowed",
            Some(1.0),
        ),
        ("UTF-8;q=0.9", "utf-8", Some(0.9)),
    ]
}

#[test]
fn testDefaultValue() -> TestResult {
    for (accept_header, value, expected_quality) in provide_default_value_data() {
        let header = AcceptHeader::from_string(Some(accept_header.as_bytes()));
        assert_eq!(
            expected_quality,
            header.get(value.as_bytes())?.map(AcceptHeaderItem::quality),
            "{accept_header:?} {value:?}"
        );
    }
    Ok(())
}
