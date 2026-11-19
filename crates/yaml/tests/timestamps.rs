//! YAML timestamps through `Inline::parse()`, against Symfony on PHP
//! 8.4.27 (oracle/recorders/yaml_timestamps.php, `scripts/record.sh yaml`):
//! the plain result (int or float timestamp, or the invalid-date
//! exception) and the `PARSE_DATETIME` object, with its formats.
//!
//! Upstream: none

mod common;

use common::{TestResult, load, rows};
use php_value::{
    Value,
    datetime::DateTimeImmutable,
    vectors::{Outcome, outcome, phpunit_equals},
};
use symfony_yaml::{Flags, Inline, References};

fn check(input: &str, flags: Flags, expected: &Outcome) -> Result<(), String> {
    let mut inline = Inline::default();
    inline.initialize(Flags::NONE, Some(-1), None);
    let actual = inline.parse(input.as_bytes(), flags, &mut References::new(), None);
    match (expected, actual) {
        (Outcome::Value(expected), Ok(actual)) => {
            let same_type = expected.debug_type() == actual.debug_type();
            let equal = match (expected, &actual) {
                (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
                _ => phpunit_equals(expected, &actual),
            };
            if same_type && equal {
                Ok(())
            } else {
                Err(format!("{input:?}: PHP {expected:?}, Rust {actual:?}"))
            }
        }
        (Outcome::Throws { message, .. }, Err(error))
            if *message == error.get_message().to_string() =>
        {
            Ok(())
        }
        (expected, actual) => Err(format!("{input:?}: PHP {expected:?}, Rust {actual:?}")),
    }
}

#[test]
fn timestamps_match_symfony() -> TestResult {
    let json = load(include_str!("data/timestamps.json"))?;
    let cases = rows(&json, "timestamps")?;
    let mut failures = Vec::new();
    for row in cases {
        let input = row[0].as_str().ok_or("no input")?;
        for (column, flags) in [(2, Flags::NONE), (3, Flags::PARSE_DATETIME)] {
            let expected = outcome(&row[column]).map_err(|e| e.to_string())?;
            if let Err(failure) = check(input, flags, &expected) {
                failures.push(failure);
            }
        }
        // The DateTimeImmutable view, for accepted inputs.
        if let Some(formats) = row[1].as_array() {
            let mut inline = Inline::default();
            let parsed = inline
                .parse(
                    input.as_bytes(),
                    Flags::PARSE_DATETIME,
                    &mut References::new(),
                    None,
                )
                .map_err(|e| e.to_string())?;
            let time = parsed
                .as_object()
                .and_then(|o| o.downcast_ref::<DateTimeImmutable>())
                .ok_or("not a DateTimeImmutable")?;
            let actual = [
                time.format("U"),
                time.format("u"),
                time.format("Y-m-d\\TH:i:s.uP"),
                time.timezone().name(),
                time.format("c"),
                time.format("Y-m-d\\TH:i:s.vP"),
                time.format("T"),
                time.format("Z"),
            ];
            let expected: Vec<&str> = formats.iter().filter_map(|f| f.as_str()).collect();
            if actual
                .iter()
                .map(String::as_str)
                .ne(expected.iter().copied())
            {
                failures.push(format!("{input:?}: PHP {expected:?}, Rust {actual:?}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
    assert!(cases.len() > 1200);
    Ok(())
}
