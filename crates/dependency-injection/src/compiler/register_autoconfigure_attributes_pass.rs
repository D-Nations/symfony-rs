//! Port of Symfony\Component\DependencyInjection\Compiler\RegisterAutoconfigureAttributesPass.php:
//! reads `#[Autoconfigure]` and `#[Lazy]` attributes on definitions that
//! are autoconfigured and don't have the `container.ignore_attributes`
//! tag.
//!
//! Upstream hands each attribute, cast to an array, to the YAML loader's
//! `parseDefinitions()` as an `_instanceof` entry. The loader is not part
//! of this port; [`parse_instanceof`] is its `_instanceof` path
//! (`parseDefinition()` with `checkDefinition()`, `parseCallable()`,
//! `resolveServices()` and `validateAttributes()` for the keys an
//! instanceof conditional accepts). YAML tags (`!iterator`, `!service`, …)
//! cannot occur in attribute values and are not handled.
//!
//! `$attribute->newInstance()` is done for `Autoconfigure`,
//! `AutoconfigureTag`, `AutoconfigureResourceTag` and `Lazy`, whose
//! constructors only store their arguments; another subclass runs PHP code
//! in its constructor and is refused.
//!
//! Upstream: symfony/dependency-injection Compiler/RegisterAutoconfigureAttributesPass.php

use php_value::{PhpArray, Value, array::Key};

use super::autowire_required_properties_pass::attributes_of;
use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::{BoundArgument, INSTANCEOF_BINDING, ServiceClosureArgument};
use crate::class_metadata::{AttributeMetadata, ClassMetadata};
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, Result};
use crate::reference::Reference;

/// `Autoconfigure::class`.
pub const AUTOCONFIGURE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Autoconfigure";
/// `AutoconfigureTag::class`.
pub const AUTOCONFIGURE_TAG: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutoconfigureTag";
/// `AutoconfigureResourceTag::class`.
pub const AUTOCONFIGURE_RESOURCE_TAG: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutoconfigureResourceTag";
/// `Lazy::class`.
pub const LAZY: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Lazy";

/// `Autoconfigure::__construct()`'s parameters, in order.
const AUTOCONFIGURE_PARAMETERS: [&str; 11] = [
    "tags",
    "calls",
    "bind",
    "lazy",
    "public",
    "shared",
    "autowire",
    "properties",
    "configurator",
    "constructor",
    "resourceTags",
];

/// `ContentLoaderTrait::INSTANCEOF_KEYWORDS`.
const INSTANCEOF_KEYWORDS: [&str; 11] = [
    "shared",
    "lazy",
    "public",
    "properties",
    "configurator",
    "calls",
    "tags",
    "resource_tags",
    "autowire",
    "bind",
    "constructor",
];

/// `RegisterAutoconfigureAttributesPass`.
#[derive(Debug, Clone, Copy, Default)]
pub struct RegisterAutoconfigureAttributesPass;

impl RegisterAutoconfigureAttributesPass {
    /// `accept($definition)`.
    pub fn accept(definition: &Definition) -> bool {
        definition.is_autoconfigured() && !definition.has_tag("container.ignore_attributes")
    }

    /// `processClass($container, $class)`.
    pub fn process_class(container: &mut ContainerBuilder, class: &ClassMetadata) -> Result<()> {
        let registry = container.class_registry();
        let autoconfigure: Vec<_> =
            attributes_of(&registry, &class.attributes, AUTOCONFIGURE, true).collect();
        let lazy: Vec<_> = attributes_of(&registry, &class.attributes, LAZY, true).collect();

        if !autoconfigure.is_empty() && !lazy.is_empty() {
            return Err(Error::autowiring_failed(
                class.name.as_str(),
                "Using both attributes #[Lazy] and #[Autoconfigure] on an argument is not allowed; use the \"lazy\" parameter of #[Autoconfigure] instead.",
            )
            .into_autoconfigure_failed());
        }

        for attribute in autoconfigure.into_iter().chain(lazy) {
            register_for_autoconfiguration(container, class, attribute)?;
        }
        Ok(())
    }
}

