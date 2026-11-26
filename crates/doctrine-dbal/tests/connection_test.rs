//! Port of tests/ConnectionTest.php.
//!
//! The PHP test mocks the driver, its connection and `Connection` itself.
//! Here `connection_support::TestDriver` stands for the stubs and mocks: it
//! records the calls its connection, statements and results get, so a
//! test that expects `executeStatement()` to get an SQL and parameters
//! checks what reaches the driver instead (the SQL prepared, each value
//! bound with its binding type, after the type's conversion).
//!
//! Upstream: doctrine/dbal tests/ConnectionTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{
    Script, TestDriver, array, i, list, named_types, no_types, none, positional_types, s,
};
use doctrine_dbal::{Connection, DriverManager, Exception, ParameterType, WrapperParameterType};
use php_value::{PhpArray, Value};

fn connection_params() -> PhpArray {
    array([
        ("driver", s("pdo_mysql")),
        ("host", s("localhost")),
        ("user", s("root")),
        ("password", s("password")),
        ("port", i(1234)),
    ])
}

fn set_up() -> Connection {
    DriverManager::get_connection(connection_params(), None).unwrap()
}

#[test]
fn testIsConnected() {
    assert!(!set_up().is_connected());
}

#[test]
fn testNoTransactionActiveByDefault() {
    assert!(!set_up().is_transaction_active());
}

#[tokio::test]
async fn testCommitWithNoActiveTransactionThrowsException() {
    let e = set_up().commit().await.unwrap_err();
    assert!(e.is_instance_of(Exception::CONNECTION_EXCEPTION));
}

#[tokio::test]
async fn testRollbackWithNoActiveTransactionThrowsException() {
    let e = set_up().roll_back().await.unwrap_err();
    assert!(e.is_instance_of(Exception::CONNECTION_EXCEPTION));
}

#[test]
fn testSetRollbackOnlyNoActiveTransactionThrowsException() {
    let e = set_up().set_rollback_only().unwrap_err();
    assert!(e.is_instance_of(Exception::CONNECTION_EXCEPTION));
}

#[test]
fn testIsRollbackOnlyNoActiveTransactionThrowsException() {
    let e = set_up().is_rollback_only().unwrap_err();
    assert!(e.is_instance_of(Exception::CONNECTION_EXCEPTION));
}

#[test]
fn testGetDriver() {
    assert_eq!(
        set_up().get_driver().class(),
        "Doctrine\\DBAL\\Driver\\PDO\\MySQL\\Driver"
    );
}

/// `getQueryMethods()`.
#[tokio::test]
async fn testDriverExceptionIsWrapped() {
    for method in ["executeQuery", "executeStatement", "prepare"] {
        let connection = connection_support::sqlite();
        let statement = "MUUHAAAAHAAAA";
        let e = match method {
            "executeQuery" => connection
                .execute_query(statement, &none(), &no_types())
                .await
                .map(drop)
                .unwrap_err(),
            "executeStatement" => connection
                .execute_statement(statement, &none(), &no_types())
                .await
                .map(drop)
                .unwrap_err(),
            _ => connection.prepare(statement).await.map(drop).unwrap_err(),
        };
        assert!(e.is_instance_of("Doctrine\\DBAL\\Exception"), "{method}");
        let message = e.message().to_string_lossy();
        assert!(
            message.contains(
                "An exception occurred while executing a query: SQLSTATE[HY000]: General error: 1 near \"MUUHAAAAHAAAA\""
            ),
            "{method}: {message}"
        );
    }
}

#[test]
fn testIsAutoCommit() {
    assert!(set_up().is_auto_commit());
}

#[tokio::test]
async fn testSetAutoCommit() {
    let connection = set_up();
    connection.set_auto_commit(false).await.unwrap();
    assert!(!connection.is_auto_commit());
}

#[tokio::test]
async fn testConnectStartsTransactionInNoAutoCommitMode() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.set_auto_commit(false).await.unwrap();
    assert!(!conn.is_transaction_active());
    conn.execute_query("SELECT 1", &none(), &no_types())
        .await
        .unwrap();
    assert!(conn.is_transaction_active());
}

