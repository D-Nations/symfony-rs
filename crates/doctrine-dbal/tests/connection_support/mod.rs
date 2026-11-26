//! The test doubles of the connection tests: a driver that records the
//! calls it gets (PHPUnit's `createStub(Driver::class)` and mocks), and the
//! `SQLite` connection helpers of the functional tests.
//!
//! Upstream: none
#![allow(dead_code)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test helpers; a panic is a test failure"
)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use doctrine_dbal::driver::api::ExceptionConverter;
use doctrine_dbal::driver::{
    BoxFuture, Connection as DriverConnection, Driver, PlatformBox, Result as DriverResult,
    Statement as DriverStatement,
};
use doctrine_dbal::platforms::AbstractPlatform;
use doctrine_dbal::{
    Connection, DriverManager, Exception, ParameterType, ParameterTypes, Query,
    ServerVersionProvider, TransactionIsolationLevel, WrapperParameterType,
};
use php_value::{Key, PhpArray, PhpString, Value};

/// The calls a [`TestDriver`] got, in order.
#[derive(Debug, Default)]
pub struct Calls(Mutex<Vec<String>>);

impl Calls {
    pub fn push(&self, call: impl Into<String>) {
        self.0.lock().unwrap().push(call.into());
    }

    pub fn all(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }

    pub fn count(&self, call: &str) -> usize {
        self.all().iter().filter(|c| c.as_str() == call).count()
    }
}

/// What the doubles answer.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// `supportsSavepoints()` of the platform (a stub's `false` by
    /// default).
    pub savepoints: bool,
    /// The rows (column name, value) every query gives.
    pub rows: Vec<Vec<(&'static str, Value)>>,
    /// `getServerVersion()` of the driver connection.
    pub server_version: String,
    /// The driver's `getDatabasePlatform()` asks the version provider.
    pub platform_reads_version: bool,
    /// The `beginTransaction()` calls (1-based) that throw a
    /// `RuntimeException`.
    pub begin_failures: Vec<usize>,
    /// What `commit()` and `rollBack()` of the driver connection throw.
    pub commit_error: Option<Exception>,
    pub rollback_error: Option<Exception>,
}

/// A driver whose connection, statements and results record their calls.
pub struct TestDriver {
    pub calls: Arc<Calls>,
    script: Script,
    begins: Arc<AtomicUsize>,
}

impl TestDriver {
    pub fn new(script: Script) -> Arc<Self> {
        Arc::new(Self {
            calls: Arc::new(Calls::default()),
            script,
            begins: Arc::new(AtomicUsize::new(0)),
        })
    }

    /// `new Connection($params, $driver)` over this driver.
    pub fn connection(self: &Arc<Self>, params: PhpArray) -> Connection {
        Connection::new(params, self.clone(), None)
    }
}

/// The platform of the doubles: `createStub(AbstractPlatform::class)`, its
/// strings empty, with `supportsSavepoints()` as configured.
#[derive(Debug)]
pub struct TestPlatform {
    savepoints: bool,
}

macro_rules! declarations {
    ($($name:ident),*) => {
        $(
            fn $name(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
                Ok(PhpString::from(""))
            }
        )*
    };
}

impl AbstractPlatform for TestPlatform {
    fn class(&self) -> &'static str {
        "Mock_AbstractPlatform"
    }

    declarations!(
        get_boolean_type_declaration_sql,
        get_integer_type_declaration_sql,
        get_big_int_type_declaration_sql,
        get_small_int_type_declaration_sql,
        get_clob_type_declaration_sql,
        get_blob_type_declaration_sql,
        get_date_time_type_declaration_sql,
        get_date_type_declaration_sql,
        get_time_type_declaration_sql
    );

    fn get_current_database_expression(&self) -> String {
        String::new()
    }

    fn get_set_transaction_isolation_sql(&self, _level: TransactionIsolationLevel) -> String {
        String::new()
    }

    fn supports_savepoints(&self) -> bool {
        self.savepoints
    }

    fn create_save_point(&self, _savepoint: &str) -> String {
        String::new()
    }

    fn release_save_point(&self, _savepoint: &str) -> String {
        String::new()
    }

    fn rollback_save_point(&self, _savepoint: &str) -> String {
        String::new()
    }
}

/// Wraps every driver exception in a `DriverException`.
struct TestConverter;

impl ExceptionConverter for TestConverter {
    fn convert(&self, exception: Exception, query: Option<Query>) -> Exception {
        Exception::driver_exception(Exception::DRIVER_EXCEPTION, exception, query)
    }
}

