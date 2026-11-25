//! `HeaderUtils` against PHP 8.4.27's answers recorded by
//! `oracle/recorders/http_foundation_header_utils.php` (`scripts/record.sh
//! http-foundation`): `split()` on 3,000 generated headers and separator
//! lists, `quote()` and `unquote()` on 633 strings, `makeDisposition()`
//! on 2,880 type, filename and fallback combinations (v8.1.11),
//! `parseQuery()` on 3,000 generated queries, separators and modes.
//!
//! Upstream: none

use php_value::vectors::{self, Error, base64};
use serde_json::Value as Json;
use symfony_http_foundation::header_utils::{parse_query, quote, split, unquote};

fn data() -> Result<Json, Error> {
    Ok(serde_json::from_str(include_str!(
        "data/header_utils.json"
    ))?)
}

fn bytes(json: &Json) -> Result<Vec<u8>, Error> {
    base64(json.as_str().ok_or("expected base64")?)
}

#[test]
fn split_matches_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["split"].as_array().ok_or("split")?;
    let mut failures = Vec::new();
    for case in cases {
        let (header, separators) = (bytes(&case[0])?, bytes(&case[1])?);
        let expected = vectors::decode(&case[2])?;
        let actual = split(&header, &separators)?;
        if expected != php_value::Value::Array(actual.clone()) {
            failures.push(format!(
                "{:?} {:?}: PHP {expected:?}, Rust {actual:?}",
                String::from_utf8_lossy(&header),
                String::from_utf8_lossy(&separators)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(30)].join("\n")
    );
    Ok(())
}

#[test]
fn quote_and_unquote_match_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["strings"].as_array().ok_or("strings")?;
    let mut failures = Vec::new();
    for case in cases {
        let s = bytes(&case[0])?;
        let (quoted, unquoted) = (bytes(&case[1])?, bytes(&case[2])?);
        if quote(&s).as_bytes() != quoted.as_slice()
            || unquote(&s).as_bytes() != unquoted.as_slice()
        {
            failures.push(format!(
                "{:?}: PHP {:?} {:?}, Rust {:?} {:?}",
                String::from_utf8_lossy(&s),
                String::from_utf8_lossy(&quoted),
                String::from_utf8_lossy(&unquoted),
                quote(&s).to_string_lossy(),
                unquote(&s).to_string_lossy()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
    Ok(())
}

#[test]
fn make_disposition_matches_php() -> Result<(), Error> {
    use symfony_http_foundation::header_utils::make_disposition;
    let data = data()?;
    let cases = data["disposition"].as_array().ok_or("disposition")?;
    let mut failures = Vec::new();
    for case in cases {
        let (kind, filename, fallback) = (bytes(&case[0])?, bytes(&case[1])?, bytes(&case[2])?);
        let actual = match make_disposition(&kind, &filename, &fallback) {
            Ok(value) => Json::String(base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                value.as_bytes(),
            )),
            Err(e) => serde_json::json!(["throw", e.class().name(), e.message().to_string_lossy()]),
        };
        if actual != case[3] {
            failures.push(format!(
                "{:?} {:?} {:?}: PHP {}, Rust {actual}",
                String::from_utf8_lossy(&kind),
                String::from_utf8_lossy(&filename),
                String::from_utf8_lossy(&fallback),
                case[3]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(30)].join("\n")
    );
    Ok(())
}

#[test]
fn parse_query_matches_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["query"].as_array().ok_or("query")?;
    assert_eq!(cases.len(), 3000);
    let mut failures = Vec::new();
    for case in cases {
        let query = bytes(&case[0])?;
        let ignore_brackets = case[1].as_bool().ok_or("ignoreBrackets")?;
        let separator = case[2].as_str().ok_or("separator")?;
        let expected = vectors::decode(&case[3])?;
        let actual = parse_query(&query, ignore_brackets, separator.as_bytes())?;
        if expected != php_value::Value::Array(actual.clone()) {
            failures.push(format!(
                "{:?} {ignore_brackets} {separator:?}: PHP {expected:?}, Rust {actual:?}",
                String::from_utf8_lossy(&query)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(30)].join("\n")
    );
    Ok(())
}
