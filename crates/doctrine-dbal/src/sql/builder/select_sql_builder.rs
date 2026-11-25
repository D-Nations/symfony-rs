//! Port of Doctrine\DBAL\SQL\Builder\SelectSQLBuilder.php and
//! DefaultSelectSQLBuilder.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/SQL/Builder/SelectSQLBuilder.php src/SQL/Builder/DefaultSelectSQLBuilder.php

use crate::Exception;
use crate::platforms::{AbstractPlatform, NotSupported};
use crate::query::{ConflictResolutionMode, SelectQuery};

/// Builds the SQL of a SELECT query.
pub trait SelectSQLBuilder {
    fn build_sql(&self, query: &SelectQuery) -> Result<String, Exception>;
}

/// The SELECT builder of most platforms.
pub struct DefaultSelectSQLBuilder<'p, P: ?Sized> {
    platform: &'p P,
    for_update_sql: Option<String>,
    skip_locked_sql: Option<String>,
}

impl<'p, P: AbstractPlatform + ?Sized> DefaultSelectSQLBuilder<'p, P> {
    /// `@internal` The SQL builder should be instantiated only by database
    /// platforms.
    pub fn new(
        platform: &'p P,
        for_update_sql: Option<&str>,
        skip_locked_sql: Option<&str>,
    ) -> Self {
        Self {
            platform,
            for_update_sql: for_update_sql.map(str::to_owned),
            skip_locked_sql: skip_locked_sql.map(str::to_owned),
        }
    }
}

impl<P: AbstractPlatform + ?Sized> SelectSQLBuilder for DefaultSelectSQLBuilder<'_, P> {
    fn build_sql(&self, query: &SelectQuery) -> Result<String, Exception> {
        let mut parts = vec!["SELECT".to_owned()];

        if query.is_distinct() {
            parts.push("DISTINCT".to_owned());
        }

        parts.push(query.get_columns().join(", "));

        let from = query.get_from();

        if !from.is_empty() {
            parts.push(format!("FROM {}", from.join(", ")));
        }

        if let Some(r#where) = query.get_where() {
            parts.push(format!("WHERE {where}"));
        }

        let group_by = query.get_group_by();

        if !group_by.is_empty() {
            parts.push(format!("GROUP BY {}", group_by.join(", ")));
        }

        if let Some(having) = query.get_having() {
            parts.push(format!("HAVING {having}"));
        }

        let order_by = query.get_order_by();

        if !order_by.is_empty() {
            parts.push(format!("ORDER BY {}", order_by.join(", ")));
        }

        let mut sql = parts.join(" ");
        let limit = query.get_limit();

        if limit.is_defined() {
            sql = self.platform.modify_limit_query(
                &sql,
                limit.get_max_results(),
                limit.get_first_result(),
            )?;
        }

        if let Some(for_update) = query.get_for_update() {
            let Some(for_update_sql) = &self.for_update_sql else {
                return Err(NotSupported::new("FOR UPDATE").into());
            };

            sql.push(' ');
            sql.push_str(for_update_sql);

            if for_update.get_conflict_resolution_mode() == ConflictResolutionMode::SkipLocked {
                let Some(skip_locked_sql) = &self.skip_locked_sql else {
                    return Err(NotSupported::new("SKIP LOCKED").into());
                };

                sql.push(' ');
                sql.push_str(skip_locked_sql);
            }
        }

        Ok(sql)
    }
}
