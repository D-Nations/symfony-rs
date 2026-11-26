//! Port of tests/Functional/ConnectionTest.php on `pdo_sqlite` in memory
//! (the functional suite's default connection). The table comes from the
//! DDL the schema manager creates for it (tests/data/connection.json).
//!
//! Upstream: doctrine/dbal tests/Functional/ConnectionTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{array, drop_and_create_table, i, no_types, none, s, sql, sqlite};
use doctrine_dbal::{Connection, DriverManager, Exception};
use php_value::{Key, Value};

const TABLE: &str = "connection_test";

async fn create_test_table(connection: &Connection) {
    drop_and_create_table(connection, TABLE).await;
    connection
        .insert(TABLE, &array([("id", i(1))]), &no_types())
        .await
        .unwrap();
}

#[tokio::test]
async fn testServerVersion() {
    let connection = sqlite();
    assert_ne!(connection.get_server_version().await.unwrap(), "");
}

#[tokio::test]
async fn testCommitWithRollbackOnlyThrowsException() {
    let connection = sqlite();
    connection.begin_transaction().await.unwrap();
    connection.set_rollback_only().unwrap();
    let e = connection.commit().await.unwrap_err();
    assert!(e.is_instance_of(Exception::CONNECTION_EXCEPTION));
}

#[tokio::test]
async fn testTransactionNestingBehavior() {
    let connection = sqlite();
    assert!(
        connection
            .get_database_platform()
            .await
            .unwrap()
            .supports_savepoints()
    );
    create_test_table(&connection).await;
    connection.begin_transaction().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 1);
    connection.begin_transaction().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 2);
    let e = connection
        .insert(TABLE, &array([("id", i(1))]), &no_types())
        .await
        .expect_err("Expected exception to be thrown because of the unique constraint.");
    assert!(e.is_instance_of(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION));
    connection.roll_back().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 1);
    connection.commit().await.unwrap();
    connection.begin_transaction().await.unwrap();
    connection.close();
    connection.begin_transaction().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 1);
}

/// The PHP test uses `/tmp/test_nesting.sqlite`; here a file of the
/// system's temporary directory named for the process.
#[tokio::test]
async fn testTransactionNestingLevelIsResetOnReconnect() {
    let this = sqlite();
    let path = std::env::temp_dir().join(format!(
        "doctrine_dbal_test_nesting_{}.sqlite",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let mut params = this.get_params().clone();
    params.insert("memory", Value::Bool(false));
    params.insert("path", s(path.to_str().unwrap()));
    let connection = DriverManager::get_connection(params, None).unwrap();
    this.execute_statement("DROP TABLE IF EXISTS test_nesting", &none(), &no_types())
        .await
        .unwrap();
    connection
        .execute_query(
            "CREATE TABLE test_nesting(test int not null)",
            &none(),
            &no_types(),
        )
        .await
        .unwrap();
    this.begin_transaction().await.unwrap();
    this.begin_transaction().await.unwrap();
    // connection closed in runtime (for example if lost or another
    // application logic)
    connection.close();
    connection.begin_transaction().await.unwrap();
    connection
        .execute_query("insert into test_nesting values (33)", &none(), &no_types())
        .await
        .unwrap();
    connection.roll_back().await.unwrap();
    let count = connection
        .fetch_one("select count(*) from test_nesting", &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(count, Some(i(0)));
    connection.close();
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn testTransactionNestingBehaviorWithSavepoints() {
    let connection = sqlite();
    create_test_table(&connection).await;
    connection.begin_transaction().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 1);
    connection.begin_transaction().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 2);
    connection.begin_transaction().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 3);
    connection.commit().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 2);
    let e = connection
        .insert(TABLE, &array([("id", i(1))]), &no_types())
        .await
        .expect_err("Expected exception to be thrown because of the unique constraint.");
    assert!(e.is_instance_of(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION));
    connection.roll_back().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 1);
    assert!(!connection.is_rollback_only().unwrap());
    connection.commit().await.expect(
        "Transaction commit after failed nested transaction should not fail when using savepoints.",
    );
}

#[tokio::test]
async fn testTransactionIsInactiveAfterConnectionClose() {
    let connection = sqlite();
    connection.begin_transaction().await.unwrap();
    connection.close();
    assert!(!connection.is_transaction_active());
}

