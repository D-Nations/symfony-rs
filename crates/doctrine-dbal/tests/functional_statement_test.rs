//! Port of tests/Functional/StatementTest.php on `pdo_sqlite` in memory.
//! The tables and the SQL of `SQLitePlatform` are recorded
//! (tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/Functional/StatementTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{array, drop_and_create_table, i, list, no_types, s, sql, sqlite};
use doctrine_dbal::{Connection, Exception, ParameterType};
use php_value::Value;
use php_value::vectors::phpunit_equals;

async fn set_up() -> Connection {
    let connection = sqlite();
    drop_and_create_table(&connection, "stmt_test").await;
    connection
}

async fn insert_ids(connection: &Connection) {
    for id in [1, 2] {
        connection
            .insert("stmt_test", &array([("id", i(id))]), &no_types())
            .await
            .unwrap();
    }
}

fn rows(rows: Vec<php_value::PhpArray>) -> Value {
    Value::Array(php_value::PhpArray::list(
        rows.into_iter().map(Value::Array),
    ))
}

#[tokio::test]
async fn testStatementIsReusableAfterFreeingResult() {
    let connection = set_up().await;
    insert_ids(&connection).await;
    let mut stmt = connection
        .prepare("SELECT id FROM stmt_test ORDER BY id")
        .await
        .unwrap();
    let mut result = stmt.execute_query().await.unwrap();
    let id = result.fetch_one().unwrap();
    assert_eq!(id, Some(i(1)));
    result.free();
    let mut result = stmt.execute_query().await.unwrap();
    assert_eq!(result.fetch_one().unwrap(), Some(i(1)));
    assert_eq!(result.fetch_one().unwrap(), Some(i(2)));
}

#[tokio::test]
async fn testReuseStatementWithLongerResults() {
    let connection = set_up().await;
    drop_and_create_table(&connection, "stmt_longer_results").await;
    connection
        .insert(
            "stmt_longer_results",
            &array([("param", s("param1")), ("val", s("X"))]),
            &no_types(),
        )
        .await
        .unwrap();
    let mut stmt = connection
        .prepare("SELECT param, val FROM stmt_longer_results ORDER BY param")
        .await
        .unwrap();
    let result = stmt
        .execute_query()
        .await
        .unwrap()
        .fetch_all_numeric()
        .unwrap();
    assert!(phpunit_equals(
        &rows(vec![list([s("param1"), s("X")])]),
        &rows(result)
    ));
    connection
        .insert(
            "stmt_longer_results",
            &array([("param", s("param2")), ("val", s("A bit longer value"))]),
            &no_types(),
        )
        .await
        .unwrap();
    let result = stmt
        .execute_query()
        .await
        .unwrap()
        .fetch_all_numeric()
        .unwrap();
    assert!(phpunit_equals(
        &rows(vec![
            list([s("param1"), s("X")]),
            list([s("param2"), s("A bit longer value")]),
        ]),
        &rows(result)
    ));
}

#[tokio::test]
async fn testIncompletelyFetchedStatementDoesNotBlockConnection() {
    let connection = set_up().await;
    insert_ids(&connection).await;
    let mut stmt1 = connection
        .prepare("SELECT id FROM stmt_test")
        .await
        .unwrap();
    let mut result = stmt1.execute_query().await.unwrap();
    result.fetch_associative().unwrap();
    let mut result = stmt1.execute_query().await.unwrap();
    // fetching only one record out of two
    result.fetch_associative().unwrap();
    assert_eq!(
        connection
            .fetch_one(
                "SELECT id FROM stmt_test WHERE id = ?",
                &list([i(1)]),
                &no_types()
            )
            .await
            .unwrap(),
        Some(i(1))
    );
}

#[tokio::test]
async fn testReuseStatementAfterFreeingResult() {
    let connection = set_up().await;
    insert_ids(&connection).await;
    let mut stmt = connection
        .prepare("SELECT id FROM stmt_test WHERE id = ?")
        .await
        .unwrap();
    stmt.bind_string(1, i(1)).unwrap();
    let mut result = stmt.execute_query().await.unwrap();
    let id = result.fetch_one().unwrap();
    assert!(phpunit_equals(&i(1), &id.unwrap()));
    result.free();
    stmt.bind_string(1, i(2)).unwrap();
    let mut result = stmt.execute_query().await.unwrap();
    let id = result.fetch_one().unwrap();
    assert!(phpunit_equals(&i(2), &id.unwrap()));
}

