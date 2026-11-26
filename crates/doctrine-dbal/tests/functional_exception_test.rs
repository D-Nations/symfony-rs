//! Port of tests/Functional/ExceptionTest.php on `pdo_sqlite` in memory.
//! The tables are created from the DDL the schema manager creates for
//! them (tests/data/connection.json). The connection tests of a network
//! driver (invalid user name, password, host) are skipped upstream on
//! `SQLite` and stay pending (parity/doctrine/dbal.tsv).
//!
//! Upstream: doctrine/dbal tests/Functional/ExceptionTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{array, ddl, drop_and_create_table, i, no_types, none, s, sqlite};
use doctrine_dbal::{DriverManager, Exception};
use php_value::Value;

#[tokio::test]
async fn testPrimaryConstraintViolationException() {
    let connection = sqlite();
    drop_and_create_table(&connection, "duplicatekey_table").await;
    connection
        .insert("duplicatekey_table", &array([("id", i(1))]), &no_types())
        .await
        .unwrap();
    let e = connection
        .insert("duplicatekey_table", &array([("id", i(1))]), &no_types())
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION));
}

#[tokio::test]
async fn testTableNotFoundException() {
    let sql = "SELECT * FROM unknown_table";
    let e = sqlite()
        .execute_query(sql, &none(), &no_types())
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::TABLE_NOT_FOUND_EXCEPTION));
}

/// `$schemaManager->createTable($table)` twice: the table's DDL twice.
#[tokio::test]
async fn testTableExistsException() {
    let connection = sqlite();
    let create = async || {
        for sql in ddl("alreadyexist_table") {
            connection
                .execute_statement(&sql, &none(), &no_types())
                .await?;
        }
        Ok::<(), Exception>(())
    };
    create().await.unwrap();
    let e = create().await.unwrap_err();
    assert!(e.is_instance_of(Exception::TABLE_EXISTS_EXCEPTION));
}

#[tokio::test]
async fn testNotNullConstraintViolationException() {
    let connection = sqlite();
    drop_and_create_table(&connection, "notnull_table").await;
    let e = connection
        .insert(
            "notnull_table",
            &array([("id", i(1)), ("val", Value::Null)]),
            &no_types(),
        )
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::NOT_NULL_CONSTRAINT_VIOLATION_EXCEPTION));
}

#[tokio::test]
async fn testInvalidFieldNameException() {
    let connection = sqlite();
    drop_and_create_table(&connection, "bad_columnname_table").await;
    let e = connection
        .insert(
            "bad_columnname_table",
            &array([("name", i(5))]),
            &no_types(),
        )
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::INVALID_FIELD_NAME_EXCEPTION));
}

#[tokio::test]
async fn testNonUniqueFieldNameException() {
    let connection = sqlite();
    drop_and_create_table(&connection, "ambiguous_list_table_1").await;
    drop_and_create_table(&connection, "ambiguous_list_table_2").await;
    let sql = "SELECT id FROM ambiguous_list_table_1, ambiguous_list_table_2";
    let e = connection
        .execute_query(sql, &none(), &no_types())
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::NON_UNIQUE_FIELD_NAME_EXCEPTION));
}

#[tokio::test]
async fn testUniqueConstraintViolationException() {
    let connection = sqlite();
    drop_and_create_table(&connection, "unique_column_table").await;
    connection
        .insert("unique_column_table", &array([("id", i(5))]), &no_types())
        .await
        .unwrap();
    let e = connection
        .insert("unique_column_table", &array([("id", i(5))]), &no_types())
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION));
}

#[tokio::test]
async fn testSyntaxErrorException() {
    let connection = sqlite();
    drop_and_create_table(&connection, "syntax_error_table").await;
    let sql = "SELECT id FRO syntax_error_table";
    let e = connection
        .execute_query(sql, &none(), &no_types())
        .await
        .unwrap_err();
    assert!(e.is_instance_of(Exception::SYNTAX_ERROR_EXCEPTION));
}

/// `isPosixSuperUser()`.
fn is_posix_super_user() -> bool {
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .is_ok_and(|output| output.stdout.trim_ascii() == b"0")
}

fn cleanup_read_only_file(filename: &std::path::Path) {
    if is_posix_super_user() {
        let _ = std::process::Command::new("chattr")
            .arg("-i")
            .arg(filename)
            .status();
    }
    let _ = std::fs::remove_file(filename);
}

/// The schema manager's `createTable()` is the table's DDL.
#[tokio::test]
async fn testConnectionExceptionSqLite() {
    use std::os::unix::fs::PermissionsExt;
    let mode = 0o444;
    let filename = std::env::temp_dir().join(format!(
        "doctrine_failed_connection_{mode}_{}.db",
        std::process::id()
    ));
    if filename.exists() {
        cleanup_read_only_file(&filename);
    }
    std::fs::File::create(&filename).unwrap();
    std::fs::set_permissions(&filename, std::fs::Permissions::from_mode(mode)).unwrap();
    if is_posix_super_user() {
        std::process::Command::new("chattr")
            .arg("+i")
            .arg(&filename)
            .status()
            .unwrap();
    }
    let conn = DriverManager::get_connection(
        array([
            ("driver", s("pdo_sqlite")),
            ("path", s(filename.to_str().unwrap())),
        ]),
        None,
    )
    .unwrap();
    let result = async {
        for sql in ddl("no_connection") {
            conn.execute_statement(&sql, &none(), &no_types()).await?;
        }
        Ok::<(), Exception>(())
    }
    .await;
    conn.close();
    cleanup_read_only_file(&filename);
    let e = result.unwrap_err();
    assert!(e.is_instance_of(Exception::READ_ONLY_EXCEPTION), "{e}");
    assert!(
        e.message().to_string_lossy().contains(
            "An exception occurred while executing a query: SQLSTATE[HY000]: General error: 8 attempt to write a readonly database"
        ),
        "{e}"
    );
}
