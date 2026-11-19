//! The PHP built-ins and edge cases recorded from PHP 8.4.27 by
//! oracle/recorders/stopwatch.php (`scripts/record.sh stopwatch`).
//!
//! Upstream: none

use std::sync::Arc;

use serde_json::Value;
use symfony_stopwatch::{
    Host, Number, Stopwatch,
    php::{Decimal, Fixed2, round, to_int},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn vectors() -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(include_str!("data/php.json"))?)
}

fn float(bits: &Value) -> Result<f64, Box<dyn std::error::Error>> {
    let hex = bits.as_str().ok_or("bits are not a string")?;
    Ok(f64::from_bits(u64::from_str_radix(hex, 16)?))
}

fn rows<'a>(vectors: &'a Value, key: &str) -> Result<&'a Vec<Value>, Box<dyn std::error::Error>> {
    vectors[key]
        .as_array()
        .ok_or_else(|| format!("no {key}").into())
}

#[test]
fn round_to_one_decimal_matches_php() -> TestResult {
    let vectors = vectors()?;
    let mut failures = Vec::new();
    let cases = rows(&vectors, "round")?;
    for row in cases {
        let (input, expected) = (float(&row[0])?, float(&row[1])?);
        let actual = round(input, 1);
        if actual.to_bits() != expected.to_bits() && !(actual.is_nan() && expected.is_nan()) {
            failures.push(format!(
                "round({input:?}, 1): PHP {expected:?}, Rust {actual:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {}:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
    assert!(cases.len() > 3000);
    Ok(())
}

#[test]
fn int_cast_and_percent_d_match_php() -> TestResult {
    let vectors = vectors()?;
    for row in rows(&vectors, "int")? {
        let input = float(&row[0])?;
        let cast = row[1].as_i64().ok_or("cast is not an int")?;
        assert_eq!(to_int(input), cast, "(int) {input:?}");
        assert_eq!(
            Decimal(Number::Float(input)).to_string(),
            row[2].as_str().ok_or("not a string")?,
            "%d of {input:?}"
        );
    }
    Ok(())
}

#[test]
fn percent_two_f_matches_php() -> TestResult {
    let vectors = vectors()?;
    for row in rows(&vectors, "fixed")? {
        let input = float(&row[0])?;
        assert_eq!(
            Fixed2(input).to_string(),
            row[1].as_str().ok_or("not a string")?,
            "%.2F of {input:?}"
        );
    }
    Ok(())
}

#[test]
fn logic_exception_messages_match_php() -> TestResult {
    let vectors = vectors()?;
    let error = |result: Result<(), symfony_stopwatch::LogicException>| {
        result
            .err()
            .map(|e| format!("LogicException: {}", e.message))
    };
    for row in rows(&vectors, "messages")? {
        let label = row[0].as_str().ok_or("no label")?;
        let expected = row[1].as_str().map(str::to_owned);
        let s = Stopwatch::new(false);
        let actual = match label {
            "stopSection on a new stopwatch" => error(s.stop_section("x")),
            "openSection of an unknown id" => error(s.open_section(Some("x"))),
            "stop of an unknown event" => error(s.stop("x").map(drop)),
            "getEvent of an unknown event" => error(s.get_event("x").map(drop)),
            "lap of an unknown event" => error(s.lap("x").map(drop)),
            "stop of a stopped event" => {
                s.start("x", None);
                s.stop("x")?;
                error(s.stop("x").map(drop))
            }
            "reopen a section from another level" => {
                s.open_section(None)?;
                s.open_section(None)?;
                s.stop_section("inner")?;
                s.stop_section("outer")?;
                error(s.open_section(Some("inner")))
            }
            other => return Err(format!("unknown case {other}").into()),
        };
        assert_eq!(actual, expected, "{label}");
    }
    Ok(())
}

#[test]
fn event_and_section_order_matches_php() -> TestResult {
    let vectors = vectors()?;
    let orders = &vectors["orders"];
    let s = Stopwatch::new(false);
    s.start("b", None);
    s.start("a", Some("cat"));
    s.open_section(None)?;
    s.start("c", None);
    s.stop_section("one")?;
    s.open_section(None)?;
    s.stop_section("two")?;
    s.open_section(Some("one"))?;
    s.start("d", None);
    s.stop_section("one")?;

    let names = |events: Vec<(String, symfony_stopwatch::StopwatchEvent)>| -> Value {
        events
            .into_iter()
            .map(|(name, _)| Value::from(name))
            .collect()
    };
    let map = |events: Vec<(String, symfony_stopwatch::StopwatchEvent)>,
               f: &dyn Fn(&symfony_stopwatch::StopwatchEvent) -> Value|
     -> Value {
        Value::Object(
            events
                .iter()
                .map(|(name, event)| (name.clone(), f(event)))
                .collect(),
        )
    };
    let sections: Value = s
        .get_sections()
        .into_iter()
        .map(|(id, _)| Value::from(id))
        .collect();

    assert_eq!(orders["root events"], names(s.get_root_section_events()));
    assert_eq!(orders["sections"], sections);
    assert_eq!(orders["section one"], names(s.get_section_events("one")));
    assert_eq!(orders["section two"], names(s.get_section_events("two")));
    assert_eq!(
        orders["root categories"],
        map(s.get_root_section_events(), &|e| e.get_category().into())
    );
    assert_eq!(
        orders["section one categories"],
        map(s.get_section_events("one"), &|e| e.get_category().into())
    );
    assert_eq!(
        orders["section one periods"],
        map(s.get_section_events("one"), &|e| e
            .get_periods()
            .len()
            .into())
    );
    Ok(())
}

struct FixedMemory;
impl Host for FixedMemory {
    fn microtime(&self) -> f64 {
        1_763_500_000.0
    }
    fn memory_get_usage(&self) -> i64 {
        3 * 1024 * 1024 + 131_072
    }
}

#[test]
fn a_host_can_report_memory() -> TestResult {
    let s = Stopwatch::with_host(Arc::new(FixedMemory), false);
    s.start("x", Some("cat"));
    let event = s.stop("x")?;
    assert_eq!(event.get_memory(), 3_276_800);
    assert_eq!(event.to_string(), "cat/x: 3.12 MiB - 0 ms");
    Ok(())
}
