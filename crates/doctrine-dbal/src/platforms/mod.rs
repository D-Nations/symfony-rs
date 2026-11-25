//! Doctrine\DBAL\Platforms: the SQL dialects.
//!
//! Upstream: none

mod abstract_platform;
mod exception;

pub use abstract_platform::AbstractPlatform;
pub use exception::NotSupported;
