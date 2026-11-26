//! Port of Doctrine\DBAL\Platforms\DateIntervalUnit.php and TrimMode.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/DateIntervalUnit.php src/Platforms/TrimMode.php

/// The unit of a date interval (`DateIntervalUnit`, a string-backed enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DateIntervalUnit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl DateIntervalUnit {
    /// `->value`.
    pub fn value(self) -> &'static str {
        match self {
            Self::Second => "SECOND",
            Self::Minute => "MINUTE",
            Self::Hour => "HOUR",
            Self::Day => "DAY",
            Self::Week => "WEEK",
            Self::Month => "MONTH",
            Self::Quarter => "QUARTER",
            Self::Year => "YEAR",
        }
    }
}

/// Where `TRIM()` removes characters (`TrimMode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TrimMode {
    #[default]
    Unspecified,
    Leading,
    Trailing,
    Both,
}
