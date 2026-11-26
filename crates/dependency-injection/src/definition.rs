//! Port of Symfony\Component\DependencyInjection\Definition.php and
//! ChildDefinition.php.
//!
//! A [`Definition`] is a shared PHP object (see [`crate::object`]): the
//! handle `getDefinition()` returns is the definition the container
//! holds, and an inline definition is the same object wherever its value
//! appears. A `ChildDefinition` is a definition with a parent id
//! ([`Definition::child`]): its class name is `ChildDefinition`, it is
//! still `instanceof Definition`, and `getArgument()` / `replaceArgument()`
//! behave as upstream's override.
//!
//! The arrays (arguments, properties, method calls, tags, bindings,
//! instanceof conditionals) are PHP arrays with PHP's keys, so a removed
//! method call leaves a gap as `unset()` does. The factory and the
//! configurator are PHP values (`string|array|null`). Identifiers (class,
//! file, parent) are `String`s.
//!
//! Upstream: symfony/dependency-injection Definition.php ChildDefinition.php

use std::fmt;
use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, array::Key};

use crate::alias::{Deprecation, check_deprecation_template};
use crate::argument::BoundArgument;
use crate::container_interface::InvalidBehavior;
use crate::exception::{Error, Result};
use crate::object::{ObjectData, Shared};
use crate::reference::Reference;

const DEFAULT_DEPRECATION_TEMPLATE: &str = "The \"%service_id%\" service is deprecated. You should stop using it, as it will be removed in the future.";

/// An argument index as PHP passes it to `getArgument()` and
/// `replaceArgument()` (`int|string`): `ChildDefinition` tells `0` from
/// `"0"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Index {
    Int(i64),
    Str(String),
}

impl Index {
    /// The array key PHP uses for it.
    pub fn key(&self) -> Key {
        match self {
            Self::Int(i) => Key::Int(*i),
            Self::Str(s) => Key::from(s.as_str()),
        }
    }
}

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(i) => write!(f, "{i}"),
            Self::Str(s) => f.write_str(s),
        }
    }
}

impl From<i64> for Index {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

impl From<&str> for Index {
    fn from(s: &str) -> Self {
        Self::Str(s.to_string())
    }
}

impl From<String> for Index {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<&Key> for Index {
    fn from(key: &Key) -> Self {
        match key {
            Key::Int(i) => Self::Int(*i),
            Key::Str(s) => Self::Str(s.to_string_lossy().into_owned()),
        }
    }
}

/// `getDecoratedService()`: `[$id, $renamedId, $priority]`, with the
/// invalid behaviour as a fourth entry when it is not the default.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecoratedService {
    pub id: String,
    pub renamed_id: Option<String>,
    pub priority: i64,
    pub invalid_behavior: Option<InvalidBehavior>,
}

impl DecoratedService {
    /// The PHP array.
    pub fn to_array(&self) -> PhpArray {
        let mut array = PhpArray::list([
            Value::from(self.id.as_str()),
            self.renamed_id.as_deref().map_or(Value::Null, Value::from),
            Value::Int(self.priority),
        ]);
        if let Some(behavior) = self.invalid_behavior {
            let _ = array.push(Value::Int(behavior.as_int()));
        }
        array
    }
}

/// An entry of `addError()`: a message, or a closure that renders one
/// when `getErrors()` is called.
#[derive(Clone)]
pub enum DefinitionError {
    Text(String),
    Lazy(Arc<dyn Fn() -> String + Send + Sync>),
}

impl fmt::Debug for DefinitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => write!(f, "{text:?}"),
            Self::Lazy(_) => f.write_str("Closure"),
        }
    }
}

