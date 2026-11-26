//! Port of tests/Functional/DataAccessTest.php on `pdo_sqlite` in memory.
//! The table is created from the DDL the schema manager creates for it,
//! and the expressions are the SQL `SQLitePlatform` builds for each test
//! and provider row (both recorded, tests/data/connection.json): the
//! platform itself is ported separately. `modeProvider()`'s rows are
//! closures: the modes are written out here, with upstream's names.
//!
//! Upstream: doctrine/dbal tests/Functional/DataAccessTest.php
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]
#![allow(non_snake_case)]

mod connection_support;

use connection_support::{
    array, data, date_time, drop_and_create_table, i, list, no_types, none, positional_types, s,
    sql, sqlite,
};
use doctrine_dbal::{ArrayParameterType, Connection, ParameterType, WrapperParameterType};
use php_value::vectors::{decode, phpunit_equals};
use php_value::{Key, PhpArray, Value};

async fn set_up() -> Connection {
    let connection = sqlite();
    drop_and_create_table(&connection, "fetch_table").await;
    connection
        .insert(
            "fetch_table",
            &array([
                ("test_int", i(1)),
                ("test_string", s("foo")),
                ("test_datetime", s("2010-01-01 10:10:10")),
            ]),
            &no_types(),
        )
        .await
        .unwrap();
    connection
}

/// `array_change_key_case($row, CASE_LOWER)`.
fn lower(row: &PhpArray) -> PhpArray {
    let mut lower = PhpArray::new();
    for (key, value) in row.iter() {
        let key = match key {
            Key::Str(name) => Key::from_bytes(&name.as_bytes().to_ascii_lowercase()),
            Key::Int(_) => key.clone(),
        };
        lower.insert(key, value.clone());
    }
    lower
}

fn get(row: &PhpArray, key: impl Into<Key>) -> Value {
    row.get(&key.into()).cloned().unwrap_or(Value::Null)
}

fn assert_equals(expected: &Value, actual: &Value) {
    assert!(
        phpunit_equals(expected, actual),
        "expected {expected:?}, got {actual:?}"
    );
}

fn starts_with(value: &Value, prefix: &str) -> bool {
    value
        .as_string()
        .is_some_and(|text| text.as_bytes().starts_with(prefix.as_bytes()))
}

const SELECT: &str =
    "SELECT test_int, test_string FROM fetch_table WHERE test_int = ? AND test_string = ?";
const SELECT_DATETIME: &str =
    "SELECT test_int, test_datetime FROM fetch_table WHERE test_int = ? AND test_datetime = ?";

fn datetime_types() -> doctrine_dbal::ParameterTypes {
    positional_types([
        ParameterType::String.into(),
        WrapperParameterType::from("datetime"),
    ])
}

#[tokio::test]
async fn testPrepareWithBindValue() {
    let connection = set_up().await;
    let mut stmt = connection.prepare(SELECT).await.unwrap();
    stmt.bind_string(1, i(1)).unwrap();
    stmt.bind_string(2, s("foo")).unwrap();
    let row = stmt
        .execute_query()
        .await
        .unwrap()
        .fetch_associative()
        .unwrap();
    let row = lower(&row.unwrap());
    assert_equals(
        &Value::Array(array([("test_int", i(1)), ("test_string", s("foo"))])),
        &Value::Array(row),
    );
}

#[tokio::test]
async fn testPrepareWithFetchAllAssociative() {
    let connection = set_up().await;
    let mut stmt = connection.prepare(SELECT).await.unwrap();
    stmt.bind_string(1, i(1)).unwrap();
    stmt.bind_string(2, s("foo")).unwrap();
    let rows = stmt
        .execute_query()
        .await
        .unwrap()
        .fetch_all_associative()
        .unwrap();
    assert_equals(
        &Value::Array(array([("test_int", i(1)), ("test_string", s("foo"))])),
        &Value::Array(lower(&rows[0])),
    );
}

#[tokio::test]
async fn testPrepareWithFetchOne() {
    let connection = set_up().await;
    let mut stmt = connection
        .prepare("SELECT test_int FROM fetch_table WHERE test_int = ? AND test_string = ?")
        .await
        .unwrap();
    stmt.bind_string(1, i(1)).unwrap();
    stmt.bind_string(2, s("foo")).unwrap();
    let column = stmt.execute_query().await.unwrap().fetch_one().unwrap();
    assert_equals(&i(1), &column.unwrap());
}

#[tokio::test]
async fn testFetchAllAssociative() {
    let connection = set_up().await;
    let data = connection
        .fetch_all_associative(SELECT, &list([i(1), s("foo")]), &no_types())
        .await
        .unwrap();
    assert_eq!(data.len(), 1);
    let row = &data[0];
    assert_eq!(row.len(), 2);
    let row = lower(row);
    assert_equals(&i(1), &get(&row, "test_int"));
    assert_equals(&s("foo"), &get(&row, "test_string"));
}

