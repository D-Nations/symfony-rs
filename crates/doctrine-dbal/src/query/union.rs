//! Port of Doctrine\DBAL\Query\Union.php (`@internal`) and UnionType.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/Union.php src/Query/UnionType.php

use super::QueryPart;

/// `UNION ALL` or `UNION` (distinct). The default is `addUnion()`'s
/// default argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum UnionType {
    All,
    #[default]
    Distinct,
}

impl UnionType {
    /// Every case, in declaration order.
    pub const CASES: [Self; 2] = [Self::All, Self::Distinct];

    /// The case's PHP name.
    pub fn name(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Distinct => "DISTINCT",
        }
    }

    /// The case with this PHP name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::CASES.into_iter().find(|case| case.name() == name)
    }
}

/// One part of a UNION query; the first part has no type.
#[derive(Debug, Clone)]
pub struct Union<'c> {
    pub query: QueryPart<'c>,
    pub r#type: Option<UnionType>,
}

impl<'c> Union<'c> {
    pub fn new(query: QueryPart<'c>, r#type: Option<UnionType>) -> Self {
        Self { query, r#type }
    }
}
