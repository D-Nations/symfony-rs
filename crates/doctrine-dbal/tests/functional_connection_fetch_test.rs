//! Port of tests/Functional/Connection/FetchTest.php on `pdo_sqlite` in
//! memory. The queries are the SQL that `TestUtil::generateResultSetQuery()`
//! and `SQLitePlatform` build (recorded, tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/Functional/Connection/FetchTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{array, i, list, no_types, none, s, sql, sqlite};
use doctrine_dbal::Exception;
use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value};

fn query() -> String {
    sql("fetch_query")
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the values are built at the call"
)]
fn assert_equals(expected: Value, actual: Value) {
    assert!(
        phpunit_equals(&expected, &actual),
        "expected {expected:?}, got {actual:?}"
    );
}

fn rows(rows: Vec<PhpArray>) -> Value {
    Value::Array(list(rows.into_iter().map(Value::Array)))
}

fn ab(a: &str, b: i64) -> PhpArray {
    array([("a", s(a)), ("b", i(b))])
}

fn keyed(pairs: impl IntoIterator<Item = (Value, Value)>) -> Value {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(Key::from_value(&key).unwrap(), value);
    }
    Value::Array(array)
}

#[tokio::test]
async fn testFetchNumeric() {
    let row = sqlite()
        .fetch_numeric(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(
        Value::Array(list([s("foo"), i(1)])),
        Value::Array(row.unwrap()),
    );
}

#[tokio::test]
async fn testFetchOne() {
    let value = sqlite()
        .fetch_one(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(s("foo"), value.unwrap());
}

#[tokio::test]
async fn testFetchAssociative() {
    let row = sqlite()
        .fetch_associative(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(Value::Array(ab("foo", 1)), Value::Array(row.unwrap()));
}

#[tokio::test]
async fn testFetchAllNumeric() {
    let all = sqlite()
        .fetch_all_numeric(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(
        rows(vec![
            list([s("foo"), i(1)]),
            list([s("bar"), i(2)]),
            list([s("baz"), i(3)]),
        ]),
        rows(all),
    );
}

#[tokio::test]
async fn testFetchAllAssociative() {
    let all = sqlite()
        .fetch_all_associative(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(
        rows(vec![ab("foo", 1), ab("bar", 2), ab("baz", 3)]),
        rows(all),
    );
}

fn key_values() -> Value {
    Value::Array(array([("foo", i(1)), ("bar", i(2)), ("baz", i(3))]))
}

fn indexed() -> Value {
    Value::Array(array([
        ("foo", Value::Array(array([("b", i(1))]))),
        ("bar", Value::Array(array([("b", i(2))]))),
        ("baz", Value::Array(array([("b", i(3))]))),
    ]))
}

#[tokio::test]
async fn testFetchAllKeyValue() {
    let all = sqlite()
        .fetch_all_key_value(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(key_values(), Value::Array(all));
}

#[tokio::test]
async fn testFetchAllKeyValueWithLimit() {
    let all = sqlite()
        .fetch_all_key_value(&sql("fetch_query_limit"), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(Value::Array(array([("bar", i(2))])), Value::Array(all));
}

#[tokio::test]
async fn testFetchAllKeyValueOneColumn() {
    let e = sqlite()
        .fetch_all_key_value(&sql("dummy_select"), &none(), &no_types())
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::NO_KEY_VALUE));
}

#[tokio::test]
async fn testFetchAllAssociativeIndexed() {
    let all = sqlite()
        .fetch_all_associative_indexed(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(indexed(), Value::Array(all));
}

#[tokio::test]
async fn testFetchFirstColumn() {
    let column = sqlite()
        .fetch_first_column(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(
        Value::Array(list([s("foo"), s("bar"), s("baz")])),
        Value::Array(list(column)),
    );
}

#[tokio::test]
async fn testIterateNumeric() {
    let connection = sqlite();
    let all: Vec<PhpArray> = connection
        .iterate_numeric(&query(), &none(), &no_types())
        .await
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_equals(
        rows(vec![
            list([s("foo"), i(1)]),
            list([s("bar"), i(2)]),
            list([s("baz"), i(3)]),
        ]),
        rows(all),
    );
}

#[tokio::test]
async fn testIterateAssociative() {
    let connection = sqlite();
    let all: Vec<PhpArray> = connection
        .iterate_associative(&query(), &none(), &no_types())
        .await
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_equals(
        rows(vec![ab("foo", 1), ab("bar", 2), ab("baz", 3)]),
        rows(all),
    );
}

#[tokio::test]
async fn testIterateKeyValue() {
    let connection = sqlite();
    let pairs: Vec<(Value, Value)> = connection
        .iterate_key_value(&query(), &none(), &no_types())
        .await
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // iterator_to_array()
    assert_equals(key_values(), keyed(pairs));
}

#[tokio::test]
async fn testIterateKeyValueOneColumn() {
    let connection = sqlite();
    let e = connection
        .iterate_key_value(&sql("dummy_select"), &none(), &no_types())
        .await
        .map(drop)
        .unwrap_err();
    assert!(e.is_instance_of(Exception::NO_KEY_VALUE));
}

#[tokio::test]
async fn testIterateAssociativeIndexed() {
    let connection = sqlite();
    let pairs: Vec<(Value, PhpArray)> = connection
        .iterate_associative_indexed(&query(), &none(), &no_types())
        .await
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_equals(
        indexed(),
        keyed(pairs.into_iter().map(|(key, row)| (key, Value::Array(row)))),
    );
}

#[tokio::test]
async fn testIterateColumn() {
    let connection = sqlite();
    let column: Vec<Value> = connection
        .iterate_column(&query(), &none(), &no_types())
        .await
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_equals(
        Value::Array(list([s("foo"), s("bar"), s("baz")])),
        Value::Array(list(column)),
    );
}
