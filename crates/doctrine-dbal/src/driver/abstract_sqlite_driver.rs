//! Port of Doctrine\DBAL\Driver\AbstractSQLiteDriver.php: the base of the
//! `SQLite` drivers. A Rust trait has no state to share, so the abstract
//! class is a trait whose provided methods are the class's two methods; a
//! `SQLite` driver forwards its `Driver` methods to them.
//!
//! The platform is the stand-in of `platforms/sqlite_platform_stand_in.rs`
//! until `SQLitePlatform.php` is ported.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/AbstractSQLiteDriver.php

use std::sync::Arc;

use super::PlatformBox;
use super::api::{ExceptionConverter, sqlite};
use crate::platforms::SQLitePlatform;

/// Abstract base implementation of the `Driver` interface for `SQLite` based
/// drivers.
pub trait AbstractSQLiteDriver {
    /// `getDatabasePlatform()`: a `SQLite` platform, whatever the version.
    fn sqlite_database_platform(&self) -> PlatformBox {
        Box::new(SQLitePlatform::new())
    }

    /// `getExceptionConverter()`.
    fn sqlite_exception_converter(&self) -> Arc<dyn ExceptionConverter> {
        Arc::new(sqlite::ExceptionConverter)
    }
}
