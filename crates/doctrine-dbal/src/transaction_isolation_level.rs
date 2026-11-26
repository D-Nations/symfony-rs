//! Port of Doctrine\DBAL\TransactionIsolationLevel.php and LockMode.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/TransactionIsolationLevel.php src/LockMode.php

/// A transaction isolation level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransactionIsolationLevel {
    ReadUncommitted,
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

impl TransactionIsolationLevel {
    /// The case name (`READ_UNCOMMITTED`, ...).
    pub fn name(self) -> &'static str {
        match self {
            Self::ReadUncommitted => "READ_UNCOMMITTED",
            Self::ReadCommitted => "READ_COMMITTED",
            Self::RepeatableRead => "REPEATABLE_READ",
            Self::Serializable => "SERIALIZABLE",
        }
    }
}

/// Contains all DBAL `LockModes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockMode {
    None,
    Optimistic,
    PessimisticRead,
    PessimisticWrite,
}