#[derive(Debug, Clone)]
#[expect(clippy::struct_excessive_bools, reason = "upstream's fields")]
pub struct DefinitionData {
    /// `ChildDefinition`'s parent; `None` for a plain `Definition`.
    parent: Option<String>,
    class: Option<String>,
    file: Option<String>,
    factory: Value,
    shared: bool,
    deprecation: Option<Deprecation>,
    properties: PhpArray,
    calls: PhpArray,
    instanceof: PhpArray,
    autoconfigured: bool,
    configurator: Value,
    tags: PhpArray,
    public: bool,
    synthetic: bool,
    abstract_: bool,
    lazy: bool,
    decorated_service: Option<DecoratedService>,
    autowired: bool,
    /// The keys of `$changes`, in insertion order (every value is `true`).
    changes: Vec<String>,
    bindings: PhpArray,
    errors: Vec<DefinitionError>,
    arguments: PhpArray,
    inner_service_id: Option<String>,
    decoration_on_invalid: Option<InvalidBehavior>,
    decoration_priority: Option<i64>,
}

impl Default for DefinitionData {
    fn default() -> Self {
        Self {
            parent: None,
            class: None,
            file: None,
            factory: Value::Null,
            shared: true,
            deprecation: None,
            properties: PhpArray::new(),
            calls: PhpArray::new(),
            instanceof: PhpArray::new(),
            autoconfigured: false,
            configurator: Value::Null,
            tags: PhpArray::new(),
            public: false,
            synthetic: false,
            abstract_: false,
            lazy: false,
            decorated_service: None,
            autowired: false,
            changes: Vec::new(),
            bindings: PhpArray::new(),
            errors: Vec::new(),
            arguments: PhpArray::new(),
            inner_service_id: None,
            decoration_on_invalid: None,
            decoration_priority: None,
        }
    }
}

impl DefinitionData {
    fn change(&mut self, key: &str) {
        if !self.changes.iter().any(|k| k == key) {
            self.changes.push(key.to_string());
        }
    }
}

fn opt(s: Option<&str>) -> Value {
    s.map_or(Value::Null, Value::from)
}

impl ObjectData for DefinitionData {
    fn class(&self) -> &'static str {
        if self.parent.is_some() {
            Definition::CHILD_CLASS
        } else {
            Definition::CLASS
        }
    }

    fn properties(&self) -> PhpArray {
        let mut p = PhpArray::new();
        if let Some(parent) = &self.parent {
            p.insert("parent", Value::from(parent.as_str()));
        }
        p.insert("class", opt(self.class.as_deref()));
        p.insert("file", opt(self.file.as_deref()));
        p.insert("factory", self.factory.clone());
        p.insert("shared", Value::Bool(self.shared));
        p.insert(
            "deprecation",
            Value::Array(
                self.deprecation
                    .as_ref()
                    .map(Deprecation::to_array)
                    .unwrap_or_default(),
            ),
        );
        p.insert("properties", Value::Array(self.properties.clone()));
        p.insert("calls", Value::Array(self.calls.clone()));
        p.insert("instanceof", Value::Array(self.instanceof.clone()));
        p.insert("autoconfigured", Value::Bool(self.autoconfigured));
        p.insert("configurator", self.configurator.clone());
        p.insert("tags", Value::Array(self.tags.clone()));
        p.insert("public", Value::Bool(self.public));
        p.insert("synthetic", Value::Bool(self.synthetic));
        p.insert("abstract", Value::Bool(self.abstract_));
        p.insert("lazy", Value::Bool(self.lazy));
        p.insert(
            "decoratedService",
            self.decorated_service
                .as_ref()
                .map_or(Value::Null, |d| Value::Array(d.to_array())),
        );
        p.insert("autowired", Value::Bool(self.autowired));
        let mut changes = PhpArray::new();
        for key in &self.changes {
            changes.insert(key.as_str(), Value::Bool(true));
        }
        p.insert("changes", Value::Array(changes));
        p.insert("bindings", Value::Array(self.bindings.clone()));
        p.insert("arguments", Value::Array(self.arguments.clone()));
        p.insert("innerServiceId", opt(self.inner_service_id.as_deref()));
        p.insert(
            "decorationOnInvalid",
            self.decoration_on_invalid
                .map_or(Value::Null, |b| Value::Int(b.as_int())),
        );
        p.insert(
            "decorationPriority",
            self.decoration_priority.map_or(Value::Null, Value::Int),
        );
        p
    }
}

