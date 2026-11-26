//! Port of Doctrine\DBAL\Configuration.php: what the connection reads of
//! it. The result cache, the schema assets filter, the schema manager
//! factory and the driver middlewares are not ported yet (their
//! components are not).
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Configuration.php

use crate::Exception;

/// Configuration container for the Doctrine DBAL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Configuration {
    /// The default auto-commit mode for connections.
    auto_commit: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

impl Configuration {
    pub fn new() -> Self {
        Self { auto_commit: true }
    }

    /// Sets the default auto-commit mode for connections.
    ///
    /// If a connection is in auto-commit mode, then all its SQL statements
    /// will be executed and committed as individual transactions.
    /// Otherwise, its SQL statements are grouped into transactions that are
    /// terminated by a call to either the method commit or the method
    /// rollback. By default, new connections are in auto-commit mode.
    pub fn set_auto_commit(&mut self, auto_commit: bool) {
        self.auto_commit = auto_commit;
    }

    /// Returns the default auto-commit mode for connections.
    pub fn get_auto_commit(&self) -> bool {
        self.auto_commit
    }

    pub fn get_disable_type_comments(&self) -> bool {
        true
    }

    pub fn set_disable_type_comments(
        &mut self,
        disable_type_comments: bool,
    ) -> Result<&mut Self, Exception> {
        if !disable_type_comments {
            return Err(Exception::invalid_argument(
                "Column comments cannot be enabled anymore.",
            ));
        }
        Ok(self)
    }
}
