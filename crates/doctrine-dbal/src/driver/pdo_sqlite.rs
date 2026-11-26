//! The `SQLite` driver, on sqlx. It stands where Doctrine's
//! `Driver\PDO\SQLite\Driver` stands (with its `PDO\Connection`,
//! `PDO\Statement`, `PDO\Result` and `PDO\Exception`), and gives what
//! `pdo_sqlite` gives in PHP 8.4, as recorded from PHP
//! (`tests/data/connection.json`):
//!
//! - Values come back with `SQLite`'s storage class: INTEGER as int, REAL as
//!   float, TEXT and BLOB as strings, NULL as null.
//! - A bound value is converted as PDO does for its type: `NULL` binds
//!   null; `INTEGER` and `BOOLEAN` bind `(int) $value` (`BOOLEAN` makes an
//!   int 0 or 1 first); `STRING` and `ASCII` bind `(string) $value` as
//!   TEXT, `BINARY` and `LARGE_OBJECT` as a BLOB; null binds null for every
//!   type. An array is the string `Array`; an object without
//!   `__toString()` throws `Error`.
//! - Errors read `SQLSTATE[HY000]: General error: <code> <message>`
//!   (`SQLSTATE[23000]: Integrity constraint violation: 19 ...` for a
//!   constraint), with `SQLite`'s primary result code; opening fails with
//!   `SQLSTATE[HY000] [14] unable to open database file`. A parameter
//!   bound past the statement's parameters, or by an unknown name, fails
//!   at execution with `25 column index out of range`.
//! - `rowCount()` is 0 for a statement that returned rows and otherwise
//!   `SQLite`'s `changes()` (the last INSERT, UPDATE or DELETE on the
//!   connection, as `sqlite3_changes()` gives it); `exec()` gives
//!   `changes()` after the last statement.
//! - `lastInsertId()` is `SQLite`'s `last_insert_rowid()` as a string, and
//!   throws `NoIdentityValue` when it is `0`.
//! - `quote()` stops at the first NUL byte and doubles single quotes.
//! - The database is created when missing; foreign keys stay off and the
//!   busy timeout is 60 seconds, PDO's defaults.
//!
//! What PDO does that this driver does not (README.md, Differences): it
//! reads only the first statement of `prepare()` and `query()` (sqlx runs
//! them all); it binds a string that is not UTF-8 as TEXT (sqlx cannot;
//! such a value is refused); it reads `driverOptions` as PDO attributes
//! (they are ignored).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/PDO/`SQLite`/Driver.php src/Driver/PDO/Connection.php
//! Upstream: doctrine/dbal src/Driver/PDO/Statement.php src/Driver/PDO/Result.php
//! Upstream: doctrine/dbal src/Driver/PDO/Exception.php src/Driver/PDO/Exception/InvalidConfiguration.php

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use php_value::{Key, PhpArray, PhpString, Value};
use sqlx::sqlite::{SqliteArguments, SqliteConnectOptions, SqliteConnection, SqliteRow};
use sqlx::{
    Arguments, AssertSqlSafe, Column, Connection as _, Decode, Executor, Row, SqlSafeStr, Sqlite,
    Statement, TypeInfo, ValueRef,
};
use tokio::sync::Mutex;

use super::api::ExceptionConverter;
use super::{
    AbstractSQLiteDriver, BoxFuture, Connection as DriverConnection, Driver as DriverInterface,
    PlatformBox, Result as DriverResult, Statement as DriverStatement,
};
use crate::sql::{Parser, Visitor};
use crate::{Exception, ParameterType, ServerVersionProvider};

/// `Doctrine\DBAL\Driver\PDO\SQLite\Driver`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Driver;

impl Driver {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Driver\\PDO\\SQLite\\Driver";

    pub fn new() -> Self {
        Self
    }
}

impl AbstractSQLiteDriver for Driver {}

/// `InvalidConfiguration::notAStringOrNull($key, $value)`.
fn not_a_string_or_null(key: &str, value: &Value) -> Exception {
    Exception::driver(
        Exception::PDO_INVALID_CONFIGURATION,
        format!(
            "The {key} configuration parameter is expected to be either a string or null, got {}.",
            value.debug_type()
        ),
        None,
        0,
    )
}

/// `isset($params[$key])`.
fn isset<'a>(params: &'a PhpArray, key: &str) -> Option<&'a Value> {
    params.get(&Key::from(key)).filter(|value| !value.is_null())
}

