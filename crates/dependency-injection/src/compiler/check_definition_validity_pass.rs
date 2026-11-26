//! Port of Symfony\Component\DependencyInjection\Compiler\CheckDefinitionValidityPass.php.
//!
//! Validates each definition on its own: synthetic services are public,
//! other non-abstract services have a class, tag attributes are scalars
//! and public ids hold no env placeholder. `class_exists()` and
//! `interface_exists()` read the container's class registry.
//!
//! Upstream: symfony/dependency-injection Compiler/CheckDefinitionValidityPass.php

use php_value::{PhpArray, Value};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::class_metadata::ClassKind;
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::definition::Definition;
use crate::exception::{Error, Result};

/// `CheckDefinitionValidityPass`.
#[derive(Debug, Default)]
pub struct CheckDefinitionValidityPass;

impl CheckDefinitionValidityPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\CheckDefinitionValidityPass";
}

/// `preg_match(ContainerBuilder::ANONYMOUS_ID_REGEXP, $id)`:
/// `/^\.\d+_[^~]*+~[._a-zA-Z\d]{7}$/`. The possessive `[^~]*+` stops at
/// the first `~` after the `_`, so what follows it is the whole tail.
pub(crate) fn is_anonymous_id(id: &str) -> bool {
    let Some(rest) = id.strip_prefix('.') else {
        return false;
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return false;
    }
    let Some(rest) = rest[digits..].strip_prefix('_') else {
        return false;
    };
    let Some(at) = rest.find('~') else {
        return false;
    };
    let tail = &rest[at + 1..];
    // `$` also matches before a final newline.
    let tail = tail.strip_suffix('\n').unwrap_or(tail);
    tail.len() == 7
        && tail
            .bytes()
            .all(|b| b == b'.' || b == b'_' || b.is_ascii_alphanumeric())
}

/// `!$value`: null, false, 0, 0.0, "", "0" and [].
fn is_falsy(value: &Value) -> bool {
    !value.is_truthy()
}

fn validate_attributes(id: &str, tag: &str, attributes: &PhpArray, path: &[String]) -> Result<()> {
    for (name, value) in attributes.iter() {
        let name = String::from_utf8_lossy(&name.bytes()).into_owned();
        match value {
            Value::Array(value) => {
                let mut path = path.to_vec();
                path.push(name);
                validate_attributes(id, tag, value, &path)?;
            }
            Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => {}
            Value::Object(_) => {
                let mut path = path.to_vec();
                path.push(name);
                let name = path.join(".");
                return Err(Error::runtime(format!(
                    "A \"tags\" attribute must be of a scalar-type for service \"{id}\", tag \"{tag}\", attribute \"{name}\"."
                )));
            }
        }
    }
    Ok(())
}

impl CompilerPassInterface for CheckDefinitionValidityPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        // `$usedEnvs` is one variable for the whole method.
        let mut used_envs: Vec<String> = Vec::new();
        let definitions: Vec<(String, Definition)> = container
            .get_definitions()
            .iter()
            .map(|(id, definition)| (id.to_string(), definition.clone()))
            .collect();
        for (id, definition) in definitions {
            if definition.has_errors() {
                continue;
            }

            // synthetic service is public
            if definition.is_synthetic() && !definition.is_public() {
                return Err(Error::runtime(format!(
                    "A synthetic service (\"{id}\") must be public."
                )));
            }

            // non-synthetic, non-abstract service has class
            let has_class = definition
                .class()
                .is_some_and(|c| !c.is_empty() && c != "0");
            let has_factory = !is_falsy(&definition.factory());
            if !definition.is_abstract()
                && !definition.is_synthetic()
                && !has_class
                && !definition.has_tag("container.service_locator")
                && (!has_factory || !is_anonymous_id(&id))
            {
                if has_factory {
                    return Err(Error::runtime(format!(
                        "Please add the class to service \"{id}\" even if it is constructed by a factory since we might need to add method calls based on compile-time checks."
                    )));
                }
                if container
                    .class_registry()
                    .class(&id)
                    .is_some_and(|c| c.kind != ClassKind::Trait)
                {
                    if id.starts_with('\\') && id.matches('\\').count() > 1 {
                        return Err(Error::runtime(format!(
                            "The definition for \"{id}\" has no class attribute, and appears to reference a class or interface. Please specify the class attribute explicitly or remove the leading backslash by renaming the service to \"{}\" to get rid of this error.",
                            &id[1..]
                        )));
                    }
                    return Err(Error::runtime(format!(
                        "The definition for \"{id}\" has no class attribute, and appears to reference a class or interface in the global namespace. Leaving out the \"class\" attribute is only allowed for namespaced classes. Please specify the class attribute explicitly to get rid of this error."
                    )));
                }
                return Err(Error::runtime(format!(
                    "The definition for \"{id}\" has no class. If you intend to inject this service dynamically at runtime, please mark it as synthetic=true. If this is an abstract definition solely used by child definitions, please add abstract=true, otherwise specify a class to get rid of this error."
                )));
            }

            // tag attribute values must be scalars
            for (name, tags) in definition.tags().iter() {
                let name = String::from_utf8_lossy(&name.bytes()).into_owned();
                for attributes in tags.as_array().into_iter().flat_map(PhpArray::values) {
                    if let Some(attributes) = attributes.as_array() {
                        validate_attributes(&id, &name, attributes, &[])?;
                    }
                }
            }

            if definition.is_public() {
                check_dynamic(
                    container,
                    &id,
                    &mut used_envs,
                    "A service name (\"%s\") cannot contain dynamic values.",
                )?;
            }
        }

        let aliases: Vec<(String, bool)> = container
            .get_aliases()
            .iter()
            .map(|(id, alias)| (id.to_string(), alias.is_public()))
            .collect();
        for (id, public) in aliases {
            if public {
                check_dynamic(
                    container,
                    &id,
                    &mut used_envs,
                    "An alias name (\"%s\") cannot contain dynamic values.",
                )?;
            }
        }
        Ok(())
    }
}

fn check_dynamic(
    container: &mut ContainerBuilder,
    id: &str,
    used_envs: &mut Vec<String>,
    message: &str,
) -> Result<()> {
    let resolved = container.resolve_env_placeholders(
        Value::from(id),
        &EnvFormat::Placeholder,
        Some(used_envs),
    )?;
    if !used_envs.is_empty() {
        let resolved = resolved
            .to_php_string()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Err(Error::env_parameter(&[resolved], Some(message)));
    }
    Ok(())
}
