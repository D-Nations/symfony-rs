//! Port of Symfony\Component\DependencyInjection\Compiler\AutowirePass.php:
//! inspects existing service definitions and wires the autowired ones
//! using the type hints of their classes.
//!
//! Reflection is the container's class metadata. Parameter attributes
//! are instantiated by [`new_autowire_attribute`]: the core classes
//! (`#[Autowire]` and its subclasses `AutowireInline`, `AutowireCallable`,
//! `AutowireMethodOf`, `AutowireIterator`, `AutowireLocator`,
//! `AutowireServiceClosure`, and `#[Target]`, `#[Lazy]`,
//! `#[AutowireDecorated]`) become objects of their class with their
//! public properties, as their constructors build them; any other class
//! (a user subclass of `Autowire`) is refused, since its constructor is
//! PHP code. An object written inside an attribute's arguments
//! (`new AutowireDecorated()`) is given in the metadata as the object
//! that [`new_autowire_attribute`] returns. Expressions are refused, as
//! there is no `ExpressionLanguage` here.
//!
//! `AutowireInline::buildDefinition()` calls `YamlFileLoader`'s
//! `parseDefinition()`, from the loaders that are not ported: the part
//! it reaches (the `class`/`factory`, `arguments`, `calls`, `properties`
//! and `parent` keys, with `autowire: true`) is ported here as
//! [`build_inline_definition`].
//!
//! The anonymous default-argument object is an object of the class
//! `AutowirePass@anonymous` that lives only while a call is autowired.
//! The reference that upstream keeps on the argument of a decorated
//! service (`restorePreviousValue`) is the position of that argument.
//!
//! Upstream: symfony/dependency-injection Compiler/AutowirePass.php

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

use php_value::object::GenericObject;
use php_value::{PhpArray, Value, array::Key};
use symfony_deprecation_contracts::trigger_deprecation;

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::autowire_required_properties_pass::{
    AUTOWIRE, AUTOWIRE_DECORATED, TARGET, named_type, new_attribute_instance,
};
use super::compiler_pass_interface::CompilerPassInterface;
use super::export_type::{DeclaringClass, export_type};
use super::resolve_bindings_pass::{parse_target_name, parsed_name};
use crate::argument::{
    EnvClosureArgument, IteratorArgument, ServiceClosureArgument, ServiceLocatorArgument,
};
use crate::class_metadata::{
    AttributeMetadata, ClassKind, ClassMetadata, ClassRegistry, FunctionReflector,
    ParameterMetadata,
};
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::container_interface::InvalidBehavior;
use crate::definition::{Definition, DefinitionError};
use crate::exception::{Error, ErrorKind, LazyMessage, Result};
use crate::reference::Reference;

/// `AutowireInline::class`.
pub const AUTOWIRE_INLINE: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutowireInline";
/// `AutowireCallable::class`.
pub const AUTOWIRE_CALLABLE: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutowireCallable";
/// `AutowireMethodOf::class`.
pub const AUTOWIRE_METHOD_OF: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutowireMethodOf";
/// `AutowireIterator::class`.
pub const AUTOWIRE_ITERATOR: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutowireIterator";
/// `AutowireLocator::class`.
pub const AUTOWIRE_LOCATOR: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutowireLocator";
/// `AutowireServiceClosure::class`.
pub const AUTOWIRE_SERVICE_CLOSURE: &str =
    "Symfony\\Component\\DependencyInjection\\Attribute\\AutowireServiceClosure";
/// `Lazy::class`.
pub const LAZY: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\Lazy";

/// The class of the anonymous default-argument object.
const DEFAULT_ARGUMENT: &str =
    "Symfony\\Component\\DependencyInjection\\Compiler\\AutowirePass@anonymous";

const CLASS: &str = "Symfony\\Component\\DependencyInjection\\Compiler\\AutowirePass";

// ----- attribute classes -----

fn key(class: &str) -> String {
    class.trim_start_matches('\\').to_ascii_lowercase()
}

/// The parent of a core attribute class.
fn core_parent(class: &str) -> Option<&'static str> {
    let k = key(class);
    [
        (AUTOWIRE_INLINE, AUTOWIRE),
        (AUTOWIRE_CALLABLE, AUTOWIRE_INLINE),
        (AUTOWIRE_METHOD_OF, AUTOWIRE_CALLABLE),
        (AUTOWIRE_ITERATOR, AUTOWIRE),
        (AUTOWIRE_LOCATOR, AUTOWIRE),
        (AUTOWIRE_SERVICE_CLOSURE, AUTOWIRE),
    ]
    .into_iter()
    .find(|(c, _)| key(c) == k)
    .map(|(_, p)| p)
}

/// `is_a($class, $parent, true)` for attribute classes: the core
/// hierarchy, then the class registry for user subclasses.
pub(crate) fn attribute_is_a(registry: &ClassRegistry, class: &str, parent: &str) -> bool {
    let mut current = Some(class.trim_start_matches('\\').to_string());
    let mut seen = 0;
    while let Some(c) = current {
        if key(&c) == key(parent) {
            return true;
        }
        seen += 1;
        if seen > 64 {
            return false;
        }
        current = core_parent(&c)
            .map(String::from)
            .or_else(|| registry.class(&c).and_then(|m| m.parent.clone()));
    }
    false
}

fn object_class(value: &Value) -> Option<String> {
    value.as_object().map(|o| o.class().to_string())
}

fn property(value: &Value, name: &str) -> Value {
    value
        .as_object()
        .and_then(|o| o.properties().get(&Key::from(name)).cloned())
        .unwrap_or_default()
}

fn is_instance(registry: &ClassRegistry, value: &Value, class: &str) -> bool {
    object_class(value).is_some_and(|c| attribute_is_a(registry, &c, class))
}

fn object(class: &str, properties: PhpArray) -> Value {
    Value::Object(Arc::new(GenericObject::new(class, properties)))
}

/// An argument of the attribute: by name, then by position (`None` when
/// absent).
fn arg(attribute: &AttributeMetadata, position: i64, name: &str) -> Option<Value> {
    attribute
        .arguments
        .get(&Key::from(name))
        .or_else(|| attribute.arguments.get(&Key::Int(position)))
        .cloned()
}

fn string_of(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.to_string_lossy().into_owned()),
        _ => None,
    }
}

/// `(array) $exclude`.
fn exclude_list(value: Option<Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a
            .values()
            .filter_map(Value::to_php_string)
            .map(|s| s.to_string_lossy().into_owned())
            .collect(),
        Some(other) => other
            .to_php_string()
            .map(|s| vec![s.to_string_lossy().into_owned()])
            .unwrap_or_default(),
    }
}

fn autowire_object(class: &str, value: Value, lazy: Value) -> Value {
    let mut properties = PhpArray::new();
    properties.insert("value", value);
    properties.insert("lazy", lazy);
    object(class, properties)
}

/// `Autowire::__construct()`'s `$lazy`: a string is a list of one.
fn normalize_lazy(lazy: Option<Value>) -> Value {
    match lazy {
        Some(Value::String(s)) => Value::Array(PhpArray::list([Value::String(s)])),
        Some(Value::Null) | None => Value::Bool(false),
        Some(lazy) => lazy,
    }
}

/// `preg_match` of `AutowireLocator`'s type check:
/// `^\??fqcn(?:(?:\|fqcn)*+|(?:&fqcn)*+)$`.
fn is_locator_type(type_: &str) -> bool {
    fn is_fqcn(s: &str) -> bool {
        !s.is_empty()
            && s.split('\\').all(|part| {
                let bytes = part.as_bytes();
                bytes
                    .first()
                    .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f)
                    && bytes
                        .iter()
                        .all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x7f)
            })
    }
    let type_ = type_.strip_prefix('?').unwrap_or(type_);
    if type_.contains('|') && type_.contains('&') {
        return false;
    }
    let glue = if type_.contains('&') { '&' } else { '|' };
    type_.split(glue).all(is_fqcn)
}

/// `$attribute->newInstance()` for the core attribute classes this pass
/// reads; other classes are refused.
pub fn new_autowire_attribute(attribute: &AttributeMetadata) -> Result<Value> {
    let class = attribute.class.trim_start_matches('\\');
    let k = key(class);
    if k == key(LAZY) {
        let mut properties = PhpArray::new();
        properties.insert(
            "lazy",
            arg(attribute, 0, "lazy").unwrap_or(Value::Bool(true)),
        );
        return Ok(object(class, properties));
    }
    if k == key(AUTOWIRE_INLINE) {
        let class_arg = arg(attribute, 0, "class").unwrap_or_default();
        let parent = arg(attribute, 4, "parent").unwrap_or_default();
        if class_arg.is_null() && parent.is_null() {
            return Err(Error::logic(
                "#[AutowireInline] attribute should declare either $class or $parent.",
            ));
        }
        let empty = || Value::Array(PhpArray::new());
        let mut value = PhpArray::new();
        let class_key = if class_arg.as_array().is_some() {
            "factory"
        } else {
            "class"
        };
        value.insert(class_key, class_arg);
        value.insert(
            "arguments",
            arg(attribute, 1, "arguments").unwrap_or_else(empty),
        );
        value.insert("calls", arg(attribute, 2, "calls").unwrap_or_else(empty));
        value.insert(
            "properties",
            arg(attribute, 3, "properties").unwrap_or_else(empty),
        );
        value.insert("parent", parent);
        let lazy = normalize_lazy(arg(attribute, 5, "lazy"));
        return Ok(autowire_object(class, Value::Array(value), lazy));
    }
    if k == key(AUTOWIRE_CALLABLE) {
        let callable = arg(attribute, 0, "callable").filter(|v| !v.is_null());
        let service = arg(attribute, 1, "service").filter(|v| !v.is_null());
        let method = arg(attribute, 2, "method").filter(|v| !v.is_null());
        if callable.is_some() == service.is_some() {
            return Err(Error::logic(
                "#[AutowireCallable] attribute must declare exactly one of $callable or $service.",
            ));
        }
        if service.is_none() && method.is_some() {
            return Err(Error::logic(
                "#[AutowireCallable] attribute cannot have a $method without a $service.",
            ));
        }
        let value = callable.unwrap_or_else(|| {
            let service = service.as_ref().and_then(string_of).unwrap_or_default();
            Value::Array(PhpArray::list([
                Value::from(Reference::new(service)),
                method.unwrap_or_else(|| Value::from("__invoke")),
            ]))
        });
        let lazy = normalize_lazy(arg(attribute, 3, "lazy"));
        return Ok(autowire_object(class, value, lazy));
    }
    if k == key(AUTOWIRE_METHOD_OF) {
        let service = arg(attribute, 0, "service")
            .as_ref()
            .and_then(string_of)
            .unwrap_or_default();
        let value = Value::Array(PhpArray::list([Value::from(Reference::new(service))]));
        let lazy = normalize_lazy(arg(attribute, 1, "lazy"));
        return Ok(autowire_object(class, value, lazy));
    }
    if k == key(AUTOWIRE_SERVICE_CLOSURE) {
        let service = arg(attribute, 0, "service")
            .as_ref()
            .and_then(string_of)
            .unwrap_or_default();
        let value = ServiceClosureArgument::new(Value::from(Reference::new(service))).to_value();
        return Ok(autowire_object(class, value, Value::Bool(false)));
    }
    if k == key(AUTOWIRE_ITERATOR) || k == key(AUTOWIRE_LOCATOR) {
        return new_tagged_attribute(attribute, class, k == key(AUTOWIRE_LOCATOR));
    }
    new_attribute_instance(attribute)
}