#[tokio::test]
async fn testReuseStatementWithReboundValue() {
    let connection = set_up().await;
    insert_ids(&connection).await;
    let mut stmt = connection
        .prepare("SELECT id FROM stmt_test WHERE id = ?")
        .await
        .unwrap();
    stmt.bind_string(1, i(1)).unwrap();
    let mut result = stmt.execute_query().await.unwrap();
    assert!(phpunit_equals(&i(1), &result.fetch_one().unwrap().unwrap()));
    stmt.bind_string(1, i(2)).unwrap();
    let mut result = stmt.execute_query().await.unwrap();
    assert!(phpunit_equals(&i(2), &result.fetch_one().unwrap().unwrap()));
}

#[tokio::test]
async fn testBindInvalidNamedParameter() {
    let connection = set_up().await;
    let mut statement = connection.prepare(&sql("dummy_select_foo")).await.unwrap();
    let e = async {
        statement.bind_string("bar", s("baz"))?;
        statement.execute_query().await
    }
    .await
    .unwrap_err();
    assert!(e.is_instance_of(Exception::DRIVER_EXCEPTION));
}

#[tokio::test]
async fn testParameterBindingOrder() {
    let connection = set_up().await;
    // some supported drivers don't support selecting an untyped literal
    // from a dummy table, so we wrap it into a function that assumes its
    // type
    let mut stmt = connection.prepare(&sql("length_length")).await.unwrap();
    stmt.bind_string(2, s("banana")).unwrap();
    stmt.bind_string(1, s("apple")).unwrap();
    let row = stmt.execute_query().await.unwrap().fetch_numeric().unwrap();
    assert!(phpunit_equals(
        &Value::Array(list([i(5), i(6)])),
        &Value::Array(row.unwrap())
    ));
}

#[tokio::test]
async fn testFetchInColumnMode() {
    let connection = set_up().await;
    let result = connection
        .execute_query(
            &sql("dummy_select"),
            &connection_support::none(),
            &no_types(),
        )
        .await
        .unwrap()
        .fetch_one()
        .unwrap();
    assert!(phpunit_equals(&i(1), &result.unwrap()));
}

#[tokio::test]
async fn testExecWithRedundantParameters() {
    let connection = set_up().await;
    let mut stmt = connection.prepare(&sql("dummy_select")).await.unwrap();
    // we want to make sure the exception is thrown by the DBAL code, not by
    // PHPUnit due to a PHP-level error, but the wrapper connection wraps
    // everything in a DBAL exception
    let e = async {
        stmt.bind_string(1, Value::Null)?;
        stmt.execute_query().await
    }
    .await
    .unwrap_err();
    assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"));
}

#[tokio::test]
async fn testExecuteQuery() {
    let connection = set_up().await;
    let result = connection
        .prepare(&sql("dummy_select"))
        .await
        .unwrap()
        .execute_query()
        .await
        .unwrap()
        .fetch_one()
        .unwrap();
    assert!(phpunit_equals(&i(1), &result.unwrap()));
}

#[tokio::test]
async fn testExecuteStatement() {
    let connection = set_up().await;
    connection
        .insert("stmt_test", &array([("id", i(1))]), &no_types())
        .await
        .unwrap();
    let mut stmt = connection
        .prepare("UPDATE stmt_test SET name = ? WHERE id = 1")
        .await
        .unwrap();
    stmt.bind_string(1, s("bar")).unwrap();
    assert_eq!(stmt.execute_statement().await.unwrap(), 1);
    let mut stmt = connection
        .prepare("UPDATE stmt_test SET name = ? WHERE id = ?")
        .await
        .unwrap();
    stmt.bind_string(1, s("foo")).unwrap();
    stmt.bind_value(2, i(1), ParameterType::String).unwrap();
    assert_eq!(stmt.execute_statement().await.unwrap(), 1);
}
