//! Port of Doctrine\DBAL\SQL\Builder\WithSQLBuilder.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/SQL/Builder/WithSQLBuilder.php

use crate::Exception;
use crate::query::CommonTableExpression;

/// Builds the WITH clause of a query.
#[derive(Debug, Clone, Copy, Default)]
pub struct WithSQLBuilder;

impl WithSQLBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build_sql(
        &self,
        first_expression: &CommonTableExpression<'_>,
        other_expressions: &[CommonTableExpression<'_>],
    ) -> Result<String, Exception> {
        let mut cte_parts = Vec::new();

        for part in std::iter::once(first_expression).chain(other_expressions) {
            let mut cte_part = part.name.clone();

            if let Some(columns) = &part.columns
                && !columns.is_empty()
            {
                cte_part = cte_part + " (" + &columns.join(", ") + ")";
            }

            cte_part = cte_part + " AS (" + &part.query.to_sql()? + ")";
            cte_parts.push(cte_part);
        }

        Ok(format!("WITH {}", cte_parts.join(", ")))
    }
}
