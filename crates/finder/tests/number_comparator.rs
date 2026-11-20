//! NumberComparatorTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43. Names and assertions are
//! PHPUnit's; provider rows are replayed from tests/data/providers.json.
//!
//! Upstream: symfony/finder Tests/Comparator/NumberComparatorTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, bytes, provider};
use php_value::Value;
use symfony_finder::{NumberComparator, php::Scalar};

fn list(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.values().cloned().collect(),
        other => panic!("not a list: {other:?}"),
    }
}

fn scalar(v: &Value) -> Scalar {
    match v {
        Value::Int(i) => Scalar::Int(*i),
        Value::String(s) => Scalar::String(s.as_bytes().to_vec()),
        other => panic!("not a scalar: {other:?}"),
    }
}

#[test]
fn testConstructor() {
    let rows = provider("NumberComparatorTest", "testConstructor");
    assert_eq!(rows.len(), 1);
    for (_, args) in rows {
        for s in list(&args[0]) {
            assert!(NumberComparator::new(Some(&bytes(&s))).is_ok(), "{s:?}");
        }
        for f in list(&args[1]) {
            let input = match &f {
                Value::Null => None,
                v => Some(bytes(v)),
            };
            let e = NumberComparator::new(input.as_deref()).err();
            assert!(
                e.is_some_and(|e| e.is_instance_of("InvalidArgumentException")),
                r"__construct() throws an \InvalidArgumentException if the test expression is not valid. ({f:?})"
            );
        }
    }
}

#[test]
fn testTest() -> TestResult {
    let rows = provider("NumberComparatorTest", "testTest");
    assert_eq!(rows.len(), 18);
    for (_, args) in rows {
        let c = NumberComparator::new(Some(&bytes(&args[0])))?;
        for m in list(&args[1]) {
            assert!(
                c.test(scalar(&m)),
                "->test() tests a string against the expression"
            );
        }
        for m in list(&args[2]) {
            assert!(
                !c.test(scalar(&m)),
                "->test() tests a string against the expression"
            );
        }
    }
    Ok(())
}
