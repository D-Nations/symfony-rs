//! Port of Doctrine\DBAL\Query\ForUpdate.php (`@internal`) and
//! ForUpdate\ConflictResolutionMode.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Query/ForUpdate.php src/Query/ForUpdate/ConflictResolutionMode.php

/// What a `SELECT ... FOR UPDATE` does with a locked row. The default is
/// `forUpdate()`'s default argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ConflictResolutionMode {
    /// Wait for the row to be unlocked
    #[default]
    Ordinary,
    /// Skip the row if it is locked
    SkipLocked,
}

impl ConflictResolutionMode {
    /// Every case, in declaration order.
    pub const CASES: [Self; 2] = [Self::Ordinary, Self::SkipLocked];

    /// The case's PHP name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Ordinary => "ORDINARY",
            Self::SkipLocked => "SKIP_LOCKED",
        }
    }

    /// The case with this PHP name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::CASES.into_iter().find(|case| case.name() == name)
    }
}

/// The FOR UPDATE clause of a query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForUpdate {
    conflict_resolution_mode: ConflictResolutionMode,
}

impl ForUpdate {
    pub fn new(conflict_resolution_mode: ConflictResolutionMode) -> Self {
        Self {
            conflict_resolution_mode,
        }
    }

    pub fn get_conflict_resolution_mode(&self) -> ConflictResolutionMode {
        self.conflict_resolution_mode
    }
}
