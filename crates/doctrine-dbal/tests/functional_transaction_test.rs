//! Port of tests/Functional/TransactionTest.php on `pdo_sqlite` in memory.
//! The connection-loss tests kill the session from a privileged
//! connection, which upstream skips on `SQLite` ("Unsupported test
//! platform"): they stay pending (parity/doctrine/dbal.tsv).
//!
//! Upstream: doctrine/dbal tests/Functional/TransactionTest.php
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

/// `(string) $value` of a count.
fn string(value: Option<Value>) -> String {
    match value {
        Some(Value::Int(i)) => i.to_string(),
        Some(Value::String(s)) => s.to_string_lossy().into_owned(),
        other => panic!("not a count: {other:?}"),
    }
}

#[tokio::test]
async fn testNestedTransactionWalkthrough() {
    let connection = sqlite();
    assert!(
        connection
            .get_database_platform()
            .await
            .unwrap()
            .supports_savepoints()
    );
    drop_and_create_table(&connection, "storage").await;
    let query = "SELECT count(test_int) FROM storage";
    assert_eq!(
        string(
            connection
                .fetch_one(query, &none(), &no_types())
                .await
                .unwrap()
        ),
        "0"
    );
    let result = connection
        .transactional(async |connection: &Connection| {
            connection
                .transactional(async |connection: &Connection| {
                    connection
                        .insert("storage", &array([("test_int", i(1))]), &no_types())
                        .await?;
                    connection.fetch_one(query, &none(), &no_types()).await
                })
                .await
        })
        .await
        .unwrap();
    assert_eq!(string(result), "1");
    assert_eq!(
        string(
            connection
                .fetch_one(query, &none(), &no_types())
                .await
                .unwrap()
        ),
        "1"
    );
}
