//! Port of tests/Functional/AutoIncrementColumnTest.php on `pdo_sqlite` in
//! memory. The table is what `SQLitePlatform` builds (recorded,
//! tests/data/connection.json); the SQL Server branches do not apply.
//!
//! Upstream: doctrine/dbal tests/Functional/AutoIncrementColumnTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{array, drop_and_create_table, i, no_types, none, sqlite};
use doctrine_dbal::Connection;
use php_value::Value;
use php_value::vectors::phpunit_equals;

async fn set_up() -> Connection {
    let connection = sqlite();
    drop_and_create_table(&connection, "auto_increment_table").await;
    connection
}

async fn max_id(connection: &Connection) -> Value {
    connection
        .fetch_one(
            "SELECT MAX(id) FROM auto_increment_table",
            &none(),
            &no_types(),
        )
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn testInsertAutoGeneratesValue() {
    let connection = set_up().await;
    connection
        .insert("auto_increment_table", &array([("val", i(0))]), &no_types())
        .await
        .unwrap();
    assert!(phpunit_equals(&i(1), &max_id(&connection).await));
}

#[tokio::test]
async fn testInsertIdentityValue() {
    let connection = set_up().await;
    connection
        .insert(
            "auto_increment_table",
            &array([("id", i(2)), ("val", i(0))]),
            &no_types(),
        )
        .await
        .unwrap();
    assert!(phpunit_equals(&i(2), &max_id(&connection).await));

    // using an explicit value for an autoincrement column does not affect
    // the next value on the following platforms (not SQLite)
    connection
        .insert("auto_increment_table", &array([("val", i(0))]), &no_types())
        .await
        .unwrap();
    assert!(phpunit_equals(&i(3), &max_id(&connection).await));
}
