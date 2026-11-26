//! Port of Doctrine\DBAL\Driver\API\ExceptionConverter.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/API/ExceptionConverter.php

use crate::{Exception, Query};

pub mod sqlite;

/// Converts driver-level exceptions into DBAL exceptions.
pub trait ExceptionConverter: Send + Sync {
    /// Converts a given driver-level exception into a DBAL-level driver
    /// exception (a `DriverException` or one of its subclasses), using the
    /// vendor-specific error code and `SQLSTATE` of the exception.
    fn convert(&self, exception: Exception, query: Option<Query>) -> Exception;
}
