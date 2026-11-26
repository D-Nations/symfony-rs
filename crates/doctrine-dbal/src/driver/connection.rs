//! Port of Doctrine\DBAL\Driver\Connection.php: the connection interface
//! that driver connections implement.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/Connection.php

use php_value::{PhpString, Value};

use super::{BoxFuture, Result as DriverResult, Statement};
use crate::Exception;

/// A driver connection. Its methods take `&self`: a statement keeps using
/// the connection it was prepared on, as in PHP.
pub trait Connection: Send + Sync {
    /// Prepares a statement for execution and returns a Statement object.
    fn prepare<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<Box<dyn Statement>, Exception>>;

    /// Executes an SQL statement, returning a result set.
    fn query<'a>(&'a self, sql: &'a str)
    -> BoxFuture<'a, Result<Box<dyn DriverResult>, Exception>>;

    /// Quotes a string for use in a query.
    fn quote(&self, value: &[u8]) -> PhpString;

    /// Executes an SQL statement and return the number of affected rows.
    fn exec<'a>(&'a self, sql: &'a str) -> BoxFuture<'a, Result<i64, Exception>>;

    /// Returns the ID of the last inserted row (`int|string`), or throws
    /// when the driver cannot return one.
    fn last_insert_id(&self) -> BoxFuture<'_, Result<Value, Exception>>;

    /// Initiates a transaction.
    fn begin_transaction(&self) -> BoxFuture<'_, Result<(), Exception>>;

    /// Commits a transaction.
    fn commit(&self) -> BoxFuture<'_, Result<(), Exception>>;

    /// Rolls back the current transaction, as initiated by
    /// `begin_transaction()`.
    fn roll_back(&self) -> BoxFuture<'_, Result<(), Exception>>;

    /// `getServerVersion()` (`ServerVersionProvider`): the database server
    /// version.
    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>>;
}
