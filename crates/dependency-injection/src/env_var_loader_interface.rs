//! Port of Symfony\Component\DependencyInjection\EnvVarLoaderInterface.php.
//!
//! Upstream: symfony/dependency-injection EnvVarLoaderInterface.php

use php_value::PhpArray;

use crate::exception::Result;

/// `EnvVarLoaderInterface`: key/value pairs added to the env vars that
/// `%env()%` can read.
pub trait EnvVarLoaderInterface: Send + Sync {
    /// `loadEnvVars()`: the pairs; each value a string or a `\Stringable`
    /// object.
    fn load_env_vars(&self) -> Result<PhpArray>;
}