impl Driver for TestDriver {
    fn class(&self) -> &'static str {
        "Mock_Driver"
    }

    fn connect<'a>(
        &'a self,
        _params: &'a PhpArray,
    ) -> BoxFuture<'a, Result<Arc<dyn DriverConnection>, Exception>> {
        Box::pin(async move {
            self.calls.push("connect");
            let connection: Arc<dyn DriverConnection> = Arc::new(TestConnection {
                calls: self.calls.clone(),
                script: self.script.clone(),
                begins: self.begins.clone(),
            });
            Ok(connection)
        })
    }

    fn get_database_platform<'a>(
        &'a self,
        version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, Result<PlatformBox, Exception>> {
        Box::pin(async move {
            if self.script.platform_reads_version {
                let version = version_provider.get_server_version().await?;
                self.calls.push(format!("getDatabasePlatform {version}"));
            } else {
                self.calls.push("getDatabasePlatform");
            }
            let platform: PlatformBox = Box::new(TestPlatform {
                savepoints: self.script.savepoints,
            });
            Ok(platform)
        })
    }

    fn get_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        Arc::new(TestConverter)
    }
}

struct TestConnection {
    calls: Arc<Calls>,
    script: Script,
    begins: Arc<AtomicUsize>,
}

impl TestConnection {
    fn result(&self) -> Box<dyn DriverResult> {
        Box::new(TestResult {
            rows: self.script.rows.iter().cloned().collect(),
            columns: self.script.rows.first().map_or(0, Vec::len),
        })
    }
}

impl DriverConnection for TestConnection {
    fn prepare<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverStatement>, Exception>> {
        Box::pin(async move {
            self.calls.push(format!("prepare {sql}"));
            let statement: Box<dyn DriverStatement> = Box::new(TestStatement {
                calls: self.calls.clone(),
                result: Some(self.result()),
            });
            Ok(statement)
        })
    }

    fn query<'a>(
        &'a self,
        sql: &'a str,
    ) -> BoxFuture<'a, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move {
            self.calls.push(format!("query {sql}"));
            Ok(self.result())
        })
    }

    fn quote(&self, value: &[u8]) -> PhpString {
        let mut quoted = b"'".to_vec();
        quoted.extend_from_slice(value);
        quoted.push(b'\'');
        PhpString::from(quoted)
    }

    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>> {
        Box::pin(async move {
            self.calls.push(format!("exec {sql}"));
            Ok(0)
        })
    }

    fn last_insert_id(&self) -> BoxFuture<'_, Result<Value, Exception>> {
        Box::pin(async move { Ok(Value::string("0")) })
    }

    fn begin_transaction(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            let call = self.begins.fetch_add(1, Ordering::SeqCst) + 1;
            self.calls.push("beginTransaction");
            if self.script.begin_failures.contains(&call) {
                return Err(Exception::new("RuntimeException", ""));
            }
            Ok(())
        })
    }

    fn commit(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            self.calls.push("commit");
            self.script.commit_error.clone().map_or(Ok(()), Err)
        })
    }

    fn roll_back(&self) -> BoxFuture<'_, Result<(), Exception>> {
        Box::pin(async move {
            self.calls.push("rollBack");
            self.script.rollback_error.clone().map_or(Ok(()), Err)
        })
    }

    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        Box::pin(async move {
            self.calls.push("getServerVersion");
            Ok(self.script.server_version.clone())
        })
    }
}

struct TestStatement {
    calls: Arc<Calls>,
    result: Option<Box<dyn DriverResult>>,
}

/// A value as the calls show it.
pub fn show(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => format!("{f:?}"),
        Value::String(s) => format!("'{}'", s.to_string_lossy()),
        other => format!("{other:?}"),
    }
}

impl DriverStatement for TestStatement {
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        r#type: ParameterType,
    ) -> Result<(), Exception> {
        let param = match param {
            Key::Int(i) => i.to_string(),
            Key::Str(s) => s.to_string_lossy().into_owned(),
        };
        self.calls
            .push(format!("bind {param} {} {}", show(&value), r#type.name()));
        Ok(())
    }

    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>> {
        Box::pin(async move {
            self.calls.push("execute");
            self.result
                .take()
                .ok_or_else(|| Exception::new(Exception::UNSUPPORTED, "executed twice"))
        })
    }
}

struct TestResult {
    rows: VecDeque<Vec<(&'static str, Value)>>,
    columns: usize,
}

impl DriverResult for TestResult {
    fn fetch_numeric(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(self
            .rows
            .pop_front()
            .map(|row| PhpArray::list(row.into_iter().map(|(_, value)| value))))
    }

    fn fetch_associative(&mut self) -> Result<Option<PhpArray>, Exception> {
        Ok(self.rows.pop_front().map(|row| {
            let mut array = PhpArray::new();
            for (name, value) in row {
                array.insert(name, value);
            }
            array
        }))
    }

    fn row_count(&self) -> Result<i64, Exception> {
        Ok(0)
    }

    fn column_count(&self) -> Result<usize, Exception> {
        Ok(self.columns)
    }

    fn free(&mut self) {
        self.rows.clear();
    }

    fn class(&self) -> &'static str {
        "Mock_Result"
    }
}

/// An array from pairs.
pub fn array<K: Into<Key>>(pairs: impl IntoIterator<Item = (K, Value)>) -> PhpArray {
    let mut array = PhpArray::new();
    for (key, value) in pairs {
        array.insert(key, value);
    }
    array
}

/// A list.
pub fn list(values: impl IntoIterator<Item = Value>) -> PhpArray {
    PhpArray::list(values)
}

/// No parameters.
pub fn none() -> PhpArray {
    PhpArray::new()
}

/// No types.
pub fn no_types() -> ParameterTypes {
    ParameterTypes::new()
}

/// Types by position.
pub fn positional_types(types: impl IntoIterator<Item = WrapperParameterType>) -> ParameterTypes {
    types
        .into_iter()
        .zip(0_i64..)
        .map(|(type_, index)| (Key::Int(index), type_))
        .collect()
}

/// Types by name.
pub fn named_types<'a>(
    types: impl IntoIterator<Item = (&'a str, WrapperParameterType)>,
) -> ParameterTypes {
    types
        .into_iter()
        .map(|(name, type_)| (Key::from(name), type_))
        .collect()
}

pub fn s(text: &str) -> Value {
    Value::string(text)
}

pub fn i(value: i64) -> Value {
    Value::Int(value)
}

/// `DriverManager::getConnection(['driver' => 'pdo_sqlite', 'memory' =>
/// true])`, the functional tests' connection.
pub fn sqlite() -> Connection {
    DriverManager::get_connection(
        array([("driver", s("pdo_sqlite")), ("memory", Value::Bool(true))]),
        None,
    )
    .unwrap()
}

/// A current-thread runtime for one test.
pub fn run<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// tests/data/connection.json (`scripts/record.sh doctrine-dbal-connection`).
pub fn data() -> &'static serde_json::Value {
    static DATA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
    DATA.get_or_init(|| {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/connection.json"
        ))
        .expect("tests/data/connection.json (scripts/record.sh doctrine-dbal-connection)");
        serde_json::from_str(&text).expect("valid JSON")
    })
}

