//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveReferencesToAliasesPass.php.
//!
//! Replaces references to aliases (and aliases of aliases) with
//! references to the service they end at, triggering the deprecation
//! of a deprecated alias that a service or alias not itself deprecated
//! references.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveReferencesToAliasesPass.php

use php_value::Value;
use symfony_deprecation_contracts::trigger_deprecation;

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, ErrorKind, Result};
use crate::reference::Reference;

/// `ResolveReferencesToAliasesPass`.
#[derive(Debug, Clone)]
pub struct ResolveReferencesToAliasesPass {
    state: RecursivePassState,
}

impl Default for ResolveReferencesToAliasesPass {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolveReferencesToAliasesPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveReferencesToAliasesPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }

    /// `getDefinitionId($id, $container)`.
    fn get_definition_id(&self, id: &str, container: &ContainerBuilder) -> Result<String> {
        if !container.has_alias(id) {
            return Ok(id.to_string());
        }

        let alias = container.get_alias(id)?;

        if alias.is_deprecated() {
            let current = self.state.current_id.clone().unwrap_or_default();
            let has_definition = container.has_definition(&current);
            let referencing_is_deprecated = if has_definition {
                container.get_definition(&current)?.is_deprecated()
            } else {
                container.get_alias(&current)?.is_deprecated()
            };
            if !referencing_is_deprecated && let Some(deprecation) = alias.deprecation(id) {
                let format = format!(
                    "{}. It is being referenced by the \"%s\" {}",
                    rtrim_dot_space(&deprecation.message),
                    if has_definition { "service." } else { "alias." }
                );
                let message = symfony_config::php::sprintf(
                    format.as_bytes(),
                    &[Value::from(current.as_str())],
                )
                .map_err(|e| {
                    Error::new(
                        ErrorKind::Other(e.class().name().to_string()),
                        String::from_utf8_lossy(e.get_message()).into_owned(),
                    )
                })?;
                trigger_deprecation(
                    deprecation.package.as_bytes(),
                    deprecation.version.as_bytes(),
                    &message,
                );
            }
        }

        let mut id = id.to_string();
        let mut seen: Vec<String> = Vec::new();
        loop {
            if seen.contains(&id) {
                let mut path = seen.clone();
                path.push(id.clone());
                return Err(Error::service_circular_reference(id, path));
            }

            seen.push(id.clone());
            id = container.get_alias(&id)?.id();
            if !container.has_alias(&id) {
                break;
            }
        }

        Ok(id)
    }
}

/// `rtrim($message, '. ')`.
fn rtrim_dot_space(message: &str) -> &str {
    message.trim_end_matches(['.', ' '])
}

impl CompilerPassInterface for ResolveReferencesToAliasesPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)?;

        let aliases: Vec<_> = container
            .get_aliases()
            .iter()
            .map(|(id, alias)| (id.to_string(), alias.clone()))
            .collect();
        for (id, alias) in aliases {
            let alias_id = alias.id();
            self.state.current_id = Some(id.clone());

            let def_id = self.get_definition_id(&alias_id, container)?;
            if alias_id != def_id {
                let new_alias = container.set_alias(&id, def_id)?;
                new_alias.set_public(alias.is_public());

                if let Some(deprecation) = alias.deprecation("%alias_id%") {
                    new_alias.set_deprecated(
                        &deprecation.package,
                        &deprecation.version,
                        &deprecation.message,
                    )?;
                }
            }
        }
        Ok(())
    }
}

impl AbstractRecursivePass for ResolveReferencesToAliasesPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let Some(reference) = Reference::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };

        let id = reference.id();
        let def_id = self.get_definition_id(id, container)?;

        Ok(if def_id == id {
            value
        } else {
            Reference::with_behavior(def_id, reference.invalid_behavior()).into()
        })
    }
}
