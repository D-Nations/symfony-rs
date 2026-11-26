//! The sqlx `SQLite` driver against what `pdo_sqlite` gives through DBAL in
//! PHP 8.4 (tests/data/connection.json, `scripts/record.sh
//! doctrine-dbal-connection`): value types, the binding of each value with
//! each parameter type, errors and their conversion, `rowCount()`,
//! `executeStatement()`, `lastInsertId()`, `quote()`, column names and
//! connection errors.
//!
//! Upstream: none
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{assert_outcome, data, no_types, none, sqlite, text};
use doctrine_dbal::{Exception, ParameterType};
use php_value::vectors::decode;
use php_value::{Key, PhpArray, Value};

fn rows(rows: Vec<PhpArray>) -> Value {
    Value::Array(PhpArray::list(rows.into_iter().map(Value::Array)))
}

fn parameter_type(json: &serde_json::Value) -> ParameterType {
    assert_eq!(json[0], "enum");
    ParameterType::from_name(json[2].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn bindingMatrix() {
    for case in data()["bind"].as_array().unwrap() {
        let type_ = parameter_type(&case[0]);
        let value = decode(&case[1]).unwrap();
        let connection = sqlite();
        let outcome = async {
            let mut statement = connection.prepare("SELECT typeof(?), ?").await?;
            statement.bind_value(1, value.clone(), type_)?;
            statement.bind_value(2, value.clone(), type_)?;
            let row = statement.execute_query().await?.fetch_numeric()?;
            Ok::<_, Exception>(row.map_or(Value::Bool(false), Value::Array))
        }
        .await;
        assert_outcome(&format!("{type_:?} {value:?}"), &case[2], outcome);
    }
}

/// `prepare()` and `query()` with several statements: PDO runs the first
/// only, sqlx runs them all (README.md, Differences); `exec()` runs them
/// all in both.
fn several_statements(sql: &str) -> bool {
    sql.trim_end_matches(';').contains(';')
}

#[tokio::test]
async fn errors() {
    for case in data()["errors"].as_array().unwrap() {
        let sql = text(&case[0]);
        let outcome = sqlite()
            .execute_statement(&sql, &none(), &no_types())
            .await
            .map(Value::Int);
        assert_outcome(&format!("executeStatement {sql}"), &case[1], outcome);
        if several_statements(&sql) {
            continue;
        }
        let connection = sqlite();
        let outcome = async {
            Ok(rows(
                connection
                    .execute_query(&sql, &none(), &no_types())
                    .await?
                    .fetch_all_numeric()?,
            ))
        }
        .await;
        assert_outcome(&format!("executeQuery {sql}"), &case[2], outcome);
        let connection = sqlite();
        let outcome = async {
            Ok(rows(
                connection
                    .prepare(&sql)
                    .await?
                    .execute_query()
                    .await?
                    .fetch_all_numeric()?,
            ))
        }
        .await;
        assert_outcome(&format!("prepare {sql}"), &case[3], outcome);
    }
}

#[tokio::test]
async fn fetchedValues() {
    for case in data()["fetched"].as_array().unwrap() {
        let sql = text(&case[0]);
        let outcome = sqlite()
            .fetch_numeric(&sql, &none(), &no_types())
            .await
            .map(|row| row.map_or(Value::Bool(false), Value::Array));
        assert_outcome(&format!("fetchNumeric {sql}"), &case[1], outcome);
        let outcome = sqlite()
            .fetch_associative(&sql, &none(), &no_types())
            .await
            .map(|row| row.map_or(Value::Bool(false), Value::Array));
        assert_outcome(&format!("fetchAssociative {sql}"), &case[2], outcome);
    }
}

fn arg(step: &PhpArray, index: i64) -> Value {
    step.get(&Key::Int(index)).cloned().unwrap_or(Value::Null)
}

fn arg_text(step: &PhpArray, index: i64) -> String {
    arg(step, index)
        .as_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// One connection through a sequence of operations.
#[tokio::test]
async fn sequence() {
    let connection = sqlite();
    for case in data()["sequence"].as_array().unwrap() {
        let Value::Array(step) = decode(&case[0]).unwrap() else {
            panic!("step");
        };
        let op = arg_text(&step, 0);
        let outcome: Result<Value, Exception> = async {
            Ok(match op.as_str() {
                "lastInsertId" => connection.last_insert_id().await?,
                "statement" => Value::Int(
                    connection
                        .execute_statement(&arg_text(&step, 1), &none(), &no_types())
                        .await?,
                ),
                "statementParams" => {
                    let Value::Array(params) = arg(&step, 2) else {
                        panic!("params");
                    };
                    Value::Int(
                        connection
                            .execute_statement(&arg_text(&step, 1), &params, &no_types())
                            .await?,
                    )
                }
                "rowCount" => Value::Int(
                    connection
                        .execute_query(&arg_text(&step, 1), &none(), &no_types())
                        .await?
                        .row_count()?,
                ),
                "quote" => Value::String(
                    connection
                        .quote(arg(&step, 1).as_string().unwrap().as_bytes())
                        .await?,
                ),
                "columnName" => {
                    let Value::Int(index) = arg(&step, 2) else {
                        panic!("index");
                    };
                    Value::String(
                        connection
                            .execute_query(&arg_text(&step, 1), &none(), &no_types())
                            .await?
                            .get_column_name(index)?,
                    )
                }
                "bindNamed" | "bindIndex" => {
                    let mut statement = connection.prepare(&arg_text(&step, 1)).await?;
                    let param = Key::from_value(&arg(&step, 2)).unwrap();
                    statement.bind_value(param, arg(&step, 3), ParameterType::String)?;
                    rows(statement.execute_query().await?.fetch_all_numeric()?)
                }
                "database" => connection
                    .get_database()
                    .await?
                    .unwrap_or(Value::Bool(false)),
                other => panic!("unknown step {other}"),
            })
        }
        .await;
        assert_outcome(&format!("{step:?}"), &case[1], outcome);
    }
}

#[tokio::test]
async fn connectErrors() {
    let cases = data()["connect"].as_array().unwrap();
    let params = [
        connection_support::array([
            ("driver", connection_support::s("pdo_sqlite")),
            ("path", connection_support::s("/nonexistent/dir/db.sqlite")),
        ]),
        connection_support::array([
            ("driver", connection_support::s("pdo_sqlite")),
            ("user", Value::Int(1)),
            ("memory", Value::Bool(true)),
        ]),
    ];
    for (params, expected) in params.into_iter().zip(cases) {
        let connection = doctrine_dbal::DriverManager::get_connection(params, None).unwrap();
        let outcome = connection
            .execute_query("SELECT 1", &none(), &no_types())
            .await
            .map(|_| Value::Null);
        assert_outcome("connect", expected, outcome);
    }
}
