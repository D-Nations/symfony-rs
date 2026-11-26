//! Port of Symfony\Component\DependencyInjection\Compiler\AliasDeprecatedPublicServicesPass.php.
//!
//! A public service tagged `container.private` moves to
//! `.container.private.<id>`, and its id becomes a deprecated public
//! alias; references to it are updated.
//!
//! Upstream: symfony/dependency-injection Compiler/AliasDeprecatedPublicServicesPass.php

use php_value::{Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

/// `AliasDeprecatedPublicServicesPass`.
#[derive(Debug)]
pub struct AliasDeprecatedPublicServicesPass {
    state: RecursivePassState,
    aliases: IdMap<String>,
}

impl Default for AliasDeprecatedPublicServicesPass {
    fn default() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            aliases: IdMap::new(),
        }
    }
}

impl AliasDeprecatedPublicServicesPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\AliasDeprecatedPublicServicesPass";

    pub fn new() -> Self {
        Self::default()
    }
}

/// `$tags[0][$name] ?? null`, as a string.
fn attribute(tags: &php_value::PhpArray, name: &str) -> Option<String> {
    let value = tags
        .get(&Key::Int(0))
        .and_then(Value::as_array)
        .and_then(|t| t.get(&Key::from(name)))?;
    if value.is_null() {
        return None;
    }
    Some(
        value
            .to_php_string()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    )
}

impl CompilerPassInterface for AliasDeprecatedPublicServicesPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let tagged = container.find_tagged_service_ids("container.private", false)?;
        for (id, tags) in tagged.iter() {
            let Some(package) = attribute(tags, "package") else {
                return Err(Error::invalid_argument(format!(
                    "The \"package\" attribute is mandatory for the \"container.private\" tag on the \"{id}\" service."
                )));
            };

            let Some(version) = attribute(tags, "version") else {
                return Err(Error::invalid_argument(format!(
                    "The \"version\" attribute is mandatory for the \"container.private\" tag on the \"{id}\" service."
                )));
            };

            let definition = container.get_definition(id)?;
            if definition.is_private() {
                continue;
            }

            let alias_id = format!(".container.private.{id}");
            container
                .set_alias(id, alias_id.as_str())?
                .set_public(true)
                .set_deprecated(
                    &package,
                    &version,
                    "Accessing the \"%alias_id%\" service directly from the container is deprecated, use dependency injection instead.",
                )?;

            container.set_definition(&alias_id, definition)?;

            self.aliases.insert(id, alias_id);
        }

        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for AliasDeprecatedPublicServicesPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Some(reference) = Reference::from_value(&value)
            && let Some(alias_id) = self.aliases.get(reference.id())
        {
            return Ok(
                Reference::with_behavior(alias_id.as_str(), reference.invalid_behavior()).into(),
            );
        }
        self.parent_process_value(container, value, is_root)
    }
}
