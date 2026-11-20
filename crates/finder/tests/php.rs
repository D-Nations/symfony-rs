//! PHP 8.4.27's answers for the PHP behaviour this port writes itself,
//! recorded by oracle/recorders/finder_vectors.php (tests/data/php.json):
//! preg_match on Finder's patterns, PHP 8 comparisons, strnatcmp & co.,
//! NumberComparator, and the dates DateComparator parses.
//!
//! Upstream: none

mod common;

use std::cmp::Ordering;

use common::{b64, php_json};
use serde_json::Value as Json;
use symfony_finder::{
    NumberComparator,
    pcre::{Pcre, PcreError},
    php::{Scalar, compare, loose_eq, strcasecmp, strnatcasecmp, strnatcmp},
};

fn ordering(n: i64) -> Ordering {
    n.cmp(&0)
}

#[test]
fn preg_match_matches_php() {
    let json = php_json();
    let subjects: Vec<Vec<u8>> = json["subjects"]
        .as_array()
        .into_iter()
        .flatten()
        .map(b64)
        .collect();
    let mut failures = Vec::new();
    let mut unsupported = Vec::new();
    for row in json["pcre"].as_array().into_iter().flatten() {
        let pattern = b64(&row[0]);
        let expected = row[1].as_array().cloned().unwrap_or_default();
        let compiled = Pcre::new(&pattern);
        for (subject, expected) in subjects.iter().zip(expected) {
            let actual = match &compiled {
                Ok(p) => p.is_match(subject).ok().map(i64::from),
                Err(PcreError::Unsupported(_)) => {
                    unsupported.push(String::from_utf8_lossy(&pattern).into_owned());
                    break;
                }
                Err(PcreError::Compile(_)) => None,
            };
            if actual != expected.as_i64() {
                failures.push(format!(
                    "{} on {:?}: PHP {expected}, Rust {actual:?} ({:?})",
                    String::from_utf8_lossy(&pattern),
                    String::from_utf8_lossy(subject),
                    compiled.as_ref().map(Pcre::translated)
                ));
            }
        }
    }
    unsupported.dedup();
    assert_eq!(
        unsupported,
        ["/(*FAIL)/", "/(?R)/", "/(?J)a/", "/a/r"],
        "patterns refused as unsupported"
    );
    assert!(
        failures.is_empty(),
        "{} differences:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

fn scalar(json: &Json) -> Scalar {
    match json[0].as_str() {
        Some("s") => Scalar::String(json[1].as_str().unwrap_or_default().as_bytes().to_vec()),
        Some("i") => Scalar::Int(json[1].as_i64().unwrap_or_default()),
        _ => Scalar::Float(json[1].as_f64().unwrap_or_default()),
    }
}

#[test]
fn comparisons_match_php() {
    let json = php_json();
    let scalars: Vec<Scalar> = json["scalars"]
        .as_array()
        .into_iter()
        .flatten()
        .map(scalar)
        .collect();
    let results = json["compare"].as_array().cloned().unwrap_or_default();
    let mut k = 0;
    for x in &scalars {
        for y in &scalars {
            let r = &results[k];
            k += 1;
            assert_eq!(
                compare(x, y),
                ordering(r[0].as_i64().unwrap_or(9)),
                "{x:?} <=> {y:?}"
            );
            assert_eq!(
                loose_eq(x, y),
                r[1].as_bool().unwrap_or_default(),
                "{x:?} == {y:?}"
            );
        }
    }
}

#[test]
fn natural_order_matches_php() {
    let json = php_json();
    let words: Vec<Vec<u8>> = json["words"]
        .as_array()
        .into_iter()
        .flatten()
        .map(b64)
        .collect();
    let results = json["natural"].as_array().cloned().unwrap_or_default();
    let mut k = 0;
    let mut failures = Vec::new();
    for x in &words {
        for y in &words {
            let r = &results[k];
            k += 1;
            let (sx, sy) = (String::from_utf8_lossy(x), String::from_utf8_lossy(y));
            for (name, actual, expected) in [
                ("strnatcmp", strnatcmp(x, y), &r[0]),
                ("strnatcasecmp", strnatcasecmp(x, y), &r[1]),
                ("strcasecmp", strcasecmp(x, y), &r[2]),
            ] {
                if actual != ordering(expected.as_i64().unwrap_or(9)) {
                    failures.push(format!(
                        "{name}({sx:?}, {sy:?}): PHP {expected}, Rust {actual:?}"
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn number_comparator_matches_php() {
    let json = php_json();
    for row in json["number_comparator"].as_array().into_iter().flatten() {
        let input = row[0].as_str().unwrap_or_default();
        match NumberComparator::new(Some(input.as_bytes())) {
            Ok(c) => {
                assert_eq!(
                    Some(String::from_utf8_lossy(c.get_target()).as_ref()),
                    row[1].as_str(),
                    "{input:?}"
                );
                assert_eq!(Some(c.get_operator()), row[2].as_str(), "{input:?}");
            }
            Err(e) => {
                assert!(row[1].is_null(), "{input:?}: {e}");
                assert_eq!(
                    Some(format!("{}: {}", e.class(), e.message()).as_str()),
                    row[2].as_str()
                );
            }
        }
    }
}

#[test]
fn dates_match_php() {
    let json = php_json();
    let now = json["now"].as_i64().unwrap_or_default();
    for row in json["dates"].as_array().into_iter().flatten() {
        let input = row[0].as_str().unwrap_or_default();
        let c = symfony_finder::DateComparator::new_at(input.as_bytes(), now)
            .unwrap_or_else(|e| panic!("{input:?}: {e}"));
        assert_eq!(
            String::from_utf8_lossy(c.get_target()),
            row[1].as_i64().unwrap_or_default().to_string(),
            "{input:?}"
        );
    }
}
