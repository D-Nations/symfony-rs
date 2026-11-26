//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveParameterPlaceHoldersPass.php.
//!
//! Resolves `%parameter%` placeholders in definitions (class, file,
//! factory, arguments, properties, method calls, bindings, `proxy` tags
//! and array keys) and in alias ids, then resolves the parameter bag.
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveParameterPlaceHoldersPass.php

use php_value::{PhpArray, PhpString, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::alias::Alias;
use crate::container_builder::ContainerBuilder;
use crate::definition::{Definition, DefinitionError};
use crate::exception::{ErrorKind, Result};

/// `ResolveParameterPlaceHoldersPass`.
#[derive(Debug, Clone)]
pub struct ResolveParameterPlaceHoldersPass {
    state: RecursivePassState,
    resolve_arrays: bool,
    throw_on_resolve_exception: bool,
}

impl Default for ResolveParameterPlaceHoldersPass {
    fn default() -> Self {
        Self::new(true, true)
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

/// The key `array_combine()` gives a value: ints stay ints, anything
/// else is cast to string (an array is `"Array"`), then numeric strings
/// become ints.
fn combine_key(value: &Value) -> Key {
    match value {
        Value::Int(i) => Key::Int(*i),
        other => Key::from(
            other
                .to_php_string_or_stringable()
                .unwrap_or_else(|| PhpString::from("Array")),
        ),
    }
}

/// `array_combine($keys, $values)` where `$keys` is a list as long as
/// `values`.
fn array_combine(keys: &PhpArray, values: &PhpArray) -> PhpArray {
    let mut out = PhpArray::new();
    for (key, value) in keys.values().zip(values.values()) {
        out.insert(combine_key(key), value.clone());
    }
    out
}

/// `array_keys($array)`.
fn array_keys(array: &PhpArray) -> PhpArray {
    PhpArray::list(array.keys().map(Key::to_value))
}

impl ResolveParameterPlaceHoldersPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveParameterPlaceHoldersPass";

    /// `new ResolveParameterPlaceHoldersPass($resolveArrays, $throwOnResolveException)`.
    pub fn new(resolve_arrays: bool, throw_on_resolve_exception: bool) -> Self {
        Self {
            state: RecursivePassState::default(),
            resolve_arrays,
            throw_on_resolve_exception,
        }
    }

    fn resolve_aliases(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)?;

        let mut aliases: Vec<(String, Alias)> = Vec::new();
        let current: Vec<(String, Alias)> = container
            .get_aliases()
            .iter()
            .map(|(name, alias)| (name.to_string(), alias.clone()))
            .collect();
        for (name, target) in current {
            self.state.current_id = Some(name.clone());
            let resolved = container
                .get_parameter_bag()
                .resolve_value(&Value::from(name.as_str()), &[])?;
            let key = combine_key(&resolved);
            let key = String::from_utf8_lossy(&key.bytes()).into_owned();
            if let Some(entry) = aliases.iter_mut().find(|(k, _)| *k == key) {
                entry.1 = target;
            } else {
                aliases.push((key, target));
            }
        }
        container.set_aliases(aliases)
    }
}

impl CompilerPassInterface for ResolveParameterPlaceHoldersPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        if let Err(mut e) = self.resolve_aliases(container) {
            if e.is(&ErrorKind::ParameterNotFound) {
                let source = self.state.current_id.clone();
                e.update_parameter_not_found(|f| f.source_id = source);
            }
            return Err(e);
        }

        container.get_parameter_bag_mut().resolve()
    }
}

impl AbstractRecursivePass for ResolveParameterPlaceHoldersPass {
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
            let v = match container.get_parameter_bag().resolve_value(&value, &[]) {
                Ok(v) => v,
                Err(e) if e.is(&ErrorKind::ParameterNotFound) => {
                    if self.throw_on_resolve_exception {
                        return Err(e);
                    }
                    let current = self.state.current_id.clone().unwrap_or_default();
                    container
                        .get_definition(&current)?
                        .add_error(DefinitionError::Text(e.message()));
                    Value::Null
                }
                Err(e) => return Err(e),
            };

            return Ok(
                if self.resolve_arrays || !v.is_truthy() || !matches!(v, Value::Array(_)) {
                    v
                } else {
                    value
                },
            );
        }
        if let Some(definition) = Definition::from_value(&value) {
            let bindings =
                self.process_value(container, Value::Array(definition.bindings()), false)?;
            definition.set_bindings(bindings.as_array().unwrap_or(&PhpArray::new()));
            let changes = definition.changes();
            let bag = container.get_parameter_bag();
            if changes.iter().any(|c| c == "class") {
                let class = definition
                    .class()
                    .map_or(Value::Null, |c| Value::from(c.as_str()));
                let class = bag.resolve_value(&class, &[])?;
                definition.set_class(string_of(&class).as_deref());
            }
            if changes.iter().any(|c| c == "file") {
                let file = definition
                    .file()
                    .map_or(Value::Null, |c| Value::from(c.as_str()));
                let file = bag.resolve_value(&file, &[])?;
                definition.set_file(string_of(&file).as_deref());
            }
            let mut tags = definition.tags();
            let proxy = Key::from("proxy");
            if let Some(proxy_tags) = tags.get(&proxy).filter(|v| !v.is_null()) {
                let resolved = bag.resolve_value(proxy_tags, &[])?;
                tags.insert(proxy, resolved);
                definition.set_tags(tags);
            }
        }

        let value = self.parent_process_value(container, value, is_root)?;

        if let Value::Array(array) = &value
            && !array.is_empty()
        {
            let keys = container
                .get_parameter_bag()
                .resolve_value(&Value::Array(array_keys(array)), &[])?;
            return Ok(Value::Array(array_combine(
                keys.as_array().unwrap_or(&PhpArray::new()),
                array,
            )));
        }

        Ok(value)
    }
}