/// `AutowireIterator` and `AutowireLocator`.
fn new_tagged_attribute(
    attribute: &AttributeMetadata,
    class: &str,
    locator: bool,
) -> Result<Value> {
    let known = [
        "tag",
        "services",
        "indexAttribute",
        "exclude",
        "excludeSelf",
    ];
    let legacy = attribute.arguments.iter().any(|(k, _)| match k {
        Key::Int(i) => *i > 3,
        Key::Str(s) => !known.contains(&s.to_string_lossy().as_ref()),
    }) || arg(attribute, 3, "excludeSelf")
        .is_some_and(|v| !matches!(v, Value::Bool(_)))
        || arg(attribute, 2, "exclude")
            .as_ref()
            .and_then(string_of)
            .is_some_and(|s| {
                s.starts_with("get") && s.as_bytes().get(3).is_some_and(u8::is_ascii_uppercase)
            });
    if legacy {
        return Err(Error::logic(format!(
            "Instantiating the attribute \"{class}\" with $defaultIndexMethod or $defaultPriorityMethod is deprecated and not supported by this build-time port."
        )));
    }
    let first = arg(attribute, 0, if locator { "services" } else { "tag" }).unwrap_or_default();
    let index_attribute = arg(attribute, 1, "indexAttribute")
        .as_ref()
        .and_then(string_of);
    let exclude = exclude_list(arg(attribute, 2, "exclude"));
    let exclude_self = !matches!(arg(attribute, 3, "excludeSelf"), Some(Value::Bool(false)));

    if let Some(tag) = string_of(&first) {
        let tagged = IteratorArgument::tagged(
            &tag,
            index_attribute.as_deref(),
            locator,
            exclude,
            exclude_self,
        );
        let value = if locator {
            ServiceLocatorArgument::from_tagged(tagged).to_value()
        } else {
            tagged.to_value()
        };
        return Ok(autowire_object(class, value, Value::Bool(false)));
    }

    let services = first.as_array().cloned().unwrap_or_default();
    let mut references = PhpArray::new();
    for (k, type_) in services.iter() {
        if object_class(type_).is_some_and(|c| attribute_is_a(&ClassRegistry::new(), &c, AUTOWIRE))
        {
            references.insert(k.clone(), type_.clone());
            continue;
        }
        let key_text = String::from_utf8_lossy(&k.bytes()).into_owned();
        let Some(mut t) = string_of(type_).filter(|t| is_locator_type(t)) else {
            let given = string_of(type_).unwrap_or_else(|| type_.debug_type());
            return Err(Error::invalid_argument(format!(
                "\"{given}\" is not a PHP type for key \"{key_text}\"."
            )));
        };
        let mut optional = InvalidBehavior::Exception;
        if let Some(rest) = t.strip_prefix('?') {
            t = rest.to_string();
            optional = InvalidBehavior::Ignore;
        }
        let (key, name) = match k {
            Key::Int(_) => (Key::from(t.as_str()), None),
            Key::Str(_) => (k.clone(), Some(key_text)),
        };
        references.insert(
            key,
            Value::from(Reference::typed(
                t.clone(),
                t,
                optional,
                name,
                PhpArray::new(),
            )),
        );
    }
    Ok(autowire_object(
        class,
        ServiceLocatorArgument::new(references).to_value(),
        Value::Bool(false),
    ))
}

// ----- AutowireInline::buildDefinition() -----

fn invalid(message: String) -> Error {
    Error::invalid_argument(message)
}

/// `ContentLoaderTrait::resolveServices()` for PHP values (no tagged
/// values).
fn resolve_services(value: &Value) -> Result<Value> {
    match value {
        Value::Array(array) => {
            let mut out = array.clone();
            for (k, v) in array.iter() {
                out.insert(k.clone(), resolve_services(v)?);
            }
            Ok(Value::Array(out))
        }
        Value::String(s) => {
            let s = s.to_string_lossy();
            if s.starts_with("@=") {
                return Err(Error::logic(
                    "The \"@=\" expression syntax cannot be used without the ExpressionLanguage component. Try running \"composer require symfony/expression-language\".",
                ));
            }
            let Some(rest) = s.strip_prefix('@') else {
                return Ok(value.clone());
            };
            if let Some(service) = rest.strip_prefix('>') {
                let inner = resolve_services(&Value::from(format!("@{service}")))?;
                return Ok(ServiceClosureArgument::new(inner).to_value());
            }
            if let Some(id) = rest.strip_prefix('@') {
                return Ok(Value::from(format!("@{id}")));
            }
            let (id, behavior) = if let Some(id) = rest.strip_prefix('!') {
                (id, InvalidBehavior::IgnoreUninitialized)
            } else if let Some(id) = rest.strip_prefix('?') {
                (id, InvalidBehavior::Ignore)
            } else {
                (rest, InvalidBehavior::Exception)
            };
            Ok(Value::from(Reference::with_behavior(id, behavior)))
        }
        _ => Ok(value.clone()),
    }
}

/// `ContentLoaderTrait::parseCallable()`.
fn parse_callable(callable: &Value, parameter: &str, id: &str, file: &str) -> Result<Value> {
    if Reference::from_value(callable).is_some() {
        return Ok(Value::Array(PhpArray::list([
            callable.clone(),
            Value::from("__invoke"),
        ])));
    }
    if let Value::String(s) = callable {
        let text = s.to_string_lossy().into_owned();
        if text.starts_with("@=") {
            if parameter != "factory" {
                return Err(invalid(format!(
                    "Using expressions in \"{parameter}\" for the \"{id}\" service is not supported in \"{file}\"."
                )));
            }
            return Ok(callable.clone());
        }
        if let Some(rest) = text.strip_prefix('@') {
            if !text.contains(':') {
                return Ok(Value::Array(PhpArray::list([
                    resolve_services(callable)?,
                    Value::from("__invoke"),
                ])));
            }
            return Err(invalid(format!(
                "The value of the \"{parameter}\" option for the \"{id}\" service must be the id of the service without the \"@\" prefix (replace \"{text}\" with \"{rest}\" in \"{file}\")."
            )));
        }
        return Ok(callable.clone());
    }
    if let Value::Array(array) = callable {
        let first = array.get(&Key::Int(0)).filter(|v| !v.is_null());
        let second = array.get(&Key::Int(1)).filter(|v| !v.is_null());
        if let (Some(first), Some(second)) = (first, second) {
            return Ok(Value::Array(PhpArray::list([
                resolve_services(first)?,
                second.clone(),
            ])));
        }
        if parameter == "factory"
            && second.is_some()
            && array.get(&Key::Int(0)).is_none_or(Value::is_null)
        {
            return Ok(callable.clone());
        }
        return Err(invalid(format!(
            "Parameter \"{parameter}\" must contain an array with two elements for service \"{id}\" in \"{file}\"."
        )));
    }
    Err(invalid(format!(
        "Parameter \"{parameter}\" must be a string or an array for service \"{id}\" in \"{file}\"."
    )))
}

fn isset<'a>(array: &'a PhpArray, name: &str) -> Option<&'a Value> {
    array.get(&Key::from(name)).filter(|v| !v.is_null())
}

