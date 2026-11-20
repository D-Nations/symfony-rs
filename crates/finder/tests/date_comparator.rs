//! DateComparatorTest.php at symfony/finder
//! 2f4c0f26106520d82e1b71759d2fb8d32d8b7a43. Names and assertions are
//! PHPUnit's; provider rows (with their `strtotime()` values, UTC) are
//! replayed from tests/data/providers.json.
//!
//! Upstream: symfony/finder Tests/Comparator/DateComparatorTest.php
#![allow(non_snake_case)]

mod common;

use common::{TestResult, bytes, provider};
use php_value::Value;
use symfony_finder::DateComparator;

#[test]
fn testConstructor() {
    for test in [&b"foobar"[..], b""] {
        let e = DateComparator::new(test).err();
        assert!(
            e.is_some_and(|e| e.is_instance_of("InvalidArgumentException")),
            r"__construct() throws an \InvalidArgumentException if the test expression is not valid."
        );
    }
}

fn ints(v: &Value) -> Vec<i64> {
    match v {
        Value::Array(a) => a
            .values()
            .map(|v| match v {
                Value::Int(i) => *i,
                other => panic!("not an int: {other:?}"),
            })
            .collect(),
        other => panic!("not a list: {other:?}"),
    }
}

#[test]
fn testTest() -> TestResult {
    let rows = provider("DateComparatorTest", "testTest");
    assert_eq!(rows.len(), 8);
    for (_, args) in rows {
        let c = DateComparator::new(&bytes(&args[0]))?;
        for m in ints(&args[1]) {
            assert!(c.test(m), "->test() tests a string against the expression");
        }
        for m in ints(&args[2]) {
            assert!(!c.test(m), "->test() tests a string against the expression");
        }
    }
    Ok(())
}
