//! Port of Symfony\Component\DependencyInjection\Compiler\ResettableServicePass.php:
//! gives the `services_resetter` the services tagged `kernel.reset`.
//!
//! Upstream: symfony/dependency-injection Compiler/ResettableServicePass.php

use php_value::{PhpArray, Value, array::Key};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::IteratorArgument;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `ResettableServicePass`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResettableServicePass;

impl CompilerPassInterface for ResettableServicePass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResettableServicePass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        if !container.has("services_resetter") {
            return Ok(());
        }

        let mut services = PhpArray::new();
        let mut methods = PhpArray::new();
        let mut has_non_shared = false;

        let tagged = container.find_tagged_service_ids("kernel.reset", true)?;
        for (id, tags) in tagged.iter() {
            let def = container.get_definition(id)?;
            let is_shared = def.is_shared();

            if is_shared {
                services.insert(
                    id,
                    Value::from(Reference::with_behavior(
                        id,
                        InvalidBehavior::IgnoreUninitialized,
                    )),
                );
            }

            for attributes in tags.values() {
                let attributes = match attributes {
                    Value::Array(a) => a.clone(),
                    _ => PhpArray::new(),
                };
                let Some(method) = attributes
                    .get(&Key::from("method"))
                    .filter(|v| !v.is_null())
                else {
                    return Err(Error::runtime(format!(
                        "Tag \"kernel.reset\" requires the \"method\" attribute to be set on service \"{id}\"."
                    )));
                };
                let mut method = method.clone();

                let on_invalid = attributes.get(&Key::from("on_invalid"));
                if on_invalid
                    .is_some_and(|v| matches!(v, Value::String(s) if s.as_bytes() == b"ignore"))
                {
                    let name = method
                        .to_php_string()
                        .map(|s| s.to_string_lossy().into_owned());
                    method = Value::from(format!("?{}", name.unwrap_or_default()));
                }

                if is_shared {
                    let list = match methods.get(&Key::from(id)) {
                        Some(Value::Array(a)) => a.clone(),
                        _ => PhpArray::new(),
                    };
                    let mut list = list;
                    let _ = list.push(method);
                    methods.insert(id, Value::Array(list));
                } else {
                    has_non_shared = true;
                    let mut tag = PhpArray::new();
                    tag.insert("method", method);
                    def.add_tag("container.tracked_for_reset", tag);
                }
            }
        }

        if services.is_empty() && !has_non_shared {
            container.remove_alias("services_resetter");
            container.remove_definition("services_resetter");

            return Ok(());
        }

        let resetter = container.find_definition("services_resetter")?;
        resetter
            .set_argument(0, IteratorArgument::new(services).to_value())
            .set_argument(1, Value::Array(methods));

        if has_non_shared {
            let map = Definition::new(Some("WeakMap"), PhpArray::new());
            map.set_factory(Value::Array(PhpArray::list([
                Value::from(Reference::new("service_container")),
                Value::from("getResetMap"),
            ])));
            resetter.set_argument(2, map.to_value());
        }
        Ok(())
    }
}
