//! Shared test helpers.
//!
//! `compile()` runs `ContainerBuilder::compile()` without the default
//! passes that are still `PendingPass`es (`ResolveAutowireInlineAttributesPass`
//! and `AutowirePass`, not ported yet). Each test that uses it builds a
//! container those two passes leave unchanged (nothing is autowired).
//! Once they are ported, nothing is filtered and this is `compile()`.
#![allow(dead_code)]

use std::cell::RefCell;
use std::sync::{Arc, Mutex, RwLock};

use php_value::Value;
use symfony_dependency_injection::compiler::PendingPass;
use symfony_dependency_injection::container_builder::EnvVarResolver;
use symfony_dependency_injection::env_var_processor_interface::EnvVarProcessorInterface;
use symfony_dependency_injection::parameter_bag::ParameterBag;
use symfony_dependency_injection::{
    ContainerBuilder, ContainerInterface, Error, EnvVarProcessor, InvalidBehavior, Result,
};
use symfony_dotenv::Globals;

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

/// The builder's parameters, as the container `EnvVarProcessor` reads
/// `env(NAME)` defaults and `resolve:` parameters from.
struct Parameters(Mutex<ParameterBag>);

impl ContainerInterface for Parameters {
    fn set(&self, _id: &str, _service: Option<Value>) -> Result<()> {
        Ok(())
    }

    fn get(&self, id: &str, _invalid_behavior: InvalidBehavior) -> Result<Option<Value>> {
        Err(Error::service_not_found(id, None, Vec::new(), None))
    }

    fn has(&self, _id: &str) -> bool {
        false
    }

    fn initialized(&self, _id: &str) -> bool {
        false
    }

    fn get_parameter(&self, name: &str) -> Result<Value> {
        self.0.lock().unwrap().get(name)
    }

    fn has_parameter(&self, name: &str) -> bool {
        self.0.lock().unwrap().has(name)
    }

    fn set_parameter(&self, name: &str, value: Value) -> Result<()> {
        self.0.lock().unwrap().set(name, value)
    }
}

/// `Container::getEnv()`'s default `EnvVarProcessor`, reading
/// `getenv()` from its own `Globals` (`putenv()` writes there) in place
/// of the process environment.
#[derive(Default)]
pub struct EnvProcessor {
    globals: Arc<RwLock<Globals>>,
}

impl EnvProcessor {
    pub fn new() -> Self {
        Self {
            globals: Arc::new(RwLock::new(Globals::new())),
        }
    }

    /// `putenv($setting)`.
    pub fn putenv(&self, setting: &str) {
        self.globals.write().unwrap().putenv(setting.as_bytes());
    }
}

impl EnvVarResolver for EnvProcessor {
    fn get_env(
        &self,
        prefix: &str,
        name: &str,
        parameters: &ParameterBag,
        get_env: &mut dyn FnMut(&str) -> Result<Value>,
    ) -> Result<Value> {
        let container = Arc::new(Parameters(Mutex::new(parameters.clone())));
        let processor = EnvVarProcessor::new(container, None).with_globals(self.globals.clone());
        let get_env = RefCell::new(get_env);
        // `Container::getEnv()`: a name without a prefix is a `string:` one.
        let prefix = if prefix.is_empty() { "string" } else { prefix };
        processor.get_env(prefix, name, &|n| (get_env.borrow_mut())(n))
    }
}