/// `ContentLoaderTrait::parseDefinition($id, $service, $file,
/// ['autowire' => true], true)` for the keys of an `#[AutowireInline]`
/// value.
#[expect(clippy::too_many_lines, reason = "one upstream method")]
pub(crate) fn build_inline_definition(
    id: &str,
    service: &PhpArray,
    file: &str,
) -> Result<Definition> {
    if id.starts_with('_')
        && id[1..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(invalid(format!(
            "Service names that start with an underscore are reserved. Rename the \"{id}\" service or define it in XML instead."
        )));
    }
    let keywords = [
        "alias",
        "parent",
        "class",
        "shared",
        "synthetic",
        "lazy",
        "public",
        "abstract",
        "deprecated",
        "factory",
        "file",
        "arguments",
        "properties",
        "configurator",
        "calls",
        "tags",
        "resource_tags",
        "decorates",
        "decoration_inner_name",
        "decoration_priority",
        "decoration_on_invalid",
        "decorates_tag",
        "autowire",
        "autoconfigure",
        "bind",
        "constructor",
    ];
    for (k, _) in service.iter() {
        let name = String::from_utf8_lossy(&k.bytes()).into_owned();
        if ![
            "class",
            "factory",
            "arguments",
            "calls",
            "properties",
            "parent",
        ]
        .contains(&name.as_str())
        {
            if keywords.contains(&name.as_str()) {
                return Err(Error::logic(format!(
                    "The configuration key \"{name}\" of an #[AutowireInline] attribute is not supported by this build-time port."
                )));
            }
            return Err(invalid(format!(
                "The configuration key \"{name}\" is unsupported for definition \"{id}\" in \"{file}\". Allowed configuration keys are \"{}\".",
                keywords.join("\", \"")
            )));
        }
    }

    let definition = if let Some(parent) = isset(service, "parent") {
        let parent = parent
            .to_php_string()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Some(rest) = parent.strip_prefix('@') {
            return Err(invalid(format!(
                "The value of the \"parent\" option for the \"{id}\" service must be the id of the service without the \"@\" prefix (replace \"{parent}\" with \"{rest}\")."
            )));
        }
        Definition::child(&parent)
    } else {
        Definition::new(None, PhpArray::new())
    };
    definition.set_autowired(true);
    definition.set_changes(Vec::new());

    if let Some(class) = isset(service, "class") {
        let class = class
            .to_php_string()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        definition.set_class(Some(&class));
    }
    if let Some(factory) = isset(service, "factory") {
        definition.set_factory(parse_callable(factory, "factory", id, file)?);
    }
    if let Some(arguments) = isset(service, "arguments")
        && let Value::Array(a) = resolve_services(arguments)?
    {
        definition.set_arguments(a);
    }
    if let Some(properties) = isset(service, "properties")
        && let Value::Array(p) = resolve_services(properties)?
    {
        definition.set_properties(p);
    }
    if let Some(calls) = isset(service, "calls") {
        let Some(calls) = calls.as_array() else {
            return Err(invalid(format!(
                "Parameter \"calls\" must be an array for service \"{id}\" in \"{file}\"."
            )));
        };
        for (k, call) in calls.iter() {
            let Some(call) = call.as_array() else {
                return Err(invalid(format!(
                    "Invalid method call for service \"{id}\": expected map or array, \"{}\" given in \"{file}\".",
                    call.debug_type()
                )));
            };
            if let Key::Str(k) = k {
                return Err(invalid(format!(
                    "Invalid method call for service \"{id}\", did you forget a leading dash before \"{}: ...\" in \"{file}\"?",
                    k.to_string_lossy()
                )));
            }
            let (method, args, returns_clone) = if let Some(Value::String(m)) =
                call.get(&Key::from("method"))
            {
                (
                    m.to_string_lossy().into_owned(),
                    call.get(&Key::from("arguments"))
                        .cloned()
                        .unwrap_or(Value::Array(PhpArray::new())),
                    call.get(&Key::from("returns_clone"))
                        .is_some_and(Value::is_truthy),
                )
            } else if let (1, Some((Key::Str(m), args))) = (call.len(), call.first()) {
                (m.to_string_lossy().into_owned(), args.clone(), false)
            } else if !call.get(&Key::Int(0)).is_some_and(Value::is_truthy) {
                return Err(invalid(format!(
                    "Invalid call for service \"{id}\": the method must be defined as the first index of an array or as the only key of a map in \"{file}\"."
                )));
            } else {
                (
                    call.get(&Key::Int(0))
                        .and_then(Value::to_php_string)
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    call.get(&Key::Int(1))
                        .filter(|v| !v.is_null())
                        .cloned()
                        .unwrap_or(Value::Array(PhpArray::new())),
                    call.get(&Key::Int(2)).is_some_and(Value::is_truthy),
                )
            };
            let Value::Array(_) = args else {
                return Err(invalid(format!(
                    "The second parameter for function call \"{method}\" must be an array of its arguments for service \"{id}\" in \"{file}\"."
                )));
            };
            let Value::Array(args) = resolve_services(&args)? else {
                continue;
            };
            definition.add_method_call(&method, args, returns_clone)?;
        }
    }
    Ok(definition)
}

/// The class that declares a reflected function: its metadata, for
/// `$parameter->getDeclaringClass()`.
fn declaring_class(
    registry: &ClassRegistry,
    reflector: &FunctionReflector,
) -> Option<Arc<ClassMetadata>> {
    reflector.class().and_then(|c| registry.class(c))
}

/// `$attribute->buildDefinition($value, $type, $parameter)` of
/// `AutowireInline`, `AutowireCallable` and `AutowireMethodOf`.
pub(crate) fn build_definition(
    registry: &ClassRegistry,
    attribute: &Value,
    value: Value,
    type_: &str,
    parameter: &ParameterMetadata,
    reflector: &FunctionReflector,
) -> Result<Definition> {
    let class = object_class(attribute).unwrap_or_default();
    let mut value = value;
    if attribute_is_a(registry, &class, AUTOWIRE_CALLABLE) {
        if attribute_is_a(registry, &class, AUTOWIRE_METHOD_OF) {
            let mut array = value.as_array().cloned().unwrap_or_default();
            array.insert(1, Value::from(parameter.name.as_str()));
            value = Value::Array(array);
        }
        let lazy = property(attribute, "lazy");
        let definition_class = match &lazy {
            Value::Array(list) => list
                .first()
                .and_then(|(_, v)| v.to_php_string())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            _ if !type_.is_empty() => type_.to_string(),
            _ => "Closure".to_string(),
        };
        let argument = match value {
            Value::Array(mut array) => {
                if !array.contains_key(&Key::Int(1)) {
                    array.insert(1, Value::from("__invoke"));
                }
                Value::Array(array)
            }
            other => other,
        };
        let definition = Definition::new(Some(&definition_class), PhpArray::list([argument]));
        definition.set_factory(Value::Array(PhpArray::list([
            Value::from("Closure"),
            Value::from("fromCallable"),
        ])));
        definition.set_lazy(
            lazy.is_truthy()
                || definition_class != "Closure"
                    && parameter.type_.as_deref().unwrap_or_default() != "callable",
        );
        return Ok(definition);
    }

    let mut value = value.as_array().cloned().unwrap_or_default();
    if let Some(factory) = isset(&value, "factory").cloned() {
        value.insert(
            "class",
            if type_.is_empty() {
                Value::Null
            } else {
                Value::from(type_)
            },
        );
        let mut factory = factory.as_array().cloned().unwrap_or_default();
        if factory.get(&Key::Int(0)).is_none_or(Value::is_null) {
            factory.insert(
                0,
                if type_.is_empty() {
                    Value::Null
                } else {
                    Value::from(type_)
                },
            );
        }
        if factory.get(&Key::Int(1)).is_none_or(Value::is_null) {
            factory.insert(1, Value::from("__invoke"));
        }
        value.insert("factory", Value::Array(factory));
    }
    let class = declaring_class(registry, reflector);
    let id = class.as_ref().map(|c| c.name.clone()).unwrap_or_default();
    let file = class.and_then(|c| c.file.clone()).unwrap_or_default();
    build_inline_definition(&id, &value, &file)
}

// ----- the pass -----

/// `$getValue`'s captured variables.
#[derive(Debug, Clone)]
struct GetValue {
    type_: String,
    parameter: ParameterMetadata,
    class: String,
    method: String,
    name: String,
    target: Option<Value>,
    names_id: usize,
    current_id: String,
}

impl GetValue {
    /// `$class !== $currentId ? $class.'::'.$method : $method`.
    fn function(&self) -> String {
        if self.class == self.current_id {
            self.method.clone()
        } else {
            format!("{}::{}", self.class, self.method)
        }
    }
}

/// Where the argument that `$restorePreviousValue` writes lives.
#[derive(Debug, Clone)]
enum RestoreTarget {
    /// A call of the method list being autowired.
    Local { generation: u64, position: usize },
    /// A definition already written back.
    Definition {
        definition: Definition,
        constructor: bool,
        call_key: Key,
    },
}

#[derive(Debug, Clone)]
struct Restore {
    target: RestoreTarget,
    key: Key,
    get_value: GetValue,
}

/// A call of `autowireCalls()`: the constructor's reflector or a method
/// name, its arguments, and the call array.
#[derive(Debug, Clone)]
enum CallMethod {
    Reflector(FunctionReflector),
    Name(String),
}

#[derive(Debug, Clone)]
struct Call {
    key: Key,
    method: CallMethod,
    arguments: PhpArray,
    raw: PhpArray,
}

/// `$this->typesClone`: the container snapshot and the type maps the
/// lazy "type not found" messages read.
struct TypesClone {
    container: ContainerBuilder,
    populated: bool,
    types: HashMap<String, String>,
    ambiguous: HashMap<String, Vec<String>>,
    autowiring_aliases: HashMap<String, Vec<String>>,
}

type SharedTypes = Arc<Mutex<Option<TypesClone>>>;

/// `AutowirePass`.
pub struct AutowirePass {
    state: RecursivePassState,
    throw_on_autowiring_exception: bool,
    decorated_class: Option<String>,
    decorated_id: Option<String>,
    restore: Option<Restore>,
    /// `$defaultArgument->names` of each autowired method.
    default_names: Vec<Vec<String>>,
    types_clone: SharedTypes,
    generation: u64,
    pending_patches: Vec<(usize, Key, Value)>,
}

impl std::fmt::Debug for AutowirePass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutowirePass")
            .field(
                "throw_on_autowiring_exception",
                &self.throw_on_autowiring_exception,
            )
            .finish_non_exhaustive()
    }
}