/// The primary result code and message of a `SQLite` error.
fn sqlite_error(error: &sqlx::Error) -> Option<(i64, String)> {
    let sqlx::Error::Database(database) = error else {
        return None;
    };
    let code = database
        .code()
        .and_then(|code| code.parse::<i64>().ok())
        .map_or(1, |code| code & 0xff);
    Some((code, database.message().to_owned()))
}

/// The `SQLSTATE` and its description that `pdo_sqlite` gives a result code.
fn sql_state(code: i64) -> (&'static str, &'static str) {
    if code == 19 {
        ("23000", "Integrity constraint violation")
    } else {
        ("HY000", "General error")
    }
}

/// `PDO\Exception::new($pdoException)` for an error of a statement.
fn statement_error(code: i64, message: &str) -> Exception {
    let (state, description) = sql_state(code);
    Exception::driver(
        Exception::PDO_EXCEPTION,
        format!("SQLSTATE[{state}]: {description}: {code} {message}"),
        Some(state.to_owned()),
        code,
    )
}

/// `PDO\Exception::new($pdoException)` for what sqlx returned.
fn pdo_exception(error: &sqlx::Error) -> Exception {
    match sqlite_error(error) {
        Some((code, message)) => statement_error(code, &message),
        None => Exception::driver(Exception::PDO_EXCEPTION, error.to_string(), None, 0),
    }
}

/// `PDO\Exception::new($pdoException)` for an error of `new PDO()`.
fn connect_error(error: &sqlx::Error) -> Exception {
    match sqlite_error(error) {
        Some((code, message)) => {
            let (state, _) = sql_state(code);
            Exception::driver(
                Exception::PDO_EXCEPTION,
                format!("SQLSTATE[{state}] [{code}] {message}"),
                Some(state.to_owned()),
                code,
            )
        }
        None => Exception::driver(Exception::PDO_EXCEPTION, error.to_string(), None, 0),
    }
}

/// A PDO error raised outside `SQLite` (no `SQLSTATE`).
fn pdo_message(message: &str) -> Exception {
    Exception::driver(Exception::PDO_EXCEPTION, message, None, 0)
}

impl DriverInterface for Driver {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn connect<'a>(
        &'a self,
        params: &'a PhpArray,
    ) -> BoxFuture<'a, Result<Arc<dyn DriverConnection>, Exception>> {
        Box::pin(async move {
            for key in ["user", "password"] {
                if let Some(value) = isset(params, key)
                    && !matches!(value, Value::String(_))
                {
                    return Err(not_a_string_or_null(key, value));
                }
            }
            // `constructPdoDsn()`: the path, else `:memory:` when `memory`
            // is set (whatever its value), else SQLite's temporary
            // database.
            let filename: Vec<u8> = if let Some(path) = isset(params, "path") {
                path.to_php_string()
                    .ok_or_else(|| {
                        Exception::new(Exception::UNSUPPORTED, "a path that is not a string")
                    })?
                    .as_bytes()
                    .to_vec()
            } else if isset(params, "memory").is_some() {
                b":memory:".to_vec()
            } else {
                Vec::new()
            };
            let path = php_value::path::to_path(&filename).ok_or_else(|| {
                Exception::new(
                    Exception::UNSUPPORTED,
                    "a path that this system cannot name",
                )
            })?;
            let options = SqliteConnectOptions::new()
                .filename(path)
                .create_if_missing(true)
                .foreign_keys(false)
                .busy_timeout(Duration::from_mins(1));
            let connection = SqliteConnection::connect_with(&options)
                .await
                .map_err(|e| connect_error(&e))?;
            Ok(Arc::new(PdoConnection {
                inner: Arc::new(Mutex::new(Inner {
                    connection,
                    in_transaction: false,
                })),
            }) as Arc<dyn DriverConnection>)
        })
    }

    fn get_database_platform<'a>(
        &'a self,
        _version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        Box::pin(async move { Ok(self.sqlite_database_platform()) })
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        self.sqlite_exception_converter()
    }
}

struct Inner {
    connection: SqliteConnection,
    /// PDO's own transaction flag.
    in_transaction: bool,
}

/// `Doctrine\DBAL\Driver\PDO\Connection` on a `SQLite` database.
struct PdoConnection {
    inner: Arc<Mutex<Inner>>,
}

/// A SQL text rewritten for sqlx: `:name` placeholders become `?N`, `N`
/// being the index `SQLite` gives the name (its first occurrence).
#[derive(Default)]
struct Placeholders {
    sql: Vec<u8>,
    names: Vec<(Vec<u8>, usize)>,
    next_index: usize,
}