#[tokio::test]
async fn testFetchAllWithTypes() {
    let connection = set_up().await;
    let datetime_string = "2010-01-01 10:10:10";
    let datetime = date_time(datetime_string);
    let data = connection
        .fetch_all_associative(SELECT_DATETIME, &list([i(1), datetime]), &datetime_types())
        .await
        .unwrap();
    assert_eq!(data.len(), 1);
    let row = &data[0];
    assert_eq!(row.len(), 2);
    let row = lower(row);
    assert_equals(&i(1), &get(&row, "test_int"));
    assert!(starts_with(&get(&row, "test_datetime"), datetime_string));
}

#[tokio::test]
async fn testFetchAssociative() {
    let connection = set_up().await;
    let row = connection
        .fetch_associative(SELECT, &list([i(1), s("foo")]), &no_types())
        .await
        .unwrap()
        .unwrap();
    let row = lower(&row);
    assert_equals(&i(1), &get(&row, "test_int"));
    assert_equals(&s("foo"), &get(&row, "test_string"));
}

#[tokio::test]
async fn testFetchAssocWithTypes() {
    let connection = set_up().await;
    let datetime_string = "2010-01-01 10:10:10";
    let row = connection
        .fetch_associative(
            SELECT_DATETIME,
            &list([i(1), date_time(datetime_string)]),
            &datetime_types(),
        )
        .await
        .unwrap()
        .unwrap();
    let row = lower(&row);
    assert_equals(&i(1), &get(&row, "test_int"));
    assert!(starts_with(&get(&row, "test_datetime"), datetime_string));
}

#[tokio::test]
async fn testFetchArray() {
    let connection = set_up().await;
    let row = connection
        .fetch_numeric(SELECT, &list([i(1), s("foo")]), &no_types())
        .await
        .unwrap()
        .unwrap();
    assert_equals(&i(1), &get(&row, 0));
    assert_equals(&s("foo"), &get(&row, 1));
}

#[tokio::test]
async fn testFetchArrayWithTypes() {
    let connection = set_up().await;
    let datetime_string = "2010-01-01 10:10:10";
    let row = connection
        .fetch_numeric(
            SELECT_DATETIME,
            &list([i(1), date_time(datetime_string)]),
            &datetime_types(),
        )
        .await
        .unwrap()
        .unwrap();
    let row = lower(&row);
    assert_equals(&i(1), &get(&row, 0));
    assert!(starts_with(&get(&row, 1), datetime_string));
}

#[tokio::test]
async fn testFetchColumn() {
    let connection = set_up().await;
    let test_int = connection
        .fetch_one(
            "SELECT test_int FROM fetch_table WHERE test_int = ? AND test_string = ?",
            &list([i(1), s("foo")]),
            &no_types(),
        )
        .await
        .unwrap();
    assert_equals(&i(1), &test_int.unwrap());
    let test_string = connection
        .fetch_one(
            "SELECT test_string FROM fetch_table WHERE test_int = ? AND test_string = ?",
            &list([i(1), s("foo")]),
            &no_types(),
        )
        .await
        .unwrap();
    assert_equals(&s("foo"), &test_string.unwrap());
}

