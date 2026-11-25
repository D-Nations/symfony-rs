//! Port of Symfony\Component\DependencyInjection\Compiler\AbstractRecursivePass.php.
//!
//! A recursive pass implements [`AbstractRecursivePass`] (its state in a
//! [`RecursivePassState`]) and its `process()` calls
//! [`recursive_process`], which is upstream's `process()`. A pass that
//! overrides `processValue()` implements
//! [`AbstractRecursivePass::process_value`] and calls
//! [`AbstractRecursivePass::parent_process_value`] for
//! `parent::processValue()`. The container is an argument of each method
//! instead of the `$container` property, which upstream sets only for the
//! time of `process()`.
//!
//! `getConstructor()` and `getReflectionMethod()` read the container's
//! class metadata ([`crate::class_metadata`]) where upstream reflects.
//! The port has no `ExpressionLanguage` and no `Expression` values: a
//! factory written as an expression (`@=...`) fails with upstream's
//! message for a missing `ExpressionLanguage` component.
//!
//! Upstream: symfony/dependency-injection Compiler/AbstractRecursivePass.php

use php_value::{PhpArray, Value, array::Key};

use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::Argument;
use crate::class_metadata::FunctionReflector;
use crate::container_builder::ContainerBuilder;
use crate::definition::Definition;
use crate::exception::{Error, ErrorKind, Result};
use crate::reference::Reference;

/// The properties of `AbstractRecursivePass`.
#[derive(Debug, Clone, Default)]
pub struct RecursivePassState {
    /// `$currentId`: the id of the definition being walked.
    pub current_id: Option<String>,
    /// `$skipScalars`.
    pub skip_scalars: bool,
    process_expressions: bool,
    in_expression: bool,
}

fn key_string(key: &Key) -> String {
    String::from_utf8_lossy(&key.bytes()).into_owned()
}

/// `\is_scalar($v)` or `!$v`.
fn is_scalar_or_falsy(value: &Value) -> bool {
    matches!(
        value,
        Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_)
    ) || !value.is_truthy()
}

/// `$class !== $this->currentId`.
fn differs(class: &str, current_id: Option<&str>) -> bool {
    current_id != Some(class)
}

/// `AbstractRecursivePass::process($container)`: walks the container's
/// definitions (`processValue($container->getDefinitions(), true)`; the
/// array it returns is dropped, as upstream drops it).
pub fn recursive_process<P: AbstractRecursivePass + ?Sized>(
    pass: &mut P,
    container: &mut ContainerBuilder,
) -> Result<()> {
    let mut definitions = PhpArray::new();
    for (id, definition) in container.get_definitions().iter() {
        definitions.insert(id, definition.to_value());
    }
    pass.process_value(container, Value::Array(definitions), true)
        .map(|_| ())
}

/// `AbstractRecursivePass`.
pub trait AbstractRecursivePass: CompilerPassInterface {
    /// The pass's `AbstractRecursivePass` properties.
    fn state(&mut self) -> &mut RecursivePassState;

