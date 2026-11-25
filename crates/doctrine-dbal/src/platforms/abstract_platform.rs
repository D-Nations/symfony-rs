//! Port of Doctrine\DBAL\Platforms\AbstractPlatform.php, one group of
//! methods at a time.
//!
//! The PHP class is abstract: its abstract methods are required trait
//! methods, its concrete methods are default methods with upstream's body.
//! A platform (`SQLitePlatform`, ...) implements the trait and overrides
//! what its PHP class overrides. Tests stand in for PHPUnit's
//! `createMock(AbstractPlatform::class)` with their own implementation.
//!
//! Methods are added with the slice that first needs them, each group under
//! a comment that names the slice (Query, Types, ...), so slices merge
//! without reordering one another.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/AbstractPlatform.php

use crate::exception::InvalidArgumentException;
use crate::sql::builder::{
    DefaultSelectSQLBuilder, DefaultUnionSQLBuilder, SelectSQLBuilder, UnionSQLBuilder,
    WithSQLBuilder,
};

/// Base contract for the SQL dialects (`AbstractPlatform`).
pub trait AbstractPlatform {
    // Slice Query: the SQL builders and LIMIT/UNION methods that
    // Query\QueryBuilder and SQL\Builder\Default*SQLBuilder use.

    /// `createSelectSQLBuilder()`.
    fn create_select_sql_builder(&self) -> Box<dyn SelectSQLBuilder + '_> {
        Box::new(DefaultSelectSQLBuilder::new(
            self,
            Some("FOR UPDATE"),
            Some("SKIP LOCKED"),
        ))
    }

    /// `createUnionSQLBuilder()`.
    fn create_union_sql_builder(&self) -> Box<dyn UnionSQLBuilder + '_> {
        Box::new(DefaultUnionSQLBuilder::new(self))
    }

    /// `createWithSQLBuilder()`.
    fn create_with_sql_builder(&self) -> WithSQLBuilder {
        WithSQLBuilder::new()
    }

    /// Adds a driver-specific LIMIT clause to the query
    /// (`modifyLimitQuery()`). Final in PHP: platforms override
    /// [`Self::do_modify_limit_query`] instead.
    fn modify_limit_query(
        &self,
        query: &str,
        limit: Option<i64>,
        offset: i64,
    ) -> Result<String, InvalidArgumentException> {
        if offset < 0 {
            return Err(InvalidArgumentException::new(format!(
                "Offset must be a positive integer or zero, {offset} given."
            )));
        }

        Ok(self.do_modify_limit_query(query, limit, offset))
    }

    /// Adds an platform-specific LIMIT clause to the query
    /// (`doModifyLimitQuery()`, protected in PHP).
    fn do_modify_limit_query(&self, query: &str, limit: Option<i64>, offset: i64) -> String {
        let mut query = query.to_owned();

        if let Some(limit) = limit {
            query = format!("{query} LIMIT {limit}");
        }

        if offset > 0 {
            query = format!("{query} OFFSET {offset}");
        }

        query
    }

    /// Returns the SQL of a part of a UNION query
    /// (`getUnionSelectPartSQL()`).
    fn get_union_select_part_sql(&self, sub_query: &str) -> String {
        format!("({sub_query})")
    }

    /// Returns the `UNION ALL` keyword.
    fn get_union_all_sql(&self) -> String {
        "UNION ALL".to_owned()
    }

    /// Returns the compatible `UNION DISTINCT` keyword.
    fn get_union_distinct_sql(&self) -> String {
        "UNION".to_owned()
    }
}
