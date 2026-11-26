//! The state an `AbstractPlatform` keeps between calls (its
//! `$doctrineTypeMapping` and `$_keywords` properties), and the argument
//! types the trait's schema methods take.
//!
//! Upstream: none

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use php_value::{PhpArray, PhpString, Value};

use crate::exception::Exception;
use crate::schema::{ForeignKeyConstraint, Index, UniqueConstraint};

use super::keywords::KeywordList;

/// The mutable part of a platform (PHP's instance properties), shared
/// through `&self` as PHP objects are.
#[derive(Debug, Default)]
pub struct PlatformState {
    type_mapping: Mutex<Option<BTreeMap<String, String>>>,
    keywords: OnceLock<Arc<KeywordList>>,
}

impl PlatformState {
    pub fn new() -> Self {
        Self::default()
    }

    /// `$this->doctrineTypeMapping`, initialized by `init` on first use.
    pub(crate) fn with_type_mapping<T>(
        &self,
        init: impl FnOnce() -> Result<BTreeMap<String, String>, Exception>,
        f: impl FnOnce(&mut BTreeMap<String, String>) -> T,
    ) -> Result<T, Exception> {
        let mut guard = self
            .type_mapping
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if guard.is_none() {
            *guard = Some(init()?);
        }
        match guard.as_mut() {
            Some(mapping) => Ok(f(mapping)),
            None => Err(Exception::new(Exception::UNSUPPORTED, "type mapping")),
        }
    }

    /// `$this->_keywords ??= $create()`.
    pub(crate) fn keywords(&self, create: impl FnOnce() -> KeywordList) -> Arc<KeywordList> {
        Arc::clone(self.keywords.get_or_init(|| Arc::new(create())))
    }
}

/// The `$options` array `_getCreateTableSQL()` takes: the table's options
/// with the keys `buildCreateTableSQL()` adds (`None`: the key is not
/// set).
#[derive(Debug, Clone, Default)]
pub struct CreateTableOptions {
    /// The table's own options (`comment`, `alter`, `without_rowid`, ...).
    pub table: PhpArray,
    /// `primary`: the quoted primary key column names.
    pub primary: Option<Vec<String>>,
    /// `primary_index`.
    pub primary_index: Option<Index>,
    /// `indexes`.
    pub indexes: Option<Vec<Index>>,
    /// `uniqueConstraints`.
    pub unique_constraints: Option<Vec<UniqueConstraint>>,
    /// `foreignKeys`.
    pub foreign_keys: Option<Vec<ForeignKeyConstraint>>,
}

/// A PHP string as UTF-8 text (identifiers and SQL here are `String`s).
pub(crate) fn utf8(s: &PhpString) -> Result<String, Exception> {
    match s.to_str() {
        Some(text) => Ok(text.to_owned()),
        None => Err(Exception::new(
            Exception::UNSUPPORTED,
            "a string that is not UTF-8",
        )),
    }
}

/// `$value` passed to a `string` parameter of `method` under
/// `strict_types`.
pub(crate) fn string_argument(
    value: &Value,
    method: &str,
    position: usize,
    parameter: &str,
) -> Result<String, Exception> {
    match value {
        Value::String(s) => utf8(s),
        other => Err(Exception::new(
            Exception::TYPE_ERROR,
            format!(
                "{method}(): Argument #{position} (${parameter}) must be of type string, {} given",
                other.value_name()
            ),
        )),
    }
}

/// `'...' . $value`: the string conversion of a value concatenated into
/// SQL.
pub(crate) fn concat_value(value: &Value) -> Result<String, Exception> {
    match value.to_php_string_or_stringable() {
        Some(s) => utf8(&s),
        None => Err(Exception::new(
            Exception::UNSUPPORTED,
            format!("string conversion of {}", value.debug_type()),
        )),
    }
}
