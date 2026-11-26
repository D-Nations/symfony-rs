//! A stand-in for Doctrine\DBAL\Platforms\SQLitePlatform until
//! `SQLitePlatform.php` is ported (another slice): the methods that the
//! connection, its tests and the query builder call on `SQLite`, with
//! `SQLitePlatform`'s bodies, and the other required methods refusing with
//! an `Unsupported` exception. Replace this file with the real platform
//! when it is merged; the name `SQLitePlatform` is the same.
//!
//! It ports no PHP file, so it names no upstream file.
//!
//! Upstream: none

use php_value::{PhpArray, PhpString};

use super::AbstractPlatform;
use crate::sql::builder::{DefaultSelectSQLBuilder, SelectSQLBuilder};
use crate::{Exception, TransactionIsolationLevel};

/// The `SQLite` platform (stand-in).
#[derive(Debug, Clone, Copy, Default)]
pub struct SQLitePlatform;

impl SQLitePlatform {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\SQLitePlatform";

    pub fn new() -> Self {
        Self
    }
}

fn not_in_stand_in(method: &str) -> Exception {
    Exception::new(
        Exception::UNSUPPORTED,
        format!("SQLitePlatform::{method}() (SQLitePlatform.php is not ported yet)"),
    )
}

impl AbstractPlatform for SQLitePlatform {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn get_boolean_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BOOLEAN"))
    }

    fn get_integer_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Err(not_in_stand_in("getIntegerTypeDeclarationSQL"))
    }

    fn get_big_int_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Err(not_in_stand_in("getBigIntTypeDeclarationSQL"))
    }

    fn get_small_int_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Err(not_in_stand_in("getSmallIntTypeDeclarationSQL"))
    }

    fn get_clob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("CLOB"))
    }

    fn get_blob_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("BLOB"))
    }

    fn get_date_time_type_declaration_sql(
        &self,
        _column: &PhpArray,
    ) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DATETIME"))
    }

    fn get_date_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("DATE"))
    }

    fn get_time_type_declaration_sql(&self, _column: &PhpArray) -> Result<PhpString, Exception> {
        Ok(PhpString::from("TIME"))
    }

    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(self, None, None))
    }

    fn get_current_database_expression(&self) -> String {
        "'main'".to_owned()
    }

    fn get_transaction_isolation_level_sql(&self, level: TransactionIsolationLevel) -> String {
        match level {
            TransactionIsolationLevel::ReadUncommitted => "0",
            TransactionIsolationLevel::ReadCommitted
            | TransactionIsolationLevel::RepeatableRead
            | TransactionIsolationLevel::Serializable => "1",
        }
        .to_owned()
    }

    fn get_set_transaction_isolation_sql(&self, level: TransactionIsolationLevel) -> String {
        format!(
            "PRAGMA read_uncommitted = {}",
            self.get_transaction_isolation_level_sql(level)
        )
    }

    fn supports_identity_columns(&self) -> bool {
        true
    }
}
