//! Port of tests/Functional/BooleanBindingTest.php on `pdo_sqlite` in
//! memory. The table is what `SQLitePlatform` builds (recorded,
//! tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/Functional/BooleanBindingTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{drop_and_create_table, no_types, none, sqlite};
use doctrine_dbal::{Connection, ConnectionStandIn, ParameterType, WrapperParameterType};
use php_value::Value;

async fn set_up() -> Connection {
    let connection = sqlite();
    drop_and_create_table(&connection, "boolean_test_table").await;
    connection
}

/// `booleanProvider()`.
fn boolean_provider() -> [(&'static str, Option<bool>); 3] {
    [("true", Some(true)), ("false", Some(false)), ("null", None)]
}

fn value(input: Option<bool>) -> Value {
    input.map_or(Value::Null, Value::Bool)
}

async fn insert_and_read(input: Option<bool>, r#type: WrapperParameterType) {
    let connection = set_up().await;
    let mut query_builder = connection.create_query_builder();
    let placeholder = query_builder.create_named_parameter(value(input), r#type, None);
    let result = query_builder
        .insert("boolean_test_table")
        .values(&[("val", &placeholder)])
        .execute_statement()
        .await
        .unwrap();

    assert_eq!(result, 1);

    let stored = connection
        .fetch_one("SELECT val FROM boolean_test_table", &none(), &no_types())
        .await
        .unwrap()
        .unwrap();
    let converted = connection
        .convert_to_php_value(&stored, "boolean")
        .await
        .unwrap();
    assert_eq!(converted, value(input));
}

#[tokio::test]
async fn testBooleanParameterInsert() {
    for (name, input) in boolean_provider() {
        eprintln!("{name}");
        insert_and_read(input, ParameterType::Boolean.into()).await;
    }
}

#[tokio::test]
async fn testBooleanTypeInsert() {
    for (name, input) in boolean_provider() {
        eprintln!("{name}");
        insert_and_read(input, WrapperParameterType::from("boolean")).await;
    }
}