#[tokio::test]
async fn testTransactionIsNotActiveAfterTransactionalInAutoCommitMode() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.set_auto_commit(true).await.unwrap();
    assert!(!conn.is_transaction_active());
    conn.transactional(async |_connection: &Connection| Ok(()))
        .await
        .unwrap();
    assert!(!conn.is_transaction_active());
}

#[tokio::test]
async fn testTransactionIsActiveAfterTransactionalInNoAutoCommitMode() {
    let driver = TestDriver::new(Script {
        savepoints: true,
        ..Script::default()
    });
    let conn = driver.connection(none());
    conn.set_auto_commit(false).await.unwrap();
    conn.transactional(async |_connection: &Connection| Ok(()))
        .await
        .unwrap();
    assert!(conn.is_transaction_active());
}

#[tokio::test]
async fn testCommitStartsTransactionInNoAutoCommitMode() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.set_auto_commit(false).await.unwrap();
    conn.execute_query("SELECT 1", &none(), &no_types())
        .await
        .unwrap();
    conn.commit().await.unwrap();
    assert!(conn.is_transaction_active());
}

#[tokio::test]
async fn testBeginTransactionFailureAfterCommitInNoAutoCommitMode() {
    let driver = TestDriver::new(Script {
        begin_failures: vec![2],
        ..Script::default()
    });
    let conn = driver.connection(none());
    conn.set_auto_commit(false).await.unwrap();
    doctrine_dbal::ServerVersionProvider::get_server_version(&conn)
        .await
        .unwrap();
    // The RuntimeException is not a driver exception: it goes through.
    let e = conn.commit().await.unwrap_err();
    assert_eq!(e.class(), "RuntimeException");
    assert!(conn.is_transaction_active());
    assert_eq!(driver.calls.count("beginTransaction"), 2);
}

#[tokio::test]
async fn testRollBackStartsTransactionInNoAutoCommitMode() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.set_auto_commit(false).await.unwrap();
    conn.execute_query("SELECT 1", &none(), &no_types())
        .await
        .unwrap();
    conn.roll_back().await.unwrap();
    assert!(conn.is_transaction_active());
}

#[tokio::test]
async fn testSwitchingAutoCommitModeCommitsAllCurrentTransactions() {
    let driver = TestDriver::new(Script {
        savepoints: true,
        ..Script::default()
    });
    let conn = driver.connection(none());
    conn.begin_transaction().await.unwrap();
    conn.begin_transaction().await.unwrap();
    conn.set_auto_commit(false).await.unwrap();
    assert_eq!(conn.get_transaction_nesting_level(), 1);
    conn.begin_transaction().await.unwrap();
    conn.begin_transaction().await.unwrap();
    conn.set_auto_commit(true).await.unwrap();
    assert!(!conn.is_transaction_active());
}

#[tokio::test]
async fn testEmptyInsert() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.insert("footable", &none(), &no_types()).await.unwrap();
    assert_eq!(
        driver.calls.all(),
        ["connect", "exec INSERT INTO footable () VALUES ()"]
    );
}

fn named(types: &[(&'static str, &'static str)]) -> doctrine_dbal::ParameterTypes {
    named_types(
        types
            .iter()
            .map(|(name, type_)| (*name, WrapperParameterType::from(*type_))),
    )
}