#[tokio::test]
async fn testFetchOneWithTypes() {
    let connection = set_up().await;
    let datetime_string = "2010-01-01 10:10:10";
    let column = connection
        .fetch_one(
            "SELECT test_datetime FROM fetch_table WHERE test_int = ? AND test_datetime = ?",
            &list([i(1), date_time(datetime_string)]),
            &datetime_types(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(column, Value::String(_)));
    assert!(starts_with(&column, datetime_string));
}

#[tokio::test]
async fn testExecuteQueryBindDateTimeType() {
    let connection = set_up().await;
    let value = connection
        .fetch_one(
            "SELECT count(*) AS c FROM fetch_table WHERE test_datetime = ?",
            &list([date_time("2010-01-01 10:10:10")]),
            &positional_types([WrapperParameterType::from("datetime")]),
        )
        .await
        .unwrap();
    assert_equals(&i(1), &value.unwrap());
}

#[tokio::test]
async fn testExecuteStatementBindDateTimeType() {
    let connection = set_up().await;
    let datetime = date_time("2010-02-02 20:20:20");
    let sql = "INSERT INTO fetch_table (test_int, test_string, test_datetime) VALUES (?, ?, ?)";
    let affected_rows = connection
        .execute_statement(
            sql,
            &list([i(50), s("foo"), datetime.clone()]),
            &positional_types([
                ParameterType::Integer.into(),
                ParameterType::String.into(),
                WrapperParameterType::from("datetime"),
            ]),
        )
        .await
        .unwrap();
    assert_eq!(affected_rows, 1);
    let count = connection
        .execute_query(
            "SELECT count(*) AS c FROM fetch_table WHERE test_datetime = ?",
            &list([datetime]),
            &positional_types([WrapperParameterType::from("datetime")]),
        )
        .await
        .unwrap()
        .fetch_one()
        .unwrap();
    assert_equals(&i(1), &count.unwrap());
}

#[tokio::test]
async fn testPrepareQueryBindValueDateTimeType() {
    let connection = set_up().await;
    let mut stmt = connection
        .prepare("SELECT count(*) AS c FROM fetch_table WHERE test_datetime = ?")
        .await
        .unwrap();
    stmt.bind_value(1, date_time("2010-01-01 10:10:10"), "datetime")
        .unwrap();
    let result = stmt.execute_query().await.unwrap().fetch_one().unwrap();
    assert_equals(&i(1), &result.unwrap());
}

#[tokio::test]
async fn testNativeArrayListSupport() {
    let connection = set_up().await;
    for n in 100..110 {
        connection
            .insert(
                "fetch_table",
                &array([
                    ("test_int", i(n)),
                    ("test_string", s(&format!("foo{n}"))),
                    ("test_datetime", s("2010-01-01 10:10:10")),
                ]),
                &no_types(),
            )
            .await
            .unwrap();
    }
    let expected = Value::Array(list(
        [100, 101, 102, 103, 104].map(|n| Value::Array(list([i(n)]))),
    ));
    let data = connection
        .execute_query(
            "SELECT test_int FROM fetch_table WHERE test_int IN (?)",
            &list([Value::Array(list([100, 101, 102, 103, 104].map(i)))]),
            &positional_types([ArrayParameterType::Integer.into()]),
        )
        .await
        .unwrap()
        .fetch_all_numeric()
        .unwrap();
    assert_eq!(data.len(), 5);
    assert_equals(
        &expected,
        &Value::Array(list(data.into_iter().map(Value::Array))),
    );
    let data = connection
        .execute_query(
            "SELECT test_int FROM fetch_table WHERE test_string IN (?)",
            &list([Value::Array(list(
                ["foo100", "foo101", "foo102", "foo103", "foo104"].map(s),
            ))]),
            &positional_types([ArrayParameterType::String.into()]),
        )
        .await
        .unwrap()
        .fetch_all_numeric()
        .unwrap();
    assert_eq!(data.len(), 5);
    assert_equals(
        &expected,
        &Value::Array(list(data.into_iter().map(Value::Array))),
    );
}

fn provider(method: &str) -> &'static Vec<serde_json::Value> {
    data()["providers"]["Functional/DataAccessTest"][method]["rows"]
        .as_array()
        .unwrap()
}

#[tokio::test]
async fn testTrimExpression() {
    let connection = set_up().await;
    let rows = provider("testTrimExpression");
    assert!(!rows.is_empty());
    for (index, row) in rows.iter().enumerate() {
        let expected = decode(&row[1][3]).unwrap();
        let sql = sql(&format!("trim_{index}"));
        let row = connection
            .fetch_associative(&sql, &none(), &no_types())
            .await
            .unwrap()
            .unwrap();
        let row = lower(&row);
        assert!(
            phpunit_equals(&expected, &get(&row, "trimmed")),
            "{sql}: expected {expected:?}, got {row:?}"
        );
    }
}

/// `modeProvider()`: the interval bound, written as a literal, or as an
/// expression.
const MODES: [&str; 3] = ["bind", "literal", "expression"];

/// `assertDateExpression()`.
async fn assert_date_expression(test: &str, interval: i64, expected: &str) {
    for mode in MODES {
        let connection = set_up().await;
        let query = sql(&format!("{test}_{mode}"));
        let mut stmt = connection.prepare(&query).await.unwrap();
        if mode == "bind" {
            stmt.bind_value(1, i(interval), ParameterType::Integer)
                .unwrap();
        }
        let date = stmt.execute_query().await.unwrap().fetch_one().unwrap();
        let Some(Value::String(date)) = date else {
            panic!("{test} {mode}: {date:?}");
        };
        // date('Y-m-d H:i:s', strtotime($date))
        let parsed = php_value::date_create::date_create(date.as_bytes(), None)
            .unwrap()
            .unwrap();
        assert_eq!(parsed.format("Y-m-d H:i:s"), expected, "{test} {mode}");
    }
}

#[tokio::test]
async fn testDateAddSeconds() {
    assert_date_expression("testDateAddSeconds", 1, "2010-01-01 10:10:11").await;
}

