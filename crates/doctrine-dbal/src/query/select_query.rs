//! Port of Doctrine\DBAL\Query\SelectQuery.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/SelectQuery.php

use super::{ForUpdate, Limit};

/// The parts of a SELECT query, as a `SelectSQLBuilder` reads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectQuery {
    distinct: bool,
    columns: Vec<String>,
    from: Vec<String>,
    r#where: Option<String>,
    group_by: Vec<String>,
    having: Option<String>,
    order_by: Vec<String>,
    limit: Limit,
    for_update: Option<ForUpdate>,
}

impl SelectQuery {
    /// `@internal` This class should be instantiated only by `QueryBuilder`.
    #[allow(
        clippy::too_many_arguments,
        clippy::fn_params_excessive_bools,
        reason = "upstream's constructor"
    )]
    pub fn new(
        distinct: bool,
        columns: Vec<String>,
        from: Vec<String>,
        r#where: Option<String>,
        group_by: Vec<String>,
        having: Option<String>,
        order_by: Vec<String>,
        limit: Limit,
        for_update: Option<ForUpdate>,
    ) -> Self {
        Self {
            distinct,
            columns,
            from,
            r#where,
            group_by,
            having,
            order_by,
            limit,
            for_update,
        }
    }

    pub fn is_distinct(&self) -> bool {
        self.distinct
    }

    pub fn get_columns(&self) -> &[String] {
        &self.columns
    }

    pub fn get_from(&self) -> &[String] {
        &self.from
    }

    pub fn get_where(&self) -> Option<&str> {
        self.r#where.as_deref()
    }

    pub fn get_group_by(&self) -> &[String] {
        &self.group_by
    }

    pub fn get_having(&self) -> Option<&str> {
        self.having.as_deref()
    }

    pub fn get_order_by(&self) -> &[String] {
        &self.order_by
    }

    pub fn get_limit(&self) -> Limit {
        self.limit
    }

    pub fn get_for_update(&self) -> Option<ForUpdate> {
        self.for_update
    }
}