#[tokio::test]
async fn testUpdateWithDifferentColumnsInDataAndIdentifiers() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.update(
        "TestTable",
        &array([("text", s("some text")), ("is_edited", Value::Bool(true))]),
        &array([("id", i(1)), ("name", s("foo"))]),
        &named(&[
            ("text", "string"),
            ("is_edited", "boolean"),
            ("id", "integer"),
            ("name", "string"),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        driver.calls.all(),
        [
            "connect",
            "prepare UPDATE TestTable SET text = ?, is_edited = ? WHERE id = ? AND name = ?",
            "getDatabasePlatform",
            "bind 1 'some text' STRING",
            "bind 2 1 BOOLEAN",
            "bind 3 1 INTEGER",
            "bind 4 'foo' STRING",
            "execute",
        ]
    );
}

#[tokio::test]
async fn testUpdateWithSameColumnInDataAndIdentifiers() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.update(
        "TestTable",
        &array([("text", s("some text")), ("is_edited", Value::Bool(true))]),
        &array([("id", i(1)), ("is_edited", Value::Bool(false))]),
        &named(&[
            ("text", "string"),
            ("is_edited", "boolean"),
            ("id", "integer"),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        driver.calls.all(),
        [
            "connect",
            "prepare UPDATE TestTable SET text = ?, is_edited = ? WHERE id = ? AND is_edited = ?",
            "getDatabasePlatform",
            "bind 1 'some text' STRING",
            "bind 2 1 BOOLEAN",
            "bind 3 1 INTEGER",
            "bind 4 0 BOOLEAN",
            "execute",
        ]
    );
}

#[tokio::test]
async fn testUpdateWithIsNull() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.update(
        "TestTable",
        &array([("text", s("some text")), ("is_edited", Value::Null)]),
        &array([("id", Value::Null), ("name", s("foo"))]),
        &named(&[
            ("text", "string"),
            ("is_edited", "boolean"),
            ("id", "integer"),
            ("name", "string"),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        driver.calls.all(),
        [
            "connect",
            "prepare UPDATE TestTable SET text = ?, is_edited = ? WHERE id IS NULL AND name = ?",
            "getDatabasePlatform",
            "bind 1 'some text' STRING",
            "bind 2 null BOOLEAN",
            "bind 3 'foo' STRING",
            "execute",
        ]
    );
}

#[tokio::test]
async fn testDeleteWithIsNull() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.delete(
        "TestTable",
        &array([("id", Value::Null), ("name", s("foo"))]),
        &named(&[("id", "integer"), ("name", "string")]),
    )
    .await
    .unwrap();
    assert_eq!(
        driver.calls.all(),
        [
            "connect",
            "prepare DELETE FROM TestTable WHERE id IS NULL AND name = ?",
            "getDatabasePlatform",
            "bind 1 'foo' STRING",
            "execute",
        ]
    );
}

/// `fetchModeProvider()`: each fetch method gives what the result gives,
/// and the query reaches the driver with its parameter.
#[tokio::test]
async fn testFetch() {
    let query = "SELECT * FROM foo WHERE bar = ?";
    let params = list([i(666)]);
    let types = positional_types([ParameterType::Integer.into()]);
    let rows = || vec![vec![("foo", s("bar"))], vec![("foo", s("baz"))]];
    let row = |value: &str| array([("foo", s(value))]);
    for method in [
        "numeric",
        "associative",
        "one",
        "all-numeric",
        "all-associative",
        "first-column",
    ] {
        let driver = TestDriver::new(Script {
            rows: rows(),
            ..Script::default()
        });
        let conn = driver.connection(none());
        let actual = match method {
            "numeric" => {
                let result = conn.fetch_numeric(query, &params, &types).await.unwrap();
                assert_eq!(result, Some(list([s("bar")])));
                continue_with(&driver)
            }
            "associative" => {
                let result = conn
                    .fetch_associative(query, &params, &types)
                    .await
                    .unwrap();
                assert_eq!(result, Some(row("bar")));
                continue_with(&driver)
            }
            "one" => {
                let result = conn.fetch_one(query, &params, &types).await.unwrap();
                assert_eq!(result, Some(s("bar")));
                continue_with(&driver)
            }
            "all-numeric" => {
                let result = conn
                    .fetch_all_numeric(query, &params, &types)
                    .await
                    .unwrap();
                assert_eq!(result, vec![list([s("bar")]), list([s("baz")])]);
                continue_with(&driver)
            }
            "all-associative" => {
                let result = conn
                    .fetch_all_associative(query, &params, &types)
                    .await
                    .unwrap();
                assert_eq!(result, vec![row("bar"), row("baz")]);
                continue_with(&driver)
            }
            _ => {
                let result = conn
                    .fetch_first_column(query, &params, &types)
                    .await
                    .unwrap();
                assert_eq!(result, vec![s("bar"), s("baz")]);
                continue_with(&driver)
            }
        };
        assert_eq!(
            actual,
            [
                "connect",
                "prepare SELECT * FROM foo WHERE bar = ?",
                "bind 1 666 INTEGER",
                "execute",
            ],
            "{method}"
        );
    }
}

fn continue_with(driver: &TestDriver) -> Vec<String> {
    driver.calls.all()
}

#[tokio::test]
async fn testCallConnectOnce() {
    let driver = TestDriver::new(Script::default());
    let conn = driver.connection(none());
    conn.execute_query("SELECT 1", &none(), &no_types())
        .await
        .unwrap();
    conn.execute_query("SELECT 2", &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(driver.calls.count("connect"), 1);
}

#[tokio::test]
async fn testPlatformDetectionTriggersConnectionIfRequiredByTheDriver() {
    let driver = TestDriver::new(Script {
        server_version: "6.6.6".to_owned(),
        platform_reads_version: true,
        ..Script::default()
    });
    let connection = driver.connection(none());
    let platform = connection.get_database_platform().await.unwrap();
    assert_eq!(platform.class(), "Mock_AbstractPlatform");
    assert_eq!(
        driver.calls.all(),
        ["connect", "getServerVersion", "getDatabasePlatform 6.6.6"]
    );
}

#[tokio::test]
async fn testPlatformDetectionDoesNotTriggerConnectionIfNotRequiredByTheDriver() {
    let driver = TestDriver::new(Script::default());
    let connection = driver.connection(none());
    let platform = connection.get_database_platform().await.unwrap();
    assert_eq!(platform.class(), "Mock_AbstractPlatform");
    assert_eq!(driver.calls.all(), ["getDatabasePlatform"]);
}

#[tokio::test]
async fn testPlatformDetectionFetchedFromParameters() {
    let driver = TestDriver::new(Script {
        platform_reads_version: true,
        ..Script::default()
    });
    let connection = driver.connection(array([("serverVersion", s("8.0"))]));
    connection.get_database_platform().await.unwrap();
    assert_eq!(driver.calls.all(), ["getDatabasePlatform 8.0"]);
}

#[tokio::test]
async fn testPlatformDetectionFetchedFromPrimaryReplicaParameters() {
    let driver = TestDriver::new(Script {
        platform_reads_version: true,
        ..Script::default()
    });
    let connection = driver.connection(array([(
        "primary",
        Value::Array(array([("serverVersion", s("8.0"))])),
    )]));
    connection.get_database_platform().await.unwrap();
    assert_eq!(driver.calls.all(), ["getDatabasePlatform 8.0"]);
}

/// The PHP test overrides `rollBack()` in a subclass; here the driver
/// connection's rollback throws the `ConnectionException`, which is not a
/// driver exception and goes through `Connection::rollBack()` as it is.
#[tokio::test]
async fn testItPreservesTheOriginalExceptionOnRollbackFailure() {
    let driver = TestDriver::new(Script {
        rollback_error: Some(Exception::new(
            Exception::CONNECTION_EXCEPTION,
            "Rollback exception",
        )),
        ..Script::default()
    });
    let connection = driver.connection(array([("memory", Value::Bool(true))]));
    let e = connection
        .transactional(async |_connection: &Connection| -> Result<(), Exception> {
            Err(Exception::new(
                Exception::CONNECTION_EXCEPTION,
                "Original exception",
            ))
        })
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::CONNECTION_EXCEPTION));
    assert_eq!(e.message().as_bytes(), b"Rollback exception");
    let previous = e.previous().unwrap();
    assert_eq!(previous.message().as_bytes(), b"Original exception");
}