impl Visitor for Placeholders {
    type Error = std::convert::Infallible;

    fn accept_positional_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.next_index += 1;
        self.sql.extend_from_slice(sql);
        Ok(())
    }

    fn accept_named_parameter(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        let index = if let Some((_, index)) = self.names.iter().find(|(name, _)| name == sql) {
            *index
        } else {
            self.next_index += 1;
            self.names.push((sql.to_vec(), self.next_index));
            self.next_index
        };
        self.sql.extend_from_slice(format!("?{index}").as_bytes());
        Ok(())
    }

    fn accept_other(&mut self, sql: &[u8]) -> Result<(), Self::Error> {
        self.sql.extend_from_slice(sql);
        Ok(())
    }
}

/// What preparing a statement gives: the SQL sqlx runs, its named
/// parameters, its parameter count and its column names.
struct Prepared {
    sql: String,
    names: Vec<(Vec<u8>, usize)>,
    parameter_count: usize,
    columns: Vec<PhpString>,
}

async fn prepare(inner: &mut Inner, sql: &str) -> Result<Prepared, Exception> {
    let mut placeholders = Placeholders::default();
    let Ok(()) = Parser::new(false).parse(sql.as_bytes(), &mut placeholders);
    let (sql, names) = if placeholders.names.is_empty() {
        (sql.to_owned(), Vec::new())
    } else {
        // The rewrite only replaces ASCII placeholders in UTF-8 text.
        let rewritten = String::from_utf8(placeholders.sql)
            .map_err(|_| Exception::new(Exception::UNSUPPORTED, "SQL that is not UTF-8"))?;
        (rewritten, placeholders.names)
    };
    let statement = (&mut inner.connection)
        .prepare(AssertSqlSafe(sql.clone()).into_sql_str())
        .await
        .map_err(|e| pdo_exception(&e))?;
    let parameter_count = match statement.parameters() {
        Some(sqlx::Either::Right(count)) => count,
        Some(sqlx::Either::Left(types)) => types.len(),
        None => 0,
    };
    let columns = statement
        .columns()
        .iter()
        .map(|column| PhpString::from(column.name()))
        .collect();
    Ok(Prepared {
        sql,
        names,
        parameter_count,
        columns,
    })
}

/// A value read from `SQLite`, as `pdo_sqlite` gives it.
fn column_value(row: &SqliteRow, index: usize) -> Result<Value, Exception> {
    let decode_error = |e: sqlx::error::BoxDynError| {
        Exception::driver(Exception::PDO_EXCEPTION, e.to_string(), None, 0)
    };
    let raw = row.try_get_raw(index).map_err(|e| pdo_exception(&e))?;
    if raw.is_null() {
        return Ok(Value::Null);
    }
    let storage = raw.type_info().name().to_owned();
    Ok(match storage.as_str() {
        "INTEGER" => Value::Int(<i64 as Decode<Sqlite>>::decode(raw).map_err(decode_error)?),
        "REAL" => Value::Float(<f64 as Decode<Sqlite>>::decode(raw).map_err(decode_error)?),
        _ => Value::String(PhpString::from(
            <Vec<u8> as Decode<Sqlite>>::decode(raw).map_err(decode_error)?,
        )),
    })
}

/// Runs a statement and reads its rows into a result.
async fn run(
    inner: &mut Inner,
    sql: &str,
    arguments: SqliteArguments,
    columns: Vec<PhpString>,
) -> Result<Box<dyn DriverResult>, Exception> {
    let rows = sqlx::query_with(AssertSqlSafe(sql.to_owned()), arguments)
        .fetch_all(&mut inner.connection)
        .await
        .map_err(|e| pdo_exception(&e))?;
    let row_count = if rows.is_empty() {
        changes(inner).await?
    } else {
        0
    };
    let mut values = VecDeque::with_capacity(rows.len());
    for row in &rows {
        let mut row_values = Vec::with_capacity(columns.len());
        for index in 0..row.columns().len() {
            row_values.push(column_value(row, index)?);
        }
        values.push_back(row_values);
    }
    Ok(Box::new(PdoResult {
        columns,
        rows: values,
        row_count,
    }))
}

/// `SQLite`'s `changes()`.
async fn changes(inner: &mut Inner) -> Result<i64, Exception> {
    sqlx::query_scalar::<Sqlite, i64>("SELECT changes()")
        .fetch_one(&mut inner.connection)
        .await
        .map_err(|e| pdo_exception(&e))
}

