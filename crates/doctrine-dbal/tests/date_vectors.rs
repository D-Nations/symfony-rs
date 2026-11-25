//! The PHP date functions the Types use (`php_value::date_create`,
//! `php_value::date_interval`) against the answers PHP 8.4.27 gave in the
//! oracle (tests/data/php.json, keys `create_from_format`, `date_create`
//! and `date_interval`, `oracle/recorders/doctrine_dbal.php`). A text the
//! port reads must give PHP's answer, `false` and exceptions included; a
//! text it refuses (`Unsupported`) is counted, and the texts the Types
//! tests and database drivers give must not be among them.
//!
//! Upstream: none

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

mod common;

use common::data;
use php_value::{
    base64,
    date_create::{create_from_format, date_create},
    date_interval::DateInterval,
};
use serde_json::Value as Json;

fn text(field: &Json) -> Vec<u8> {
    base64::decode_strict(field.as_str().unwrap().as_bytes()).unwrap()
}

/// The recorded outcome as (kind, text).
fn outcome(json: &Json) -> (String, String) {
    let kind = json[0].as_str().unwrap().to_owned();
    let detail = match kind.as_str() {
        "ok" => json[1].as_str().unwrap().to_owned(),
        "throws" => format!(
            "{}: {}",
            json[1].as_str().unwrap(),
            json[2].as_str().unwrap()
        ),
        _ => String::new(),
    };
    (kind, detail)
}

#[test]
fn create_from_format_vectors() {
    let rows = data()["create_from_format"].as_array().unwrap();
    let (mut read, mut refused) = (0, 0);
    for row in rows {
        let format = row[0].as_str().unwrap();
        let input = text(&row[1]);
        let expected = outcome(&row[2]);
        let shown = String::from_utf8_lossy(&input);
        match create_from_format(format.as_bytes(), &input, None) {
            Err(_) => refused += 1,
            Ok(result) => {
                read += 1;
                let actual = match result {
                    Some(date) => ("ok".to_owned(), date.format("Y-m-d H:i:s.u e")),
                    None => ("false".to_owned(), String::new()),
                };
                assert_eq!(actual, expected, "createFromFormat({format:?}, {shown:?})");
            }
        }
    }
    eprintln!("createFromFormat: {read} read as PHP does, {refused} refused");
    assert!(read > refused, "{read} read, {refused} refused");
}

#[test]
fn date_create_vectors() {
    let rows = data()["date_create"].as_array().unwrap();
    let (mut read, mut refused) = (0, 0);
    for row in rows {
        let input = text(&row[0]);
        let expected = outcome(&row[1]);
        let shown = String::from_utf8_lossy(&input);
        let trimmed = shown.trim_matches([' ', '\t']);
        match date_create(&input, None) {
            Err(_) => refused += 1,
            Ok(result) => {
                read += 1;
                match result {
                    // The clock's time: only the zone can be compared.
                    Ok(date) if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("now") => {
                        assert_eq!(expected.0, "ok", "{shown:?}");
                        assert!(expected.1.ends_with(" UTC"), "{shown:?}");
                        assert_eq!(date.timezone().name(), "UTC");
                    }
                    Ok(date) => assert_eq!(
                        ("ok".to_owned(), date.format("Y-m-d H:i:s.u e")),
                        expected,
                        "new DateTimeImmutable({shown:?})"
                    ),
                    Err(message) => assert_eq!(
                        (
                            "throws".to_owned(),
                            format!(
                                "DateMalformedStringException: {}",
                                message.to_string_lossy()
                            )
                        ),
                        expected,
                        "new DateTimeImmutable({shown:?})"
                    ),
                }
            }
        }
    }
    eprintln!("new DateTimeImmutable(): {read} read as PHP does, {refused} refused");
}

#[test]
fn date_interval_vectors() {
    let rows = data()["date_interval"].as_array().unwrap();
    let (mut read, mut refused) = (0, 0);
    for row in rows {
        let input = text(&row[0]);
        let expected = outcome(&row[1]);
        let shown = String::from_utf8_lossy(&input);
        match DateInterval::new(&input) {
            Err(_) => refused += 1,
            Ok(result) => {
                read += 1;
                let actual = match result {
                    Ok(interval) => ("ok".to_owned(), interval.format("%R %y %m %d %h %i %s")),
                    Err(message) => (
                        "throws".to_owned(),
                        format!(
                            "DateMalformedIntervalStringException: {}",
                            message.to_string_lossy()
                        ),
                    ),
                };
                assert_eq!(actual, expected, "new DateInterval({shown:?})");
            }
        }
    }
    eprintln!("new DateInterval(): {read} read as PHP does, {refused} refused");
    assert!(read > refused, "{read} read, {refused} refused");
}

/// The texts of the upstream Types tests and the formats of DBAL's
/// platforms are read, not refused.
#[test]
fn texts_the_types_read_are_supported() {
    for (format, text) in [
        ("!Y-m-d", "1985-09-01"),
        ("!Y-m-d", "abcdefg"),
        ("!", "invalid date string"),
        ("", "invalid datetime string"),
        ("Y-m-d H:i:s", "2016-01-01 15:58:59.123456"),
        ("Y-m-d H:i:s T", "2016-01-01 15:58:59 UTC"),
        ("!H:i:s", "5:30:55"),
        ("Y-m-d H:i:sO", "2016-01-01 15:58:59+0200"),
        ("Y-m-d H:i:s.u", "2016-01-01 15:58:59.123456"),
        ("Y-m-d H:i:sP", "2016-01-01 15:58:59+02:00"),
    ] {
        assert!(
            create_from_format(format.as_bytes(), text.as_bytes(), None).is_ok(),
            "{format:?} {text:?}"
        );
    }
    for text in [
        "abcdefg",
        "invalid date string",
        "invalid date-ish string",
        "1985/09/01 10:10:10.12345",
        "2016-01-01 15:58:59.123456 UTC",
        "1985-09-01 00:00:00.123456",
        "",
        "now",
    ] {
        assert!(date_create(text.as_bytes(), None).is_ok(), "{text:?}");
    }
    for text in ["P2Y1DT1H2M3S", "P02Y00M01DT01H02M03S", "abcdefg", ""] {
        assert!(DateInterval::new(text.as_bytes()).is_ok(), "{text:?}");
    }
}
