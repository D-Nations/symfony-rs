//! Tests/StaticEnvVarLoaderTest.php at symfony/dependency-injection
//! 9b34cad8. Names and assertions are PHPUnit's.
//!
//! Upstream: symfony/dependency-injection Tests/StaticEnvVarLoaderTest.php
#![allow(non_snake_case, clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use php_value::{PhpArray, php_array};
use symfony_dependency_injection::{EnvVarLoaderInterface, Result, StaticEnvVarLoader};

struct InnerLoader {
    env_vars: Mutex<PhpArray>,
}

impl EnvVarLoaderInterface for InnerLoader {
    fn load_env_vars(&self) -> Result<PhpArray> {
        Ok(self.env_vars.lock().unwrap().clone())
    }
}

#[test]
fn testLoadEnvVarsCachesInnerLoaderEnvVars() {
    let inner_loader = Arc::new(InnerLoader {
        env_vars: Mutex::new(php_array!["FOO" => "BAR"]),
    });

    let loader = StaticEnvVarLoader::new(inner_loader.clone());
    assert_eq!(php_array!["FOO" => "BAR"], loader.load_env_vars().unwrap());

    *inner_loader.env_vars.lock().unwrap() = php_array!["BAR" => "BAZ"];
    assert_eq!(php_array!["FOO" => "BAR"], loader.load_env_vars().unwrap());
}