/// Executes `BEGIN`, `COMMIT` or `ROLLBACK`.
async fn transaction_statement(inner: &mut Inner, sql: &'static str) -> Result<(), Exception> {
    (&mut inner.connection)
        .execute(sql)
        .await
        .map_err(|e| pdo_exception(&e))?;
    Ok(())
}

impl DriverConnection for PdoConnection {
    fn prepare<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverStatement>, Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            let prepared = prepare(&mut inner, sql).await?;
            Ok(Box::new(PdoStatement {
                inner: Arc::clone(&self.inner),
                prepared,
                bound: BTreeMap::new(),
                out_of_range: false,
            }) as Box<dyn DriverStatement>)
        })
    }

    fn query<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            let prepared = prepare(&mut inner, sql).await?;
            run(
                &mut inner,
                &prepared.sql,
                SqliteArguments::default(),
                prepared.columns,
            )
            .await
        })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        // sqlite3_mprintf("'%q'"): the text ends at the first NUL byte.
        let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
        let mut quoted = vec![b'\''];
        for &byte in &value[..end] {
            if byte == b'\'' {
                quoted.push(b'\'');
            }
            quoted.push(byte);
        }
        quoted.push(b'\'');
        PhpString::from(quoted)
    }

    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            (&mut inner.connection)
                .execute(AssertSqlSafe(sql.to_owned()))
                .await
                .map_err(|e| pdo_exception(&e))?;
            changes(&mut inner).await
        })
    }

    fn last_insert_id(&self) -> BoxFuture<'_, Result<Value, Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            let id = sqlx::query_scalar::<Sqlite, i64>("SELECT last_insert_rowid()")
                .fetch_one(&mut inner.connection)
                .await
                .map_err(|e| pdo_exception(&e))?;
            // pdo_sqlite returns '0' when there is none.
            if id == 0 {
                return Err(Exception::no_identity_value(None));
            }
            Ok(Value::String(PhpString::from(id.to_string())))
        })
    }

    fn begin_transaction(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            if inner.in_transaction {
                return Err(pdo_message("There is already an active transaction"));
            }
            transaction_statement(&mut inner, "BEGIN").await?;
            inner.in_transaction = true;
            Ok(())
        })
    }

    fn commit(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            if !inner.in_transaction {
                return Err(pdo_message("There is no active transaction"));
            }
            transaction_statement(&mut inner, "COMMIT").await?;
            inner.in_transaction = false;
            Ok(())
        })
    }

    fn roll_back(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            if !inner.in_transaction {
                return Err(pdo_message("There is no active transaction"));
            }
            inner.in_transaction = false;
            transaction_statement(&mut inner, "ROLLBACK").await
        })
    }

    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        Box::pin(async move {
            let mut inner = self.inner.lock().await;
            sqlx::query_scalar::<Sqlite, String>("SELECT sqlite_version()")
                .fetch_one(&mut inner.connection)
                .await
                .map_err(|e| pdo_exception(&e))
        })
    }
}

/// `Doctrine\DBAL\Driver\PDO\Statement`.
struct PdoStatement {
    inner: Arc<Mutex<Inner>>,
    prepared: Prepared,
    /// Bound values by `SQLite` parameter index.
    bound: BTreeMap<usize, (Value, ParameterType)>,
    /// A value was bound to a parameter the statement does not have.
    out_of_range: bool,
}

/// What PDO binds for a value of a parameter type.
enum Bound {
    Null,
    Int(i64),
    Text(String),
    Blob(Vec<u8>),
}

/// `(string) $value` as PDO converts a bound value.
fn bound_string(value: &Value) -> Result<Vec<u8>, Exception> {
    if let Value::Array(_) = value {
        // "Array to string conversion" (a warning).
        return Ok(b"Array".to_vec());
    }
    value
        .to_php_string_or_stringable()
        .map(|s| s.as_bytes().to_vec())
        .ok_or_else(|| {
            let class = value
                .as_object()
                .map_or_else(|| value.debug_type(), |o| o.class().to_owned());
            Exception::new(
                Exception::ERROR,
                format!("Object of class {class} could not be converted to string"),
            )
        })
}

