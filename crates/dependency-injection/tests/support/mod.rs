//! Shared test helpers.
//!
//! `compile()` runs `ContainerBuilder::compile()` without the default
//! passes that are still `PendingPass`es (`ResolveAutowireInlineAttributesPass`
//! and `AutowirePass`, not ported yet). Each test that uses it builds a
//! container those two passes leave unchanged (nothing is autowired).
//! Once they are ported, nothing is filtered and this is `compile()`.
#![allow(dead_code)]

use php_value::Value;
use symfony_dependency_injection::compiler::PendingPass;
use symfony_dependency_injection::container_builder::EnvVarResolver;
use symfony_dependency_injection::parameter_bag::ParameterBag;
use symfony_dependency_injection::{ContainerBuilder, Error, Result};

/// Drops the pending passes from the container's optimization passes.
pub fn skip_pending_passes(container: &mut ContainerBuilder) {
    let config = container.get_compiler_pass_config();
    let passes = config.get_optimization_passes();
    if passes
        .iter()
        .any(|pass| pass.with::<PendingPass, _>(|_| ()).is_some())
    {
        let kept = passes
            .into_iter()
            .filter(|pass| pass.with::<PendingPass, _>(|_| ()).is_none())
            .collect();
        config.set_optimization_passes(kept);
    }
}

/// `$container->compile($resolveEnvPlaceholders)` with the ported passes.
pub fn compile(container: &mut ContainerBuilder, resolve_env_placeholders: bool) -> Result<()> {
    skip_pending_passes(container);
    container.compile(resolve_env_placeholders)
}

/// `EnvVarProcessor` with nothing in `$_ENV`, `$_SERVER` or `getenv()`:
/// a plain name gives its `env(NAME)` default parameter.
pub struct EnvDefaults;

impl EnvVarResolver for EnvDefaults {
    fn get_env(
        &self,
        prefix: &str,
        name: &str,
        parameters: &ParameterBag,
        _get_env: &mut dyn FnMut(&str) -> Result<Value>,
    ) -> Result<Value> {
        assert_eq!("", prefix, "EnvDefaults reads plain names only");
        let default = format!("env({name})");
        if !parameters.has(&default) {
            return Err(Error::env_not_found(format!(
                "Environment variable not found: \"{name}\"."
            )));
        }
        parameters.get(&default)
    }
}
