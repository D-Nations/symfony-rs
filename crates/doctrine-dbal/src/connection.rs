//! Port of Doctrine\DBAL\Connection.php: a database connection that wraps
//! a driver connection, with parameter expansion and type conversion,
//! the `fetch*()` helpers, `insert()`/`update()`/`delete()`, nested
//! transactions with savepoints and the conversion of driver exceptions.
//!
//! The API is async (README.md, decision 22 in PLAN.md): every method that
//! may reach the database returns a future. Methods take `&self`, as PHP
//! shares the object; the state lives behind a lock that is never held
//! across an `.await`. One connection serves one task at a time, as one
//! PDO handle does: two tasks that run statements on it at once see each
//! other's transactions.
//!
//! `Connection` is a `ConnectionStandIn` for the `QueryBuilder` and the
//! `ExpressionBuilder`, which are synchronous: `get_database_platform()`
//! there gives the platform once it is known (a `SQLite` driver gives it
//! without the database) and `quote()` needs a connection that is already
//! open (README.md, Differences).
//!
//! What is not ported: the result cache (`executeCacheQuery()`,
//! `QueryCacheProfile`), the schema manager (`createSchemaManager()`),
//! `getNativeConnection()`, middlewares and the deprecation notices.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Connection.php

use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::task::{Context, Poll, Waker};

use php_value::{Key, PhpArray, PhpString, Value};

use crate::driver::api::ExceptionConverter;
use crate::driver::{
    BoxFuture, Connection as DriverConnection, Driver, PlatformBox, Statement as DriverStatement,
};
use crate::platforms::AbstractPlatform;
use crate::sql::Parser;
use crate::types;
use crate::{
    Configuration, ConnectionStandIn, Exception, ExpandArrayParameters, ParameterType,
    ParameterTypes, Query, Result, ServerVersionProvider, Statement, StaticServerVersionProvider,
    TransactionIsolationLevel, WrapperParameterType,
};

type StdResult<T> = std::result::Result<T, Exception>;

/// The mutable state of a connection.
#[derive(Debug, Clone, Copy)]
struct State {
    /// The current auto-commit mode of this connection.
    auto_commit: bool,
    /// The transaction nesting level.
    transaction_nesting_level: i64,
    /// The currently active transaction isolation level or `None` before
    /// it has been determined.
    transaction_isolation_level: Option<TransactionIsolationLevel>,
    /// Whether the current transaction is marked for rollback only.
    is_rollback_only: bool,
}

/// A database connection (`Doctrine\DBAL\Connection`).
pub struct Connection {
    params: PhpArray,
    driver: Arc<dyn Driver>,
    config: Configuration,
    /// The wrapped driver connection.
    conn: Mutex<Option<Arc<dyn DriverConnection>>>,
    state: Mutex<State>,
    /// The database platform object used by the connection, once known.
    platform: OnceLock<PlatformBox>,
    exception_converter: OnceLock<Arc<dyn ExceptionConverter>>,
    parser: OnceLock<Parser>,
}

impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("driver", &self.driver.class())
            .field("connected", &self.is_connected())
            .field("state", &*self.state())
            .finish_non_exhaustive()
    }
}

/// `$params['serverVersion']` or `$params['primary']['serverVersion']`,
/// when set and not null.
fn server_version_param(params: &PhpArray) -> Option<PhpString> {
    let key = Key::from("serverVersion");
    let set = |array: &PhpArray| {
        array
            .get(&key)
            .filter(|value| !value.is_null())
            .and_then(Value::to_php_string)
    };
    set(params).or_else(|| {
        params
            .get(&Key::from("primary"))
            .and_then(Value::as_array)
            .and_then(set)
    })
}

/// The key of a column name for SQL.
fn column_sql(key: &Key) -> String {
    String::from_utf8_lossy(&key.bytes()).into_owned()
}

/// `$columnName . ' IS NULL'` / `$columnName . ' = ?'` per criterion:
/// the columns and values that bind, and the conditions.
fn get_criteria_condition(criteria: &PhpArray) -> (Vec<Key>, Vec<Value>, Vec<String>) {
    let mut columns = Vec::new();
    let mut values = Vec::new();
    let mut conditions = Vec::new();
    for (column_name, value) in criteria.iter() {
        if value.is_null() {
            conditions.push(format!("{} IS NULL", column_sql(column_name)));
            continue;
        }
        columns.push(column_name.clone());
        values.push(value.clone());
        conditions.push(format!("{} = ?", column_sql(column_name)));
    }
    (columns, values, conditions)
}