impl Clone for AutowirePass {
    fn clone(&self) -> Self {
        Self::new(self.throw_on_autowiring_exception)
    }
}

impl Default for AutowirePass {
    fn default() -> Self {
        Self::new(true)
    }
}

fn truthy(s: Option<&str>) -> bool {
    s.is_some_and(|s| !s.is_empty() && s != "0")
}

fn index_key(i: usize) -> Key {
    Key::Int(i64::try_from(i).unwrap_or(i64::MAX))
}

fn is_marker(value: &Value) -> bool {
    object_class(value).is_some_and(|c| c == DEFAULT_ARGUMENT)
}

fn marker(value: Value, names_id: usize) -> Value {
    let mut properties = PhpArray::new();
    properties.insert("value", value);
    properties.insert("names", Value::Int(i64::try_from(names_id).unwrap_or(0)));
    object(DEFAULT_ARGUMENT, properties)
}

fn marker_names_id(value: &Value) -> usize {
    match property(value, "names") {
        Value::Int(i) => usize::try_from(i).unwrap_or(0),
        _ => 0,
    }
}

/// `$parameter->allowsNull()`.
fn allows_null(parameter: &ParameterMetadata) -> bool {
    let Some(t) = parameter.type_.as_deref().map(str::trim) else {
        return true;
    };
    if t.starts_with('?') || t.eq_ignore_ascii_case("mixed") {
        return true;
    }
    t.split('|').any(|p| p.trim().eq_ignore_ascii_case("null"))
}

/// `ksort($arguments, \SORT_NATURAL)`.
fn ksort_natural(arguments: &PhpArray) -> PhpArray {
    let mut entries: Vec<(Key, Value)> = arguments
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    entries.sort_by(|(a, _), (b, _)| php_value::string::strnatcmp(&a.bytes(), &b.bytes()));
    let mut out = PhpArray::new();
    for (k, v) in entries {
        out.insert(k, v);
    }
    out
}

/// `/env_[a-f0-9]{16}_\w+_[a-f0-9]{32}/`.
fn has_env_placeholder(text: &str) -> bool {
    let bytes = text.as_bytes();
    let hex = |b: &u8| b.is_ascii_digit() || (b'a'..=b'f').contains(b);
    let word = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
    (0..bytes.len()).any(|at| {
        let Some(rest) = bytes.get(at..).filter(|r| r.starts_with(b"env_")) else {
            return false;
        };
        let rest = &rest[4..];
        if rest.len() < 17 || !rest[..16].iter().all(hex) || rest[16] != b'_' {
            return false;
        }
        let tail = &rest[17..];
        let run = tail.iter().take_while(|b| word(b)).count();
        // `\w+` backtracks to leave `_` and 32 hex digits.
        (1..run).any(|end| {
            end + 33 <= run && tail[end] == b'_' && tail[end + 1..end + 33].iter().all(hex)
        })
    })
}

/// `preg_match('/(?:^%env\(|:)default:/', $value)`.
fn has_default_processor(value: &str) -> bool {
    value.starts_with("%env(default:") || value.contains(":default:")
}

/// `canDefinitionBeAutowired($id)`.
fn can_be_autowired(container: &ContainerBuilder, id: &str) -> Result<bool> {
    let definition = container.find_definition(id)?;
    Ok(!definition.is_abstract() && !definition.has_tag("container.excluded"))
}

/// `resolveAlias($id)`.
fn resolve_alias(container: &ContainerBuilder, id: &str) -> String {
    let mut id = id.to_string();
    let mut seen: Vec<String> = Vec::new();
    while container.has_alias(&id) && !seen.contains(&id) {
        seen.push(id.clone());
        id = container.get_alias(&id).map(|a| a.id()).unwrap_or_default();
    }
    id
}

fn alias_target(container: &ContainerBuilder, id: &str) -> String {
    container.get_alias(id).map(|a| a.id()).unwrap_or_default()
}

/// `getCombinedAlias($type, $name, $target)`.
fn get_combined_alias(
    container: &ContainerBuilder,
    type_: &str,
    name: Option<&str>,
    target: Option<&str>,
) -> Result<Option<String>> {
    let prefix = if truthy(target) && truthy(name) {
        "."
    } else {
        ""
    };
    let suffix = if truthy(name) {
        format!(" ${}", target.or(name).unwrap_or_default())
    } else {
        String::new()
    };
    let parsed: Option<String> = target
        .map(String::from)
        .or_else(|| truthy(name).then(|| parsed_name(name.unwrap_or_default())));

    let alias = format!("{prefix}{type_}{suffix}");
    if container.has(&alias) && can_be_autowired(container, &alias)? {
        return Ok(Some(alias));
    }

    let fallback = |t: &str| -> Result<Option<String>> {
        if !prefix.is_empty() {
            return get_combined_alias(container, t, name, None);
        }
        if name != parsed.as_deref() && truthy(parsed.as_deref()) {
            return get_combined_alias(container, t, parsed.as_deref(), None);
        }
        Ok(None)
    };

    let members: Vec<&str> = if type_.contains('&') {
        type_.trim_matches(['(', ')']).split('&').collect()
    } else if type_.contains('|') {
        type_.split('|').collect()
    } else {
        return fallback(type_);
    };

    let mut alias: Option<String> = None;
    for t in members {
        let id = format!("{prefix}{t}{suffix}");
        if !container.has_alias(&id) {
            return fallback(t);
        }
        match &alias {
            None => alias = Some(alias_target(container, &id)),
            Some(a) => {
                if alias_target(container, &id) != *a
                    && resolve_alias(container, &id) != resolve_alias(container, a)
                {
                    return fallback(t);
                }
            }
        }
    }
    Ok(alias)
}

/// The `Target` attribute at index 0 of a reference's attributes
/// (`array_filter(...)[0]`), with its name.
fn first_target(reference: &Reference) -> Option<Value> {
    reference
        .attributes()
        .and_then(|a| a.get(&Key::Int(0)))
        .filter(|a| object_class(a).is_some_and(|c| key(&c) == key(TARGET)))
        .cloned()
}

