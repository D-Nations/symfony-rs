//! Port of tests/Functional/NamedParametersTest.php on `pdo_sqlite` in
//! memory. The `ticketProvider` rows are upstream's, recorded
//! (tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/Functional/NamedParametersTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod common;
mod connection_support;

use connection_support::{array, data, drop_and_create_table, i, no_types, sqlite};
use doctrine_dbal::Connection;
use php_value::vectors::{decode, phpunit_equals};
use php_value::{Key, PhpArray, Value};

async fn set_up() -> Connection {
    let connection = sqlite();
    drop_and_create_table(&connection, "ddc1372_foobar").await;
    for (id, foo_value, bar_value) in [
        (1, 1, 1),
        (2, 1, 2),
        (3, 1, 3),
        (4, 1, 4),
        (5, 2, 1),
        (6, 2, 2),
    ] {
        connection
            .insert(
                "ddc1372_foobar",
                &array([("id", i(id)), ("foo", i(foo_value)), ("bar", i(bar_value))]),
                &no_types(),
            )
            .await
            .unwrap();
    }
    connection
}

/// `array_change_key_case($row, CASE_LOWER)`.
fn lower_keys(row: &PhpArray) -> PhpArray {
    let mut lower = PhpArray::new();
    for (key, value) in row.iter() {
        let key = match key {
            Key::Str(name) => Key::from_bytes(&name.as_bytes().to_ascii_lowercase()),
            Key::Int(_) => key.clone(),
        };
        lower.insert(key, value.clone());
    }
    lower
}

#[tokio::test]
async fn testTicket() {
    let rows = data()["providers"]["Functional/NamedParametersTest"]["testTicket"]["rows"]
        .as_array()
        .unwrap();
    assert_ne!(rows.as_slice(), &[] as &[serde_json::Value]);
    let connection = set_up().await;
    for row in rows {
        let name = row[0].as_str().unwrap();
        let args = row[1].as_array().unwrap();
        let Value::String(query) = decode(&args[0]).unwrap() else {
            panic!("query");
        };
        let params = common::array(&args[1]);
        let types = common::types(&args[2]);
        let expected = decode(&args[3]).unwrap();
        let data = connection
            .fetch_all_associative(&query.to_string_lossy(), &params, &types)
            .await
            .unwrap();
        let data = Value::Array(PhpArray::list(
            data.iter().map(|row| Value::Array(lower_keys(row))),
        ));
        assert!(
            phpunit_equals(&data, &expected),
            "{name}: expected {expected:?}, got {data:?}"
        );
    }
}
