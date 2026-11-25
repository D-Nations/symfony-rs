//! `html_entity_decode()` and `json_decode()` against PHP 8.4.27's answers
//! recorded by `oracle/recorders/php_value_html_entity_decode.php` and
//! `php_value_json_decode.php` (`scripts/record.sh php-value-decode`).
//!
//! Upstream: none

use php_value::{
    Value,
    html::html_entity_decode,
    json::{JsonError, decode_assoc},
    vectors,
};
use serde_json::Value as Json;

/// Reads a vector file whose values nest past serde_json's default limit.
fn cases(text: &str) -> Result<Vec<Json>, vectors::Error> {
    let mut reader = serde_json::Deserializer::from_str(text);
    reader.disable_recursion_limit();
    match reader.into_iter::<Json>().next() {
        Some(Ok(Json::Array(cases))) => Ok(cases),
        Some(Err(e)) => Err(e.into()),
        _ => Err("not an array".into()),
    }
}

/// `===`, with floats compared by their bits (`-0.0` is not `0.0` here,
/// and `INF` must stay `INF`).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => x.to_bits() == y.to_bits(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((kx, vx), (ky, vy))| kx == ky && same(vx, vy))
        }
        _ => a == b,
    }
}

fn shown(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(60).collect()
}

#[test]
fn html_entity_decode_matches_php() -> Result<(), vectors::Error> {
    let cases = cases(include_str!("data/html_entity_decode.json"))?;
    assert!(cases.len() >= 10_000, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let input = vectors::base64(case[0].as_str().ok_or("input")?)?;
        let expected = vectors::base64(case[1].as_str().ok_or("result")?)?;
        let got = html_entity_decode(&input);
        if got != expected {
            failures.push(format!(
                "{:?}: PHP {:?}, Rust {:?}",
                shown(&input),
                shown(&expected),
                shown(&got)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(20)].join("\n")
    );
    Ok(())
}

fn json_decode_cases() -> Result<(), vectors::Error> {
    let cases = cases(include_str!("data/json_decode.json"))?;
    assert!(cases.len() >= 10_000, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let input = vectors::base64(case[0].as_str().ok_or("input")?)?;
        let bigint = case[1].as_bool().ok_or("flag")?;
        let got = decode_assoc(&input, 512, bigint);
        let result = &case[2];
        let ok = if result[0] == "ok" {
            let expected = vectors::decode(&result[1])?;
            matches!(&got, Ok(v) if same(v, &expected))
        } else {
            let code = result[1].as_i64().ok_or("code")?;
            let message = result[2].as_str().ok_or("message")?;
            matches!(got, Err(e) if e.code() == code && e.message() == message)
        };
        if !ok {
            failures.push(format!(
                "{:?} (bigint {bigint}): PHP {}, Rust {}",
                shown(&input),
                result.to_string().chars().take(80).collect::<String>(),
                format!("{got:?}").chars().take(80).collect::<String>()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(20)].join("\n")
    );
    Ok(())
}

#[test]
fn json_decode_matches_php() -> Result<(), String> {
    // the typed vectors of 511-level documents are read recursively
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(|| json_decode_cases().map_err(|e| e.to_string()))
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "the replay panicked".to_owned())?
}

#[test]
fn json_errors_have_php_codes() {
    assert_eq!(Err(JsonError::Syntax), decode_assoc(b"", 512, true));
    assert_eq!(4, JsonError::Syntax.code());
    assert_eq!(
        Ok(Value::string("9223372036854775808")),
        decode_assoc(b"9223372036854775808", 512, true)
    );
}
