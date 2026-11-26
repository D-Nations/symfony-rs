//! Port of Symfony\Component\DependencyInjection\Compiler\ValidateEnvPlaceholdersPass.php:
//! validates environment variable placeholders used in extension
//! configuration with dummy values.
//!
//! An extension takes part through
//! [`ExtensionInterface::get_configuration`](crate::extension::ExtensionInterface::get_configuration);
//! one that returns no configuration is skipped before its config is
//! resolved (upstream resolves it first). Errors of symfony-config keep
//! their class name and message.
//!
//! Upstream: symfony/dependency-injection Compiler/ValidateEnvPlaceholdersPass.php

use php_value::{PhpArray, Value};
use symfony_config::definition::{Processor, set_placeholder};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::container_builder::ContainerBuilder;
use crate::exception::{Error, ErrorKind, Result};
use crate::id_map::IdMap;
use crate::parameter_bag::ParameterBag;

/// `ValidateEnvPlaceholdersPass`.
#[derive(Debug, Clone, Default)]
pub struct ValidateEnvPlaceholdersPass {
    extension_config: IdMap<PhpArray>,
}

/// `self::TYPE_FIXTURES[$type] ?? null`.
fn type_fixture(type_: &str) -> Value {
    match type_ {
        "array" => Value::Array(PhpArray::new()),
        "bool" => Value::Bool(false),
        "float" => Value::Float(0.0),
        "int" => Value::Int(0),
        "string" => Value::from(""),
        _ => Value::Null,
    }
}

/// A symfony-config error, with its class and message.
pub(crate) fn config_error(e: &symfony_config::definition::Error) -> Error {
    Error::new(
        ErrorKind::Other(e.class().name().to_string()),
        String::from_utf8_lossy(e.get_message()).into_owned(),
    )
}

impl ValidateEnvPlaceholdersPass {
    pub fn new() -> Self {
        Self::default()
    }

    /// `getExtensionConfig()`: the processed configs by extension name,
    /// which it then forgets.
    pub fn get_extension_config(&mut self) -> IdMap<PhpArray> {
        std::mem::take(&mut self.extension_config)
    }

    /// `getPlaceholderValues($env, $defaultBag, $envTypes)`.
    fn get_placeholder_values(
        env: &str,
        default_bag: &ParameterBag,
        env_types: &IdMap<Vec<String>>,
    ) -> Result<PhpArray> {
        let Some(i) = env.find(':') else {
            let (default, default_type) =
                Self::get_parameter_default_and_default_type(&format!("env({env})"), default_bag)?;
            let mut values = PhpArray::new();
            values.insert(default_type.as_str(), default);
            return Ok(values);
        };

        let prefix = &env[..i];
        if prefix == "default" {
            let mut parts: Vec<&str> = env.split(':').collect();
            parts.remove(0); // Remove 'default' prefix
            let parameter = if parts.is_empty() {
                ""
            } else {
                parts.remove(0) // Retrieve and remove parameter
            };

            let (default_parameter, default_parameter_type) =
                Self::get_parameter_default_and_default_type(parameter, default_bag)?;

            let mut values = PhpArray::new();
            values.insert(default_parameter_type.as_str(), default_parameter);
            for (k, v) in
                Self::get_placeholder_values(&parts.join(":"), default_bag, env_types)?.iter()
            {
                values.insert(k.clone(), v.clone());
            }
            return Ok(values);
        }

        let mut values = PhpArray::new();
        let string = vec!["string".to_string()];
        for type_ in env_types.get(prefix).unwrap_or(&string) {
            values.insert(type_.as_str(), type_fixture(type_));
        }

        Ok(values)
    }

    /// `getParameterDefaultAndDefaultType($name, $defaultBag)`.
    fn get_parameter_default_and_default_type(
        name: &str,
        default_bag: &ParameterBag,
    ) -> Result<(Value, String)> {
        let default = if default_bag.has(name) {
            default_bag.get(name)?
        } else {
            type_fixture("string")
        };
        let default_type = if default.is_null() {
            "string".to_string()
        } else {
            default.debug_type()
        };

        Ok((default, default_type))
    }
}

impl CompilerPassInterface for ValidateEnvPlaceholdersPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\ValidateEnvPlaceholdersPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        self.extension_config = IdMap::new();

        let extensions: Vec<(String, _)> = container
            .get_extensions()
            .iter()
            .map(|(name, e)| (name.to_string(), e.clone()))
            .collect();
        if extensions.is_empty() {
            return Ok(());
        }

        let resolving_bag = container.get_parameter_bag();
        if !resolving_bag.is_env_placeholder() {
            return Ok(());
        }

        let default_bag = ParameterBag::new(resolving_bag.all())?;
        let env_types = resolving_bag.get_provided_types();
        let mut all = resolving_bag.get_env_placeholders();
        all.union(&resolving_bag.get_unused_env_placeholders());
        for (env, placeholders) in all.iter() {
            let values = Self::get_placeholder_values(env, &default_bag, &env_types)?;

            for placeholder in placeholders {
                set_placeholder(placeholder.as_bytes(), values.clone())
                    .map_err(|e| config_error(&e))?;
            }
        }

        let processor = Processor::new();

        for (name, extension) in extensions {
            let config: Vec<PhpArray> = container
                .get_extension_config(&name)
                .into_iter()
                .filter(|c| !c.is_empty())
                .collect();
            if config.is_empty() {
                // this extension has no semantic configuration or was not called
                continue;
            }

            let resolved = container.get_parameter_bag().resolve_value(
                &Value::Array(PhpArray::list(config.into_iter().map(Value::Array))),
                &[],
            )?;
            let resolved = match resolved {
                Value::Array(a) => a,
                _ => PhpArray::new(),
            };
            let configs: Vec<PhpArray> = resolved
                .values()
                .map(|v| match v {
                    Value::Array(a) => a.clone(),
                    _ => PhpArray::new(),
                })
                .collect();

            let Some(configuration) = extension.get_configuration(&configs, container) else {
                continue;
            };

            let processed = processor
                .process_configuration(configuration.as_ref(), &resolved)
                .map_err(|e| config_error(&e))?;
            self.extension_config.insert(name, processed);
        }

        container
            .get_parameter_bag()
            .clear_unused_env_placeholders();
        Ok(())
    }
}
