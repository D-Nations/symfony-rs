//! Port of Symfony\Component\DependencyInjection\Compiler\RegisterEnvVarProcessorsPass.php:
//! creates the `container.env_var_processors_locator` service and
//! records the types each env var prefix provides.
//!
//! A processor's static `getProvidedTypes()` is not run: its result is the
//! method's [`MethodMetadata::returns`](crate::class_metadata::MethodMetadata::returns)
//! in the class metadata, and a class without one is refused.
//!
//! Upstream: symfony/dependency-injection Compiler/RegisterEnvVarProcessorsPass.php

use php_value::{PhpArray, Value};

use super::compiler_pass_interface::CompilerPassInterface;
use super::service_locator_tag_pass::ServiceLocatorTagPass;
use crate::container_builder::ContainerBuilder;
use crate::env_var_processor::PROVIDED_TYPES;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;
use crate::reference::Reference;

const ALLOWED_TYPES: [&str; 6] = ["array", "bool", "float", "int", "string", "BackedEnum"];

/// `EnvVarProcessorInterface::class`.
const ENV_VAR_PROCESSOR_INTERFACE: &str =
    "Symfony\\Component\\DependencyInjection\\EnvVarProcessorInterface";

/// `EnvVarProcessor::class`.
const ENV_VAR_PROCESSOR: &str = "Symfony\\Component\\DependencyInjection\\EnvVarProcessor";

/// `RegisterEnvVarProcessorsPass`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegisterEnvVarProcessorsPass;

fn to_string(value: &Value) -> String {
    value
        .to_php_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl CompilerPassInterface for RegisterEnvVarProcessorsPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\RegisterEnvVarProcessorsPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let mut types: IdMap<Vec<String>> = IdMap::new();
        let mut processors = PhpArray::new();
        let tagged = container.find_tagged_service_ids("container.env_var_processor", false)?;
        for (id, _tags) in tagged.iter() {
            let class = container.get_definition(id)?.class();
            let registry = container.class_registry();
            let Some(r) = container.get_reflection_class(class.as_deref(), true)? else {
                return Err(Error::invalid_argument(format!(
                    "Class \"{}\" used for service \"{id}\" cannot be found.",
                    class.unwrap_or_default()
                )));
            };
            if r.name.eq_ignore_ascii_case(ENV_VAR_PROCESSOR_INTERFACE)
                || !registry.is_a(&r.name, ENV_VAR_PROCESSOR_INTERFACE)
            {
                return Err(Error::invalid_argument(format!(
                    "Service \"{id}\" must implement interface \"{ENV_VAR_PROCESSOR_INTERFACE}\"."
                )));
            }
            let class = class.unwrap_or_default();
            let provided = registry
                .find_method(&r, "getProvidedTypes")
                .and_then(|(_, m)| m.returns);
            let Some(Value::Array(provided)) = provided else {
                return Err(Error::logic(format!(
                    "Calling \"{class}::getProvidedTypes()\" needs PHP code to run, which this build-time port does not do; the class metadata must give its return value."
                )));
            };
            for (prefix, type_) in provided.iter() {
                let prefix = String::from_utf8_lossy(&prefix.bytes()).into_owned();
                processors.insert(prefix.as_str(), Value::from(Reference::new(id)));
                types.insert(prefix, validate_provided_types(&to_string(type_), &class)?);
            }
        }

        let bag = container.get_parameter_bag();
        if bag.is_env_placeholder() {
            for (prefix, type_) in PROVIDED_TYPES {
                if !types.contains_key(prefix) {
                    types.insert(*prefix, validate_provided_types(type_, ENV_VAR_PROCESSOR)?);
                }
            }
            bag.set_provided_types(types);
        }

        if !processors.is_empty() {
            let locator = ServiceLocatorTagPass::register(container, &processors, None)?;
            container
                .set_alias("container.env_var_processors_locator", locator.id())?
                .set_public(true);
        }
        Ok(())
    }
}

/// `validateProvidedTypes($types, $class)`.
fn validate_provided_types(types: &str, class: &str) -> Result<Vec<String>> {
    let types: Vec<String> = types.split('|').map(String::from).collect();

    for type_ in &types {
        if !ALLOWED_TYPES.contains(&type_.as_str()) {
            return Err(Error::invalid_argument(format!(
                "Invalid type \"{type_}\" returned by \"{class}::getProvidedTypes()\", expected one of \"{}\".",
                ALLOWED_TYPES.join("\", \"")
            )));
        }
    }

    Ok(types)
}
