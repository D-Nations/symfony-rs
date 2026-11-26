//! Port of tests/Functional/WriteTest.php on `pdo_sqlite` in memory. The
//! tables and the empty identity insert are what `SQLitePlatform` builds
//! (tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/Functional/WriteTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{
    array, date_time, ddl, drop_and_create_table, i, list, named_types, no_types, none,
    positional_types, s, sql, sqlite,
};
use doctrine_dbal::{Connection, Exception, ParameterType, ParameterTypes, WrapperParameterType};
use php_value::{PhpArray, Value};

/// A numeric `int|string` id as an integer, as `assertGreaterThan()`
/// compares it.
fn int(value: &Value) -> i64 {
    match value {
        Value::Int(i) => *i,
        Value::String(s) => s.to_string_lossy().parse().unwrap(),
        other => panic!("not an id: {other:?}"),
    }
}

async fn set_up() -> Connection {
    let connection = sqlite();
    drop_and_create_table(&connection, "write_table").await;
    connection
        .execute_statement("DELETE FROM write_table", &none(), &no_types())
        .await
        .unwrap();
    connection
}

fn types(pairs: &[(&'static str, &'static str)]) -> ParameterTypes {
    named_types(
        pairs
            .iter()
            .map(|(name, type_)| (*name, WrapperParameterType::from(*type_))),
    )
}

