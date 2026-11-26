//! Doctrine\DBAL\Platforms: the SQL dialects.
//!
//! Upstream: none

mod abstract_platform;
mod exception;
mod sqlite_platform_stand_in;

pub use abstract_platform::AbstractPlatform;
pub use exception::NotSupported;
pub use sqlite_platform_stand_in::SQLitePlatform;
