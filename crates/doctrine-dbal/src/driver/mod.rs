//! Port of Doctrine\DBAL\Driver.php: the interface that all DBAL drivers
//! implement, and the `Driver` namespace (the driver-level connection,
//! statement, result and exceptions).
//!
//! The interfaces are object-safe traits, so a connection holds its driver
//! as `Arc<dyn Driver>` as PHP holds an object of any class. What can wait
//! on the database returns a [`BoxFuture`]: the DBAL API is asynchronous
//! (PLAN.md, the async decision). Results are read into memory when the
//! statement executes, so fetching does not wait.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver.php

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use php_value::PhpArray;

use crate::Exception;
use crate::ServerVersionProvider;
use crate::platforms::AbstractPlatform;

mod abstract_sqlite_driver;
pub mod api;
mod connection;
mod fetch_utils;
pub mod pdo_sqlite;
mod result;
mod statement;

pub use abstract_sqlite_driver::AbstractSQLiteDriver;
pub use connection::Connection;
pub use fetch_utils::FetchUtils;
pub use result::Result;
pub use statement::Statement;

/// A future that a driver or connection method returns.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A database platform as a driver gives it.
pub type PlatformBox = Box<dyn AbstractPlatform + Send + Sync>;

/// Driver interface. Interface that all DBAL drivers must implement.
pub trait Driver: Send + Sync {
    /// `get_debug_type($driver)`: the class name.
    fn class(&self) -> &str;

    /// Attempts to create a connection with the database.
    fn connect<'a>(
        &'a self,
        params: &'a PhpArray,
    ) -> BoxFuture<'a, std::result::Result<Arc<dyn Connection>, Exception>>;

    /// Gets the DatabasePlatform instance that provides all the metadata
    /// about the platform this driver connects to.
    fn get_database_platform<'a>(
        &'a self,
        version_provider: &'a dyn ServerVersionProvider,
    ) -> BoxFuture<'a, std::result::Result<PlatformBox, Exception>>;

    /// Gets the ExceptionConverter that can be used to convert driver-level
    /// exceptions into DBAL exceptions.
    fn get_exception_converter(&self) -> Arc<dyn api::ExceptionConverter>;
}