/// `explode('::', $callable, 2)` for a string with `::`, `[$x,
/// '__invoke']` for a Reference or a Definition, the value otherwise.
fn normalize_callable(value: Value) -> Value {
    match &value {
        Value::String(s) => {
            let bytes = s.as_bytes();
            if let Some(at) = bytes.windows(2).position(|w| w == b"::") {
                return Value::Array(PhpArray::list([
                    Value::String(PhpString::from(bytes[..at].to_vec())),
                    Value::String(PhpString::from(bytes[at + 2..].to_vec())),
                ]));
            }
            value
        }
        Value::Object(_)
            if Reference::from_value(&value).is_some()
                || Definition::from_value(&value).is_some() =>
        {
            Value::Array(PhpArray::list([value, Value::from("__invoke")]))
        }
        _ => value,
    }
}

/// `Definition` (and `ChildDefinition`): how to create a service.
#[derive(Clone)]
pub struct Definition(Shared<DefinitionData>);

impl Default for Definition {
    fn default() -> Self {
        Self::new(None, PhpArray::new())
    }
}

impl Definition {
    pub const CLASS: &'static str = "Symfony\\Component\\DependencyInjection\\Definition";
    pub const CHILD_CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\ChildDefinition";

    /// `new Definition($class, $arguments)`.
    pub fn new(class: Option<&str>, arguments: PhpArray) -> Self {
        let definition = Self(Shared::new(DefinitionData::default()));
        if let Some(class) = class {
            definition.set_class(Some(class));
        }
        definition.0.write(|d| d.arguments = arguments);
        definition
    }

    /// `new Definition($class)`.
    pub fn with_class(class: &str) -> Self {
        Self::new(Some(class), PhpArray::new())
    }

    /// `new ChildDefinition($parent)`: no changes recorded, no class.
    pub fn child(parent: &str) -> Self {
        Self(Shared::new(DefinitionData {
            parent: Some(parent.to_string()),
            ..DefinitionData::default()
        }))
    }

    /// `$definition instanceof ChildDefinition`.
    pub fn is_child(&self) -> bool {
        self.0.read(|d| d.parent.is_some())
    }

    /// `ChildDefinition::getParent()`; `None` for a plain definition.
    pub fn parent(&self) -> Option<String> {
        self.0.read(|d| d.parent.clone())
    }

    /// `ChildDefinition::setParent()`. On a plain definition this makes it
    /// a child; callers only use it on children.
    pub fn set_parent(&self, parent: &str) -> &Self {
        self.0.write(|d| d.parent = Some(parent.to_string()));
        self
    }

    /// `getChanges()`: the keys of the changed settings, in order.
    pub fn changes(&self) -> Vec<String> {
        self.0.read(|d| d.changes.clone())
    }

    /// `setChanges($changes)`, with the keys of the array.
    pub fn set_changes(&self, changes: Vec<String>) -> &Self {
        self.0.write(|d| d.changes = changes);
        self
    }

    /// `setFactory($factory)`.
    pub fn set_factory(&self, factory: impl Into<Value>) -> &Self {
        let factory = normalize_callable(factory.into());
        self.0.write(|d| {
            d.change("factory");
            d.factory = factory;
        });
        self
    }

    /// `getFactory()`: a string, a `[class-or-service, method]` array, or
    /// null.
    pub fn factory(&self) -> Value {
        self.0.read(|d| d.factory.clone())
    }

