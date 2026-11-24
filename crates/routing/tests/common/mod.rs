//! Helpers shared by the ported routing tests.
//!
//! Upstream: none
#![allow(dead_code)]

use php_value::{PhpArray, PhpString, Value};

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `['key' => 'value', ...]` with string values.
pub fn strings(pairs: &[(&str, &str)]) -> PhpArray {
    pairs.iter().map(|(k, v)| (*k, Value::string(*v))).collect()
}

/// `['key' => value, ...]`.
pub fn values(pairs: Vec<(&str, Value)>) -> PhpArray {
    pairs.into_iter().collect()
}

/// `['a', 'b', ...]`.
pub fn list(items: &[&str]) -> PhpArray {
    PhpArray::list(items.iter().map(|s| Value::string(*s)))
}

pub fn s(text: &str) -> PhpString {
    PhpString::from(text)
}
