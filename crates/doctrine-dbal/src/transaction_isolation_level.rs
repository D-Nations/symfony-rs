//! Port of Doctrine\DBAL\TransactionIsolationLevel.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/TransactionIsolationLevel.php

/// A transaction isolation level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransactionIsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl TransactionIsolationLevel {
    /// Every case, in declaration order.
    pub const CASES: [Self; 4] = [
        Self::ReadUncommitted,
        Self::ReadCommitted,
        Self::RepeatableRead,
        Self::Serializable,
    ];

    /// The case's PHP name (`TransactionIsolationLevel::READ_COMMITTED->name`).
    pub fn name(self) -> &'static str {
        match self {
            Self::ReadUncommitted => "READ_UNCOMMITTED",
            Self::ReadCommitted => "READ_COMMITTED",
            Self::RepeatableRead => "REPEATABLE_READ",
            Self::Serializable => "SERIALIZABLE",
        }
    }
}