#[tokio::test]
async fn testExecuteUpdate() {
    let connection = set_up().await;
    let sql = "INSERT INTO write_table (test_int) VALUES (1)";
    let affected = connection
        .execute_statement(sql, &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(
        affected, 1,
        "executeStatement() should return the number of affected rows!"
    );
}

#[tokio::test]
async fn testExecuteStatementWithTypes() {
    let connection = set_up().await;
    let sql = "INSERT INTO write_table (test_int, test_string) VALUES (?, ?)";
    let affected = connection
        .execute_statement(
            sql,
            &list([i(1), s("foo")]),
            &positional_types([ParameterType::Integer.into(), ParameterType::String.into()]),
        )
        .await
        .unwrap();
    assert_eq!(
        affected, 1,
        "executeStatement() should return the number of affected rows!"
    );
}

async fn prepare_insert(connection: &Connection, typed: bool) -> i64 {
    let sql = "INSERT INTO write_table (test_int, test_string) VALUES (?, ?)";
    let mut stmt = connection.prepare(sql).await.unwrap();
    if typed {
        stmt.bind_value(1, i(1), ParameterType::Integer).unwrap();
        stmt.bind_value(2, s("foo"), ParameterType::String).unwrap();
    } else {
        stmt.bind_string(1, i(1)).unwrap();
        stmt.bind_string(2, s("foo")).unwrap();
    }
    stmt.execute_statement().await.unwrap()
}

#[tokio::test]
async fn testPrepareRowCountReturnsAffectedRows() {
    let connection = set_up().await;
    assert_eq!(prepare_insert(&connection, false).await, 1);
}

#[tokio::test]
async fn testPrepareWithPrimitiveTypes() {
    let connection = set_up().await;
    assert_eq!(prepare_insert(&connection, true).await, 1);
}

#[tokio::test]
async fn testPrepareWithDoctrineMappingTypes() {
    let connection = set_up().await;
    assert_eq!(prepare_insert(&connection, true).await, 1);
}

#[tokio::test]
async fn testPrepareWithDoctrineMappingTypeNames() {
    let connection = set_up().await;
    assert_eq!(prepare_insert(&connection, true).await, 1);
}

async fn insert_rows(connection: &Connection) {
    assert_eq!(
        connection
            .insert(
                "write_table",
                &array([("test_int", i(1)), ("test_string", s("foo"))]),
                &no_types()
            )
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .insert(
                "write_table",
                &array([("test_int", i(2)), ("test_string", s("bar"))]),
                &no_types()
            )
            .await
            .unwrap(),
        1
    );
}

async fn count_all(connection: &Connection, sql: &str) -> usize {
    connection
        .fetch_all_associative(sql, &none(), &no_types())
        .await
        .unwrap()
        .len()
}

#[tokio::test]
async fn testInsert() {
    let connection = set_up().await;
    insert_rows(&connection).await;
}

#[tokio::test]
async fn testDelete() {
    let connection = set_up().await;
    insert_rows(&connection).await;
    assert_eq!(
        connection
            .delete("write_table", &array([("test_int", i(2))]), &no_types())
            .await
            .unwrap(),
        1
    );
    assert_eq!(count_all(&connection, "SELECT * FROM write_table").await, 1);
    assert_eq!(
        connection
            .delete("write_table", &array([("test_int", i(1))]), &no_types())
            .await
            .unwrap(),
        1
    );
    assert_eq!(count_all(&connection, "SELECT * FROM write_table").await, 0);
}

#[tokio::test]
async fn testDeleteAll() {
    let connection = set_up().await;
    insert_rows(&connection).await;
    assert_eq!(
        connection
            .delete("write_table", &none(), &no_types())
            .await
            .unwrap(),
        2
    );
    assert_eq!(count_all(&connection, "SELECT * FROM write_table").await, 0);
}

#[tokio::test]
async fn testUpdate() {
    let connection = set_up().await;
    insert_rows(&connection).await;
    let update = async |set: &str, criteria: &str| {
        connection
            .update(
                "write_table",
                &array([("test_string", s(set))]),
                &array([("test_string", s(criteria))]),
                &no_types(),
            )
            .await
            .unwrap()
    };
    assert_eq!(update("bar", "foo").await, 1);
    assert_eq!(update("baz", "bar").await, 2);
    assert_eq!(update("baz", "bar").await, 0);
}

#[tokio::test]
async fn testUpdateAll() {
    let connection = set_up().await;
    insert_rows(&connection).await;
    assert_eq!(
        connection
            .update(
                "write_table",
                &array([("test_string", s("baz"))]),
                &none(),
                &no_types()
            )
            .await
            .unwrap(),
        2
    );
}

#[tokio::test]
async fn testLastInsertId() {
    let connection = set_up().await;
    assert!(
        connection
            .get_database_platform()
            .await
            .unwrap()
            .supports_identity_columns()
    );
    assert_eq!(
        connection
            .insert(
                "write_table",
                &array([("test_int", i(2)), ("test_string", s("bar"))]),
                &no_types()
            )
            .await
            .unwrap(),
        1
    );
    let num = connection.last_insert_id().await.unwrap();
    assert!(
        int(&num) > 0,
        "LastInsertId() should be non-negative number."
    );
}

#[tokio::test]
async fn testLastInsertIdNewConnection() {
    let _ = set_up().await;
    let connection = sqlite();
    let e = connection.last_insert_id().await.unwrap_err();
    assert!(e.is_instance_of(Exception::DRIVER_EXCEPTION));
    assert!(
        e.previous()
            .unwrap()
            .is_instance_of(Exception::NO_IDENTITY_VALUE)
    );
}

fn date_time_format(value: &Value) -> Value {
    let date = value
        .as_object()
        .unwrap()
        .downcast_ref::<php_value::datetime::DateTime>()
        .unwrap();
    s(&date.format(&sql("datetime_format")))
}

#[tokio::test]
async fn testInsertWithKeyValueTypes() {
    let connection = set_up().await;
    let test_string = date_time("2013-04-14 10:10:10");
    connection
        .insert(
            "write_table",
            &array([("test_int", s("30")), ("test_string", test_string.clone())]),
            &types(&[("test_string", "datetime"), ("test_int", "integer")]),
        )
        .await
        .unwrap();
    let data = connection
        .fetch_one(
            "SELECT test_string FROM write_table WHERE test_int = 30",
            &none(),
            &no_types(),
        )
        .await
        .unwrap();
    assert_eq!(data, Some(date_time_format(&test_string)));
}

#[tokio::test]
async fn testUpdateWithKeyValueTypes() {
    let connection = set_up().await;
    let test_string = date_time("2013-04-14 10:10:10");
    let types = types(&[("test_string", "datetime"), ("test_int", "integer")]);
    connection
        .insert(
            "write_table",
            &array([("test_int", s("30")), ("test_string", test_string)]),
            &types,
        )
        .await
        .unwrap();
    let test_string = date_time("2013-04-15 10:10:10");
    connection
        .update(
            "write_table",
            &array([("test_string", test_string.clone())]),
            &array([("test_int", s("30"))]),
            &types,
        )
        .await
        .unwrap();
    let data = connection
        .fetch_one(
            "SELECT test_string FROM write_table WHERE test_int = 30",
            &none(),
            &no_types(),
        )
        .await
        .unwrap();
    assert_eq!(data, Some(date_time_format(&test_string)));
}

#[tokio::test]
async fn testDeleteWithKeyValueTypes() {
    let connection = set_up().await;
    let val = date_time("2013-04-14 10:10:10");
    let types = types(&[("test_string", "datetime"), ("test_int", "integer")]);
    connection
        .insert(
            "write_table",
            &array([("test_int", s("30")), ("test_string", val.clone())]),
            &types,
        )
        .await
        .unwrap();
    connection
        .delete(
            "write_table",
            &array([("test_int", i(30)), ("test_string", val)]),
            &types,
        )
        .await
        .unwrap();
    let data = connection
        .fetch_one(
            "SELECT test_string FROM write_table WHERE test_int = 30",
            &none(),
            &no_types(),
        )
        .await
        .unwrap();
    assert_eq!(data, None);
}

#[tokio::test]
async fn testEmptyIdentityInsert() {
    let connection = set_up().await;
    assert!(
        connection
            .get_database_platform()
            .await
            .unwrap()
            .supports_identity_columns()
    );
    let _ = connection
        .execute_statement("DROP TABLE test_empty_identity", &none(), &no_types())
        .await;
    for statement in ddl("test_empty_identity") {
        connection
            .execute_statement(&statement, &none(), &no_types())
            .await
            .unwrap();
    }
    let sql = sql("empty_identity_insert");
    connection
        .execute_statement(&sql, &none(), &no_types())
        .await
        .unwrap();
    let first_id = connection.last_insert_id().await.unwrap();
    connection
        .execute_statement(&sql, &none(), &no_types())
        .await
        .unwrap();
    let second_id = connection.last_insert_id().await.unwrap();
    assert!(int(&second_id) > int(&first_id));
}

async fn rows_with_30(connection: &Connection) -> Vec<PhpArray> {
    connection
        .fetch_all_associative(
            "SELECT * FROM write_table WHERE test_int = 30",
            &none(),
            &no_types(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn testUpdateWhereIsNull() {
    let connection = set_up().await;
    let types = types(&[("test_string", "string"), ("test_int", "integer")]);
    connection
        .insert(
            "write_table",
            &array([("test_int", s("30")), ("test_string", Value::Null)]),
            &types,
        )
        .await
        .unwrap();
    assert_eq!(rows_with_30(&connection).await.len(), 1);
    connection
        .update(
            "write_table",
            &array([("test_int", i(10))]),
            &array([("test_string", Value::Null)]),
            &types,
        )
        .await
        .unwrap();
    assert_eq!(rows_with_30(&connection).await.len(), 0);
}

#[tokio::test]
async fn testDeleteWhereIsNull() {
    let connection = set_up().await;
    let types_ = types(&[("test_string", "string"), ("test_int", "integer")]);
    connection
        .insert(
            "write_table",
            &array([("test_int", s("30")), ("test_string", Value::Null)]),
            &types_,
        )
        .await
        .unwrap();
    assert_eq!(rows_with_30(&connection).await.len(), 1);
    connection
        .delete(
            "write_table",
            &array([("test_string", Value::Null)]),
            &types(&[("test_string", "string")]),
        )
        .await
        .unwrap();
    assert_eq!(rows_with_30(&connection).await.len(), 0);
}
