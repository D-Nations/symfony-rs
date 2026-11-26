//! Port of Doctrine\DBAL\Driver\Statement.php: the driver-level statement.
//!
//! Doctrine source copyright (c) Doctrine Project; distributed under MIT.
//!
//! Upstream: doctrine/dbal src/Driver/Statement.php

use php_value::{Key, Value};

use super::{BoxFuture, Result as DriverResult};
use crate::{Exception, ParameterType};

/// Driver-level statement.
pub trait Statement: Send {
    /// Binds a value to a corresponding named or positional placeholder in
    /// the SQL statement that was used to prepare the statement. `param`
    /// is a name (`:name` or `name`) or a 1-indexed position.
    fn bind_value(
        &mut self,
        param: &Key,
        value: Value,
        r#type: ParameterType,
    ) -> Result<(), Exception>;

    /// Executes a prepared statement.
    fn execute(&mut self) -> BoxFuture<'_, Result<Box<dyn DriverResult>, Exception>>;
}
