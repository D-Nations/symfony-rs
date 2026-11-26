//! Port of Symfony\Component\DependencyInjection\Compiler\ReplaceAliasByActualDefinitionPass.php.
//!
//! Replaces aliases of private services by the services themselves,
//! renamed to the alias, and updates the references to them.
//!
//! Upstream: symfony/dependency-injection Compiler/ReplaceAliasByActualDefinitionPass.php

use php_value::Value;

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::alias::Alias;
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, ErrorKind, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// `ReplaceAliasByActualDefinitionPass`.
#[derive(Debug)]
pub struct ReplaceAliasByActualDefinitionPass {
    state: RecursivePassState,
    replacements: IdMap<String>,
}

impl Default for ReplaceAliasByActualDefinitionPass {
    fn default() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            replacements: IdMap::new(),
        }
    }
}

impl ReplaceAliasByActualDefinitionPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ReplaceAliasByActualDefinitionPass";

    pub fn new() -> Self {
        Self::default()
    }
}

impl CompilerPassInterface for ReplaceAliasByActualDefinitionPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        // First collect all alias targets that need to be replaced
        let mut seen_alias_targets: Vec<String> = Vec::new();
        let mut replacements: IdMap<String> = IdMap::new();

        // Sort aliases so non-deprecated ones come first. This ensures that when
        // multiple aliases point to the same private definition, non-deprecated
        // aliases get priority for renaming. Otherwise, the definition might be
        // renamed to a deprecated alias ID, causing the original service ID to
        // become an alias to the deprecated one (inverting the alias chain).
        let mut aliases: Vec<(String, Alias)> = container
            .get_aliases()
            .iter()
            .map(|(id, alias)| (id.to_string(), alias.clone()))
            .collect();
        aliases.sort_by_key(|(_, alias)| alias.is_deprecated());

        for (definition_id, target) in aliases {
            let target_id = target.id();
            // Special case: leave this target alone
            if target_id == "service_container" {
                continue;
            }
            // Check if target needs to be replaced
            if let Some(replacement) = replacements.get(&target_id).cloned() {
                container
                    .set_alias(&definition_id, replacement.as_str())?
                    .set_public(target.is_public());

                if let Some(deprecation) = target.deprecation("%alias_id%") {
                    container.get_alias(&definition_id)?.set_deprecated(
                        &deprecation.package,
                        &deprecation.version,
                        &deprecation.message,
                    )?;
                }
            }
            // No need to process the same target twice
            if seen_alias_targets.contains(&target_id) {
                continue;
            }
            // Process new target
            seen_alias_targets.push(target_id.clone());
            let definition = match container.get_definition(&target_id) {
                Ok(definition) => definition,
                Err(e) if e.is(&ErrorKind::ServiceNotFound) => {
                    let id = e.id().unwrap_or_default().to_string();
                    if let Some(rest) = id.strip_prefix('@') {
                        return Err(Error::service_not_found(
                            id.clone(),
                            e.source_id().map(str::to_string),
                            vec![rest.to_string()],
                            None,
                        ));
                    }
                    return Err(e);
                }
                Err(e) => return Err(e),
            };
            if definition.is_public() {
                continue;
            }
            // Remove private definition and schedule for replacement
            definition.set_public(target.is_public());
            container.set_definition(&definition_id, definition.clone())?;
            container.remove_definition(&target_id);
            replacements.insert(target_id, definition_id);

            if target.is_public()
                && let Some(deprecation) = target.deprecation("%service_id%")
            {
                definition.add_tag("container.private", deprecation.to_array());
            }
        }
        self.replacements = replacements;

        let result = recursive_process(self, container);
        self.replacements = IdMap::new();
        result
    }
}

impl AbstractRecursivePass for ReplaceAliasByActualDefinitionPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let mut value = value;
        if let Some(reference) = Reference::from_value(&value)
            && let Some(new_id) = self.replacements.get(reference.id()).cloned()
        {
            // Perform the replacement
            let reference_id = reference.id().to_string();
            value = Reference::with_behavior(new_id.as_str(), reference.invalid_behavior()).into();
            let current_id = self.state.current_id.clone().unwrap_or_default();
            container.log(
                Self::CLASS,
                &format!(
                    "Changed reference of service \"{current_id}\" previously pointing to \"{reference_id}\" to \"{new_id}\"."
                ),
            )?;
        }
        self.parent_process_value(container, value, is_root)
    }
}
