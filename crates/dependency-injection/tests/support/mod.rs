//! Shared test helpers.
//!
//! `compile()` runs `ContainerBuilder::compile()` without the default
//! passes that are still `PendingPass`es (`ResolveAutowireInlineAttributesPass`
//! and `AutowirePass`, not ported yet). Each test that uses it builds a
//! container those two passes leave unchanged (nothing is autowired).
//! Once they are ported, nothing is filtered and this is `compile()`.
#![allow(dead_code)]

use symfony_dependency_injection::compiler::PendingPass;
use symfony_dependency_injection::{ContainerBuilder, Result};

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
