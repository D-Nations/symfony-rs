//! `http_build_query_with()` against PHP 8.4.27's answers recorded by
//! oracle/recorders/php_value_http_build_query.php (`scripts/record.sh
//! php-value`): hand-picked and generated arrays (every scalar type,
//! floats, nested and empty arrays, int and special-character keys), each
//! with `PHP_QUERY_RFC1738` and `PHP_QUERY_RFC3986`.
//!
//! Upstream: none

use php_value::{
    Value,
    url::{QueryEncoding, http_build_query_with},
    vectors,
};
use serde_json::Value as Json;

#[test]
fn http_build_query_matches_php() -> Result<(), vectors::Error> {
    let cases: Vec<Json> = serde_json::from_str(include_str!("data/http_build_query.json"))?;
    assert!(cases.len() >= 1000, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let Value::Array(data) = vectors::decode(&case[0])? else {
            return Err("data is not an array".into());
        };
        let separator = vectors::base64(case[1].as_str().ok_or("separator")?)?;
        let encoding = if case[2].as_bool().ok_or("encoding")? {
            QueryEncoding::Rfc3986
        } else {
            QueryEncoding::Rfc1738
        };
        let expected = vectors::base64(case[3].as_str().ok_or("result")?)?;
        let got = http_build_query_with(&data, &separator, encoding).ok_or("refused")?;
        if got != expected {
            failures.push(format!(
                "{data:?} {encoding:?}: PHP {:?}, Rust {:?}",
                String::from_utf8_lossy(&expected),
                String::from_utf8_lossy(&got)
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
