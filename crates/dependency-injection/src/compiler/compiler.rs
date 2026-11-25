//! Port of Symfony\Component\DependencyInjection\Compiler\Compiler.php.
//!
//! `ContainerBuilder` owns its compiler, and the passes it runs reach
//! that compiler through the container (`$container->getCompiler()`).
//! So [`Compiler::compile`] runs the passes of a compiler that is not
//! the container's, and `ContainerBuilder::compile()` runs its own
//! compiler's passes with [`Compiler::run`], which takes the pass list
//! before the passes borrow the container.
//!
//! Upstream: symfony/dependency-injection Compiler/Compiler.php

use php_value::Value;

use super::compiler_pass_interface::Pass;
use super::pass_config::{PassConfig, PassType};
use super::service_reference_graph::ServiceReferenceGraph;
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::exception::{Error, Result};

/// `Compiler`.
#[derive(Debug, Clone, Default)]
pub struct Compiler {
    pass_config: PassConfig,
    log: Vec<String>,
    service_reference_graph: ServiceReferenceGraph,
}

impl Compiler {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getPassConfig()`.
    pub fn get_pass_config(&self) -> &PassConfig {
        &self.pass_config
    }

    /// `getPassConfig()`, to change it.
    pub fn get_pass_config_mut(&mut self) -> &mut PassConfig {
        &mut self.pass_config
    }

    /// `getServiceReferenceGraph()`.
    pub fn get_service_reference_graph(&self) -> &ServiceReferenceGraph {
        &self.service_reference_graph
    }

    /// `getServiceReferenceGraph()`, to change it.
    pub fn get_service_reference_graph_mut(&mut self) -> &mut ServiceReferenceGraph {
        &mut self.service_reference_graph
    }

    /// `addPass($pass, $type, $priority)`.
    pub fn add_pass(&mut self, pass: Pass, type_: PassType, priority: i64) {
        self.pass_config.add_pass(pass, type_, priority);
    }

    /// `log($pass, $message)`; `pass_class` is `$pass::class`.
    pub fn log(&mut self, pass_class: &str, message: &str) {
        let message = if message.contains('\n') {
            String::from_utf8_lossy(php_value::string::trim(message.as_bytes()))
                .replace('\n', &format!("\n{pass_class}: "))
        } else {
            message.to_string()
        };
        self.log.push(format!("{pass_class}: {message}"));
    }

    /// `getLog()`.
    pub fn get_log(&self) -> &[String] {
        &self.log
    }

    /// `compile($container)` with this compiler's passes.
    pub fn compile(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let passes = self.pass_config.get_passes();
        let result = Self::run(&passes, container);
        self.service_reference_graph.clear();
        result
    }

    /// The `try`/`catch` of `compile()`: runs `passes` in order. A failure
    /// has the env placeholders of its messages (and of its previous
    /// errors') turned back into `%env(NAME)%`, and is wrapped in an
    /// `EnvParameterException` when there were any. Clearing the graph
    /// (the `finally`) is the caller's.
    pub fn run(passes: &[Pass], container: &mut ContainerBuilder) -> Result<()> {
        for pass in passes {
            if let Err(mut e) = pass.process(container) {
                let mut used_envs = Vec::new();
                let mut prev = Some(&mut e);
                while let Some(current) = prev {
                    let message = current.message();
                    let resolved = container.resolve_env_placeholders(
                        Value::from(message.as_str()),
                        &EnvFormat::Placeholder,
                        Some(&mut used_envs),
                    )?;
                    let resolved = match resolved {
                        Value::String(s) => s.to_string_lossy().into_owned(),
                        _ => message.clone(),
                    };
                    if resolved != message {
                        current.set_message(resolved);
                    }
                    prev = current.previous_mut();
                }
                if !used_envs.is_empty() {
                    return Err(Error::env_parameter(&used_envs, None).with_previous(e));
                }
                return Err(e);
            }
        }
        Ok(())
    }
}
