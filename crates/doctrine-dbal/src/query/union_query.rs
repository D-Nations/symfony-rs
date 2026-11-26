//! Port of Doctrine\DBAL\Query\UnionQuery.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/UnionQuery.php

use super::{Limit, Union};

/// The parts of a UNION query, as a `UnionSQLBuilder` reads them.
#[derive(Debug, Clone)]
pub struct UnionQuery<'c> {
    union_parts: Vec<Union<'c>>,
    order_by: Vec<String>,
    limit: Limit,
}

impl<'c> UnionQuery<'c> {
    /// `@internal` This class should be instantiated only by `QueryBuilder`.
    pub fn new(union_parts: Vec<Union<'c>>, order_by: Vec<String>, limit: Limit) -> Self {
        Self {
            union_parts,
            order_by,
            limit,
        }
    }

    pub fn get_union_parts(&self) -> &[Union<'c>] {
        &self.union_parts
    }

    pub fn get_order_by(&self) -> &[String] {
        &self.order_by
    }

    pub fn get_limit(&self) -> Limit {
        self.limit
    }
}
