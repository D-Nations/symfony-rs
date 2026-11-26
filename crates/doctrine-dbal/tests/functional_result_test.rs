//! Port of tests/Functional/ResultTest.php on `pdo_sqlite` in memory. The
//! `methodProvider` rows are closures; they are written out here, with
//! upstream's names and expected values.
//!
//! Upstream: doctrine/dbal tests/Functional/ResultTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{no_types, none, sql, sqlite};
use doctrine_dbal::{Exception, Result};
use php_value::{PhpArray, Value};

fn false_or(row: Option<PhpArray>) -> Value {
    row.map_or(Value::Bool(false), Value::Array)
}

fn list(rows: Vec<PhpArray>) -> Value {
    Value::Array(PhpArray::list(rows.into_iter().map(Value::Array)))
}

type Method = fn(&mut Result<'_>) -> std::result::Result<Value, Exception>;

fn method_provider() -> Vec<(&'static str, Method, Value)> {
    let empty = Value::Array(PhpArray::new());
    vec![
        (
            "fetchNumeric",
            |result| result.fetch_numeric().map(false_or),
            Value::Bool(false),
        ),
        (
            "fetchAssociative",
            |result| result.fetch_associative().map(false_or),
            Value::Bool(false),
        ),
        (
            "fetchOne",
            |result| {
                result
                    .fetch_one()
                    .map(|value| value.unwrap_or(Value::Bool(false)))
            },
            Value::Bool(false),
        ),
        (
            "fetchAllNumeric",
            |result| result.fetch_all_numeric().map(list),
            empty.clone(),
        ),
        (
            "fetchAllAssociative",
            |result| result.fetch_all_associative().map(list),
            empty.clone(),
        ),
        (
            "fetchFirstColumn",
            |result| {
                result
                    .fetch_first_column()
                    .map(|values| Value::Array(PhpArray::list(values)))
            },
            empty,
        ),
    ]
}

#[tokio::test]
async fn testExceptionHandling() {
    for (name, method, expected) in method_provider() {
        let connection = sqlite();
        let mut result = connection
            .execute_query(&sql("dummy_select"), &none(), &no_types())
            .await
            .unwrap();
        result.free();
        // The drivers that enforce the command sequencing internally throw
        // an exception; pdo_sqlite silently returns an empty result.
        let value = method(&mut result).unwrap();
        assert!(value.identical(&expected), "{name}: {value:?}");
    }
}
