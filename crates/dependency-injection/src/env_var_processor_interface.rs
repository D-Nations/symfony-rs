//! Port of Symfony\Component\DependencyInjection\EnvVarProcessorInterface.php.
//!
//! Upstream: symfony/dependency-injection EnvVarProcessorInterface.php

use php_value::Value;

use crate::exception::Result;

/// `EnvVarProcessorInterface`: objects that manage environment-like
/// variables.
pub trait EnvVarProcessorInterface: Send + Sync {
    /// `getEnv($prefix, $name, $getEnv)`: the value of the variable `name`
    /// in the namespace `prefix` (when the prefix is empty, null values
    /// are kept as is). `get_env` fetches more env vars.
    fn get_env(
        &self,
        prefix: &str,
        name: &str,
        get_env: &dyn Fn(&str) -> Result<Value>,
    ) -> Result<Value>;

    /// `getProvidedTypes()`: the PHP types `get_env()` returns, keyed by
    /// prefix, in order.
    fn get_provided_types() -> Vec<(String, String)>
    where
        Self: Sized;
}