impl CompilerPassInterface for RegisterAutoconfigureAttributesPass {
    fn class(&self) -> &'static str {
        "Symfony\\Component\\DependencyInjection\\Compiler\\RegisterAutoconfigureAttributesPass"
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        let definitions: Vec<Definition> = container.get_definitions().values().cloned().collect();
        for definition in definitions {
            if !Self::accept(&definition) {
                continue;
            }
            let class = definition.class();
            if let Some(class) = container.get_reflection_class(class.as_deref(), false)? {
                Self::process_class(container, &class)?;
            }
        }
        Ok(())
    }
}

fn key(class: &str) -> String {
    class.trim_start_matches('\\').to_ascii_lowercase()
}

/// `(array) $attribute->newInstance()`.
fn attribute_to_array(attribute: &AttributeMetadata) -> Result<PhpArray> {
    let argument = |position: i64, name: &str| {
        attribute
            .arguments
            .get(&Key::from(name))
            .or_else(|| attribute.arguments.get(&Key::Int(position)))
            .cloned()
    };
    let class = key(&attribute.class);
    let mut properties = PhpArray::new();
    if class == key(LAZY) {
        properties.insert("lazy", argument(0, "lazy").unwrap_or(Value::Bool(true)));
        return Ok(properties);
    }
    let tag = |type_: &str| -> (String, Value) {
        let name = argument(0, "name").filter(|v| !v.is_null());
        let attributes = argument(1, "attributes").unwrap_or(Value::Array(PhpArray::new()));
        let mut tag = PhpArray::new();
        match name.as_ref().and_then(Value::to_php_string) {
            Some(name) => tag.insert(Key::from(name), attributes),
            None => tag.insert(0, attributes),
        }
        (
            type_.to_string(),
            Value::Array(PhpArray::list([Value::Array(tag)])),
        )
    };
    let given: Vec<(String, Value)> = if class == key(AUTOCONFIGURE) {
        AUTOCONFIGURE_PARAMETERS
            .iter()
            .zip(0..)
            .filter_map(|(name, position)| {
                argument(position, name).map(|v| ((*name).to_string(), v))
            })
            .collect()
    } else if class == key(AUTOCONFIGURE_TAG) {
        vec![tag("tags")]
    } else if class == key(AUTOCONFIGURE_RESOURCE_TAG) {
        vec![tag("resourceTags")]
    } else {
        return Err(Error::logic(format!(
            "Instantiating the attribute \"{}\" needs PHP code to run, which this build-time port does not do.",
            attribute.class.trim_start_matches('\\')
        )));
    };
    for name in AUTOCONFIGURE_PARAMETERS {
        properties.insert(name, Value::Null);
    }
    for (name, value) in given {
        properties.insert(name.as_str(), value);
    }
    Ok(properties)
}

/// `registerForAutoconfiguration($container, $class, $attribute)`.
fn register_for_autoconfiguration(
    container: &mut ContainerBuilder,
    class: &ClassMetadata,
    attribute: &AttributeMetadata,
) -> Result<()> {
    let mut attribute = attribute_to_array(attribute)?;

    for type_ in ["tags", "resourceTags"] {
        let Some(Value::Array(tags)) = attribute.get(&Key::from(type_)).cloned() else {
            continue;
        };
        let mut replaced = tags.clone();
        for (i, tag) in tags.iter() {
            if let Value::Array(tag) = tag
                && tag.len() == 1
                && tag.keys().next() == Some(&Key::Int(0))
            {
                let mut named = PhpArray::new();
                named.insert(
                    class.name.as_str(),
                    tag.get(&Key::Int(0)).cloned().unwrap_or_default(),
                );
                replaced.insert(i.clone(), Value::Array(named));
            }
        }
        attribute.insert(type_, Value::Array(replaced));
    }
    if let Some(resource_tags) = attribute.get(&Key::from("resourceTags")).cloned()
        && !resource_tags.is_null()
    {
        attribute.insert("resource_tags", resource_tags);
    }
    attribute.remove(&Key::from("resourceTags"));

    let definition = container.register_for_autoconfiguration(&class.name);
    let file = class.file.clone().unwrap_or_default();
    parse_instanceof(&class.name, &definition, &attribute, &file, false)
}

