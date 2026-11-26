//! Doctrine\DBAL\Platforms: the SQL dialects.
//!
//! Upstream: none

mod abstract_platform;
mod exception;

pub use abstract_platform::AbstractPlatform;
pub use exception::NotSupported;

// Slice SQLite platform.
mod date_interval_unit;
pub mod keywords;
mod no_columns_specified_for_table;
mod platform_state;
mod sqlite_platform;

pub use abstract_platform::AsDynPlatform;
pub use date_interval_unit::{DateIntervalUnit, TrimMode};
pub use no_columns_specified_for_table::{
    NO_COLUMNS_SPECIFIED_FOR_TABLE, PLATFORM_EXCEPTION, no_columns_specified_for_table,
};
pub use platform_state::{CreateTableOptions, PlatformState};
pub use sqlite_platform::SQLitePlatform;
