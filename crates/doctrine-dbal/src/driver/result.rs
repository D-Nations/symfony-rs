//! Port of Doctrine\DBAL\Driver\Result.php: the driver-level statement
//! execution result.
//!
//! Rows are lists (`list<mixed>`, a [`PhpArray`] with keys 0, 1, ...) or
//! maps by column name (`array<string,mixed>`); PHP's `false` for "no more
//! rows" is `None`. The `fetchAll*()` methods default to upstream's
//! `FetchUtils`, which drivers use for them.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/Result.php

use php_value::PhpArray;
use php_value::{PhpString, Value};

use super::FetchUtils;
use crate::Exception;

/// Driver-level statement execution result.
pub trait Result: Send {
    /// Returns the next row of the result as a numeric array or `None` if
    /// there are no more rows.
    fn fetch_numeric(&mut self) -> std::result::Result<Option<PhpArray>, Exception>;

    /// Returns the next row of the result as an associative array or
    /// `None` if there are no more rows.
    fn fetch_associative(&mut self) -> std::result::Result<Option<PhpArray>, Exception>;

    /// Returns the first value of the next row of the result or `None` if
    /// there are no more rows.
    fn fetch_one(&mut self) -> std::result::Result<Option<Value>, Exception> {
        FetchUtils::fetch_one(self)
    }

    /// Returns an array containing all of the result rows represented as
    /// numeric arrays.
    fn fetch_all_numeric(&mut self) -> std::result::Result<Vec<PhpArray>, Exception> {
        FetchUtils::fetch_all_numeric(self)
    }

    /// Returns an array containing all of the result rows represented as
    /// associative arrays.
    fn fetch_all_associative(&mut self) -> std::result::Result<Vec<PhpArray>, Exception> {
        FetchUtils::fetch_all_associative(self)
    }

    /// Returns an array containing the values of the first column of the
    /// result.
    fn fetch_first_column(&mut self) -> std::result::Result<Vec<Value>, Exception> {
        FetchUtils::fetch_first_column(self)
    }

    /// Returns the number of rows affected by the DELETE, INSERT, or UPDATE
    /// statement that produced the result.
    fn row_count(&self) -> std::result::Result<i64, Exception>;

    /// Returns the number of columns in the result (0 if they cannot be
    /// counted).
    fn column_count(&self) -> std::result::Result<usize, Exception>;

    /// Discards the non-fetched portion of the result, enabling the
    /// originating statement to be executed again.
    fn free(&mut self);

    /// `getColumnName($index)`, the optional method (`@method`) of the
    /// interface: `None` for a result that does not have it.
    fn get_column_name(&self, _index: i64) -> Option<std::result::Result<PhpString, Exception>> {
        None
    }

    /// `get_debug_type($result)`, which messages show.
    fn class(&self) -> &str;
}