fn bind_argument(value: &Value, r#type: ParameterType) -> Result<Bound, Exception> {
    if value.is_null() {
        return Ok(Bound::Null);
    }
    Ok(match r#type {
        ParameterType::Null => Bound::Null,
        ParameterType::Integer => Bound::Int(php_value::numeric::int_cast(value)),
        ParameterType::Boolean => match value {
            Value::Int(i) => Bound::Int(i64::from(*i != 0)),
            other => Bound::Int(php_value::numeric::int_cast(other)),
        },
        ParameterType::String | ParameterType::Ascii => {
            Bound::Text(String::from_utf8(bound_string(value)?).map_err(|_| {
                Exception::new(
                    Exception::UNSUPPORTED,
                    "a string that is not UTF-8 bound as TEXT (sqlx binds text as UTF-8)",
                )
            })?)
        }
        ParameterType::Binary | ParameterType::LargeObject => Bound::Blob(bound_string(value)?),
    })
}

fn add_argument(arguments: &mut SqliteArguments, bound: Bound) -> Result<(), Exception> {
    let added = match bound {
        Bound::Null => arguments.add(Option::<i64>::None),
        Bound::Int(i) => arguments.add(i),
        Bound::Text(text) => arguments.add(text),
        Bound::Blob(blob) => arguments.add(blob),
    };
    added.map_err(|e| Exception::driver(Exception::PDO_EXCEPTION, e.to_string(), None, 0))
}

impl DriverStatement for PdoStatement {
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        r#type: ParameterType,
    ) -> Result<(), Exception> {
        let index = match param {
            Key::Int(position) => {
                if *position < 1 {
                    return Err(Exception::new(
                        Exception::VALUE_ERROR,
                        "PDOStatement::bindValue(): Argument #1 ($param) must be greater than or equal to 1",
                    ));
                }
                usize::try_from(*position).ok()
            }
            Key::Str(name) => {
                let mut name = name.as_bytes().to_vec();
                if name.first() != Some(&b':') {
                    name.insert(0, b':');
                }
                self.prepared
                    .names
                    .iter()
                    .find(|(known, _)| *known == name)
                    .map(|(_, index)| *index)
            }
        };
        match index.filter(|index| *index <= self.prepared.parameter_count) {
            Some(index) => {
                self.bound.insert(index, (value, r#type));
            }
            None => self.out_of_range = true,
        }
        Ok(())
    }

    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move {
            if self.out_of_range {
                return Err(statement_error(25, "column index out of range"));
            }
            let mut arguments = SqliteArguments::default();
            for index in 1..=self.prepared.parameter_count {
                let bound = match self.bound.get(&index) {
                    Some((value, r#type)) => bind_argument(value, *r#type)?,
                    None => Bound::Null,
                };
                add_argument(&mut arguments, bound)?;
            }
            let mut inner = self.inner.lock().await;
            run(
                &mut inner,
                &self.prepared.sql,
                arguments,
                self.prepared.columns.clone(),
            )
            .await
        })
    }
}

/// `Doctrine\DBAL\Driver\PDO\Result`: the rows, read when the statement
/// executed.
struct PdoResult {
    columns: Vec<PhpString>,
    rows: VecDeque<Vec<Value>>,
    row_count: i64,
}

impl DriverResult for PdoResult {
    fn fetch_numeric(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(self.rows.pop_front().map(PhpArray::list))
    }

    fn fetch_associative(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(self.rows.pop_front().map(|row| {
            let mut array = PhpArray::new();
            for (name, value) in self.columns.iter().zip(row) {
                array.insert(Key::from(name.clone()), value);
            }
            array
        }))
    }

    fn row_count(&self) -> Result<i64, Exception> {
        Ok(self.row_count)
    }

    fn column_count(&self) -> Result<usize, Exception> {
        Ok(self.columns.len())
    }

    fn free(&mut self) {
        self.rows.clear();
    }

    fn get_column_name(&self, index: i64) -> Option<Result<PhpString, Exception>> {
        if index < 0 {
            let value_error = Exception::new(
                Exception::VALUE_ERROR,
                "PDOStatement::getColumnMeta(): Argument #1 ($column) must be greater than or equal to 0",
            );
            return Some(Err(Exception::invalid_column_index(
                index,
                Some(value_error),
            )));
        }
        Some(
            usize::try_from(index)
                .ok()
                .and_then(|index| self.columns.get(index))
                .cloned()
                .ok_or_else(|| statement_error(25, "column index out of range")),
        )
    }

    fn class(&self) -> &'static str {
        "Doctrine\\DBAL\\Driver\\PDO\\Result"
    }
}