fn isset<'a>(service: &'a PhpArray, key: &str) -> Option<&'a Value> {
    service.get(&Key::from(key)).filter(|v| !v.is_null())
}

fn string(value: &Value) -> String {
    value
        .to_php_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn key_string(key: &Key) -> String {
    String::from_utf8_lossy(&key.bytes()).into_owned()
}

/// The `_instanceof` path of `ContentLoaderTrait::parseDefinition()`:
/// `$service` configures `$definition`, the conditional of type `$id`.
#[expect(clippy::too_many_lines, reason = "one upstream method")]
pub fn parse_instanceof(
    id: &str,
    definition: &Definition,
    service: &PhpArray,
    file: &str,
    track_bindings: bool,
) -> Result<()> {
    // checkDefinition()
    for (key, _) in service.iter() {
        let key = key_string(key);
        if !INSTANCEOF_KEYWORDS.contains(&key.as_str()) {
            return Err(Error::invalid_argument(format!(
                "The configuration key \"{key}\" is unsupported for definition \"{id}\" in \"{file}\". Allowed configuration keys are \"{}\".",
                INSTANCEOF_KEYWORDS.join("\", \"")
            )));
        }
    }

    if let Some(shared) = isset(service, "shared") {
        definition.set_shared(shared.is_truthy());
    }

    if let Some(lazy) = isset(service, "lazy") {
        definition.set_lazy(lazy.is_truthy());
        if let Value::String(interface) = lazy {
            let mut tag = PhpArray::new();
            tag.insert("interface", Value::String(interface.clone()));
            definition.add_tag("proxy", tag);
        }
    }

    if let Some(public) = isset(service, "public") {
        definition.set_public(public.is_truthy());
    }

    if let Some(constructor) = isset(service, "constructor") {
        if !definition.factory().is_null() {
            return Err(Error::logic(format!(
                "The \"{id}\" service cannot declare a factory as well as a constructor."
            )));
        }

        definition.set_factory(Value::Array(PhpArray::list([
            Value::Null,
            constructor.clone(),
        ])));
    }

    if let Some(properties) = isset(service, "properties") {
        let Value::Array(properties) = resolve_services(properties)? else {
            return Err(Error::invalid_argument(format!(
                "Parameter \"properties\" must be an array for service \"{id}\" in \"{file}\"."
            )));
        };
        definition.set_properties(properties);
    }

    if let Some(configurator) = isset(service, "configurator") {
        definition.set_configurator(parse_callable(configurator, "configurator", id, file)?);
    }

    if let Some(calls) = isset(service, "calls") {
        let Value::Array(calls) = calls else {
            return Err(Error::invalid_argument(format!(
                "Parameter \"calls\" must be an array for service \"{id}\" in \"{file}\"."
            )));
        };
        for (k, call) in calls.iter() {
            let Value::Array(call) = call else {
                return Err(Error::invalid_argument(format!(
                    "Invalid method call for service \"{id}\": expected map or array, \"{}\" given in \"{file}\".",
                    call.debug_type()
                )));
            };

            if let Key::Str(k) = k {
                return Err(Error::invalid_argument(format!(
                    "Invalid method call for service \"{id}\", did you forget a leading dash before \"{}: ...\" in \"{file}\"?",
                    String::from_utf8_lossy(k.as_bytes())
                )));
            }

            let (method, args, returns_clone) = if let Some(Value::String(method)) =
                isset(call, "method")
            {
                (
                    method.to_string_lossy().into_owned(),
                    call.get(&Key::from("arguments"))
                        .cloned()
                        .unwrap_or(Value::Array(PhpArray::new())),
                    call.get(&Key::from("returns_clone"))
                        .is_some_and(Value::is_truthy),
                )
            } else if call.len() == 1
                && let Some((Key::Str(method), args)) = call.iter().next()
            {
                (method.to_string_lossy().into_owned(), args.clone(), false)
            } else if !call.get(&Key::Int(0)).is_some_and(Value::is_truthy) {
                return Err(Error::invalid_argument(format!(
                    "Invalid call for service \"{id}\": the method must be defined as the first index of an array or as the only key of a map in \"{file}\"."
                )));
            } else {
                (
                    string(call.get(&Key::Int(0)).unwrap_or(&Value::Null)),
                    call.get(&Key::Int(1))
                        .filter(|v| !v.is_null())
                        .cloned()
                        .unwrap_or(Value::Array(PhpArray::new())),
                    call.get(&Key::Int(2)).is_some_and(Value::is_truthy),
                )
            };

            let Value::Array(_) = args else {
                return Err(Error::invalid_argument(format!(
                    "The second parameter for function call \"{method}\" must be an array of its arguments for service \"{id}\" in \"{file}\"."
                )));
            };

            let Value::Array(args) = resolve_services(&args)? else {
                unreachable!("an array resolves to an array");
            };
            definition.add_method_call(&method, args, returns_clone)?;
        }
    }

    for type_ in ["tags", "resource_tags"] {
        let tags = service
            .get(&Key::from(type_))
            .filter(|v| !v.is_null())
            .cloned()
            .unwrap_or(Value::Array(PhpArray::new()));
        let Value::Array(tags) = tags else {
            return Err(Error::invalid_argument(format!(
                "Parameter \"{type_}\" must be an array for service \"{id}\" in \"{file}\"."
            )));
        };

        for tag in tags.values() {
            let mut tag = match tag {
                Value::Array(tag) => tag.clone(),
                other => {
                    let mut tag = PhpArray::new();
                    tag.insert("name", other.clone());
                    tag
                }
            };

            let single = match tag.iter().next() {
                Some((name, Value::Array(attributes))) if tag.len() == 1 => {
                    Some((name.clone(), attributes.clone()))
                }
                _ => None,
            };
            let name = if let Some((name, attributes)) = single {
                tag = attributes;
                match name {
                    Key::Str(s) => Value::String(s),
                    Key::Int(i) => Value::Int(i),
                }
            } else {
                let Some(name) = isset(&tag, "name").cloned() else {
                    return Err(Error::invalid_argument(format!(
                        "A \"{type_}\" entry is missing a \"name\" key for service \"{id}\" in \"{file}\"."
                    )));
                };
                tag.remove(&Key::from("name"));
                name
            };

            let name = match &name {
                Value::String(s) if !s.as_bytes().is_empty() => s.to_string_lossy().into_owned(),
                _ => {
                    return Err(Error::invalid_argument(format!(
                        "The tag name for service \"{id}\" in \"{file}\" must be a non-empty string."
                    )));
                }
            };

            validate_attributes(
                &|attribute| {
                    format!(
                        "A \"{id}\" attribute must be of a scalar-type for service \"{name}\", tag \"{attribute}\", attribute \"{type_}\" in \"{file}\"."
                    )
                },
                &tag,
                &[],
            )?;

            match type_ {
                "tags" => definition.add_tag(&name, tag),
                _ => definition.add_resource_tag(&name, tag),
            };
        }
    }

    if let Some(autowire) = isset(service, "autowire") {
        definition.set_autowired(autowire.is_truthy());
    }

    if let Some(bind) = isset(service, "bind") {
        let mut bindings = definition.bindings();

        let Value::Array(_) = bind else {
            return Err(Error::invalid_argument(format!(
                "Parameter \"bind\" must be an array for service \"{id}\" in \"{file}\"."
            )));
        };

        let Value::Array(resolved) = resolve_services(bind)? else {
            unreachable!("an array resolves to an array");
        };
        // array_merge()
        for (k, v) in resolved.iter() {
            match k {
                Key::Int(_) => {
                    let _ = bindings.push(v.clone());
                }
                Key::Str(_) => bindings.insert(k.clone(), v.clone()),
            }
        }
        let mut wrapped = PhpArray::new();
        for (argument, value) in bindings.iter() {
            let value = if BoundArgument::from_value(value).is_some() {
                value.clone()
            } else {
                Value::from(BoundArgument::new(
                    value.clone(),
                    track_bindings,
                    INSTANCEOF_BINDING,
                    Some(file.to_string()),
                ))
            };
            wrapped.insert(argument.clone(), value);
        }
        definition.set_bindings(&wrapped);
    }

    Ok(())
}

/// `validateAttributes($message, $attributes, $path)`.
fn validate_attributes(
    message: &dyn Fn(&str) -> String,
    attributes: &PhpArray,
    path: &[String],
) -> Result<()> {
    for (name, value) in attributes.iter() {
        let mut full = path.to_vec();
        full.push(key_string(name));
        match value {
            Value::Array(value) => validate_attributes(message, value, &full)?,
            Value::Object(_) => {
                return Err(Error::invalid_argument(message(&full.join("."))));
            }
            _ => {}
        }
    }
    Ok(())
}

/// `parseCallable($callable, $parameter, $id, $file)`.
fn parse_callable(callable: &Value, parameter: &str, id: &str, file: &str) -> Result<Value> {
    if Reference::from_value(callable).is_some() {
        return Ok(Value::Array(PhpArray::list([
            callable.clone(),
            Value::from("__invoke"),
        ])));
    }

    if let Value::String(s) = callable {
        let s = s.to_string_lossy().into_owned();
        if s.starts_with("@=") {
            if parameter != "factory" {
                return Err(Error::invalid_argument(format!(
                    "Using expressions in \"{parameter}\" for the \"{id}\" service is not supported in \"{file}\"."
                )));
            }
            return Err(expression_unsupported());
        }

        if let Some(rest) = s.strip_prefix('@') {
            if !s.contains(':') {
                return Ok(Value::Array(PhpArray::list([
                    resolve_services(callable)?,
                    Value::from("__invoke"),
                ])));
            }

            return Err(Error::invalid_argument(format!(
                "The value of the \"{parameter}\" option for the \"{id}\" service must be the id of the service without the \"@\" prefix (replace \"{s}\" with \"{rest}\" in \"{file}\")."
            )));
        }

        return Ok(callable.clone());
    }

    if let Value::Array(callable) = callable {
        let first = callable.get(&Key::Int(0)).filter(|v| !v.is_null());
        let second = callable.get(&Key::Int(1)).filter(|v| !v.is_null());
        if let (Some(first), Some(second)) = (first, second) {
            return Ok(Value::Array(PhpArray::list([
                resolve_services(first)?,
                second.clone(),
            ])));
        }

        if parameter == "factory" && second.is_some() && first.is_none() {
            return Ok(Value::Array(callable.clone()));
        }

        return Err(Error::invalid_argument(format!(
            "Parameter \"{parameter}\" must contain an array with two elements for service \"{id}\" in \"{file}\"."
        )));
    }

    Err(Error::invalid_argument(format!(
        "Parameter \"{parameter}\" must be a string or an array for service \"{id}\" in \"{file}\"."
    )))
}

fn expression_unsupported() -> Error {
    Error::logic(
        "The \"@=\" expression syntax cannot be used without the ExpressionLanguage component. Try running \"composer require symfony/expression-language\".",
    )
}

/// `resolveServices($value, $file)` for values without YAML tags.
fn resolve_services(value: &Value) -> Result<Value> {
    match value {
        Value::Array(array) => {
            let mut resolved = PhpArray::new();
            for (k, v) in array.iter() {
                resolved.insert(k.clone(), resolve_services(v)?);
            }
            Ok(Value::Array(resolved))
        }
        Value::String(s) if s.as_bytes().starts_with(b"@=") => Err(expression_unsupported()),
        Value::String(s) if s.as_bytes().starts_with(b"@") => {
            let s = s.to_string_lossy().into_owned();
            if let Some(rest) = s.strip_prefix("@>") {
                let argument = resolve_services(&Value::from(format!("@{rest}")))?;
                return Ok(Value::from(ServiceClosureArgument::new(argument)));
            }
            if let Some(rest) = s.strip_prefix("@@") {
                return Ok(Value::from(format!("@{rest}")));
            }
            let (id, behavior) = if let Some(rest) = s.strip_prefix("@!") {
                (rest, InvalidBehavior::IgnoreUninitialized)
            } else if let Some(rest) = s.strip_prefix("@?") {
                (rest, InvalidBehavior::Ignore)
            } else {
                (&s[1..], InvalidBehavior::Exception)
            };
            Ok(Value::from(Reference::with_behavior(id, behavior)))
        }
        _ => Ok(value.clone()),
    }
}
