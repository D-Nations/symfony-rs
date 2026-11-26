//! Port of the execution tests of tests/Query/QueryBuilderTest.php
//! (`testFetch*`, `testExecuteQuery`); the SQL-building tests are in
//! `query_builder_test.rs`.
//!
//! The PHP tests mock `Connection::executeQuery()` and the `Result` it
//! returns. Here the builder runs on a real `Connection` over
//! `connection_support::TestDriver`, whose results give the rows the mocked
//! `Result` returns. A test that expects `executeQuery()` to get an SQL,
//! parameters and types checks what reaches the driver instead: the SQL
//! queried or prepared (named parameters made positional by the
//! connection) and each value bound with its binding type.
//!
//! Upstream: doctrine/dbal tests/Query/QueryBuilderTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{Script, TestDriver, array, i, list, none, s};
use doctrine_dbal::query::QueryBuilder;
use doctrine_dbal::{Connection, WrapperParameterType};
use php_value::vectors::phpunit_equals;
use php_value::{Key, PhpArray, Value};

/// A row of `fetchProvider()`.
struct Row {
    select: &'static str,
    from: &'static str,
    r#where: &'static str,
    parameters: PhpArray,
    types: Vec<(Key, WrapperParameterType)>,
    /// The calls the driver gets for `executeQuery($expectedSql, ...)`
    /// (`getSQL()` reads the platform first).
    calls: Vec<&'static str>,
}

fn string_type() -> WrapperParameterType {
    WrapperParameterType::from("string")
}

/// `fetchProvider()`.
fn fetch_provider() -> Vec<(&'static str, Row)> {
    vec![
        (
            "select *, no parameters",
            Row {
                select: "*",
                from: "user",
                r#where: "username LIKE \"jw*\"",
                parameters: none(),
                types: vec![],
                calls: vec![
                    "getDatabasePlatform",
                    "connect",
                    "query SELECT * FROM user WHERE username LIKE \"jw*\"",
                ],
            },
        ),
        (
            "select *, positional parameter",
            Row {
                select: "*",
                from: "user",
                r#where: "username = ?",
                parameters: list([s("jwage")]),
                types: vec![(Key::from("username"), string_type())],
                calls: vec![
                    "getDatabasePlatform",
                    "connect",
                    "prepare SELECT * FROM user WHERE username = ?",
                    "bind 1 'jwage' STRING",
                    "execute",
                ],
            },
        ),
        (
            "select multiple, named parameter",
            Row {
                select: "id, username, password",
                from: "user",
                r#where: "username = :username",
                parameters: array([("username", s("jwage"))]),
                types: vec![(Key::from("username"), string_type())],
                calls: vec![
                    "getDatabasePlatform",
                    "connect",
                    "prepare SELECT id, username, password FROM user WHERE username = ?",
                    "bind 1 'jwage' STRING",
                    "execute",
                ],
            },
        ),
        (
            "select multiple, named parameters",
            Row {
                select: "id, username",
                from: "user",
                r#where: "password = :password AND username != :username AND id != :id",
                parameters: array([
                    ("password", s("changeme")),
                    ("username", s("support")),
                    ("id", i(42)),
                ]),
                types: vec![
                    (Key::from("password"), string_type()),
                    (Key::from("username"), string_type()),
                    (Key::from("id"), WrapperParameterType::from("integer")),
                ],
                calls: vec![
                    "getDatabasePlatform",
                    "connect",
                    "prepare SELECT id, username FROM user WHERE password = ? AND username != ? AND id != ?",
                    "bind 1 'changeme' STRING",
                    "bind 2 'support' STRING",
                    "bind 3 42 INTEGER",
                    "execute",
                ],
            },
        ),
    ]
}

/// `$qb->select(...)->from(...)->where(...)->setParameters(...)`.
fn builder<'c>(conn: &'c Connection, row: &Row) -> QueryBuilder<'c, Connection> {
    let mut qb = QueryBuilder::new(conn);
    qb.select(&[row.select])
        .from(row.from, None)
        .r#where(row.r#where, [])
        .set_parameters(row.parameters.clone(), row.types.clone());
    qb
}

