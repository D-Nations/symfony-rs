//! `parse_str()` against PHP 8.4.27's answers recorded by
//! `oracle/recorders/php_value_parse_str.php` (`scripts/record.sh
//! php-value`): every raw input of the request vectors plus nesting,
//! `max_input_vars` and key cases.
//!
//! Upstream: none

use php_value::{Value, vars::parse_str, vectors};
use serde_json::Value as Json;

/// `===` on the decoded values: same types, same keys in the same order.
fn identical(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Array(a), Value::Array(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && identical(va, vb))
        }
        (Value::String(a), Value::String(b)) => a.as_bytes() == b.as_bytes(),
        _ => false,
    }
}

#[test]
fn parse_str_matches_php() -> Result<(), vectors::Error> {
    // Names nest up to 100 groups deep: past serde_json's default limit.
    let mut reader = serde_json::Deserializer::from_str(include_str!("data/parse_str.json"));
    reader.disable_recursion_limit();
    let cases: Vec<Json> = match reader.into_iter::<Json>().next() {
        Some(Ok(Json::Array(cases))) => cases,
        Some(Err(e)) => return Err(e.into()),
        _ => return Err("parse_str.json is not an array".into()),
    };
    assert!(cases.len() >= 254, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let input = vectors::base64(case[0].as_str().ok_or("input")?)?;
        let expected = vectors::decode(&case[1])?;
        let got = Value::Array(parse_str(&input));
        if !identical(&expected, &got) {
            failures.push(
                String::from_utf8_lossy(&input)
                    .chars()
                    .take(80)
                    .collect::<String>(),
            );
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ: {failures:?}",
        failures.len(),
        cases.len()
    );
    Ok(())
}
