//! HeaderUtils against PHP 8.4.27's answers recorded by
//! oracle/recorders/http_foundation_header_utils.php (`scripts/record.sh
//! http-foundation`): `split()` on 3,000 generated headers and separator
//! lists, `quote()` and `unquote()` on 633 strings.
//!
//! Upstream: none

use php_value::vectors::{self, Error, base64};
use serde_json::Value as Json;
use symfony_http_foundation::header_utils::{quote, split, unquote};

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
