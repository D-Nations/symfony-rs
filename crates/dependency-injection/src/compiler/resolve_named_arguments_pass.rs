//! Port of Symfony\Component\DependencyInjection\Compiler\ResolveNamedArgumentsPass.php.
//!
//! Resolves named (`$name`) and typed (`Foo\Bar`) arguments of
//! constructors, factories and method calls to their position, and
//! gives `AbstractArgument`s their context. Parameters and their types
//! come from the class metadata (`getReflectionMethod()`).
//!
//! Upstream: symfony/dependency-injection Compiler/ResolveNamedArgumentsPass.php

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use super::export_type::{DeclaringClass, export_type};
use crate::argument::AbstractArgument;
use crate::class_metadata::ClassKind;
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `ResolveNamedArgumentsPass`.
#[derive(Debug, Clone)]
pub struct ResolveNamedArgumentsPass {
    state: RecursivePassState,
}

impl Default for ResolveNamedArgumentsPass {
    fn default() -> Self {
        Self::new()
    }
}

/// `$value instanceof AbstractArgument && $value->getText().'.' === $value->getTextWithContext()`.
fn context_free_abstract(value: &Value) -> Option<AbstractArgument> {
    AbstractArgument::from_value(value)
        .filter(|a| format!("{}.", a.text()) == a.text_with_context())
}

fn key_string(key: &Key) -> String {
    String::from_utf8_lossy(&key.bytes()).into_owned()
}

impl ResolveNamedArgumentsPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\ResolveNamedArgumentsPass";

    pub fn new() -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self { state }
    }

    fn current_id(&self) -> String {
        self.state.current_id.clone().unwrap_or_default()
    }

    /// The body of the `foreach ($calls as $i => $call)` loop: the
    /// resolved arguments of one call, when they changed.
    #[expect(clippy::too_many_lines, reason = "one upstream loop")]
    fn resolve_call(
        &mut self,
        container: &mut ContainerBuilder,
        value: &Definition,
        method: &str,
        arguments: &PhpArray,
    ) -> Result<Option<PhpArray>> {
        let current_id = self.current_id();
        let mut method = method.to_string();
        let mut parameters = None;
        let mut class = String::new();
        let mut declaring: (Option<String>, Option<String>) = (None, None);
        let mut resolved_keys = PhpArray::new();
        let mut resolved_arguments = PhpArray::new();

        for (key, argument) in arguments.iter() {
            if let Some(abstract_argument) = context_free_abstract(argument) {
                let which = match key {
                    Key::Int(i) => (1 + i).to_string(),
                    Key::Str(_) => format!("\"{}\"", key_string(key)),
                };
                let of = if method == "__construct" {
                    format!("service \"{current_id}\"")
                } else {
                    format!("method call \"{current_id}::{method}()\"")
                };
                abstract_argument.set_context(&format!("Argument {which} of {of}"));
            }

            let Key::Str(name) = key else {
                resolved_keys.insert(key.clone(), key.to_value());
                resolved_arguments.insert(key.clone(), argument.clone());
                continue;
            };
            let key_name = name.to_string_lossy().into_owned();

            if parameters.is_none() {
                let r = self.get_reflection_method(container, value, &method)?;
                class = if r.is_method() {
                    r.class().unwrap_or_default().to_string()
                } else {
                    current_id.clone()
                };
                if let Some(declaring_class) = r.class() {
                    let parent = container
                        .class_registry()
                        .class(declaring_class)
                        .and_then(|c| c.parent.clone());
                    declaring = (Some(declaring_class.to_string()), parent);
                }
                method = r.name().to_string();
                parameters = Some(r.parameters());
            }
            let parameters = parameters.as_deref().unwrap_or_default();
            let named = if class == current_id {
                method.clone()
            } else {
                format!("{class}::{method}")
            };

            if !key_name.is_empty()
                && !key_name.starts_with('$')
                && !container
                    .class_registry()
                    .reflect(&key_name)
                    .ok()
                    .flatten()
                    .is_some_and(|c| c.kind != ClassKind::Trait)
            {
                return Err(Error::invalid_argument(format!(
                    "Invalid service \"{current_id}\": did you forget to add the \"$\" prefix to argument \"{key_name}\"?"
                )));
            }

            if key_name.starts_with('$') {
                let found = parameters.iter().position(|p| key_name[1..] == p.name);
                let Some(mut j) = found else {
                    return Err(Error::invalid_argument(format!(
                        "Invalid service \"{current_id}\": method \"{named}()\" has no argument named \"{key_name}\". Check your service definition."
                    )));
                };
                let p = &parameters[j];
                if p.is_variadic
                    && let Value::Array(variadics) = argument
                {
                    for variadic_argument in variadics.values() {
                        let at = i64::try_from(j).unwrap_or(i64::MAX);
                        resolved_keys.insert(Key::Int(at), Value::Int(at));
                        resolved_arguments.insert(Key::Int(at), variadic_argument.clone());
                        j += 1;
                    }
                } else {
                    let at = Key::Int(i64::try_from(j).unwrap_or(i64::MAX));
                    resolved_keys.insert(at.clone(), Value::from(p.name.as_str()));
                    resolved_arguments.insert(at, argument.clone());
                }
                continue;
            }

            if !argument.is_null()
                && Reference::from_value(argument).is_none()
                && Definition::from_value(argument).is_none()
            {
                return Err(Error::invalid_argument(format!(
                    "Invalid service \"{current_id}\": the value of argument \"{key_name}\" of method \"{named}()\" must be null, an instance of \"{}\" or an instance of \"{}\", \"{}\" given.",
                    Reference::CLASS,
                    Definition::CLASS,
                    argument.debug_type()
                )));
            }

            let class_info = DeclaringClass {
                name: declaring.0.as_deref(),
                parent: declaring.1.as_deref(),
            };
            let mut type_found = false;
            for (j, p) in parameters.iter().enumerate() {
                let at = Key::Int(i64::try_from(j).unwrap_or(i64::MAX));
                if !resolved_arguments.contains_key(&at)
                    && export_type(p, true, class_info).as_deref() == Some(key_name.as_str())
                {
                    resolved_keys.insert(at.clone(), Value::from(p.name.as_str()));
                    resolved_arguments.insert(at, argument.clone());
                    type_found = true;
                }
            }

            if !type_found {
                return Err(Error::invalid_argument(format!(
                    "Invalid service \"{current_id}\": method \"{named}()\" has no argument type-hinted as \"{key_name}\". Check your service definition."
                )));
            }
        }

        if resolved_arguments == *arguments {
            return Ok(None);
        }
        resolved_arguments.ksort();

        if !value.is_autowired() && !resolved_arguments.is_list() {
            resolved_keys.ksort();
            let mut combined = PhpArray::new();
            for (k, v) in resolved_keys.values().zip(resolved_arguments.values()) {
                let k = match k {
                    Value::Int(i) => Key::Int(*i),
                    other => Key::from(other.to_php_string().unwrap_or_default()),
                };
                combined.insert(k, v.clone());
            }
            resolved_arguments = combined;
        }

        Ok(Some(resolved_arguments))
    }
}

