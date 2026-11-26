//! Port of Doctrine\DBAL\ServerVersionProvider.php and
//! Connection\StaticServerVersionProvider.php.
//!
//! `getServerVersion()` may connect (the connection implements it), so it
//! returns a future.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/ServerVersionProvider.php src/Connection/StaticServerVersionProvider.php

use crate::Exception;
use crate::driver::BoxFuture;

/// What gives the database server version.
pub trait ServerVersionProvider: Send + Sync {
    /// Returns the database server version.
    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>>;
}

/// A version given in the connection parameters (`serverVersion`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticServerVersionProvider {
    version: String,
}

impl StaticServerVersionProvider {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
        }
    }
}

impl ServerVersionProvider for StaticServerVersionProvider {
    fn get_server_version(&self) -> BoxFuture<'_, Result<String, Exception>> {
        let version = self.version.clone();
        Box::pin(async move { Ok(version) })
    }
}
