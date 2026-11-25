//! `AcceptHeader` and `AcceptHeaderItem` against PHP 8.4.27's answers
//! recorded by `oracle/recorders/http_foundation_accept_header.php`
//! (`scripts/record.sh http-foundation`): 1,500 generated headers through
//! `fromString()`, `__toString()` and `all()`, with `has()` and `get()`
//! for 6,000 generated queries; `filter()` with 7 patterns on 3 headers;
//! `AcceptHeaderItem::fromString()` on 500 generated items. A thrown
//! error is compared by class and by PHP's message up to its "called in"
//! suffix, which is not reproduced.
//!
//! Upstream: none

use php_value::vectors::{self, Error, base64};
use serde_json::Value as Json;
use symfony_http_foundation::accept_header::{AcceptHeader, AcceptHeaderItem};
use symfony_http_foundation::exception::Error as HttpError;

fn data() -> Result<Json, Error> {
    Ok(serde_json::from_str(include_str!(
        "data/accept_header.json"
    ))?)
}

fn bytes(json: &Json) -> Result<Vec<u8>, Error> {
    base64(json.as_str().ok_or("expected base64")?)
}

/// The recorder's `item_answer()` for an item.
fn answer(item: &AcceptHeaderItem) -> Result<Json, Error> {
    let encode = |b: &[u8]| -> Json { Json::String(php_value::base64::encode(b)) };
    Ok(Json::Array(vec![
        encode(item.value()),
        Json::String(
            item.quality()
                .to_be_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        ),
        Json::from(item.index()),
        Json::Null, // attributes: compared separately
        encode(item.to_php_string().map_err(|e| e.to_string())?.as_bytes()),
    ]))
}

/// Whether `got` is PHP's `expected` item answer.
fn same_item(expected: &Json, got: &AcceptHeaderItem) -> Result<bool, Error> {
    let mine = answer(got)?;
    let attributes = vectors::decode(&expected[3])?;
    Ok(expected[0] == mine[0]
        && expected[1] == mine[1]
        && expected[2] == mine[2]
        && expected[4] == mine[4]
        && attributes == php_value::Value::Array(got.attributes().clone()))
}

/// Whether `got` is PHP's `["throw", class, message]`.
fn same_error(expected: &Json, got: &HttpError) -> bool {
    expected[0] == "throw"
        && expected[1] == got.class().name()
        && expected[2]
            .as_str()
            .is_some_and(|m| m.as_bytes().starts_with(got.message().as_bytes()))
}

fn report(failures: &[String], total: usize) {
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        total,
        failures[..failures.len().min(30)].join("\n")
    );
}

#[test]
fn headers_match_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["headers"].as_array().ok_or("headers")?;
    let mut failures = Vec::new();
    let mut total = 0;
    for case in cases {
        let string = bytes(&case[0])?;
        let label = String::from_utf8_lossy(&string).into_owned();
        let mut header = AcceptHeader::from_string(Some(&string));
        total += 1;
        let unsorted = header.to_php_string().map_err(|e| e.to_string())?;
        if bytes(&case[1])? != unsorted.as_bytes() {
            failures.push(format!("{label:?} __toString: {unsorted:?}"));
        }
        let expected_all = case[2].as_array().ok_or("all")?;
        let all = header.all();
        let mut same = expected_all.len() == all.len();
        for ((key, item), expected) in all.iter().zip(expected_all) {
            same &= vectors::decode(&expected[0])? == key.to_value();
            same &= same_item(&expected[1], item)?;
        }
        if !same {
            failures.push(format!("{label:?} all(): {all:?}"));
        }
        for query in case[3].as_array().ok_or("queries")? {
            total += 1;
            let value = bytes(&query[0])?;
            let qlabel = String::from_utf8_lossy(&value).into_owned();
            let has_ok = match header.has(&value) {
                Ok(has) => query[1] == Json::Bool(has),
                Err(e) => same_error(&query[1], &e),
            };
            let get_ok = match header.get(&value) {
                Ok(None) => query[2].is_null(),
                Ok(Some(item)) => query[2][0] != "throw" && same_item(&query[2], item)?,
                Err(e) => same_error(&query[2], &e),
            };
            if !has_ok || !get_ok {
                failures.push(format!(
                    "{label:?} query {qlabel:?}: has {has_ok}, get {get_ok}"
                ));
            }
        }
    }
    report(&failures, total);
    Ok(())
}

#[test]
fn filter_matches_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["filters"].as_array().ok_or("filters")?;
    let mut failures = Vec::new();
    for case in cases {
        let (string, pattern) = (bytes(&case[0])?, bytes(&case[1])?);
        let header = AcceptHeader::from_string(Some(&string));
        let got = match header.filter(&pattern) {
            Ok(mut filtered) => Json::Array(
                filtered
                    .all()
                    .iter()
                    .map(|(key, _)| {
                        Json::String(match key {
                            php_value::Key::Int(i) => i.to_string(),
                            php_value::Key::Str(s) => s.to_string_lossy().into_owned(),
                        })
                    })
                    .collect(),
            ),
            Err(e) => Json::String(e.to_string()),
        };
        if got != case[2] {
            failures.push(format!(
                "{:?} {:?}: PHP {}, Rust {got}",
                String::from_utf8_lossy(&string),
                String::from_utf8_lossy(&pattern),
                case[2]
            ));
        }
    }
    report(&failures, cases.len());
    Ok(())
}

#[test]
fn items_match_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["items"].as_array().ok_or("items")?;
    let mut failures = Vec::new();
    for case in cases {
        let string = bytes(&case[0])?;
        let ok = match AcceptHeaderItem::from_string(Some(&string)) {
            Ok(item) => case[1][0] != "throw" && same_item(&case[1], &item)?,
            Err(e) => same_error(&case[1], &e),
        };
        if !ok {
            failures.push(format!(
                "{:?}: PHP {}",
                String::from_utf8_lossy(&string),
                case[1]
            ));
        }
    }
    report(&failures, cases.len());
    Ok(())
}

#[test]
fn get_among_overlapping_ranges_matches_php() -> Result<(), Error> {
    let data = data()?;
    let cases = data["specific"].as_array().ok_or("specific")?;
    let mut failures = Vec::new();
    for case in cases {
        let (string, query) = (bytes(&case[0])?, bytes(&case[1])?);
        let header = AcceptHeader::from_string(Some(&string));
        let ok = match header.get(&query) {
            Ok(None) => case[2].is_null(),
            Ok(Some(item)) => case[2][0] != "throw" && same_item(&case[2], item)?,
            Err(e) => same_error(&case[2], &e),
        };
        if !ok {
            failures.push(format!(
                "{:?} get {:?}: PHP {}",
                String::from_utf8_lossy(&string),
                String::from_utf8_lossy(&query),
                case[2]
            ));
        }
    }
    report(&failures, cases.len());
    Ok(())
}
