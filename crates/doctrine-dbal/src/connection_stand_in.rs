//! A stand-in for Doctrine\DBAL\Connection until Connection.php is ported:
//! the part of it that the query builder uses. It does not port
//! Connection.php, so it names no upstream file.
//!
//! `createExpressionBuilder()` and `createQueryBuilder()` are default
//! methods with Connection's body (`new ExpressionBuilder($this)`,
//! `new QueryBuilder($this)`). The platform and quoting come from the
//! implementation: in PHP they need the driver connection. Tests implement
//! it in place of PHPUnit's `createMock(Connection::class)`.
//!
//! Upstream: none

use crate::Exception;
use crate::platforms::AbstractPlatform;
use crate::query::QueryBuilder;
use crate::query::expression::ExpressionBuilder;

/// What the query builder needs of `Doctrine\DBAL\Connection`.
pub trait ConnectionStandIn {
    /// The platform type that `get_database_platform()` gives.
    type Platform: AbstractPlatform + ?Sized;

    /// `getDatabasePlatform()`: the database platform of the connection.
    /// PHP may connect to detect it, and throws when that fails.
    fn get_database_platform(&self) -> Result<&Self::Platform, Exception>;

    /// `quote()`: quotes a string so that it can be safely used as a
    /// literal (the driver connection's quoting).
    fn quote(&self, value: &str) -> Result<String, Exception>;

    /// `createExpressionBuilder()`.
    fn create_expression_builder(&self) -> ExpressionBuilder<'_, Self> {
        ExpressionBuilder::new(self)
    }

    /// `createQueryBuilder()`.
    fn create_query_builder(&self) -> QueryBuilder<'_, Self> {
        QueryBuilder::new(self)
    }
}