/// A recorded base64 string as text.
pub fn text(json: &serde_json::Value) -> String {
    let bytes = php_value::vectors::base64(json.as_str().expect("base64")).expect("base64");
    String::from_utf8(bytes).expect("UTF-8")
}

/// The SQL that `SQLitePlatform` builds for `name` (recorded).
pub fn sql(name: &str) -> String {
    text(&data()["sql"][name])
}

/// The DDL of a functional test's table, as the schema manager creates it
/// on `pdo_sqlite` (recorded); `sqlite_sequence` is `SQLite`'s own.
pub fn ddl(table: &str) -> Vec<String> {
    data()["ddl"][table]
        .as_array()
        .unwrap_or_else(|| panic!("no DDL for {table}"))
        .iter()
        .map(text)
        .filter(|sql| !sql.starts_with("CREATE TABLE sqlite_sequence"))
        .collect()
}

/// `dropAndCreateTable()`: the table from its recorded DDL.
pub async fn drop_and_create_table(connection: &Connection, table: &str) {
    connection
        .execute_statement(
            &format!("DROP TABLE IF EXISTS {table}"),
            &none(),
            &no_types(),
        )
        .await
        .unwrap();
    for sql in ddl(table) {
        connection
            .execute_statement(&sql, &none(), &no_types())
            .await
            .unwrap();
    }
}

/// Checks an outcome against a recorded one: ["ok", typed] or ["throws",
/// class, message, code, sqlstate, previous, query].
pub fn assert_outcome(
    context: &str,
    expected: &serde_json::Value,
    actual: Result<Value, Exception>,
) {
    match (expected[0].as_str(), actual) {
        (Some("ok"), Ok(value)) => {
            let expected = php_value::vectors::decode(&expected[1]).expect("typed value");
            assert!(
                expected.identical(&value),
                "{context}: expected {expected:?}, got {value:?}"
            );
        }
        (Some("throws"), Err(e)) => {
            assert_eq!(e.class(), expected[1].as_str().unwrap(), "{context}: {e}");
            assert_eq!(
                e.message().to_string_lossy(),
                text(&expected[2]),
                "{context}"
            );
            assert_eq!(e.code(), expected[3].as_i64().unwrap(), "{context}");
            assert_eq!(e.sql_state(), expected[4].as_str(), "{context}");
            let previous = expected[5].as_array().unwrap();
            match (previous.first(), e.previous()) {
                (Some(first), Some(actual)) => {
                    assert_eq!(actual.class(), first[0].as_str().unwrap(), "{context}");
                    assert_eq!(
                        actual.message().to_string_lossy(),
                        text(&first[1]),
                        "{context}"
                    );
                }
                (None, None) => {}
                (first, actual) => panic!("{context}: previous {first:?} / {actual:?}"),
            }
            assert_eq!(
                e.query().map(|query| query.get_sql().to_owned()),
                expected[6].as_str().map(|_| text(&expected[6])),
                "{context}"
            );
        }
        (_, actual) => panic!("{context}: expected {expected}, got {actual:?}"),
    }
}

/// `new DateTime($text)` as a value.
pub fn date_time(text: &str) -> Value {
    let date = php_value::date_create::date_create(text.as_bytes(), None)
        .expect("a text php_value reads")
        .expect("a valid date");
    php_value::datetime::DateTime::object(date)
}
