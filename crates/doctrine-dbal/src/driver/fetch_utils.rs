//! Port of Doctrine\DBAL\Driver\FetchUtils.php (`@internal`): the
//! `fetchAll*()` and `fetchOne()` of a driver result, from its
//! `fetchNumeric()` and `fetchAssociative()`.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/FetchUtils.php

use php_value::{Key, PhpArray, Value};

use super::Result as DriverResult;
use crate::Exception;

/// The helpers, as associated functions of the PHP class.
pub struct FetchUtils;

impl FetchUtils {
    pub fn fetch_one<R: DriverResult + ?Sized>(result: &mut R) -> Result<Option<Value>, Exception> {
        let Some(row) = result.fetch_numeric()? else {
            return Ok(None);
        };
        // `$row[0]`: a row without columns gives null (with a warning).
        Ok(Some(row.get(&Key::Int(0)).cloned().unwrap_or(Value::Null)))
    }

    pub fn fetch_all_numeric<R: DriverResult + ?Sized>(
        result: &mut R,
    ) -> Result<Vec<PhpArray>, Exception> {
        let mut rows = Vec::new();
        while let Some(row) = result.fetch_numeric()? {
            rows.push(row);
        }
        Ok(rows)
    }

    pub fn fetch_all_associative<R: DriverResult + ?Sized>(
        result: &mut R,
    ) -> Result<Vec<PhpArray>, Exception> {
        let mut rows = Vec::new();
        while let Some(row) = result.fetch_associative()? {
            rows.push(row);
        }
        Ok(rows)
    }

    pub fn fetch_first_column<R: DriverResult + ?Sized>(
        result: &mut R,
    ) -> Result<Vec<Value>, Exception> {
        let mut rows = Vec::new();
        while let Some(value) = result.fetch_one()? {
            rows.push(value);
        }
        Ok(rows)
    }
}
