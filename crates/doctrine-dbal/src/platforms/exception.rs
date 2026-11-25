//! Port of Doctrine\DBAL\Platforms\Exception\NotSupported.php and its
//! marker interface PlatformException.php.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Platforms/Exception/NotSupported.php src/Platforms/Exception/PlatformException.php

use std::fmt;

/// `NotSupported`: an operation the platform does not support (a
/// `\LogicException` in PHP).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotSupported {
    message: String,
}

impl NotSupported {
    pub const CLASS: &'static str = "Doctrine\\DBAL\\Platforms\\Exception\\NotSupported";

    /// `NotSupported::new($method)`.
    pub fn new(method: &str) -> Self {
        Self {
            message: format!("Operation \"{method}\" is not supported by platform."),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for NotSupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for NotSupported {}
