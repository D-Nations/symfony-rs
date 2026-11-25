//! Port of Doctrine\DBAL\Query\From.php and Join.php (`@internal`).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/From.php src/Query/Join.php

/// A table in the FROM clause, with its alias (PHP's `From`, renamed so
/// that it does not shadow Rust's `From` trait).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FromTable {
    pub table: String,
    pub alias: Option<String>,
}

impl FromTable {
    pub fn new(table: impl Into<String>, alias: Option<String>) -> Self {
        Self {
            table: table.into(),
            alias,
        }
    }
}

/// The kind of a join (`Join::$type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
}

impl JoinType {
    /// The SQL keyword (`'INNER'`, ...).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inner => "INNER",
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
            Self::Full => "FULL",
        }
    }
}

/// A join to a table of the FROM clause or to another join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Join {
    pub r#type: JoinType,
    pub table: String,
    pub alias: String,
    pub condition: Option<String>,
}

impl Join {
    fn new(r#type: JoinType, table: &str, alias: &str, condition: Option<&str>) -> Self {
        Self {
            r#type,
            table: table.to_owned(),
            alias: alias.to_owned(),
            condition: condition.map(str::to_owned),
        }
    }

    pub fn inner(table: &str, alias: &str, condition: Option<&str>) -> Self {
        Self::new(JoinType::Inner, table, alias, condition)
    }

    pub fn left(table: &str, alias: &str, condition: Option<&str>) -> Self {
        Self::new(JoinType::Left, table, alias, condition)
    }

    pub fn right(table: &str, alias: &str, condition: Option<&str>) -> Self {
        Self::new(JoinType::Right, table, alias, condition)
    }

    pub fn full(table: &str, alias: &str, condition: Option<&str>) -> Self {
        Self::new(JoinType::Full, table, alias, condition)
    }
}
