//! Port of Doctrine\DBAL\Result.php: the result of a query, over a driver
//! result, with the driver's exceptions converted by the connection.
//!
//! PHP's `false` for "no more rows" is `None`. The `iterate*()` methods
//! are iterators that consume the result; the driver result holds its rows
//! already, so they do not wait.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Result.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::driver::Result as DriverResult;
use crate::{Connection, Exception};

type StdResult<T> = std::result::Result<T, Exception>;

/// The result of a query (`Doctrine\DBAL\Result`).
pub struct Result<'c> {
    result: Box<dyn DriverResult>,
    connection: &'c Connection,
}

impl std::fmt::Debug for Result<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Result")
            .field("result", &self.result.class())
            .finish_non_exhaustive()
    }
}

/// `array_shift($row)`: the first value, and the rest with its integer
/// keys renumbered from 0.
fn array_shift(row: &PhpArray) -> (Value, PhpArray) {
    let mut entries = row.iter();
    let first = entries
        .next()
        .map_or(Value::Null, |(_, value)| value.clone());
    let mut rest = PhpArray::new();
    let mut next = 0;
    for (key, value) in entries {
        match key {
            Key::Int(_) => {
                rest.insert(Key::Int(next), value.clone());
                next += 1;
            }
            Key::Str(_) => rest.insert(key.clone(), value.clone()),
        }
    }
    (first, rest)
}

/// `[$key, $value] = $row`.
fn key_value(row: &PhpArray) -> (Value, Value) {
    let at = |index| row.get(&Key::Int(index)).cloned().unwrap_or(Value::Null);
    (at(0), at(1))
}

/// `$data[$key] = $value`: PHP's array key for the value. An array or an
/// object is not a key: PHP throws "Illegal offset type".
fn array_key(value: &Value) -> StdResult<Key> {
    Key::from_value(value).ok_or_else(|| {
        Exception::new(
            Exception::TYPE_ERROR,
            format!(
                "Cannot access offset of type {} on array",
                value.debug_type()
            ),
        )
    })
}

impl<'c> Result<'c> {
    /// `@internal The result can be only instantiated by its driver
    /// connection or statement.`
    pub fn new(result: Box<dyn DriverResult>, connection: &'c Connection) -> Self {
        Self { result, connection }
    }

    fn convert(&self, e: Exception) -> Exception {
        self.connection.convert_if_driver(e, None)
    }

    /// The next row as a list (`None` when there are no more rows).
    pub fn fetch_numeric(&mut self) -> StdResult<Option<PhpArray>> {
        self.result.fetch_numeric().map_err(|e| self.convert(e))
    }

    /// The next row as an associative array.
    pub fn fetch_associative(&mut self) -> StdResult<Option<PhpArray>> {
        self.result.fetch_associative().map_err(|e| self.convert(e))
    }

    /// The first value of the next row.
    pub fn fetch_one(&mut self) -> StdResult<Option<Value>> {
        self.result.fetch_one().map_err(|e| self.convert(e))
    }

    /// The remaining rows as lists.
    pub fn fetch_all_numeric(&mut self) -> StdResult<Vec<PhpArray>> {
        self.result.fetch_all_numeric().map_err(|e| self.convert(e))
    }

    /// The remaining rows as associative arrays.
    pub fn fetch_all_associative(&mut self) -> StdResult<Vec<PhpArray>> {
        self.result
            .fetch_all_associative()
            .map_err(|e| self.convert(e))
    }

    /// The first column as keys and the second as values; at least two
    /// columns are needed.
    pub fn fetch_all_key_value(&mut self) -> StdResult<PhpArray> {
        self.ensure_has_key_value()?;
        let mut data = PhpArray::new();
        for row in self.fetch_all_numeric()? {
            let (key, value) = key_value(&row);
            data.insert(array_key(&key)?, value);
        }
        Ok(data)
    }

    /// The rows by their first column, each with the other columns.
    pub fn fetch_all_associative_indexed(&mut self) -> StdResult<PhpArray> {
        let mut data = PhpArray::new();
        for row in self.fetch_all_associative()? {
            let (key, rest) = array_shift(&row);
            data.insert(array_key(&key)?, Value::Array(rest));
        }
        Ok(data)
    }

    /// The values of the first column.
    pub fn fetch_first_column(&mut self) -> StdResult<Vec<Value>> {
        self.result
            .fetch_first_column()
            .map_err(|e| self.convert(e))
    }

    /// `iterateNumeric()`.
    pub fn into_iterate_numeric(mut self) -> impl Iterator<Item = StdResult<PhpArray>> + 'c {
        std::iter::from_fn(move || self.fetch_numeric().transpose())
    }

    /// `iterateAssociative()`.
    pub fn into_iterate_associative(mut self) -> impl Iterator<Item = StdResult<PhpArray>> + 'c {
        std::iter::from_fn(move || self.fetch_associative().transpose())
    }

    /// `iterateKeyValue()`: the pairs, as PHP yields keys that need not be
    /// array keys.
    pub fn into_iterate_key_value(
        mut self,
    ) -> StdResult<impl Iterator<Item = StdResult<(Value, Value)>> + 'c> {
        self.ensure_has_key_value()?;
        Ok(std::iter::from_fn(move || {
            self.fetch_numeric()
                .map(|row| row.map(|row| key_value(&row)))
                .transpose()
        }))
    }

    /// `iterateAssociativeIndexed()`.
    pub fn into_iterate_associative_indexed(
        mut self,
    ) -> impl Iterator<Item = StdResult<(Value, PhpArray)>> + 'c {
        std::iter::from_fn(move || {
            self.fetch_associative()
                .map(|row| row.map(|row| array_shift(&row)))
                .transpose()
        })
    }

    /// `iterateColumn()`.
    pub fn into_iterate_column(mut self) -> impl Iterator<Item = StdResult<Value>> + 'c {
        std::iter::from_fn(move || self.fetch_one().transpose())
    }

    /// The number of rows affected by the last DELETE, INSERT, or UPDATE
    /// statement (as the driver counts them).
    pub fn row_count(&self) -> StdResult<i64> {
        self.result.row_count().map_err(|e| self.convert(e))
    }

    /// The number of columns in the result.
    pub fn column_count(&self) -> StdResult<usize> {
        self.result.column_count().map_err(|e| self.convert(e))
    }

    /// The name of the column at the (zero-based) index.
    pub fn get_column_name(&self, index: i64) -> StdResult<PhpString> {
        match self.result.get_column_name(index) {
            None => Err(Exception::new(
                Exception::LOGIC_EXCEPTION,
                format!(
                    "The driver result {} does not support accessing the column name.",
                    self.result.class()
                ),
            )),
            Some(name) => name.map_err(|e| self.convert(e)),
        }
    }

    /// Discards the non-fetched portion of the result.
    pub fn free(&mut self) {
        self.result.free();
    }

    fn ensure_has_key_value(&self) -> StdResult<()> {
        let column_count = self.column_count()?;
        if column_count < 2 {
            return Err(Exception::no_key_value(column_count));
        }
        Ok(())
    }
}