    /// `setDecoratedService($id, $renamedId, $priority, $invalidBehavior)`.
    pub fn set_decorated_service(
        &self,
        id: Option<&str>,
        renamed_id: Option<&str>,
        priority: i64,
        invalid_behavior: InvalidBehavior,
    ) -> Result<&Self> {
        // `if ($renamedId && $id === $renamedId)`: "" and "0" are falsy.
        if let (Some(id), Some(renamed)) = (id, renamed_id)
            && !renamed.is_empty()
            && renamed != "0"
            && id == renamed
        {
            return Err(Error::invalid_argument(format!(
                "The decorated service inner name for \"{id}\" must be different than the service name itself."
            )));
        }
        self.0.write(|d| {
            d.change("decorated_service");
            d.decorated_service = id.map(|id| DecoratedService {
                id: id.to_string(),
                renamed_id: renamed_id.map(str::to_string),
                priority,
                invalid_behavior: (invalid_behavior != InvalidBehavior::Exception)
                    .then_some(invalid_behavior),
            });
        });
        Ok(self)
    }

    /// `getDecoratedService()`.
    pub fn decorated_service(&self) -> Option<DecoratedService> {
        self.0.read(|d| d.decorated_service.clone())
    }

    /// `setClass($class)`.
    pub fn set_class(&self, class: Option<&str>) -> &Self {
        self.0.write(|d| {
            d.change("class");
            d.class = class.map(str::to_string);
        });
        self
    }

    /// `getClass()`.
    pub fn class(&self) -> Option<String> {
        self.0.read(|d| d.class.clone())
    }

    /// `setArguments($arguments)`.
    pub fn set_arguments(&self, arguments: PhpArray) -> &Self {
        self.0.write(|d| d.arguments = arguments);
        self
    }

    /// `setProperties($properties)`.
    pub fn set_properties(&self, properties: PhpArray) -> &Self {
        self.0.write(|d| d.properties = properties);
        self
    }

    /// `getProperties()`.
    pub fn properties(&self) -> PhpArray {
        self.0.read(|d| d.properties.clone())
    }

    /// `setProperty($name, $value)`.
    pub fn set_property(&self, name: &str, value: impl Into<Value>) -> &Self {
        let value = value.into();
        self.0.write(|d| d.properties.insert(name, value));
        self
    }

    /// `addArgument($argument)`.
    pub fn add_argument(&self, argument: impl Into<Value>) -> &Self {
        let argument = argument.into();
        // `$a[] = …` on a full array is a PHP warning and does nothing.
        self.0.write(|d| {
            let _ = d.arguments.push(argument);
        });
        self
    }

    /// `replaceArgument($index, $argument)`, with `ChildDefinition`'s
    /// override for a child.
    pub fn replace_argument(
        &self,
        index: impl Into<Index>,
        argument: impl Into<Value>,
    ) -> Result<&Self> {
        let index = index.into();
        let argument = argument.into();
        if self.is_child() {
            let key = match &index {
                Index::Int(i) => Key::from(format!("index_{i}")),
                Index::Str(s) if s.starts_with('$') => Key::from(s.as_str()),
                Index::Str(_) => {
                    return Err(Error::invalid_argument(
                        "The argument must be an existing index or the name of a constructor's parameter.",
                    ));
                }
            };
            self.0.write(|d| d.arguments.insert(key, argument));
            return Ok(self);
        }
        self.0.write(|d| {
            let class = d.class.clone().unwrap_or_default();
            if d.arguments.is_empty() {
                return Err(Error::out_of_bounds(format!(
                    "Cannot replace arguments for class \"{class}\" if none have been configured yet."
                )));
            }
            let key = index.key();
            if !d.arguments.contains_key(&key) {
                return Err(Error::out_of_bounds(format!(
                    "The argument \"{index}\" doesn't exist in class \"{class}\"."
                )));
            }
            d.arguments.insert(key, argument);
            Ok(())
        })?;
        Ok(self)
    }

    /// `setArgument($key, $value)`.
    pub fn set_argument(&self, key: impl Into<Index>, value: impl Into<Value>) -> &Self {
        let key = key.into().key();
        let value = value.into();
        self.0.write(|d| d.arguments.insert(key, value));
        self
    }