/// `is_string(key($types))`. The types are ordered by key with integers
/// first, so a string comes first only when every key is a string
/// (README.md, Differences).
fn first_key_is_string(types: &ParameterTypes) -> bool {
    matches!(types.keys().next(), Some(Key::Str(_)))
}

/// The types of the columns, by position: `$types[$columnName] ??
/// ParameterType::STRING`.
fn extract_type_values(columns: &[Key], types: &ParameterTypes) -> ParameterTypes {
    columns
        .iter()
        .zip(0_i64..)
        .map(|(column, index)| {
            let column_type = types
                .get(column)
                .cloned()
                .unwrap_or(WrapperParameterType::Parameter(ParameterType::String));
            (Key::Int(index), column_type)
        })
        .collect()
}

/// Polls a future once: what a synchronous caller can get from a future
/// that does not wait (a `SQLite` platform needs no I/O).
fn poll_once<F: Future>(future: F) -> Option<F::Output> {
    let mut future = pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(output) => Some(output),
        Poll::Pending => None,
    }
}

/// PHP's `finally`: an exception thrown there replaces the one on its way
/// out, which becomes its deepest previous.
fn finally(result: StdResult<()>, after: StdResult<()>) -> StdResult<()> {
    match (result, after) {
        (Err(e), Err(f)) => Err(f.chain(e)),
        (Err(e), Ok(())) | (Ok(()), Err(e)) => Err(e),
        (Ok(()), Ok(())) => Ok(()),
    }
}

impl Connection {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Connection";