#[tokio::test]
async fn testTransactionBehaviorWithRollback() {
    let connection = sqlite();
    create_test_table(&connection).await;
    connection.begin_transaction().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 1);
    let e = connection
        .insert(TABLE, &array([("id", i(1))]), &no_types())
        .await
        .expect_err("Expected exception to be thrown because of the unique constraint.");
    assert!(e.is_instance_of(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION));
    assert_eq!(connection.get_transaction_nesting_level(), 1);
    connection.roll_back().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 0);
}

#[tokio::test]
async fn testTransactionBehaviour() {
    let connection = sqlite();
    create_test_table(&connection).await;
    connection.begin_transaction().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 1);
    connection
        .insert(TABLE, &array([("id", i(2))]), &no_types())
        .await
        .unwrap();
    connection.commit().await.unwrap();
    assert_eq!(connection.get_transaction_nesting_level(), 0);
}

#[tokio::test]
async fn testTransactionalWithException() {
    let connection = sqlite();
    create_test_table(&connection).await;
    let e = connection
        .transactional(async |connection: &Connection| {
            connection
                .insert(TABLE, &array([("id", i(1))]), &no_types())
                .await
                .map(drop)
        })
        .await
        .expect_err("Expected exception to be thrown because of the unique constraint.");
    assert!(e.is_instance_of(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION));
    assert_eq!(connection.get_transaction_nesting_level(), 0);
}

#[tokio::test]
async fn testTransactionalWithThrowable() {
    let connection = sqlite();
    let e = connection
        .transactional(async |conn: &Connection| -> Result<(), Exception> {
            let dummy = conn
                .get_database_platform()
                .await?
                .get_dummy_select_sql("1");
            conn.execute_query(&dummy, &none(), &no_types()).await?;
            Err(Exception::new(Exception::ERROR, "Ooops!"))
        })
        .await
        .unwrap_err();
    assert_eq!(e.class(), Exception::ERROR);
    assert_eq!(connection.get_transaction_nesting_level(), 0);
}

#[tokio::test]
async fn testTransactional() {
    let connection = sqlite();
    create_test_table(&connection).await;
    let res = connection
        .transactional(async |connection: &Connection| {
            connection
                .insert(TABLE, &array([("id", i(2))]), &no_types())
                .await
        })
        .await
        .unwrap();
    assert_eq!(res, 1);
    assert_eq!(connection.get_transaction_nesting_level(), 0);
}

#[tokio::test]
async fn testTransactionalReturnValue() {
    let connection = sqlite();
    let res = connection
        .transactional(async |_connection: &Connection| Ok(42))
        .await
        .unwrap();
    assert_eq!(res, 42);
}

#[tokio::test]
async fn testConnectWithoutExplicitDatabaseName() {
    let this = sqlite();
    let mut params = this.get_params().clone();
    params.remove(&Key::from("dbname"));
    let connection = DriverManager::get_connection(params, None).unwrap();
    assert_eq!(
        connection
            .fetch_one(&sql("dummy_select"), &none(), &no_types())
            .await
            .unwrap(),
        Some(i(1))
    );
}

#[tokio::test]
async fn testDeterminesDatabasePlatformWhenConnectingToNonExistentDatabase() {
    let this = sqlite();
    let mut params = this.get_params().clone();
    params.insert("dbname", s("foo_bar"));
    let connection = DriverManager::get_connection(params.clone(), None).unwrap();
    assert!(!connection.is_connected());
    assert!(Value::Array(params).identical(&Value::Array(connection.get_params().clone())));
    connection.close();
}

#[tokio::test]
async fn testExceptionOnExecuteStatement() {
    let e = sqlite()
        .execute_statement("foo", &none(), &no_types())
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::DRIVER_EXCEPTION));
}

#[tokio::test]
async fn testExceptionOnExecuteQuery() {
    let e = sqlite()
        .execute_query("foo", &none(), &no_types())
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::DRIVER_EXCEPTION));
}

#[tokio::test]
async fn testExceptionOnPrepareAndExecute() {
    let connection = sqlite();
    let e = async { connection.prepare("foo").await?.execute_statement().await }
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::DRIVER_EXCEPTION));
}