    /// `processValue($value, $isRoot)`; by default the parent's.
    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        self.parent_process_value(container, value, is_root)
    }

    /// `AbstractRecursivePass::processValue()`: arrays, argument objects
    /// and definitions are walked, calling [`Self::process_value`] on
    /// each value they hold.
    fn parent_process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if let Value::Array(array) = &value {
            let mut out = array.clone();
            for (k, v) in array.iter() {
                if self.state().skip_scalars && is_scalar_or_falsy(v) {
                    continue;
                }
                if is_root {
                    if let Some(definition) = Definition::from_value(v)
                        && definition.has_tag("container.excluded")
                    {
                        continue;
                    }
                    self.state().current_id = Some(key_string(k));
                }
                let processed = self.process_value(container, v.clone(), is_root)?;
                if !v.identical(&processed) {
                    out.insert(k.clone(), processed);
                }
            }
            return Ok(Value::Array(out));
        }
        if let Some(argument) = Argument::from_value(&value) {
            let values = self.process_value(container, Value::Array(argument.values()), false)?;
            argument.set_values(match values {
                Value::Array(a) => a,
                _ => PhpArray::new(),
            })?;
            return Ok(value);
        }
        if let Some(definition) = Definition::from_value(&value) {
            let arguments =
                self.process_value(container, Value::Array(definition.arguments()), false)?;
            definition.set_arguments(into_array(arguments));
            let properties =
                self.process_value(container, Value::Array(definition.properties()), false)?;
            definition.set_properties(into_array(properties));
            let calls =
                self.process_value(container, Value::Array(definition.method_calls()), false)?;
            definition.set_method_calls(&into_array(calls))?;

            let changes = definition.changes();
            if changes.iter().any(|c| c == "factory") {
                let factory = definition.factory();
                if let Value::String(s) = &factory
                    && s.as_bytes().starts_with(b"@=")
                {
                    return Err(Error::logic(
                        "Expressions cannot be used in service factories without the ExpressionLanguage component. Try running \"composer require symfony/expression-language\".",
                    ));
                }
                let factory = self.process_value(container, factory, false)?;
                definition.set_factory(factory);
            }
            if changes.iter().any(|c| c == "configurator") {
                let configurator =
                    self.process_value(container, definition.configurator(), false)?;
                definition.set_configurator(configurator);
            }
        }
        Ok(value)
    }

    /// `enableExpressionProcessing()`.
    fn enable_expression_processing(&mut self) {
        self.state().process_expressions = true;
    }

    /// `inExpression($reset)`.
    fn in_expression(&mut self, reset: bool) -> bool {
        let state = self.state();
        let in_expression = state.in_expression;
        if reset {
            state.in_expression = false;
        }
        in_expression
    }

    /// `getConstructor($definition, $required)`.
    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn get_constructor(
        &mut self,
        container: &mut ContainerBuilder,
        definition: &Definition,
        required: bool,
    ) -> Result<Option<FunctionReflector>> {
        let current_id = self.state().current_id.clone();
        let current = current_id.as_deref().unwrap_or("");
        if definition.is_synthetic() {
            return Ok(None);
        }

        let factory = definition.factory();
        if let Value::String(factory) = &factory {
            let factory = factory.to_string_lossy().into_owned();
            if factory.starts_with("@=") {
                return Ok(Some(FunctionReflector::VariadicClosure {
                    is_method: false,
                }));
            }
            let Some(function) = container.class_registry().function(&factory) else {
                return Err(Error::runtime(format!(
                    "Invalid service \"{current}\": function \"{factory}\" does not exist."
                )));
            };
            if let Some(file) = &function.file
                && php_value::path::to_path(file.as_bytes()).is_some_and(|p| p.exists())
            {
                container.file_exists(file, &crate::container_builder::TrackContents::Yes)?;
            }
            return Ok(Some(FunctionReflector::Function((*function).clone())));
        }

        if factory.is_truthy() {
            let factory = into_array(factory);
            let class = factory.get(&Key::Int(0)).cloned().unwrap_or_default();
            let method = factory
                .get(&Key::Int(1))
                .and_then(Value::to_php_string)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();

            if method == "__construct" {
                return Err(Error::runtime(format!(
                    "Invalid service \"{current}\": \"__construct()\" cannot be used as a factory method."
                )));
            }

            let class = if let Some(reference) = Reference::from_value(&class) {
                if reference.id() == "service_container"
                    && let Some(container_class) = container
                        .class_registry()
                        .class(crate::container_builder::CONTAINER_CLASS)
                    && let Some((declaring, found)) = container
                        .class_registry()
                        .find_method(&container_class, &method)
                {
                    return Ok(Some(FunctionReflector::Method {
                        class: declaring,
                        method: found,
                    }));
                }
                let mut factory_definition = container.find_definition(reference.id())?;
                loop {
                    let class = factory_definition.class();
                    if class.is_some() {
                        break class;
                    }
                    let Some(parent) = factory_definition.parent() else {
                        break None;
                    };
                    factory_definition = container.find_definition(&parent)?;
                }
            } else if let Some(inline) = Definition::from_value(&class) {
                inline.class()
            } else if class.is_null() {
                definition.class()
            } else {
                class
                    .to_php_string()
                    .map(|s| s.to_string_lossy().into_owned())
            };

            let class_definition = Definition::new(class.as_deref(), PhpArray::new());
            return self
                .get_reflection_method(container, &class_definition, &method)
                .map(Some);
        }

        let mut definition = definition.clone();
        let class = loop {
            let class = definition.class();
            if class.is_some() {
                break class;
            }
            let Some(parent) = definition.parent() else {
                break None;
            };
            definition = container.find_definition(&parent)?;
        };

        let reflected = match container.get_reflection_class(class.as_deref(), true) {
            Ok(r) => r,
            Err(e) if e.instance_of("ReflectionException") => {
                return Err(Error::runtime(format!(
                    "Invalid service \"{current}\": {}",
                    lcfirst(&e.message())
                )));
            }
            Err(e) => return Err(e),
        };
        let Some(r) = reflected else {
            let Some(class) = class else {
                return Err(Error::runtime(format!(
                    "Invalid service \"{current}\": the class is not set."
                )));
            };
            return Err(Error::runtime(format!(
                "Invalid service \"{current}\": class \"{class}\" does not exist."
            )));
        };
        let class = class.unwrap_or_default();
        let Some((declaring, constructor)) =
            container.class_registry().find_method(&r, "__construct")
        else {
            if required {
                let named = if differs(&class, current_id.as_deref()) {
                    format!(" \"{class}\"")
                } else {
                    String::new()
                };
                return Err(Error::runtime(format!(
                    "Invalid service \"{current}\": class{named} has no constructor."
                )));
            }
            return Ok(None);
        };
        if !constructor.is_public() {
            let whose = if differs(&class, current_id.as_deref()) {
                format!("constructor of class \"{class}\"")
            } else {
                "its constructor".to_string()
            };
            return Err(Error::runtime(format!(
                "Invalid service \"{current}\": {whose} must be public. Did you miss configuring a factory or a static constructor? Try using the \"#[Autoconfigure(constructor: ...)]\" attribute for the latter."
            )));
        }
        Ok(Some(FunctionReflector::Method {
            class: declaring,
            method: constructor,
        }))
    }

    /// `getReflectionMethod($definition, $method)`.
    fn get_reflection_method(
        &mut self,
        container: &mut ContainerBuilder,
        definition: &Definition,
        method: &str,
    ) -> Result<FunctionReflector> {
        if method == "__construct" {
            return self.get_constructor(container, definition, true)?.ok_or_else(|| {
                Error::new(
                    ErrorKind::Other("TypeError".to_string()),
                    "Symfony\\Component\\DependencyInjection\\Compiler\\AbstractRecursivePass::getReflectionMethod(): Return value must be of type ReflectionFunctionAbstract, null returned",
                )
            });
        }
        let current_id = self.state().current_id.clone();
        let current = current_id.as_deref().unwrap_or("");

        let mut definition = definition.clone();
        let class = loop {
            let class = definition.class();
            if class.is_some() {
                break class;
            }
            let Some(parent) = definition.parent() else {
                break None;
            };
            definition = container.find_definition(&parent)?;
        };
        let Some(class) = class else {
            return Err(Error::runtime(format!(
                "Invalid service \"{current}\": the class is not set."
            )));
        };
        let Some(r) = container.get_reflection_class(Some(&class), true)? else {
            return Err(Error::runtime(format!(
                "Invalid service \"{current}\": class \"{class}\" does not exist."
            )));
        };
        let named = if differs(&class, current_id.as_deref()) {
            format!("{class}::{method}")
        } else {
            method.to_string()
        };
        let registry = container.class_registry();
        let Some((declaring, found)) = registry.find_method(&r, method) else {
            for magic in ["__call", "__callStatic"] {
                if registry
                    .find_method(&r, magic)
                    .is_some_and(|(_, m)| m.is_public())
                {
                    return Ok(FunctionReflector::VariadicClosure { is_method: true });
                }
            }
            return Err(Error::runtime(format!(
                "Invalid service \"{current}\": method \"{named}()\" does not exist."
            )));
        };
        if !found.is_public() {
            return Err(Error::runtime(format!(
                "Invalid service \"{current}\": method \"{named}()\" must be public."
            )));
        }
        Ok(FunctionReflector::Method {
            class: declaring,
            method: found,
        })
    }
}

fn into_array(value: Value) -> PhpArray {
    match value {
        Value::Array(a) => a,
        _ => PhpArray::new(),
    }
}

/// `lcfirst()`.
fn lcfirst(text: &str) -> String {
    let mut bytes = text.as_bytes().to_vec();
    if let Some(first) = bytes.first_mut() {
        first.make_ascii_lowercase();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
