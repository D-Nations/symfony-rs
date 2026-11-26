//! Port of Doctrine\DBAL\Query\CommonTableExpression.php (`@internal`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/CommonTableExpression.php

use super::{QueryException, QueryPart};

/// A common table expression of a WITH clause.
#[derive(Debug, Clone)]
pub struct CommonTableExpression<'c> {
    pub name: String,
    pub query: QueryPart<'c>,
    pub columns: Option<Vec<String>>,
}

impl<'c> CommonTableExpression<'c> {
    pub fn new(
        name: &str,
        query: QueryPart<'c>,
        columns: Option<Vec<String>>,
    ) -> Result<Self, QueryException> {
        if columns.as_ref().is_some_and(Vec::is_empty) {
            return Err(QueryException::new(format!(
                "Columns defined in CTE \"{name}\" should not be an empty array."
            )));
        }

        Ok(Self {
            name: name.to_owned(),
            query,
            columns,
        })
    }
}
