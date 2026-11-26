//! Port of tests/Functional/Connection/FetchEmptyTest.php on `pdo_sqlite`
//! in memory. The query is the SQL `SQLitePlatform` builds (recorded,
//! tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/Functional/Connection/FetchEmptyTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{no_types, none, sql, sqlite};

fn query() -> String {
    sql("fetch_empty_query")
}

#[tokio::test]
async fn testFetchAssociative() {
    let row = sqlite()
        .fetch_associative(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(row, None);
}

#[tokio::test]
async fn testFetchNumeric() {
    let row = sqlite()
        .fetch_numeric(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(row, None);
}

#[tokio::test]
async fn testFetchOne() {
    let value = sqlite()
        .fetch_one(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(value, None);
}

#[tokio::test]
async fn testFetchAllAssociative() {
    let all = sqlite()
        .fetch_all_associative(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(all, [] as [php_value::PhpArray; 0]);
}

#[tokio::test]
async fn testFetchAllNumeric() {
    let all = sqlite()
        .fetch_all_numeric(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(all, [] as [php_value::PhpArray; 0]);
}

#[tokio::test]
async fn testFetchFirstColumn() {
    let all = sqlite()
        .fetch_first_column(&query(), &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(all, [] as [php_value::Value; 0]);
}