    /// `getArguments()`.
    pub fn arguments(&self) -> PhpArray {
        self.0.read(|d| d.arguments.clone())
    }

    /// `getArgument($index)`, with `ChildDefinition`'s override for a child.
    pub fn argument(&self, index: impl Into<Index>) -> Result<Value> {
        let index = index.into();
        self.0.read(|d| {
            if d.parent.is_some()
                && let Some(value) = d.arguments.get(&Key::from(format!("index_{index}")))
            {
                return Ok(value.clone());
            }
            d.arguments.get(&index.key()).cloned().ok_or_else(|| {
                Error::out_of_bounds(format!(
                    "The argument \"{index}\" doesn't exist in class \"{}\".",
                    d.class.as_deref().unwrap_or("")
                ))
            })
        })
    }

    /// `setMethodCalls($calls)`: each `[$method, $arguments, $returnsClone]`
    /// through `addMethodCall()`.
    pub fn set_method_calls(&self, calls: &PhpArray) -> Result<&Self> {
        self.0.write(|d| d.calls = PhpArray::new());
        for call in calls.values() {
            let Some(call) = call.as_array() else {
                continue;
            };
            let method = call
                .get(&Key::Int(0))
                .and_then(Value::to_php_string)
                .unwrap_or_default();
            let arguments = call
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let returns_clone = call.get(&Key::Int(2)).is_some_and(Value::is_truthy);
            self.add_method_call(&method.to_string_lossy(), arguments, returns_clone)?;
        }
        Ok(self)
    }

    /// `addMethodCall($method, $arguments, $returnsClone)`.
    pub fn add_method_call(
        &self,
        method: &str,
        arguments: PhpArray,
        returns_clone: bool,
    ) -> Result<&Self> {
        // `if (!$method)`: "" and "0".
        if method.is_empty() || method == "0" {
            return Err(Error::invalid_argument("Method name cannot be empty."));
        }
        let mut call = PhpArray::list([Value::from(method), Value::Array(arguments)]);
        if returns_clone {
            let _ = call.push(Value::Bool(true));
        }
        self.0.write(|d| {
            let _ = d.calls.push(Value::Array(call));
        });
        Ok(self)
    }

    /// `removeMethodCall($method)`: every call of the method, leaving its
    /// keys unused.
    pub fn remove_method_call(&self, method: &str) -> &Self {
        self.0.write(|d| {
            let keys: Vec<Key> = d
                .calls
                .iter()
                .filter(|(_, call)| call_method(call).as_deref() == Some(method.as_bytes()))
                .map(|(k, _)| k.clone())
                .collect();
            for key in keys {
                d.calls.remove(&key);
            }
        });
        self
    }

    /// `hasMethodCall($method)`.
    pub fn has_method_call(&self, method: &str) -> bool {
        self.0.read(|d| {
            d.calls
                .values()
                .any(|call| call_method(call).as_deref() == Some(method.as_bytes()))
        })
    }

    /// `getMethodCalls()`.
    pub fn method_calls(&self) -> PhpArray {
        self.0.read(|d| d.calls.clone())
    }

    /// `setInstanceofConditionals($instanceof)`.
    pub fn set_instanceof_conditionals(&self, instanceof: PhpArray) -> &Self {
        self.0.write(|d| d.instanceof = instanceof);
        self
    }

    /// `getInstanceofConditionals()`.
    pub fn instanceof_conditionals(&self) -> PhpArray {
        self.0.read(|d| d.instanceof.clone())
    }

    /// `setAutoconfigured($autoconfigured)`.
    pub fn set_autoconfigured(&self, autoconfigured: bool) -> &Self {
        self.0.write(|d| {
            d.change("autoconfigured");
            d.autoconfigured = autoconfigured;
        });
        self
    }

    pub fn is_autoconfigured(&self) -> bool {
        self.0.read(|d| d.autoconfigured)
    }

    /// `setTags($tags)`.
    pub fn set_tags(&self, tags: PhpArray) -> &Self {
        self.0.write(|d| d.tags = tags);
        self
    }