impl CompilerPassInterface for ResolveNamedArgumentsPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for ResolveNamedArgumentsPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Some(abstract_argument) = context_free_abstract(&value) {
            abstract_argument.set_context(&format!(
                "A value found in service \"{}\"",
                self.current_id()
            ));
        }

        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };

        let original_calls = definition.method_calls();
        let mut calls = original_calls.clone();
        let constructor_key = calls
            .push(Value::Array(PhpArray::list([
                Value::from("__construct"),
                Value::Array(definition.arguments()),
            ])))
            .ok();

        let entries: Vec<(Key, Value)> =
            calls.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (i, call) in entries {
            let Some(call_array) = call.as_array() else {
                continue;
            };
            let method = call_array
                .get(&Key::Int(0))
                .and_then(Value::to_php_string)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let arguments = call_array
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(resolved) =
                self.resolve_call(container, &definition, &method, &arguments)?
            {
                let mut call_array = call_array.clone();
                call_array.insert(Key::Int(1), Value::Array(resolved));
                calls.insert(i, Value::Array(call_array));
            }
        }

        let arguments = constructor_key
            .and_then(|k| calls.remove(&k))
            .and_then(|c| c.as_array().and_then(|c| c.get(&Key::Int(1))).cloned())
            .and_then(|a| a.as_array().cloned())
            .unwrap_or_default();

        if arguments != definition.arguments() {
            definition.set_arguments(arguments);
        }
        if calls != original_calls {
            definition.set_method_calls(&calls)?;
        }

        for (key, argument) in definition.properties().iter() {
            if let Some(abstract_argument) = context_free_abstract(argument) {
                abstract_argument.set_context(&format!(
                    "Property \"{}\" of service \"{}\"",
                    key_string(key),
                    self.current_id()
                ));
            }
        }

        self.parent_process_value(container, value, is_root)
    }
}
