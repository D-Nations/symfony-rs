//! Port of Doctrine\DBAL\Query\QueryType.php (`@internal`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/QueryType.php

/// The type of query a `QueryBuilder` builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum QueryType {
    #[default]
    Select,
    Delete,
    Update,
    Insert,
    Union,
}
