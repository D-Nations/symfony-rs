//! Port of Doctrine\DBAL\Query\Limit.php (`@internal`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/Limit.php

/// The LIMIT and OFFSET of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limit {
    max_results: Option<i64>,
    first_result: i64,
}

impl Limit {
    pub fn new(max_results: Option<i64>, first_result: i64) -> Self {
        Self {
            max_results,
            first_result,
        }
    }

    pub fn is_defined(&self) -> bool {
        self.max_results.is_some() || self.first_result != 0
    }

    pub fn get_max_results(&self) -> Option<i64> {
        self.max_results
    }

    pub fn get_first_result(&self) -> i64 {
        self.first_result
    }
}