fn targets(reference: &Reference) -> PhpArray {
    PhpArray::list(
        reference
            .attributes()
            .map(|a| {
                a.values()
                    .filter(|v| object_class(v).is_some_and(|c| key(&c) == key(TARGET)))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    )
}

/// `getAutowiredReference($reference, $filterType)`.
fn get_autowired_reference(
    container: &ContainerBuilder,
    reference: &Reference,
    filter_type: bool,
) -> Result<Option<Reference>> {
    let mut type_ = reference.type_().unwrap_or_default().to_string();
    if type_ != reference.id() {
        return Ok(Some(reference.clone()));
    }
    if filter_type && let Some(m) = type_.chars().find(|c| *c == '&' || *c == '|') {
        let builtins = [
            "int", "string", "array", "bool", "float", "iterable", "object", "callable", "null",
        ];
        let mut kept: Vec<&str> = type_.split(m).filter(|t| !builtins.contains(t)).collect();
        kept.sort_unstable();
        type_ = kept.join(&m.to_string());
    }

    let target = first_target(reference).and_then(|t| string_of(&property(&t, "name")));
    let behavior = reference.invalid_behavior();
    let wired = |id: &str| Reference::typed(id, type_.clone(), behavior, None, PhpArray::new());

    if let Some(name) = target
        .clone()
        .or_else(|| reference.name().map(String::from))
    {
        if let Some(alias) = get_combined_alias(container, &type_, Some(&name), target.as_deref())?
            && can_be_autowired(container, &alias)?
        {
            return Ok(Some(wired(&alias)));
        }
        if container.has(&name) && can_be_autowired(container, &name)? {
            let prefix = format!("{type_} $");
            for (id, alias) in container.get_aliases().iter() {
                if alias.id() == name && id.starts_with(&prefix) {
                    return Ok(Some(wired(&name)));
                }
            }
        }
        if target.is_some() {
            return Ok(None);
        }
    }

    if let Some(alias) = get_combined_alias(container, &type_, None, None)?
        && can_be_autowired(container, &alias)?
    {
        return Ok(Some(wired(&alias)));
    }
    Ok(None)
}

// ----- the "type not found" messages, on the snapshot -----

fn class_exists(registry: &ClassRegistry, name: &str) -> bool {
    registry
        .class(name)
        .is_some_and(|c| matches!(c.kind, ClassKind::Class | ClassKind::Enum))
}

impl TypesClone {
    fn new(container: &ContainerBuilder) -> Result<Self> {
        let mut clone = ContainerBuilder::with_parameter_bag(container.get_parameter_bag().clone());
        clone.set_class_registry(container.class_registry());
        clone.set_aliases(
            container
                .get_aliases()
                .iter()
                .map(|(id, alias)| (id.to_string(), alias.clone())),
        )?;
        clone.set_definitions(
            container
                .get_definitions()
                .iter()
                .map(|(id, definition)| (id.to_string(), definition.clone())),
        )?;
        clone.set_resource_tracking(false);
        Ok(Self {
            container: clone,
            populated: false,
            types: HashMap::new(),
            ambiguous: HashMap::new(),
            autowiring_aliases: HashMap::new(),
        })
    }

    /// `createTypeNotFoundMessage($reference, $label, $currentId)`.
    fn message(&mut self, reference: &Reference, label: &str, current_id: &str) -> Result<String> {
        let type_ = reference.type_().unwrap_or_default().to_string();

        let mut namespace = type_.clone();
        let mut i: Option<usize> = None;
        loop {
            if let Some(i) = i {
                namespace.truncate(i);
            }
            if self.container.has_definition(&namespace) {
                let tag = self
                    .container
                    .get_definition(&namespace)?
                    .tag("container.excluded");
                if !tag.is_empty() {
                    let source = tag
                        .get(&Key::Int(0))
                        .and_then(Value::as_array)
                        .and_then(|t| t.get(&Key::from("source")))
                        .filter(|s| !s.is_null())
                        .and_then(Value::to_php_string)
                        .map_or_else(
                            || "from autowiring".to_string(),
                            |s| s.to_string_lossy().into_owned(),
                        );
                    return Ok(format!(
                        "Cannot autowire service \"{current_id}\": {label} needs an instance of \"{type_}\" but this type has been excluded {source}."
                    ));
                }
            }
            match namespace.rfind('\\') {
                Some(at) => i = Some(at),
                None => break,
            }
        }

        let registry = self.container.class_registry();
        let reflected = self
            .container
            .get_reflection_class(Some(&type_), false)
            .ok()
            .flatten();
        let message = if let Some(r) = reflected {
            let alternatives = self.type_alternatives(&type_)?;
            let mut message = if let Some(target) = first_target(reference) {
                let target = match property(&target, "name") {
                    Value::Null => String::new(),
                    name => format!(
                        "('{}')",
                        name.to_php_string()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    ),
                };
                format!("has \"#[Target{target}]\" but no such target exists.{alternatives}")
            } else {
                let why = if self.container.has(&type_) {
                    "this service is abstract"
                } else {
                    "no such service exists"
                };
                format!(
                    "references {} \"{type_}\" but {why}.{alternatives}",
                    if r.is_interface() {
                        "interface"
                    } else {
                        "class"
                    }
                )
            };
            if r.is_interface() && alternatives.is_empty() {
                message.push_str(
                    " Did you create an instantiable class that implements this interface?",
                );
            }
            message
        } else {
            let parent_message = match registry.reflect(&type_) {
                Err(e) => format!("is missing a parent class ({e})"),
                Ok(_) => "was not found".to_string(),
            };
            format!("has type \"{type_}\" but this class {parent_message}.")
        };

        Ok(format!(
            "Cannot autowire service \"{current_id}\": {label} {message}"
        ))
    }

    /// `createTypeAlternatives($container, $reference)`.
    fn type_alternatives(&mut self, type_: &str) -> Result<String> {
        if let Some(message) = self.aliases_suggestion(type_)? {
            return Ok(format!(" {message}"));
        }
        if !self.populated {
            self.populate_available_types()?;
        }
        let registry = self.container.class_registry();
        let services = self.container.get_service_ids();
        let aliases: Vec<String> = self
            .autowiring_aliases
            .get(type_)
            .map(|a| a.iter().filter(|n| !n.is_empty()).cloned().collect())
            .unwrap_or_default();
        if !aliases.is_empty() {
            return Ok(format!(
                " Did you mean to target{} \"{}\" instead?",
                if aliases.len() > 1 { " one of" } else { "" },
                aliases.join("\", \"")
            ));
        }

        let lower = type_.to_ascii_lowercase();
        let message = if !self.container.has(type_)
            && let Some(found) = services.iter().find(|s| s.to_ascii_lowercase() == lower)
        {
            return Ok(format!(" Did you mean \"{found}\"?"));
        } else if let Some(ids) = self.ambiguous.get(type_) {
            format!("one of these existing services: \"{}\"", ids.join("\", \""))
        } else if let Some(id) = self.types.get(type_) {
            format!("the existing \"{id}\" service")
        } else {
            return Ok(String::new());
        };
        Ok(format!(
            " You should maybe alias this {} to {message}.",
            if class_exists(&registry, type_) {
                "class"
            } else {
                "interface"
            }
        ))
    }

    /// `getAliasesSuggestionForType($container, $type)`.
    fn aliases_suggestion(&self, type_: &str) -> Result<Option<String>> {
        let registry = self.container.class_registry();
        let mut parents: Vec<String> = Vec::new();
        if let Some(class) = registry.class(type_) {
            let mut current = class.parent.as_deref().and_then(|p| registry.class(p));
            while let Some(parent) = current {
                parents.push(parent.name.clone());
                current = parent.parent.as_deref().and_then(|p| registry.class(p));
            }
            for interface in registry.interface_names(&class) {
                if !parents.contains(&interface) {
                    parents.push(interface);
                }
            }
        }
        let mut aliases: Vec<String> = Vec::new();
        for parent in parents {
            if self.container.has(&parent) && can_be_autowired(&self.container, &parent)? {
                aliases.push(parent);
            }
        }
        let kind = |name: &str| {
            if class_exists(&registry, name) {
                "class"
            } else {
                "interface"
            }
        };
        if aliases.len() > 1 {
            let mut message = "Try changing the type-hint to one of its parents: ".to_string();
            let (last, rest) = aliases.split_last().unwrap_or((&aliases[0], &[]));
            for alias in rest {
                let _ = write!(message, "{} \"{alias}\", ", kind(alias));
            }
            let _ = write!(message, "or {} \"{last}\".", kind(last));
            return Ok(Some(message));
        }
        Ok(aliases
            .first()
            .map(|a| format!("Try changing the type-hint to \"{a}\" instead.")))
    }

    /// `populateAvailableTypes($container)`.
    fn populate_available_types(&mut self) -> Result<()> {
        self.populated = true;
        self.types.clear();
        self.ambiguous.clear();
        self.autowiring_aliases.clear();
        let definitions: Vec<(String, Definition)> = self
            .container
            .get_definitions()
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in definitions {
            self.populate_available_type(&id, &definition)?;
        }
        let aliases: Vec<String> = self
            .container
            .get_aliases()
            .iter()
            .map(|(id, _)| id.to_string())
            .collect();
        let mut prev: Option<String> = None;
        for id in aliases {
            self.populate_autowiring_alias(&id, prev.as_deref());
            prev = Some(id);
        }
        Ok(())
    }

    /// `populateAvailableType($container, $id, $definition)`.
    fn populate_available_type(&mut self, id: &str, definition: &Definition) -> Result<()> {
        if definition.is_abstract() {
            return Ok(());
        }
        if id.is_empty() || id.starts_with('.') || definition.is_deprecated() {
            return Ok(());
        }
        let Some(class) = self
            .container
            .get_reflection_class(definition.class().as_deref(), false)?
        else {
            return Ok(());
        };
        let registry = self.container.class_registry();
        for interface in registry.interface_names(&class) {
            self.set(&interface, id);
        }
        let mut current = Some(class);
        while let Some(c) = current {
            self.set(&c.name, id);
            current = c.parent.as_deref().and_then(|p| registry.class(p));
        }
        self.populate_autowiring_alias(id, None);
        Ok(())
    }

    /// `set($type, $id)`.
    fn set(&mut self, type_: &str, id: &str) {
        if let Some(ids) = self.ambiguous.get_mut(type_) {
            ids.push(id.to_string());
            return;
        }
        match self.types.get(type_) {
            None => {
                self.types.insert(type_.to_string(), id.to_string());
            }
            Some(existing) if existing == id => {}
            Some(existing) => {
                let existing = existing.clone();
                self.types.remove(type_);
                self.ambiguous
                    .insert(type_.to_string(), vec![existing, id.to_string()]);
            }
        }
    }

    /// `populateAutowiringAlias($id, $target)`.
    fn populate_autowiring_alias(&mut self, id: &str, target: Option<&str>) {
        fn is_name(s: &str) -> bool {
            let bytes = s.as_bytes();
            bytes
                .first()
                .is_some_and(|&b| b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f)
                && bytes
                    .iter()
                    .all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b >= 0x7f)
        }
        let (type_, mut name) = match id.split_once(" $") {
            Some((t, n)) => (t, n.to_string()),
            None => (id, String::new()),
        };
        if !type_.split('\\').all(is_name) || (id.contains(" $") && !is_name(&name)) {
            return;
        }
        let registry = self.container.class_registry();
        if registry
            .class(type_)
            .is_none_or(|c| c.kind == ClassKind::Trait)
        {
            return;
        }
        if let Some(target) = target
            && let Some(rest) = target.strip_prefix(&format!(".{type_} $"))
        {
            name = rest.to_string();
        }
        let names = self
            .autowiring_aliases
            .entry(type_.to_string())
            .or_default();
        if !names.contains(&name) {
            names.push(name);
        }
    }
}

impl AutowirePass {
    /// `new AutowirePass($throwOnAutowiringException)`.
    pub fn new(throw_on_autowiring_exception: bool) -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            throw_on_autowiring_exception,
            decorated_class: None,
            decorated_id: None,
            restore: None,
            default_names: Vec::new(),
            types_clone: Arc::new(Mutex::new(None)),
            generation: 0,
            pending_patches: Vec::new(),
        }
    }

    fn current_id(&self) -> String {
        self.state.current_id.clone().unwrap_or_default()
    }

    /// `createTypeNotFoundMessageCallback($reference, $label)`.
    fn type_not_found_callback(
        &mut self,
        container: &ContainerBuilder,
        reference: &Reference,
        label: String,
    ) -> Result<Arc<dyn Fn() -> String + Send + Sync>> {
        {
            let mut types = self
                .types_clone
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if types.is_none() {
                *types = Some(TypesClone::new(container)?);
            }
        }
        let types = self.types_clone.clone();
        let reference = reference.clone();
        let current_id = self.current_id();
        Ok(Arc::new(move || {
            let mut types = types.lock().unwrap_or_else(PoisonError::into_inner);
            let Some(types) = types.as_mut() else {
                return String::new();
            };
            types
                .message(&reference, &label, &current_id)
                .unwrap_or_else(|e| e.message())
        }))
    }

    /// `$getValue()`.
    fn get_value(&mut self, container: &mut ContainerBuilder, ctx: &GetValue) -> Result<Value> {
        if ctx.target.is_none()
            && let Some(named_alias) =
                get_combined_alias(container, &ctx.type_, Some(&ctx.name), None)?
            && can_be_autowired(container, &named_alias)?
        {
            trigger_deprecation(
                b"symfony/dependency-injection",
                b"8.1",
                format!(
                    "Relying solely on the name of parameter \"${}\" of \"{}()\" to match a named autowiring alias is deprecated; use the \"#[Target]\" attribute.",
                    ctx.parameter.name,
                    ctx.function()
                )
                .as_bytes(),
            );
        }
        let reference = Reference::typed(
            ctx.type_.clone(),
            ctx.type_.clone(),
            InvalidBehavior::Exception,
            Some(ctx.name.clone()),
            PhpArray::list(ctx.target.clone()),
        );
        if let Some(found) = get_autowired_reference(container, &reference, false)? {
            return Ok(Value::from(found));
        }
        let label = format!(
            "argument \"${}\" of method \"{}()\"",
            ctx.parameter.name,
            ctx.function()
        );
        let failure = self.type_not_found_callback(container, &reference, label)?;
        if let Some(default) = &ctx.parameter.default {
            return Ok(marker(
                container.get_parameter_bag().escape_value(default),
                ctx.names_id,
            ));
        }
        if !allows_null(&ctx.parameter) {
            return Err(Error::autowiring_failed_lazy(
                ctx.current_id.clone(),
                LazyMessage::new(move || failure()),
            ));
        }
        Ok(Value::Null)
    }

    /// `$restorePreviousValue()`: writes `$getValue()` where the inner
    /// service was injected.
    fn run_restore(
        &mut self,
        container: &mut ContainerBuilder,
        restore: Restore,
        position: usize,
        arguments: &mut PhpArray,
    ) -> Result<()> {
        let value = self.get_value(container, &restore.get_value)?;
        match restore.target {
            RestoreTarget::Local {
                generation,
                position: p,
            } if generation == self.generation => {
                if p == position {
                    arguments.insert(restore.key, value);
                } else {
                    self.pending_patches.push((p, restore.key, value));
                }
            }
            RestoreTarget::Local { .. } => {}
            RestoreTarget::Definition {
                definition,
                constructor,
                call_key,
            } => {
                if constructor {
                    let mut args = definition.arguments();
                    args.insert(restore.key, value);
                    definition.set_arguments(args);
                } else {
                    let mut calls = definition.method_calls();
                    if let Some(Value::Array(call)) = calls.get_mut(&call_key) {
                        let mut args = call
                            .get(&Key::Int(1))
                            .and_then(Value::as_array)
                            .cloned()
                            .unwrap_or_default();
                        args.insert(restore.key, value);
                        call.insert(1, Value::Array(args));
                    }
                    definition.set_method_calls(&calls)?;
                }
            }
        }
        Ok(())
    }

    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn do_process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let mut value = value;
        if let Some(reference) = Reference::from_value(&value)
            .filter(|r| r.is_typed())
            .cloned()
        {
            let mut reference = reference;
            let attributes = reference.attributes().cloned().unwrap_or_default();
            for attribute in attributes.values() {
                let v = self.process_value(container, attribute.clone(), false)?;
                if attribute.identical(&v) {
                    continue;
                }
                let found_targets = targets(&reference);
                if let Some(closure) = ServiceClosureArgument::from_value(&v)
                    && let Some(r) = Reference::from_value(&closure.value())
                    && !found_targets.is_empty()
                {
                    let r = r.clone();
                    let typed = Reference::typed(
                        r.id(),
                        r.id(),
                        r.invalid_behavior(),
                        reference.name().map(String::from),
                        found_targets,
                    );
                    let wired = get_autowired_reference(container, &typed, true)?
                        .map_or_else(|| Value::from(r), Value::from);
                    return Ok(ServiceClosureArgument::new(wired).to_value());
                }
                let registry = container.class_registry();
                let Some(v_ref) = Reference::from_value(&v)
                    .filter(|_| is_instance(&registry, attribute, AUTOWIRE))
                    .cloned()
                else {
                    return Ok(v);
                };
                let invalid_behavior = if v_ref.invalid_behavior() == InvalidBehavior::Exception {
                    reference.invalid_behavior()
                } else {
                    v_ref.invalid_behavior()
                };
                reference = if v_ref.is_typed() {
                    let mut merged: Vec<Value> = v_ref
                        .attributes()
                        .map(|a| a.values().cloned().collect())
                        .unwrap_or_default();
                    merged.extend(attributes.values().cloned());
                    Reference::typed(
                        v_ref.id(),
                        v_ref.type_().unwrap_or_default(),
                        invalid_behavior,
                        v_ref.name().or(reference.name()).map(String::from),
                        PhpArray::list(merged),
                    )
                } else {
                    Reference::typed(
                        v_ref.id(),
                        reference.type_().unwrap_or_default(),
                        invalid_behavior,
                        reference.name().map(String::from),
                        attributes.clone(),
                    )
                };
                break;
            }
            if let Some(found) = get_autowired_reference(container, &reference, true)? {
                return Ok(Value::from(found));
            }
            if reference.invalid_behavior() == InvalidBehavior::RuntimeException {
                let message =
                    self.type_not_found_callback(container, &reference, "it".to_string())?;
                let id = format!(".errored.{}.{}", self.current_id(), reference.id());
                container
                    .register(&id, reference.type_())?
                    .add_error(DefinitionError::Lazy(message));
                return Ok(Value::from(Reference::typed(
                    id,
                    reference.type_().unwrap_or_default(),
                    reference.invalid_behavior(),
                    reference.name().map(String::from),
                    PhpArray::new(),
                )));
            }
            value = Value::from(reference);
        }
        let value = self.parent_process_value(container, value, is_root)?;

        let Some(definition) = Definition::from_value(&value) else {
            return Ok(value);
        };
        let class = definition.class();
        if !definition.is_autowired() || definition.is_abstract() || !truthy(class.as_deref()) {
            return Ok(value);
        }
        let Some(reflection_class) = container.get_reflection_class(class.as_deref(), false)?
        else {
            container.log(
                CLASS,
                &format!(
                    "Skipping service \"{}\": Class or interface \"{}\" cannot be loaded.",
                    self.current_id(),
                    class.unwrap_or_default()
                ),
            )?;
            return Ok(value);
        };

        let original_calls = definition.method_calls();
        let constructor = match self.get_constructor(container, &definition, false) {
            Ok(c) => c,
            Err(e) if e.is(&ErrorKind::Runtime) => {
                return Err(
                    Error::autowiring_failed(self.current_id(), e.message()).with_previous(e)
                );
            }
            Err(e) => return Err(e),
        };

        let mut calls: Vec<Call> = Vec::new();
        let has_constructor = constructor.is_some();
        if let Some(constructor) = constructor {
            let arguments = definition.arguments();
            calls.push(Call {
                key: Key::Int(0),
                method: CallMethod::Reflector(constructor),
                raw: PhpArray::list([Value::Null, Value::Array(arguments.clone())]),
                arguments,
            });
        }
        for (k, call) in original_calls.iter() {
            let raw = call.as_array().cloned().unwrap_or_default();
            let name = raw
                .get(&Key::Int(0))
                .and_then(Value::to_php_string)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            calls.push(Call {
                key: k.clone(),
                method: CallMethod::Name(name),
                arguments: raw
                    .get(&Key::Int(1))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                raw,
            });
        }

        let check_attributes = !definition.has_tag("container.ignore_attributes");
        let mut calls = self.autowire_calls(
            container,
            &definition,
            calls,
            &reflection_class,
            is_root,
            check_attributes,
            has_constructor,
        )?;

        if has_constructor && !calls.is_empty() {
            let first = calls.remove(0);
            if first.arguments != definition.arguments() {
                definition.set_arguments(first.arguments);
            }
        }

        let mut new_calls = PhpArray::new();
        for call in calls {
            let mut raw = call.raw;
            raw.insert(1, Value::Array(call.arguments));
            if has_constructor {
                let _ = new_calls.push(Value::Array(raw));
            } else {
                new_calls.insert(call.key, Value::Array(raw));
            }
        }
        if new_calls != original_calls {
            definition.set_method_calls(&new_calls)?;
        }

        Ok(value)
    }

    /// `autowireCalls($methodCalls, $reflectionClass, $isRoot,
    /// $checkAttributes)`.
    #[expect(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "upstream's state; one upstream method"
    )]
    fn autowire_calls(
        &mut self,
        container: &mut ContainerBuilder,
        definition: &Definition,
        mut calls: Vec<Call>,
        reflection_class: &ClassMetadata,
        is_root: bool,
        check_attributes: bool,
        has_constructor: bool,
    ) -> Result<Vec<Call>> {
        if is_root {
            self.decorated_id = None;
            self.decorated_class = None;
            self.restore = None;

            let current = container.get_definition(&self.current_id())?;
            self.decorated_id = current.inner_service_id();
            if let Some(decorated_id) = self.decorated_id.clone()
                && container.has(&decorated_id)
            {
                self.decorated_class = container.find_definition(&decorated_id)?.class();
            }
        }
        self.generation += 1;
        let generation = self.generation;

        let mut patched: Vec<usize> = Vec::new();
        for position in 0..calls.len() {
            let reflector = match &calls[position].method {
                CallMethod::Reflector(r) => r.clone(),
                CallMethod::Name(name) => {
                    let class_definition =
                        Definition::new(Some(&reflection_class.name), PhpArray::new());
                    self.get_reflection_method(container, &class_definition, name)?
                }
            };
            let original = calls[position].arguments.clone();
            let arguments = self.autowire_method(
                container,
                &reflector,
                original.clone(),
                check_attributes,
                position,
            )?;
            for (p, k, v) in std::mem::take(&mut self.pending_patches) {
                if let Some(call) = calls.get_mut(p) {
                    call.arguments.insert(k, v);
                }
            }
            if arguments != original {
                calls[position].arguments = arguments;
                if !patched.contains(&position) {
                    patched.push(position);
                }
            }
        }

        // use named arguments to skip complex default values
        for i in patched {
            let arguments = calls[i].arguments.clone();
            let mut out = arguments.clone();
            let mut named: Option<usize> = None;
            for (j, value) in arguments.iter() {
                if let Some(names_id) = named
                    && !is_marker(value)
                {
                    out.remove(j);
                    let name = match j {
                        Key::Int(j) => usize::try_from(*j)
                            .ok()
                            .and_then(|j| self.default_names.get(names_id)?.get(j).cloned())
                            .unwrap_or_default(),
                        Key::Str(_) => String::new(),
                    };
                    out.insert(Key::from(name.as_str()), value.clone());
                }
                if !is_marker(value) {
                    continue;
                }
                let default = property(value, "value");
                let complex = match &default {
                    Value::Array(a) => !a.is_empty(),
                    Value::Object(_) => true,
                    _ => false,
                };
                if complex {
                    out.remove(j);
                    named = Some(marker_names_id(value));
                }
                if named.is_some() {
                    out.remove(j);
                } else {
                    out.insert(j.clone(), default);
                }
            }
            calls[i].arguments = out;
        }

        if let Some(restore) = &mut self.restore
            && let RestoreTarget::Local {
                generation: g,
                position,
            } = restore.target
            && g == generation
        {
            let constructor = has_constructor && position == 0;
            let call_key = if has_constructor {
                index_key(position.saturating_sub(1))
            } else {
                calls[position].key.clone()
            };
            restore.target = RestoreTarget::Definition {
                definition: definition.clone(),
                constructor,
                call_key,
            };
        }

        Ok(calls)
    }

    /// `autowireMethod($reflectionMethod, $arguments, $checkAttributes)`.
    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn autowire_method(
        &mut self,
        container: &mut ContainerBuilder,
        reflector: &FunctionReflector,
        mut arguments: PhpArray,
        check_attributes: bool,
        position: usize,
    ) -> Result<PhpArray> {
        let current_id = self.current_id();
        let class = if reflector.is_method() {
            reflector.class().unwrap_or_default().to_string()
        } else {
            current_id.clone()
        };
        let method = reflector.name().to_string();
        let mut parameters = reflector.parameters();
        if parameters.last().is_some_and(|p| p.is_variadic) {
            parameters.pop();
        }
        let names_id = self.default_names.len();
        self.default_names.push(Vec::new());

        let registry = container.class_registry();
        let declaring_parent = reflector
            .class()
            .and_then(|c| registry.class(c))
            .and_then(|c| c.parent.clone());
        let declaring = DeclaringClass {
            name: reflector.class(),
            parent: declaring_parent.as_deref(),
        };
        let function = if reflector.is_method() {
            format!(
                "{}::{}",
                reflector.class().unwrap_or_default(),
                reflector.name()
            )
        } else {
            reflector.name().to_string()
        };

        let mut index: i64 = -1;
        'params: for (i, parameter) in parameters.iter().enumerate() {
            index = i64::try_from(i).unwrap_or(i64::MAX);
            let idx = Key::Int(index);
            self.default_names[names_id].push(parameter.name.clone());

            let by_name = Key::from(parameter.name.as_str());
            if let Some(v) = arguments.get(&by_name).cloned() {
                arguments.insert(idx.clone(), v);
                arguments.remove(&by_name);
            }
            if arguments
                .get(&idx)
                .is_some_and(|v| !matches!(v, Value::String(s) if s.is_empty()))
            {
                continue;
            }

            let type_ = export_type(parameter, true, declaring).unwrap_or_default();
            let (name, _) = parse_target_name(parameter, &function)?;
            let target = parameter
                .attributes
                .iter()
                .find(|a| key(&a.class) == key(TARGET))
                .map(new_attribute_instance)
                .transpose()?
                .map(|t| {
                    if property(&t, "name").is_null() {
                        let mut properties = PhpArray::new();
                        properties.insert("name", Value::from(parameter.name.as_str()));
                        object(TARGET, properties)
                    } else {
                        t
                    }
                });
            let ctx = GetValue {
                type_: type_.clone(),
                parameter: parameter.clone(),
                class: class.clone(),
                method: method.clone(),
                name: name.clone(),
                target: target.clone(),
                names_id,
                current_id: current_id.clone(),
            };

            if check_attributes {
                let attributes: Vec<&AttributeMetadata> = parameter
                    .attributes
                    .iter()
                    .filter(|a| attribute_is_a(&registry, &a.class, AUTOWIRE))
                    .chain(
                        parameter
                            .attributes
                            .iter()
                            .filter(|a| attribute_is_a(&registry, &a.class, LAZY)),
                    )
                    .collect();
                if attributes.len() > 1 {
                    return Err(Error::autowiring_failed(
                        current_id,
                        "Using both attributes #[Lazy] and #[Autowire] on an argument is not allowed; use the \"lazy\" parameter of #[Autowire] instead.",
                    ));
                }

                if let Some(attribute) = attributes.first() {
                    let mut attribute = new_autowire_attribute(attribute)?;
                    let is_autowire = is_instance(&registry, &attribute, AUTOWIRE);
                    let mut value = if is_autowire {
                        property(&attribute, "value")
                    } else {
                        Value::Null
                    };

                    let default = parameter.default.clone().unwrap_or_default();
                    if let Some(mut text) = string_of(&value) {
                        let is_pure_env = text.starts_with("%env(")
                            && text.ends_with(")%")
                            && text.len() >= 7
                            && !text[5..text.len() - 2].contains('%');

                        if is_pure_env
                            && parameter.default.is_some()
                            && default.is_null()
                            && !has_default_processor(&text)
                        {
                            text.insert_str(5, "default::");
                            value = Value::from(text.as_str());
                            attribute = new_attribute_instance(&AttributeMetadata {
                                class: AUTOWIRE.to_string(),
                                arguments: PhpArray::list([value.clone()]),
                            })?;
                        }
                        if is_pure_env
                            && named_type(parameter.type_.as_deref()) == Some("bool")
                            && !text.starts_with("%env(bool:")
                        {
                            text.insert_str(5, "bool:");
                            value = Value::from(text.as_str());
                            attribute = new_attribute_instance(&AttributeMetadata {
                                class: AUTOWIRE.to_string(),
                                arguments: PhpArray::list([value.clone()]),
                            })?;
                        }

                        let resolved =
                            match container.get_parameter_bag().resolve_value(&value, &[]) {
                                Ok(r) => r,
                                Err(e) if e.is(&ErrorKind::ParameterNotFound) => Value::Null,
                                Err(e) => return Err(e),
                            };
                        if let Some(resolved) = string_of(&resolved)
                            && has_env_placeholder(&resolved)
                        {
                            for t in type_.split('|') {
                                if t != "Closure" && t != "Stringable" {
                                    continue;
                                }
                                arguments.insert(
                                    idx.clone(),
                                    Value::from(EnvClosureArgument::new(
                                        resolved.clone(),
                                        default.clone(),
                                        t == "Stringable",
                                    )?),
                                );
                                continue 'params;
                            }
                        }
                    }

                    let invalid_behavior = if allows_null(parameter) {
                        InvalidBehavior::Null
                    } else {
                        InvalidBehavior::Exception
                    };
                    let mut attribute_list = vec![attribute.clone()];
                    attribute_list.extend(target.clone());
                    let typed_reference = Reference::typed(
                        if type_.is_empty() {
                            "?"
                        } else {
                            type_.as_str()
                        },
                        if type_.is_empty() {
                            "mixed"
                        } else {
                            type_.as_str()
                        },
                        invalid_behavior,
                        Some(name.clone()),
                        PhpArray::list(attribute_list),
                    );
                    let mut value =
                        match self.process_value(container, Value::from(typed_reference), false) {
                            Ok(v) => v,
                            Err(e) if e.is(&ErrorKind::ParameterNotFound) => {
                                let Some(default) = parameter.default.clone() else {
                                    return Err(Error::autowiring_failed(current_id, e.message())
                                        .with_previous(e));
                                };
                                arguments.insert(idx.clone(), marker(default, names_id));
                                continue 'params;
                            }
                            Err(e) => return Err(e),
                        };

                    let lazy = property(&attribute, "lazy");
                    if is_instance(&registry, &attribute, AUTOWIRE_INLINE) {
                        let definition = build_definition(
                            &registry, &attribute, value, &type_, parameter, reflector,
                        )?;
                        value = self.do_process_value(container, definition.to_value(), false)?;
                    } else if lazy.is_truthy() {
                        if matches!(lazy, Value::Bool(true)) && type_.contains('|') {
                            return Err(Error::autowiring_failed(
                                current_id.clone(),
                                format!(
                                    "Cannot use #[Autowire] with option \"lazy: true\" on union types for service \"{current_id}\"; set the option to the interface(s) that should be proxied instead."
                                ),
                            ));
                        }
                        if value.is_null() {
                            value = self.get_value(container, &ctx)?;
                        }
                        let Some(reference) = Reference::from_value(&value).cloned() else {
                            arguments.insert(idx.clone(), value);
                            continue 'params;
                        };
                        if reference.invalid_behavior() != InvalidBehavior::Exception
                            && !container.has(reference.id())
                        {
                            arguments.insert(idx.clone(), value);
                            continue 'params;
                        }
                        let lazy_list: Vec<String> = match &lazy {
                            Value::Array(list) => list
                                .values()
                                .filter_map(Value::to_php_string)
                                .map(|s| s.to_string_lossy().into_owned())
                                .collect(),
                            _ if type_.contains('&') && !type_.contains('|') => type_
                                .trim_matches(['(', ')'])
                                .split('&')
                                .map(String::from)
                                .collect(),
                            Value::String(s) => vec![s.to_string_lossy().into_owned()],
                            _ => Vec::new(),
                        };
                        if lazy_list.is_empty()
                            && container.has(reference.id())
                            && container.find_definition(reference.id())?.is_lazy()
                        {
                            arguments.insert(idx.clone(), value);
                            continue 'params;
                        }
                        let proxy_type = if lazy_list.is_empty() {
                            Self::resolve_proxy_type(container, &type_, reference.id())?
                        } else {
                            type_.clone()
                        };
                        let definition = Definition::new(
                            Some(&proxy_type),
                            PhpArray::list([Value::Array(PhpArray::list([value.clone()]))]),
                        );
                        definition
                            .set_factory(Value::from("current"))
                            .set_lazy(true);
                        if !lazy_list.is_empty() {
                            if container
                                .get_reflection_class(Some(&proxy_type), false)?
                                .is_none()
                            {
                                definition.set_class(Some("object"));
                            }
                            for v in &lazy_list {
                                let mut tag = PhpArray::new();
                                tag.insert("interface", Value::from(v.as_str()));
                                definition.add_tag("proxy", tag);
                            }
                        }
                        let mut id = reference.id().to_string();
                        let proxy_tag = definition.tag("proxy");
                        if definition.class().as_deref() != Some(id.as_str())
                            || !proxy_tag.is_empty()
                        {
                            let hashed = ContainerBuilder::hash(&Value::Array(PhpArray::list([
                                definition.class().map_or(Value::Null, Value::from),
                                Value::Array(proxy_tag),
                            ])));
                            id = format!("{id}.{hashed}");
                        }
                        let id = format!(".lazy.{id}");
                        container.set_definition(&id, definition)?;
                        value = Value::from(Reference::new(id));
                    }
                    arguments.insert(idx.clone(), value);
                    continue 'params;
                }

                if parameter
                    .attributes
                    .iter()
                    .any(|a| key(&a.class) == key(AUTOWIRE_DECORATED))
                {
                    let decorated = object(AUTOWIRE_DECORATED, PhpArray::new());
                    let value = self.process_value(container, decorated, false)?;
                    arguments.insert(idx.clone(), value);
                    continue 'params;
                }
            }

            if type_.is_empty() {
                if arguments.get(&idx).is_some_and(|v| !v.is_null()) {
                    continue;
                }

                // no default value? Then fail
                let Some(default) = &parameter.default else {
                    // For core classes, isDefaultValueAvailable() can
                    // be false when isOptional() returns true. If the
                    // argument *is* optional, allow it to be missing
                    if parameter.is_optional {
                        index -= 1;
                        break;
                    }
                    let exported = export_type(parameter, false, declaring).unwrap_or_default();
                    let hint = if exported.is_empty() {
                        "has no type-hint".to_string()
                    } else {
                        format!("is type-hinted \"{}\"", strip_type_backslashes(&exported))
                    };
                    return Err(Error::autowiring_failed(
                        current_id.clone(),
                        format!(
                            "Cannot autowire service \"{current_id}\": argument \"${}\" of method \"{}()\" {hint}, you should configure its value explicitly.",
                            parameter.name,
                            ctx.function()
                        ),
                    ));
                };

                // specifically pass the default value
                arguments.insert(
                    idx.clone(),
                    marker(
                        container.get_parameter_bag().escape_value(default),
                        names_id,
                    ),
                );
                continue;
            }

            if let Some(decorated_class) = self.decorated_class.clone()
                && registry.class(&decorated_class).is_some()
                && registry.is_a(&decorated_class, &type_)
            {
                if let Some(restore) = self.restore.take() {
                    // The inner service is injected only if there is only 1 argument matching the type of the decorated class
                    // across all arguments of all autowired methods.
                    // If a second matching argument is found, the default behavior is restored.
                    self.run_restore(container, restore, position, &mut arguments)?;
                    self.decorated_class = None; // Prevent further checks
                } else {
                    arguments.insert(
                        idx.clone(),
                        Value::from(Reference::typed(
                            self.decorated_id.clone().unwrap_or_default(),
                            decorated_class,
                            InvalidBehavior::Exception,
                            None,
                            PhpArray::new(),
                        )),
                    );
                    self.restore = Some(Restore {
                        target: RestoreTarget::Local {
                            generation: self.generation,
                            position,
                        },
                        key: idx.clone(),
                        get_value: ctx,
                    });
                    continue;
                }
            }

            let value = self.get_value(container, &ctx)?;
            arguments.insert(idx, value);
        }

        if !parameters.is_empty() {
            index += 1;
            if arguments.get(&Key::Int(index)).is_none_or(Value::is_null) {
                loop {
                    index -= 1;
                    if index < 0 {
                        break;
                    }
                    let k = Key::Int(index);
                    if !arguments.get(&k).is_some_and(is_marker) {
                        break;
                    }
                    arguments.remove(&k);
                }
            }
        }

        // it's possible index 1 was set, then index 0, then 2, etc
        // make sure that we re-order so they're injected as expected
        Ok(ksort_natural(&arguments))
    }

    /// `resolveProxyType($originalType, $serviceId)`.
    fn resolve_proxy_type(
        container: &mut ContainerBuilder,
        original_type: &str,
        service_id: &str,
    ) -> Result<String> {
        if !container.has(service_id) {
            return Ok(original_type.to_string());
        }
        let class = container.find_definition(service_id)?.class();
        let resolved = container
            .get_parameter_bag()
            .resolve_value(&class.map_or(Value::Null, Value::from), &[])?;
        let Some(resolved) = string_of(&resolved).filter(|r| truthy(Some(r))) else {
            return Ok(original_type.to_string());
        };
        if container
            .get_reflection_class(Some(&resolved), false)?
            .is_none()
        {
            return Ok(original_type.to_string());
        }
        Ok(resolved)
    }
}

