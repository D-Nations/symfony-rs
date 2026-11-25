//! `loose_equals()` against PHP 8.4.27's `==` answers recorded by
//! `oracle/recorders/php_value_loose_equals.php` (`scripts/record.sh
//! php-value`): every ordered pair of a pool of null, bools, ints, floats,
//! numeric and other strings, arrays and objects.
//!
//! Upstream: none

use php_value::{compare::loose_equals, vectors};
use serde_json::Value as Json;

#[test]
fn loose_equals_matches_php() -> Result<(), vectors::Error> {
    let cases: Vec<Json> = serde_json::from_str(include_str!("data/loose_equals.json"))?;
    assert!(cases.len() >= 7744, "{} cases", cases.len());
    let mut failures = Vec::new();
    for case in &cases {
        let (a, b) = (vectors::decode(&case[0])?, vectors::decode(&case[1])?);
        let expected = case[2].as_bool().ok_or("expected a bool")?;
        let got = loose_equals(&a, &b);
        if got != expected {
            failures.push(format!("{a:?} == {b:?}: PHP {expected}, Rust {got}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        cases.len(),
        failures[..failures.len().min(40)].join("\n")
    );
    Ok(())
}