#[tokio::test]
async fn testDateSubSeconds() {
    assert_date_expression("testDateSubSeconds", 1, "2010-01-01 10:10:09").await;
}

#[tokio::test]
async fn testDateAddMinutes() {
    assert_date_expression("testDateAddMinutes", 5, "2010-01-01 10:15:10").await;
}

#[tokio::test]
async fn testDateSubMinutes() {
    assert_date_expression("testDateSubMinutes", 5, "2010-01-01 10:05:10").await;
}

#[tokio::test]
async fn testDateAddHours() {
    assert_date_expression("testDateAddHours", 3, "2010-01-01 13:10:10").await;
}

#[tokio::test]
async fn testDateSubHours() {
    assert_date_expression("testDateSubHours", 3, "2010-01-01 07:10:10").await;
}

#[tokio::test]
async fn testDateAddDays() {
    assert_date_expression("testDateAddDays", 10, "2010-01-11 10:10:10").await;
}

#[tokio::test]
async fn testDateSubDays() {
    assert_date_expression("testDateSubDays", 10, "2009-12-22 10:10:10").await;
}

#[tokio::test]
async fn testDateAddWeeks() {
    assert_date_expression("testDateAddWeeks", 1, "2010-01-08 10:10:10").await;
}

#[tokio::test]
async fn testDateSubWeeks() {
    assert_date_expression("testDateSubWeeks", 1, "2009-12-25 10:10:10").await;
}

#[tokio::test]
async fn testDateAddMonths() {
    assert_date_expression("testDateAddMonths", 2, "2010-03-01 10:10:10").await;
}

#[tokio::test]
async fn testDateSubMonths() {
    assert_date_expression("testDateSubMonths", 2, "2009-11-01 10:10:10").await;
}

#[tokio::test]
async fn testDateAddQuarters() {
    assert_date_expression("testDateAddQuarters", 3, "2010-10-01 10:10:10").await;
}

#[tokio::test]
async fn testDateSubQuarters() {
    assert_date_expression("testDateSubQuarters", 3, "2009-04-01 10:10:10").await;
}

#[tokio::test]
async fn testDateAddYears() {
    assert_date_expression("testDateAddYears", 6, "2016-01-01 10:10:10").await;
}

#[tokio::test]
async fn testDateSubYears() {
    assert_date_expression("testDateSubYears", 6, "2004-01-01 10:10:10").await;
}

#[tokio::test]
async fn testSqliteDateArithmeticWithDynamicInterval() {
    let connection = set_up().await;
    drop_and_create_table(&connection, "fetch_table_date_math").await;
    for (date, days) in [("2010-01-01", 10), ("2010-06-01", 20)] {
        connection
            .insert(
                "fetch_table_date_math",
                &array([("test_date", s(date)), ("test_days", i(days))]),
                &no_types(),
            )
            .await
            .unwrap();
    }
    let row_count = connection
        .fetch_one(&sql("sqlite_date_math"), &none(), &no_types())
        .await
        .unwrap();
    assert_equals(&i(1), &row_count.unwrap());
}

#[tokio::test]
async fn testLocateExpression() {
    let connection = set_up().await;
    let row = connection
        .fetch_associative(&sql("locate"), &none(), &no_types())
        .await
        .unwrap()
        .unwrap();
    let row = lower(&row);
    let expected = array(
        [2, 1, 0, 1, 1, 4, 0, 2, 0, 1, 2]
            .into_iter()
            .enumerate()
            .map(|(n, value)| (format!("locate{}", n + 1), i(value))),
    );
    assert_equals(&Value::Array(expected), &Value::Array(row));
}

#[tokio::test]
async fn testSubstringExpression() {
    let connection = set_up().await;
    let rows = provider("testSubstringExpression");
    assert!(!rows.is_empty());
    for row in rows {
        let name = row[0].as_str().unwrap();
        let expected = decode(&row[1][3]).unwrap();
        let value = connection
            .fetch_one(&sql(&format!("substring_{name}")), &none(), &no_types())
            .await
            .unwrap();
        assert!(
            phpunit_equals(&expected, &value.clone().unwrap()),
            "{name}: {value:?}"
        );
    }
}

#[tokio::test]
async fn testQuoteSQLInjection() {
    let connection = set_up().await;
    let quoted = connection.quote(b"bar' OR '1'='1").await.unwrap();
    let sql = format!(
        "SELECT * FROM fetch_table WHERE test_string = {}",
        quoted.to_string_lossy()
    );
    let rows = connection
        .fetch_all_associative(&sql, &none(), &no_types())
        .await
        .unwrap();
    assert_eq!(
        rows.len(),
        0,
        "no result should be returned, otherwise SQL injection is possible"
    );
}