/// `preg_replace('/(^|[(|&])\\\\|^\?\\\\?/', '\1', $type)`.
fn strip_type_backslashes(type_: &str) -> String {
    let type_ = type_
        .strip_prefix("?\\")
        .or_else(|| type_.strip_prefix('?'))
        .map_or_else(|| type_.to_string(), String::from);
    let mut out = String::with_capacity(type_.len());
    let mut previous: Option<char> = None;
    for c in type_.chars() {
        if c == '\\' && matches!(previous, None | Some('(' | '|' | '&')) {
            previous = Some(c);
            continue;
        }
        out.push(c);
        previous = Some(c);
    }
    out
}

impl CompilerPassInterface for AutowirePass {
    fn class(&self) -> &str {
        CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        self.types_clone = Arc::new(Mutex::new(None));
        let result = recursive_process(self, container);
        self.decorated_class = None;
        self.decorated_id = None;
        self.restore = None;
        self.default_names.clear();
        self.pending_patches.clear();
        self.types_clone = Arc::new(Mutex::new(None));
        result
    }
}

impl AbstractRecursivePass for AutowirePass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        let registry = container.class_registry();
        if is_instance(&registry, &value, AUTOWIRE) {
            let resolved = container
                .get_parameter_bag()
                .resolve_value(&property(&value, "value"), &[])?;
            let value = self.process_value(container, resolved, false)?;
            // count env vars referenced by the attribute right away, so that removing the
            // owning service (e.g. unused with an unrelated autowiring error) does not later
            // report the env var as never used
            container.resolve_env_placeholders(value.clone(), &EnvFormat::Placeholder, None)?;
            return Ok(value);
        }

        if object_class(&value).is_some_and(|c| key(&c) == key(AUTOWIRE_DECORATED)) {
            let definition = container.get_definition(&self.current_id())?;
            return Ok(Value::from(Reference::with_behavior(
                definition
                    .inner_service_id()
                    .unwrap_or_else(|| format!("{}.inner", self.current_id())),
                definition
                    .decoration_on_invalid()
                    .unwrap_or(InvalidBehavior::Null),
            )));
        }

        match self.do_process_value(container, value.clone(), is_root) {
            Err(e) if e.is(&ErrorKind::AutowiringFailed) => {
                if self.throw_on_autowiring_exception {
                    return Err(e);
                }
                let error = match e.message_callback() {
                    Some(lazy) => DefinitionError::Lazy(Arc::new(move || lazy.get())),
                    None => DefinitionError::Text(e.message()),
                };
                container
                    .get_definition(&self.current_id())?
                    .add_error(error);
                self.parent_process_value(container, value, is_root)
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_placeholders() {
        assert!(has_env_placeholder(
            "env_0123456789abcdef_FOO_0123456789abcdef0123456789abcdef"
        ));
        assert!(has_env_placeholder(
            "x env_0123456789abcdef_bool_FOO_BAR_0123456789abcdef0123456789abcdef y"
        ));
        assert!(!has_env_placeholder("env_0123456789abcdef_FOO_0123"));
        assert!(!has_env_placeholder("%env(FOO)%"));
    }

    #[test]
    fn type_backslashes() {
        assert_eq!("Foo\\Bar|Baz", strip_type_backslashes("\\Foo\\Bar|\\Baz"));
        assert_eq!("Foo", strip_type_backslashes("?\\Foo"));
        assert_eq!("array", strip_type_backslashes("array"));
    }

    #[test]
    fn nullable_types() {
        let p = |t: Option<&str>| ParameterMetadata::new("p", t);
        assert!(allows_null(&p(None)));
        assert!(allows_null(&p(Some("?Foo"))));
        assert!(allows_null(&p(Some("Foo|null"))));
        assert!(allows_null(&p(Some("(A&B)|null"))));
        assert!(allows_null(&p(Some("mixed"))));
        assert!(!allows_null(&p(Some("Foo"))));
        assert!(!allows_null(&p(Some("A&B"))));
    }
}
