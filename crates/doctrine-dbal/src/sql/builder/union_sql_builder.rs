//! Port of Doctrine\DBAL\SQL\Builder\UnionSQLBuilder.php and
//! DefaultUnionSQLBuilder.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/SQL/Builder/UnionSQLBuilder.php src/SQL/Builder/DefaultUnionSQLBuilder.php

use crate::Exception;
use crate::platforms::AbstractPlatform;
use crate::query::{UnionQuery, UnionType};

/// Builds the SQL of a UNION query.
pub trait UnionSQLBuilder {
    fn build_sql(&self, query: &UnionQuery<'_>) -> Result<String, Exception>;
}

/// The UNION builder of every platform.
pub struct DefaultUnionSQLBuilder<'p, P: ?Sized> {
    platform: &'p P,
}

impl<'p, P: AbstractPlatform + ?Sized> DefaultUnionSQLBuilder<'p, P> {
    pub fn new(platform: &'p P) -> Self {
        Self { platform }
    }
}

impl<P: AbstractPlatform + ?Sized> UnionSQLBuilder for DefaultUnionSQLBuilder<'_, P> {
    fn build_sql(&self, query: &UnionQuery<'_>) -> Result<String, Exception> {
        let mut parts = Vec::new();

        for union in query.get_union_parts() {
            if let Some(r#type) = union.r#type {
                parts.push(if r#type == UnionType::All {
                    self.platform.get_union_all_sql()
                } else {
                    self.platform.get_union_distinct_sql()
                });
            }

            parts.push(
                self.platform
                    .get_union_select_part_sql(&union.query.to_sql()?),
            );
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

        Ok(sql)
    }
}
