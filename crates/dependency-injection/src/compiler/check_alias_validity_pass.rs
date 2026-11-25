//! Port of Symfony\Component\DependencyInjection\Compiler\CheckAliasValidityPass.php.
//!
//! An alias named after an interface must resolve to a service that
//! implements it, so injecting the aliased interface cannot cause a type
//! error at run time. Classes come from the container's class registry;
//! a `ReflectionException` (a missing ancestor) skips the alias.
//!
//! Upstream: symfony/dependency-injection Compiler/CheckAliasValidityPass.php

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, Result};

/// `CheckAliasValidityPass`.
#[derive(Debug, Default)]
pub struct CheckAliasValidityPass;

impl CheckAliasValidityPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\CheckAliasValidityPass";
}

/// The `try` body for one alias; an early `Ok(())` is a `continue`.
fn check(container: &mut ContainerBuilder, id: &str, target_id: &str) -> Result<()> {
    let target = container.get_definition(target_id)?;
    let Some(target_class) = target.class() else {
        return Ok(());
    };
    if !target.factory().is_null() {
        return Ok(());
    }

    let Some(reflection) = container.get_reflection_class(Some(id), true)? else {
        return Ok(());
    };
    if !reflection.is_interface() {
        return Ok(());
    }

    let target_reflection = container.get_reflection_class(Some(&target_class), true)?;
    if let Some(target_reflection) = target_reflection
        && !container.class_registry().is_a(&target_reflection.name, id)
    {
        return Err(Error::runtime(format!(
            "Invalid alias definition: alias \"{id}\" is referencing class \"{target_class}\" but this class does not implement \"{id}\". Because this alias is an interface, \"{target_class}\" must implement \"{id}\"."
        )));
    }
    Ok(())
}

impl CompilerPassInterface for CheckAliasValidityPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let aliases: Vec<(String, crate::alias::Alias)> = container
            .get_aliases()
            .iter()
            .map(|(id, alias)| (id.to_string(), alias.clone()))
            .collect();
        for (id, alias) in aliases {
            if alias.is_deprecated() || !container.has_definition(&alias.id()) {
                continue;
            }
            match check(container, &id, &alias.id()) {
                Err(e) if e.instance_of("ReflectionException") => {}
                result => result?,
            }
        }
        Ok(())
    }
}
