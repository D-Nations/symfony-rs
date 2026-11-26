//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveEnvPlaceholdersPass.php.
//!
//! Replaces env var placeholders by their values, by `%env(NAME)%` or by
//! a `sprintf()` format of their names ([`EnvFormat`]).
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveEnvPlaceholdersPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::EnvClosureArgument;
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::definition::Definition;
use crate::exception::Result;

/// `ResolveEnvPlaceholdersPass`.
#[derive(Debug, Clone)]
pub struct ResolveEnvPlaceholdersPass {
    state: RecursivePassState,
    format: EnvFormat,
}

impl Default for ResolveEnvPlaceholdersPass {
    fn default() -> Self {
        Self::new(EnvFormat::Resolve)
    }
}

/// `(string)` of a resolved class or file name; `null` stays `None`.
fn string_of(value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }
    value
        .to_php_string_or_stringable()
        .map(|s| s.to_string_lossy().into_owned())
}

/// The key `array_combine()` gives a value.
fn combine_key(value: &Value) -> Key {
    match value {
        Value::Int(i) => Key::Int(*i),
        other => Key::from(
            other
                .to_php_string_or_stringable()
                .unwrap_or_else(|| php_value::PhpString::from("Array")),
        ),
    }
}

impl ResolveEnvPlaceholdersPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveEnvPlaceholdersPass";

    /// `new ResolveEnvPlaceholdersPass($format)`.
    pub fn new(format: EnvFormat) -> Self {
        Self {
            state: RecursivePassState::default(),
            format,
        }
    }

    fn resolve(&self, container: &mut ContainerBuilder, value: Value) -> Result<Value> {
        container.resolve_env_placeholders(value, &self.format, None)
    }
}

impl CompilerPassInterface for ResolveEnvPlaceholdersPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for ResolveEnvPlaceholdersPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Value::String(_) = &value {
            return self.resolve(container, value);
        }
        if let Some(argument) = EnvClosureArgument::from_value(&value) {
            // the closure reads the env var at runtime, so only its expression is formatted
            if self.format == EnvFormat::Resolve {
                return Ok(value);
            }
            let resolved = self.resolve(container, Value::from(argument.value()))?;
            let resolved = string_of(&resolved).unwrap_or_default();
            return Ok(EnvClosureArgument::new(
                resolved,
                argument.default_value().clone(),
                argument.is_stringable(),
            )?
            .into());
        }
        if let Some(definition) = Definition::from_value(&value) {
            let changes = definition.changes();
            if changes.iter().any(|c| c == "class") {
                let class = definition
                    .class()
                    .map_or(Value::Null, |c| Value::from(c.as_str()));
                let class = self.resolve(container, class)?;
                definition.set_class(string_of(&class).as_deref());
            }
            if changes.iter().any(|c| c == "file") {
                let file = definition
                    .file()
                    .map_or(Value::Null, |c| Value::from(c.as_str()));
                let file = self.resolve(container, file)?;
                definition.set_file(string_of(&file).as_deref());
            }
        }

        let value = self.parent_process_value(container, value, is_root)?;

        if let Value::Array(array) = &value
            && !array.is_empty()
            && !is_root
        {
            let keys = Value::Array(PhpArray::list(array.keys().map(Key::to_value)));
            let keys = self.resolve(container, keys)?;
            let mut out = PhpArray::new();
            if let Some(keys) = keys.as_array() {
                for (key, v) in keys.values().zip(array.values()) {
                    out.insert(combine_key(key), v.clone());
                }
            }
            return Ok(Value::Array(out));
        }

        Ok(value)
    }
}