/// Runs `test` on each provider row with a driver whose results give
/// `rows`, and checks the calls the driver got.
async fn each_row<F>(rows: &[Vec<(&'static str, Value)>], test: F)
where
    F: AsyncFn(&QueryBuilder<'_, Connection>),
{
    for (name, row) in fetch_provider() {
        let driver = TestDriver::new(Script {
            rows: rows.to_vec(),
            ..Script::default()
        });
        let conn = driver.connection(none());
        let qb = builder(&conn, &row);
        test(&qb).await;
        assert_eq!(driver.calls.all(), row.calls, "{name}");
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the values are built at the call"
)]
fn assert_equals(expected: Value, actual: Value) {
    assert!(
        phpunit_equals(&expected, &actual),
        "expected {expected:?}, got {actual:?}"
    );
}

fn rows(rows: Vec<PhpArray>) -> Value {
    Value::Array(list(rows.into_iter().map(Value::Array)))
}

fn credentials() -> Vec<(&'static str, Value)> {
    vec![("username", s("jwage")), ("password", s("changeme"))]
}

#[tokio::test]
async fn testFetchAssociative() {
    each_row(&[credentials()], async |qb| {
        let row = qb.fetch_associative().await.unwrap();
        assert_equals(
            Value::Array(array([
                ("username", s("jwage")),
                ("password", s("changeme")),
            ])),
            Value::Array(row.unwrap()),
        );
    })
    .await;
}

#[tokio::test]
async fn testFetchNumeric() {
    each_row(&[credentials()], async |qb| {
        let row = qb.fetch_numeric().await.unwrap();
        assert_equals(
            Value::Array(list([s("jwage"), s("changeme")])),
            Value::Array(row.unwrap()),
        );
    })
    .await;
}

#[tokio::test]
async fn testFetchOne() {
    each_row(&[vec![("username", s("jwage"))]], async |qb| {
        let username = qb.fetch_one().await.unwrap();
        assert_equals(s("jwage"), username.unwrap());
    })
    .await;
}

fn two_users() -> Vec<Vec<(&'static str, Value)>> {
    vec![
        credentials(),
        vec![("username", s("support")), ("password", s("p@ssword"))],
    ]
}

#[tokio::test]
async fn testFetchAllAssociative() {
    each_row(&two_users(), async |qb| {
        let results = qb.fetch_all_associative().await.unwrap();
        assert_equals(
            rows(vec![
                array([("username", s("jwage")), ("password", s("changeme"))]),
                array([("username", s("support")), ("password", s("p@ssword"))]),
            ]),
            rows(results),
        );
    })
    .await;
}

#[tokio::test]
async fn testFetchAllNumeric() {
    each_row(&two_users(), async |qb| {
        let results = qb.fetch_all_numeric().await.unwrap();
        assert_equals(
            rows(vec![
                list([s("jwage"), s("changeme")]),
                list([s("support"), s("p@ssword")]),
            ]),
            rows(results),
        );
    })
    .await;
}

#[tokio::test]
async fn testFetchAllKeyValue() {
    let pairs = vec![
        vec![("username", s("jwage")), ("password", s("changeme"))],
        vec![("username", s("support")), ("password", s("p@ssw0rd"))],
    ];
    each_row(&pairs, async |qb| {
        let results = qb.fetch_all_key_value().await.unwrap();
        assert_equals(
            Value::Array(array([
                ("jwage", s("changeme")),
                ("support", s("p@ssw0rd")),
            ])),
            Value::Array(results),
        );
    })
    .await;
}

#[tokio::test]
async fn testFetchAllAssociativeIndexed() {
    let indexed = vec![vec![
        ("id", i(1)),
        ("username", s("jwage")),
        ("password", s("changeme")),
    ]];
    each_row(&indexed, async |qb| {
        let results = qb.fetch_all_associative_indexed().await.unwrap();
        let mut expected = PhpArray::new();
        expected.insert(
            1,
            Value::Array(array([
                ("username", s("jwage")),
                ("password", s("changeme")),
            ])),
        );
        assert_equals(Value::Array(expected), Value::Array(results));
    })
    .await;
}

#[tokio::test]
async fn testFetchFirstColumn() {
    let users = vec![
        vec![("username", s("jwage"))],
        vec![("username", s("support"))],
    ];
    each_row(&users, async |qb| {
        let results = qb.fetch_first_column().await.unwrap();
        assert_equals(
            Value::Array(list([s("jwage"), s("support")])),
            Value::Array(list(results)),
        );
    })
    .await;
}

/// `assertSame($mockedResult, $results)`: the result is the one the driver
/// gave (its rows, read once).
#[tokio::test]
async fn testExecuteQuery() {
    each_row(&[credentials()], async |qb| {
        let mut result = qb.execute_query().await.unwrap();
        assert_equals(
            rows(vec![array([
                ("username", s("jwage")),
                ("password", s("changeme")),
            ])]),
            rows(result.fetch_all_associative().unwrap()),
        );
        assert!(result.fetch_associative().unwrap().is_none());
    })
    .await;
}
