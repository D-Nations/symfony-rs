//! Port of Symfony\Component\DependencyInjection\Compiler\AutoAliasServicePass.php:
//! sets a service to be an alias of another one, given a format pattern.
//!
//! Upstream: symfony/dependency-injection Compiler/AutoAliasServicePass.php

use php_value::{Value, array::Key};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::alias::Alias;
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, Result};

/// `AutoAliasServicePass`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AutoAliasServicePass;

impl CompilerPassInterface for AutoAliasServicePass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\AutoAliasServicePass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let tagged = container.find_tagged_service_ids("auto_alias", false)?;
        for (service_id, tags) in tagged.iter() {
            for tag in tags.values() {
                let format = match tag {
                    Value::Array(tag) => tag.get(&Key::from("format")).filter(|v| !v.is_null()),
                    _ => None,
                };
                let Some(format) = format else {
                    return Err(Error::invalid_argument(format!(
                        "Missing tag information \"format\" on auto_alias service \"{service_id}\"."
                    )));
                };

                let alias_id = container.get_parameter_bag().resolve_value(format, &[])?;
                let alias_id = alias_id
                    .to_php_string()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if container.has_definition(&alias_id) || container.has_alias(&alias_id) {
                    let alias =
                        Alias::new(alias_id, container.get_definition(service_id)?.is_public());
                    container.set_alias(service_id, alias)?;
                }
            }
        }
        Ok(())
    }
}