    /// Initializes a new instance of the Connection class. PHP marks it
    /// `@internal`: the driver manager builds connections.
    pub fn new(params: PhpArray, driver: Arc<dyn Driver>, config: Option<Configuration>) -> Self {
        let config = config.unwrap_or_default();
        let state = State {
            auto_commit: config.get_auto_commit(),
            transaction_nesting_level: 0,
            transaction_isolation_level: None,
            is_rollback_only: false,
        };
        Self {
            params,
            driver,
            config,
            conn: Mutex::new(None),
            state: Mutex::new(state),
            platform: OnceLock::new(),
            exception_converter: OnceLock::new(),
            parser: OnceLock::new(),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn driver_connection(&self) -> Option<Arc<dyn DriverConnection>> {
        self.conn
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Gets the parameters used during instantiation.
    pub fn get_params(&self) -> &PhpArray {
        &self.params
    }

    /// Gets the name of the currently selected database (`None` for PHP's
    /// `false`, no row).
    pub async fn get_database(&self) -> StdResult<Option<Value>> {
        let platform = self.get_database_platform().await?;
        let query = platform.get_dummy_select_sql(&platform.get_current_database_expression());
        self.fetch_one(&query, &PhpArray::new(), &ParameterTypes::new())
            .await
    }

    /// Gets the DBAL driver instance.
    pub fn get_driver(&self) -> &Arc<dyn Driver> {
        &self.driver
    }

    /// Gets the Configuration used by the Connection.
    pub fn get_configuration(&self) -> &Configuration {
        &self.config
    }

    /// Gets the database platform for the connection.
    pub async fn get_database_platform(
        &self,
    ) -> StdResult<&(dyn AbstractPlatform + Send + Sync + 'static)> {
        if let Some(platform) = self.platform.get() {
            return Ok(platform.as_ref());
        }
        let platform = match server_version_param(&self.params) {
            Some(version) => {
                let provider =
                    StaticServerVersionProvider::new(version.to_string_lossy().into_owned());
                self.driver.get_database_platform(&provider).await?
            }
            None => self.driver.get_database_platform(self).await?,
        };
        Ok(self.platform.get_or_init(|| platform).as_ref())
    }

    /// Establishes the connection with the database and returns the
    /// underlying connection. PHP's is protected; here it is how a caller
    /// opens the connection before the synchronous `quote()` of the query
    /// builder.
    pub fn connect(&self) -> BoxFuture<'_, StdResult<Arc<dyn DriverConnection>>> {
        // Boxed: `connect()` and `beginTransaction()` call each other.
        Box::pin(async move {
            if let Some(connection) = self.driver_connection() {
                return Ok(connection);
            }
            let connection = match self.driver.connect(&self.params).await {
                Ok(connection) => connection,
                Err(e) => return Err(self.convert_if_driver(e, None)),
            };
            *self.conn.lock().unwrap_or_else(PoisonError::into_inner) = Some(connection.clone());
            if !self.state().auto_commit {
                self.begin_transaction().await?;
            }
            Ok(connection)
        })
    }

    /// Returns the current auto-commit mode for this connection.
    pub fn is_auto_commit(&self) -> bool {
        self.state().auto_commit
    }

    /// Sets auto-commit mode for this connection. Changing it during a
    /// transaction commits the transaction.
    pub async fn set_auto_commit(&self, auto_commit: bool) -> StdResult<()> {
        let commit_all = {
            let mut state = self.state();
            if auto_commit == state.auto_commit {
                return Ok(());
            }
            state.auto_commit = auto_commit;
            state.transaction_nesting_level != 0
        };
        if commit_all && self.is_connected() {
            self.commit_all().await?;
        }
        Ok(())
    }

    /// Prepares and executes an SQL query and returns the first row of the
    /// result as an associative array (`None` when there is no row).
    pub async fn fetch_associative(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<Option<PhpArray>> {
        self.execute_query(query, params, types)
            .await?
            .fetch_associative()
    }

    /// The first row of the result as a list.
    pub async fn fetch_numeric(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<Option<PhpArray>> {
        self.execute_query(query, params, types)
            .await?
            .fetch_numeric()
    }

    /// The value of the first column of the first row.
    pub async fn fetch_one(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<Option<Value>> {
        self.execute_query(query, params, types).await?.fetch_one()
    }

    /// Whether an actual connection to the database is established.
    pub fn is_connected(&self) -> bool {
        self.driver_connection().is_some()
    }

    /// Checks whether a transaction is currently active.
    pub fn is_transaction_active(&self) -> bool {
        self.state().transaction_nesting_level > 0
    }

    /// Executes an SQL DELETE statement on a table. A criterion whose
    /// value is null becomes `IS NULL`.
    pub async fn delete(
        &self,
        table: &str,
        criteria: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<i64> {
        let (columns, values, conditions) = get_criteria_condition(criteria);
        let mut sql = format!("DELETE FROM {table}");
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        let types = if first_key_is_string(types) {
            extract_type_values(&columns, types)
        } else {
            types.clone()
        };
        self.execute_statement(&sql, &PhpArray::list(values), &types)
            .await
    }

    /// Closes the connection.
    pub fn close(&self) {
        *self.conn.lock().unwrap_or_else(PoisonError::into_inner) = None;
        self.state().transaction_nesting_level = 0;
    }

    /// Sets the transaction isolation level.
    pub async fn set_transaction_isolation(
        &self,
        level: TransactionIsolationLevel,
    ) -> StdResult<i64> {
        self.state().transaction_isolation_level = Some(level);
        let sql = self
            .get_database_platform()
            .await?
            .get_set_transaction_isolation_sql(level);
        self.execute_statement(&sql, &PhpArray::new(), &ParameterTypes::new())
            .await
    }

    /// Gets the currently active transaction isolation level.
    pub async fn get_transaction_isolation(&self) -> StdResult<TransactionIsolationLevel> {
        if let Some(level) = self.state().transaction_isolation_level {
            return Ok(level);
        }
        let level = self
            .get_database_platform()
            .await?
            .get_default_transaction_isolation_level();
        Ok(*self
            .state()
            .transaction_isolation_level
            .get_or_insert(level))
    }

    /// Executes an SQL UPDATE statement on a table.
    pub async fn update(
        &self,
        table: &str,
        data: &PhpArray,
        criteria: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<i64> {
        let mut columns = Vec::new();
        let mut values = Vec::new();
        let mut set = Vec::new();
        for (column_name, value) in data.iter() {
            columns.push(column_name.clone());
            values.push(value.clone());
            set.push(format!("{} = ?", column_sql(column_name)));
        }
        let (criteria_columns, criteria_values, conditions) = get_criteria_condition(criteria);
        columns.extend(criteria_columns);
        values.extend(criteria_values);
        let types = if first_key_is_string(types) {
            extract_type_values(&columns, types)
        } else {
            types.clone()
        };
        let mut sql = format!("UPDATE {table} SET {}", set.join(", "));
        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }
        self.execute_statement(&sql, &PhpArray::list(values), &types)
            .await
    }

    /// Inserts a table row with specified data.
    pub async fn insert(
        &self,
        table: &str,
        data: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<i64> {
        if data.is_empty() {
            return self
                .execute_statement(
                    &format!("INSERT INTO {table} () VALUES ()"),
                    &PhpArray::new(),
                    &ParameterTypes::new(),
                )
                .await;
        }
        let columns: Vec<Key> = data.keys().cloned().collect();
        let values = PhpArray::list(data.values().cloned());
        let names: Vec<String> = columns.iter().map(column_sql).collect();
        let set = vec!["?"; columns.len()];
        let types = if first_key_is_string(types) {
            extract_type_values(&columns, types)
        } else {
            types.clone()
        };
        self.execute_statement(
            &format!(
                "INSERT INTO {table} ({}) VALUES ({})",
                names.join(", "),
                set.join(", ")
            ),
            &values,
            &types,
        )
        .await
    }

    /// Quotes a string so it can be safely used as a table or column name,
    /// even if it is a reserved name (deprecated upstream).
    pub async fn quote_identifier(&self, identifier: &str) -> StdResult<String> {
        Ok(self
            .get_database_platform()
            .await?
            .quote_identifier(identifier))
    }

    /// Quotes a string so that it can be safely used as an identifier in
    /// SQL.
    pub async fn quote_single_identifier(&self, identifier: &str) -> StdResult<String> {
        Ok(self
            .get_database_platform()
            .await?
            .quote_single_identifier(identifier))
    }

    /// The usual way to quote a string literal (the driver's quoting).
    pub async fn quote(&self, value: &[u8]) -> StdResult<PhpString> {
        Ok(self.connect().await?.quote(value))
    }

    /// Every row as a list.
    pub async fn fetch_all_numeric(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<Vec<PhpArray>> {
        self.execute_query(query, params, types)
            .await?
            .fetch_all_numeric()
    }

    /// Every row as an associative array.
    pub async fn fetch_all_associative(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<Vec<PhpArray>> {
        self.execute_query(query, params, types)
            .await?
            .fetch_all_associative()
    }

    /// The first column as keys and the second as values.
    pub async fn fetch_all_key_value(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<PhpArray> {
        self.execute_query(query, params, types)
            .await?
            .fetch_all_key_value()
    }

    /// The rows by their first column, the rest as an associative array.
    pub async fn fetch_all_associative_indexed(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<PhpArray> {
        self.execute_query(query, params, types)
            .await?
            .fetch_all_associative_indexed()
    }

    /// The values of the first column.
    pub async fn fetch_first_column(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<Vec<Value>> {
        self.execute_query(query, params, types)
            .await?
            .fetch_first_column()
    }

    /// `iterateNumeric()`.
    pub async fn iterate_numeric(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<impl Iterator<Item = StdResult<PhpArray>> + '_> {
        Ok(self
            .execute_query(query, params, types)
            .await?
            .into_iterate_numeric())
    }

    /// `iterateAssociative()`.
    pub async fn iterate_associative(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<impl Iterator<Item = StdResult<PhpArray>> + '_> {
        Ok(self
            .execute_query(query, params, types)
            .await?
            .into_iterate_associative())
    }

    /// `iterateKeyValue()`.
    pub async fn iterate_key_value(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<impl Iterator<Item = StdResult<(Value, Value)>> + '_> {
        self.execute_query(query, params, types)
            .await?
            .into_iterate_key_value()
    }

    /// `iterateAssociativeIndexed()`.
    pub async fn iterate_associative_indexed(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<impl Iterator<Item = StdResult<(Value, PhpArray)>> + '_> {
        Ok(self
            .execute_query(query, params, types)
            .await?
            .into_iterate_associative_indexed())
    }

    /// `iterateColumn()`.
    pub async fn iterate_column(
        &self,
        query: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<impl Iterator<Item = StdResult<Value>> + '_> {
        Ok(self
            .execute_query(query, params, types)
            .await?
            .into_iterate_column())
    }

    /// Prepares an SQL statement.
    pub async fn prepare(&self, sql: &str) -> StdResult<Statement<'_>> {
        let connection = self.connect().await?;
        let statement = match connection.prepare(sql).await {
            Ok(statement) => statement,
            Err(e) => {
                return Err(self.convert_if_driver(
                    e,
                    Some(Query::new(sql, PhpArray::new(), ParameterTypes::new())),
                ));
            }
        };
        Statement::new(self, statement, sql.to_owned()).await
    }

    /// Executes an, optionally parametrized, SQL query and returns its
    /// result.
    pub async fn execute_query(
        &self,
        sql: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<Result<'_>> {
        let connection = self.connect().await?;
        let mut query = Query::new(sql, params.clone(), types.clone());
        let outcome = if params.is_empty() {
            connection.query(sql).await
        } else {
            match self.prepare_and_bind(&connection, &mut query).await {
                Ok(mut statement) => statement.execute().await,
                Err(e) => Err(e),
            }
        };
        match outcome {
            Ok(result) => Ok(Result::new(result, self)),
            Err(e) => Err(self.convert_if_driver(e, Some(query))),
        }
    }

    /// Executes an SQL statement with the given parameters and returns the
    /// number of affected rows.
    pub async fn execute_statement(
        &self,
        sql: &str,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<i64> {
        let connection = self.connect().await?;
        let mut query = Query::new(sql, params.clone(), types.clone());
        let outcome = if params.is_empty() {
            connection.exec(sql).await
        } else {
            match self.prepare_and_bind(&connection, &mut query).await {
                Ok(mut statement) => match statement.execute().await {
                    Ok(result) => result.row_count(),
                    Err(e) => Err(e),
                },
                Err(e) => Err(e),
            }
        };
        outcome.map_err(|e| self.convert_if_driver(e, Some(query)))
    }

    /// The part of `executeQuery()` and `executeStatement()` between the
    /// expansion of the parameters and the execution; `query` becomes the
    /// expanded query, which a converted exception reports.
    async fn prepare_and_bind(
        &self,
        connection: &Arc<dyn DriverConnection>,
        query: &mut Query,
    ) -> StdResult<Box<dyn DriverStatement>> {
        *query = self.expand_array_parameters(query).await?;
        let mut statement = connection.prepare(query.get_sql()).await?;
        self.bind_parameters(&mut *statement, query.get_params(), query.get_types())
            .await?;
        Ok(statement)
    }

    /// Returns the current transaction nesting level.
    pub fn get_transaction_nesting_level(&self) -> i64 {
        self.state().transaction_nesting_level
    }

    /// Returns the ID of the last inserted row (PHP's `int|string`).
    pub async fn last_insert_id(&self) -> StdResult<Value> {
        let connection = self.connect().await?;
        connection
            .last_insert_id()
            .await
            .map_err(|e| self.convert_if_driver(e, None))
    }

    /// Executes a function in a transaction. The transaction is committed
    /// when the function succeeds and rolled back when it fails (but not
    /// when the connection was lost), or when the commit fails for another
    /// reason than one where the database already rolled back.
    pub async fn transactional<T, F>(&self, func: F) -> StdResult<T>
    where
        F: AsyncFnOnce(&Connection) -> StdResult<T>,
    {
        self.begin_transaction().await?;
        let res = match func(self).await {
            Ok(res) => res,
            Err(e) => {
                // Catching ConnectionLost only to prevent a rollback attempt.
                if e.is_instance_of(Exception::CONNECTION_LOST) {
                    return Err(e);
                }
                return Err(match self.roll_back().await {
                    Ok(()) => e,
                    Err(f) => f.chain(e),
                });
            }
        };
        match self.commit().await {
            Ok(()) => Ok(res),
            Err(t) => {
                let should_rollback = !(t.is_instance_of(Exception::DRIVER_EXCEPTION)
                    && [
                        Exception::TRANSACTION_ROLLED_BACK,
                        Exception::UNIQUE_CONSTRAINT_VIOLATION_EXCEPTION,
                        Exception::FOREIGN_KEY_CONSTRAINT_VIOLATION_EXCEPTION,
                        Exception::DEADLOCK_EXCEPTION,
                        Exception::CONNECTION_LOST,
                    ]
                    .iter()
                    .any(|class| t.is_instance_of(class)));
                if should_rollback && let Err(f) = self.roll_back().await {
                    return Err(f.chain(t));
                }
                Err(t)
            }
        }
    }

    /// Only `true` is accepted: nesting without savepoints is no longer
    /// supported.
    pub fn set_nest_transactions_with_savepoints(
        &self,
        nest_transactions_with_savepoints: bool,
    ) -> StdResult<()> {
        if !nest_transactions_with_savepoints {
            return Err(Exception::new(
                Exception::PHP_INVALID_ARGUMENT_EXCEPTION,
                "Calling Doctrine\\DBAL\\Connection::setNestTransactionsWithSavepoints with false \
                 to enable nesting transactions without savepoints is no longer supported.",
            ));
        }
        Ok(())
    }

    /// Always `true`.
    pub fn get_nest_transactions_with_savepoints(&self) -> bool {
        true
    }

    /// The savepoint name of the current nesting level.
    fn nested_transaction_save_point_name(level: i64) -> String {
        format!("DOCTRINE_{level}")
    }

    /// Starts a transaction, or a savepoint inside one.
    pub async fn begin_transaction(&self) -> StdResult<()> {
        let connection = self.connect().await?;
        let level = {
            let mut state = self.state();
            state.transaction_nesting_level += 1;
            state.transaction_nesting_level
        };
        if level == 1 {
            connection
                .begin_transaction()
                .await
                .map_err(|e| self.convert_if_driver(e, None))
        } else {
            self.create_savepoint(&Self::nested_transaction_save_point_name(level))
                .await
        }
    }

    /// Commits the current transaction, or releases its savepoint.
    pub async fn commit(&self) -> StdResult<()> {
        let (level, rollback_only) = {
            let state = self.state();
            (state.transaction_nesting_level, state.is_rollback_only)
        };
        if level == 0 {
            return Err(Exception::no_active_transaction());
        }
        if rollback_only {
            return Err(Exception::commit_failed_rollback_only());
        }
        let connection = self.connect().await?;
        let result = if level == 1 {
            connection
                .commit()
                .await
                .map_err(|e| self.convert_if_driver(e, None))
        } else {
            self.release_savepoint(&Self::nested_transaction_save_point_name(level))
                .await
        };
        let after = self.update_transaction_state_after_commit().await;
        finally(result, after)
    }

    async fn update_transaction_state_after_commit(&self) -> StdResult<()> {
        let begin = {
            let mut state = self.state();
            if state.transaction_nesting_level != 0 {
                state.transaction_nesting_level -= 1;
            }
            !state.auto_commit && state.transaction_nesting_level == 0
        };
        if begin {
            self.begin_transaction().await?;
        }
        Ok(())
    }

    /// Commits all current nesting transactions.
    async fn commit_all(&self) -> StdResult<()> {
        loop {
            let (level, auto_commit) = {
                let state = self.state();
                (state.transaction_nesting_level, state.auto_commit)
            };
            if level == 0 {
                return Ok(());
            }
            if !auto_commit && level == 1 {
                // When in no auto-commit mode, the last nesting commit
                // immediately starts a new transaction. Therefore we need
                // to do the final commit here and then leave to avoid an
                // infinite loop.
                return self.commit().await;
            }
            self.commit().await?;
        }
    }

    /// Cancels any database changes done during the current transaction,
    /// or since its savepoint.
    pub async fn roll_back(&self) -> StdResult<()> {
        let level = self.state().transaction_nesting_level;
        if level == 0 {
            return Err(Exception::no_active_transaction());
        }
        let connection = self.connect().await?;
        if level == 1 {
            self.state().transaction_nesting_level = 0;
            let result = connection
                .roll_back()
                .await
                .map_err(|e| self.convert_if_driver(e, None));
            let begin = {
                let mut state = self.state();
                state.is_rollback_only = false;
                !state.auto_commit
            };
            let after = if begin {
                self.begin_transaction().await
            } else {
                Ok(())
            };
            finally(result, after)
        } else {
            self.rollback_savepoint(&Self::nested_transaction_save_point_name(level))
                .await?;
            self.state().transaction_nesting_level -= 1;
            Ok(())
        }
    }

    /// Creates a new savepoint.
    pub async fn create_savepoint(&self, savepoint: &str) -> StdResult<()> {
        let platform = self.get_database_platform().await?;
        if !platform.supports_savepoints() {
            return Err(Exception::savepoints_not_supported());
        }
        self.execute_statement(
            &platform.create_save_point(savepoint),
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .map(drop)
    }

    /// Releases the given savepoint (when the platform releases them).
    pub async fn release_savepoint(&self, savepoint: &str) -> StdResult<()> {
        let platform = self.get_database_platform().await?;
        if !platform.supports_savepoints() {
            return Err(Exception::savepoints_not_supported());
        }
        if platform.supports_release_savepoints() {
            self.execute_statement(
                &platform.release_save_point(savepoint),
                &PhpArray::new(),
                &ParameterTypes::new(),
            )
            .await?;
        }
        Ok(())
    }

    /// Rolls back to the given savepoint.
    pub async fn rollback_savepoint(&self, savepoint: &str) -> StdResult<()> {
        let platform = self.get_database_platform().await?;
        if !platform.supports_savepoints() {
            return Err(Exception::savepoints_not_supported());
        }
        self.execute_statement(
            &platform.rollback_save_point(savepoint),
            &PhpArray::new(),
            &ParameterTypes::new(),
        )
        .await
        .map(drop)
    }

    /// Marks the current transaction so that the only possible outcome for
    /// the transaction to be rolled back.
    pub fn set_rollback_only(&self) -> StdResult<()> {
        let mut state = self.state();
        if state.transaction_nesting_level == 0 {
            return Err(Exception::no_active_transaction());
        }
        state.is_rollback_only = true;
        Ok(())
    }

    /// Checks whether the current transaction is marked for rollback only.
    pub fn is_rollback_only(&self) -> StdResult<bool> {
        let state = self.state();
        if state.transaction_nesting_level == 0 {
            return Err(Exception::no_active_transaction());
        }
        Ok(state.is_rollback_only)
    }

    /// Converts a given value to its database representation according to
    /// the conversion rules of a specific DBAL mapping type.
    pub async fn convert_to_database_value(&self, value: &Value, r#type: &str) -> StdResult<Value> {
        let type_ = types::get_type(r#type)?;
        let platform = self.get_database_platform().await?;
        Ok(type_.convert_to_database_value(value, platform)?)
    }

    /// Converts a given value to its PHP representation according to the
    /// conversion rules of a specific DBAL mapping type.
    pub async fn convert_to_php_value(&self, value: &Value, r#type: &str) -> StdResult<Value> {
        let type_ = types::get_type(r#type)?;
        let platform = self.get_database_platform().await?;
        Ok(type_.convert_to_php_value(value, platform)?)
    }

    /// Binds a set of parameters, some or all of which are typed with a
    /// PDO binding type or DBAL mapping type, to a given statement.
    async fn bind_parameters(
        &self,
        statement: &mut dyn DriverStatement,
        params: &PhpArray,
        types: &ParameterTypes,
    ) -> StdResult<()> {
        // Check whether parameters are positional or named. Mixing is not
        // allowed.
        let positional = matches!(params.keys().next(), Some(Key::Int(_)));
        for (bind_index, (key, value)) in (1..).zip(params.iter()) {
            let (value, binding_type) = match types.get(key) {
                Some(param_type) => self.get_binding_info(value, param_type).await?,
                None => (value.clone(), ParameterType::String),
            };
            let param = if positional {
                Key::Int(bind_index)
            } else {
                key.clone()
            };
            statement
                .bind_value(&param, value, binding_type)
                .map_err(|e| self.convert_if_driver(e, None))?;
        }
        Ok(())
    }

    /// The value and the binding type of a parameter.
    async fn get_binding_info(
        &self,
        value: &Value,
        r#type: &WrapperParameterType,
    ) -> StdResult<(Value, ParameterType)> {
        let type_ = match r#type {
            WrapperParameterType::Parameter(binding_type) => {
                return Ok((value.clone(), *binding_type));
            }
            WrapperParameterType::Array(_) => {
                return Err(Exception::new(
                    Exception::TYPE_ERROR,
                    "Doctrine\\DBAL\\Connection::getBindingInfo(): Argument #2 ($type) must be of \
                     type Doctrine\\DBAL\\Types\\Type|Doctrine\\DBAL\\ParameterType|string, \
                     Doctrine\\DBAL\\ArrayParameterType given",
                ));
            }
            WrapperParameterType::Name(name) => types::get_type(&name.to_string_lossy())?,
            WrapperParameterType::Type(type_) => type_.clone(),
        };
        let platform = self.get_database_platform().await?;
        let value = type_.convert_to_database_value(value, platform)?;
        Ok((value, type_.get_binding_type()))
    }

    /// `convertExceptionDuringQuery()`: converts a driver exception
    /// thrown while running a query.
    pub fn convert_exception_during_query(
        &self,
        e: Exception,
        sql: &str,
        params: PhpArray,
        types: ParameterTypes,
    ) -> Exception {
        self.handle_driver_exception(e, Some(Query::new(sql, params, types)))
    }

    /// `convertException()`: converts a driver exception.
    pub fn convert_exception(&self, e: Exception) -> Exception {
        self.handle_driver_exception(e, None)
    }

    /// PHP's `catch (Driver\Exception $e) { throw $this->convert...($e) }`:
    /// what is not a driver exception goes through as it is.
    pub(crate) fn convert_if_driver(&self, e: Exception, query: Option<Query>) -> Exception {
        if e.is_instance_of(Exception::DRIVER_EXCEPTION_INTERFACE) {
            self.handle_driver_exception(e, query)
        } else {
            e
        }
    }

    /// Expands the list parameters and the named parameters of a query,
    /// when it has some.
    async fn expand_array_parameters(&self, query: &Query) -> StdResult<Query> {
        let params = query.get_params();
        let types = query.get_types();
        let needs_conversion = matches!(params.keys().next(), Some(Key::Str(_)))
            || types
                .values()
                .any(|type_| matches!(type_, WrapperParameterType::Array(_)));
        if !needs_conversion {
            return Ok(query.clone());
        }
        let parser = if let Some(parser) = self.parser.get() {
            *parser
        } else {
            let parser = self.get_database_platform().await?.create_sql_parser();
            *self.parser.get_or_init(|| parser)
        };
        let mut visitor = ExpandArrayParameters::new(params, types);
        parser.parse(query.get_sql().as_bytes(), &mut visitor)?;
        let types = visitor
            .get_types()
            .iter()
            .filter_map(|(index, type_)| {
                i64::try_from(*index)
                    .ok()
                    .map(|index| (Key::Int(index), type_.clone()))
            })
            .collect();
        Ok(Query::new(
            visitor.get_sql().to_string_lossy().into_owned(),
            PhpArray::list(visitor.get_parameters().iter().cloned()),
            types,
        ))
    }

    fn handle_driver_exception(
        &self,
        driver_exception: Exception,
        query: Option<Query>,
    ) -> Exception {
        let converter = self
            .exception_converter
            .get_or_init(|| self.driver.get_exception_converter());
        let exception = converter.convert(driver_exception, query);
        if exception.is_instance_of(Exception::CONNECTION_LOST) {
            self.close();
        }
        exception
    }
}

impl ServerVersionProvider for Connection {
    /// The version of the database server, from the driver connection.
    fn get_server_version(&self) -> BoxFuture<'_, StdResult<String>> {
        Box::pin(async move { self.connect().await?.get_server_version().await })
    }
}

impl ConnectionStandIn for Connection {
    type Platform = dyn AbstractPlatform + Send + Sync;

    /// The platform, when it is known or the driver gives it without
    /// waiting on the database (the `SQLite` driver does). A driver that
    /// needs the server version first refuses: await
    /// `Connection::get_database_platform()` before building queries.
    fn get_database_platform(&self) -> StdResult<&Self::Platform> {
        if let Some(platform) = self.platform.get() {
            return Ok(platform.as_ref());
        }
        poll_once(Connection::get_database_platform(self)).unwrap_or_else(|| {
            Err(Exception::new(
                Exception::UNSUPPORTED,
                "the database platform needs the database: await \
                 Connection::get_database_platform() first",
            ))
        })
    }

    /// The driver connection's quoting, once the connection is open: await
    /// `Connection::connect()` first.
    fn quote(&self, value: &str) -> StdResult<String> {
        match self.driver_connection() {
            Some(connection) => Ok(connection
                .quote(value.as_bytes())
                .to_string_lossy()
                .into_owned()),
            None => Err(Exception::new(
                Exception::UNSUPPORTED,
                "quote() needs an open connection: await Connection::connect() first",
            )),
        }
    }
}