    /// `getTags()`.
    pub fn tags(&self) -> PhpArray {
        self.0.read(|d| d.tags.clone())
    }

    /// `getTag($name)`: the attribute arrays of each use of the tag.
    pub fn tag(&self, name: &str) -> PhpArray {
        self.0.read(|d| {
            d.tags
                .get(&Key::from(name))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
    }

    /// `addTag($name, $attributes)`.
    pub fn add_tag(&self, name: &str, attributes: PhpArray) -> &Self {
        self.0.write(|d| {
            let key = Key::from(name);
            let mut list = d
                .tags
                .get(&key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let _ = list.push(Value::Array(attributes));
            d.tags.insert(key, Value::Array(list));
        });
        self
    }

    /// `addResourceTag($name, $attributes)`.
    pub fn add_resource_tag(&self, name: &str, attributes: PhpArray) -> &Self {
        let mut excluded = PhpArray::new();
        excluded.insert("source", Value::from(format!("by tag \"{name}\"")));
        self.add_tag(name, attributes)
            .add_tag("container.excluded", excluded)
    }

    /// `hasTag($name)`.
    pub fn has_tag(&self, name: &str) -> bool {
        self.0
            .read(|d| d.tags.get(&Key::from(name)).is_some_and(|v| !v.is_null()))
    }

    /// `clearTag($name)`.
    pub fn clear_tag(&self, name: &str) -> &Self {
        self.0.write(|d| {
            d.tags.remove(&Key::from(name));
        });
        self
    }

    /// `clearTags()`.
    pub fn clear_tags(&self) -> &Self {
        self.0.write(|d| d.tags = PhpArray::new());
        self
    }

    /// `setFile($file)`.
    pub fn set_file(&self, file: Option<&str>) -> &Self {
        self.0.write(|d| {
            d.change("file");
            d.file = file.map(str::to_string);
        });
        self
    }

    pub fn file(&self) -> Option<String> {
        self.0.read(|d| d.file.clone())
    }

    /// `setShared($shared)`.
    pub fn set_shared(&self, shared: bool) -> &Self {
        self.0.write(|d| {
            d.change("shared");
            d.shared = shared;
        });
        self
    }

    pub fn is_shared(&self) -> bool {
        self.0.read(|d| d.shared)
    }

    /// `setPublic($public)`.
    pub fn set_public(&self, public: bool) -> &Self {
        self.0.write(|d| {
            d.change("public");
            d.public = public;
        });
        self
    }

    pub fn is_public(&self) -> bool {
        self.0.read(|d| d.public)
    }

    pub fn is_private(&self) -> bool {
        !self.is_public()
    }

    /// `setLazy($lazy)`.
    pub fn set_lazy(&self, lazy: bool) -> &Self {
        self.0.write(|d| {
            d.change("lazy");
            d.lazy = lazy;
        });
        self
    }

    pub fn is_lazy(&self) -> bool {
        self.0.read(|d| d.lazy)
    }

    /// `setSynthetic($synthetic)`: also public, unless public was set.
    pub fn set_synthetic(&self, synthetic: bool) -> &Self {
        let public_set = self.0.write(|d| {
            d.synthetic = synthetic;
            d.changes.iter().any(|k| k == "public")
        });
        if !public_set {
            self.set_public(true);
        }
        self
    }

    pub fn is_synthetic(&self) -> bool {
        self.0.read(|d| d.synthetic)
    }

    /// `setAbstract($abstract)`.
    pub fn set_abstract(&self, abstract_: bool) -> &Self {
        self.0.write(|d| d.abstract_ = abstract_);
        self
    }

    pub fn is_abstract(&self) -> bool {
        self.0.read(|d| d.abstract_)
    }

    /// `setDeprecated($package, $version, $message)`; an empty message
    /// takes the default template.
    pub fn set_deprecated(&self, package: &str, version: &str, message: &str) -> Result<&Self> {
        check_deprecation_template(message, "%service_id%")?;
        let message = if message.is_empty() {
            DEFAULT_DEPRECATION_TEMPLATE
        } else {
            message
        };
        self.0.write(|d| {
            d.change("deprecated");
            d.deprecation = Some(Deprecation {
                package: package.to_string(),
                version: version.to_string(),
                message: message.to_string(),
            });
        });
        Ok(self)
    }

    pub fn is_deprecated(&self) -> bool {
        self.0.read(|d| d.deprecation.is_some())
    }

    /// `getDeprecation($id)`: `%service_id%` replaced by `id`. `None` when
    /// the definition is not deprecated.
    pub fn deprecation(&self, id: &str) -> Option<Deprecation> {
        self.0.read(|d| {
            d.deprecation.as_ref().map(|dep| Deprecation {
                package: dep.package.clone(),
                version: dep.version.clone(),
                message: dep.message.replace("%service_id%", id),
            })
        })
    }

    /// `setConfigurator($configurator)`.
    pub fn set_configurator(&self, configurator: impl Into<Value>) -> &Self {
        let configurator = normalize_callable(configurator.into());
        self.0.write(|d| {
            d.change("configurator");
            d.configurator = configurator;
        });
        self
    }

    /// `getConfigurator()`.
    pub fn configurator(&self) -> Value {
        self.0.read(|d| d.configurator.clone())
    }

    pub fn is_autowired(&self) -> bool {
        self.0.read(|d| d.autowired)
    }

    /// `setAutowired($autowired)`.
    pub fn set_autowired(&self, autowired: bool) -> &Self {
        self.0.write(|d| {
            d.change("autowired");
            d.autowired = autowired;
        });
        self
    }

    /// `getBindings()`: `BoundArgument` objects by key.
    pub fn bindings(&self) -> PhpArray {
        self.0.read(|d| d.bindings.clone())
    }

    /// `setBindings($bindings)`: `"Type$name"` keys get one space before
    /// the `$`, and each value not already a `BoundArgument` is wrapped in
    /// one.
    pub fn set_bindings(&self, bindings: &PhpArray) -> &Self {
        // foreach over a copy, changing the array itself: a renamed key
        // moves to the end, as `unset()` then `$bindings[$k] = …` does.
        let mut result = bindings.clone();
        for (key, binding) in bindings.iter() {
            let mut key = key.clone();
            if let Key::Str(s) = &key {
                let bytes = s.as_bytes();
                // `0 < strpos($key, '$')`
                if bytes.iter().position(|&b| b == b'$').is_some_and(|p| p > 0) {
                    let normalized = normalize_binding_key(bytes);
                    if normalized != bytes {
                        result.remove(&key);
                        key = Key::from_bytes(&normalized);
                        result.insert(key.clone(), binding.clone());
                    }
                }
            }
            if BoundArgument::from_value(binding).is_none() {
                result.insert(
                    key,
                    BoundArgument::new(binding.clone(), true, 0, None).to_value(),
                );
            }
        }
        self.0.write(|d| d.bindings = result);
        self
    }

    /// `addError($error)`; a definition adds its own errors.
    pub fn add_error(&self, error: DefinitionError) -> &Self {
        self.0.write(|d| d.errors.push(error));
        self
    }

    /// `addError($definition)`: its errors are appended.
    pub fn add_errors_of(&self, other: &Definition) -> &Self {
        let errors = other.0.read(|d| d.errors.clone());
        self.0.write(|d| d.errors.extend(errors));
        self
    }

    /// `getErrors()`: closures are rendered and replaced by their text.
    pub fn errors(&self) -> Vec<String> {
        let errors = self.0.read(|d| d.errors.clone());
        let rendered: Vec<String> = errors
            .iter()
            .map(|e| match e {
                DefinitionError::Text(text) => text.clone(),
                DefinitionError::Lazy(f) => f(),
            })
            .collect();
        self.0.write(|d| {
            d.errors = rendered
                .iter()
                .cloned()
                .map(DefinitionError::Text)
                .collect();
        });
        rendered
    }

    /// `hasErrors()`.
    pub fn has_errors(&self) -> bool {
        self.0.read(|d| !d.errors.is_empty())
    }

    /// `$innerServiceId`.
    pub fn inner_service_id(&self) -> Option<String> {
        self.0.read(|d| d.inner_service_id.clone())
    }

    pub fn set_inner_service_id(&self, id: Option<&str>) -> &Self {
        self.0
            .write(|d| d.inner_service_id = id.map(str::to_string));
        self
    }

    /// `$decorationOnInvalid`.
    pub fn decoration_on_invalid(&self) -> Option<InvalidBehavior> {
        self.0.read(|d| d.decoration_on_invalid)
    }

    pub fn set_decoration_on_invalid(&self, behavior: Option<InvalidBehavior>) -> &Self {
        self.0.write(|d| d.decoration_on_invalid = behavior);
        self
    }

    /// `$decorationPriority`.
    pub fn decoration_priority(&self) -> Option<i64> {
        self.0.read(|d| d.decoration_priority)
    }

    pub fn set_decoration_priority(&self, priority: Option<i64>) -> &Self {
        self.0.write(|d| d.decoration_priority = priority);
        self
    }

    /// `$a === $b`.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }

    /// `clone $definition`.
    #[must_use]
    pub fn clone_object(&self) -> Self {
        Self(self.0.clone_object())
    }

    /// The definition as a PHP object value.
    pub fn to_value(&self) -> Value {
        self.0.to_value()
    }

    /// The definition, when `value` is a `Definition` object.
    pub fn from_value(value: &Value) -> Option<Self> {
        Shared::from_value(value).map(Self)
    }
}

/// The method name of a call array (`$call[0]`).
fn call_method(call: &Value) -> Option<Vec<u8>> {
    call.as_array()
        .and_then(|c| c.get(&Key::Int(0)))
        .and_then(Value::to_php_string)
        .map(|s| s.as_bytes().to_vec())
}

/// `preg_replace('/[ \t]*\$/', ' $', $key)`.
fn normalize_binding_key(key: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(key.len() + 1);
    let mut i = 0;
    while i < key.len() {
        let mut j = i;
        while j < key.len() && matches!(key[j], b' ' | b'\t') {
            j += 1;
        }
        if j < key.len() && key[j] == b'$' {
            out.extend_from_slice(b" $");
            i = j + 1;
        } else {
            out.push(key[i]);
            i += 1;
        }
    }
    out
}

impl fmt::Debug for Definition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.read(|d| {
            f.debug_struct(if d.parent.is_some() {
                "ChildDefinition"
            } else {
                "Definition"
            })
            .field("parent", &d.parent)
            .field("class", &d.class)
            .finish_non_exhaustive()
        })
    }
}

impl From<Definition> for Value {
    fn from(definition: Definition) -> Self {
        definition.to_value()
    }
}

impl From<&Definition> for Value {
    fn from(definition: &Definition) -> Self {
        definition.to_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_keys_get_one_space_before_the_dollar() {
        assert_eq!(normalize_binding_key(b"Foo$bar"), b"Foo $bar");
        assert_eq!(normalize_binding_key(b"Foo \t $bar"), b"Foo $bar");
        assert_eq!(normalize_binding_key(b"Foo $bar"), b"Foo $bar");
    }

    #[test]
    fn removed_calls_leave_gaps() {
        let def = Definition::with_class("stdClass");
        def.add_method_call("a", PhpArray::new(), false).unwrap();
        def.add_method_call("b", PhpArray::new(), false).unwrap();
        def.remove_method_call("b");
        def.add_method_call("c", PhpArray::new(), false).unwrap();
        let keys: Vec<Key> = def.method_calls().keys().cloned().collect();
        assert_eq!(keys, vec![Key::Int(0), Key::Int(2)]);
    }
}
