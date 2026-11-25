//! Port of Symfony\Component\DependencyInjection\StaticEnvVarLoader.php.
//!
//! Upstream: symfony/dependency-injection StaticEnvVarLoader.php

use std::sync::{Arc, Mutex, PoisonError};

use php_value::PhpArray;

use crate::env_var_loader_interface::EnvVarLoaderInterface;
use crate::exception::Result;

/// `StaticEnvVarLoader`: the inner loader's variables, loaded once.
pub struct StaticEnvVarLoader {
    env_var_loader: Arc<dyn EnvVarLoaderInterface>,
    env_vars: Mutex<Option<PhpArray>>,
}

impl std::fmt::Debug for StaticEnvVarLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StaticEnvVarLoader")
            .field("env_vars", &self.env_vars)
            .finish_non_exhaustive()
    }
}

impl StaticEnvVarLoader {
    pub fn new(env_var_loader: Arc<dyn EnvVarLoaderInterface>) -> Self {
        Self {
            env_var_loader,
            env_vars: Mutex::new(None),
        }
    }
}

impl EnvVarLoaderInterface for StaticEnvVarLoader {
    fn load_env_vars(&self) -> Result<PhpArray> {
        let cached = self
            .env_vars
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(vars) = cached {
            return Ok(vars);
        }
        let vars = self.env_var_loader.load_env_vars()?;
        *self.env_vars.lock().unwrap_or_else(PoisonError::into_inner) = Some(vars.clone());
        Ok(vars)
    }
}
