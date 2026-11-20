//! Port of Symfony\Component\Config\Definition\BaseNode.php, with the
//! interfaces it implements (NodeInterface.php, PrototypeNodeInterface.php).
//!
//! Every node class is a [`Node`]: a handle to one shared, mutable node,
//! as a PHP object is (cloning the handle gives the same node;
//! [`Node::clone_node`] is PHP's `clone`). What differs between the
//! classes (VariableNode, ScalarNode and its subclasses, ArrayNode,
//! PrototypedArrayNode, or a [`CustomNode`]) is its [`Kind`]; the
//! protected methods each class overrides (`validateType`,
//! `normalizeValue`, `mergeValues`, `finalizeValue`, ...) dispatch on it,
//! and live in the module of the class that defines them.
//!
//! PHP keeps placeholders in static properties; here they are
//! thread-local, which is the same for PHP's one thread and keeps
//! parallel tests apart. Parents are strong references, as in PHP, so a
//! tree is a cycle that lives until the process ends; trees are built at
//! build time.
//!
//! A method a class does not have fails with PHP's `\Error` ("Call to
//! undefined method ..."), so a node of the wrong kind is never changed
//! silently.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/BaseNode.php Definition/NodeInterface.php Definition/PrototypeNodeInterface.php

use std::{
    cell::RefCell,
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use php_value::{Key, PhpArray, PhpString, Value};
use symfony_deprecation_contracts as deprecation;

use crate::{
    definition::{
        array_node::{self, ArrayData},
        boolean_node, enum_node,
        enum_node::EnumData,
        exception::{Class, Error},
        float_node, integer_node, numeric_node,
        numeric_node::NumericData,
        prototyped_array_node::{self, PrototypedData},
        scalar_node, string_node,
        variable_node::{self, VariableData},
    },
    php::{self, Msg},
};

/// `BaseNode::DEFAULT_PATH_SEPARATOR`.
pub const DEFAULT_PATH_SEPARATOR: &str = ".";

/// A normalization or final validation closure: it gets the value and
/// returns the new one, or throws.
pub type Closure = Arc<dyn Fn(Value) -> Result<Value, Error> + Send + Sync>;

/// A node of a configuration tree.
#[derive(Clone)]
pub struct Node(Arc<Mutex<Inner>>);

#[derive(Clone)]
pub(crate) struct Inner {
    pub(crate) base: Base,
    pub(crate) kind: Kind,
}

/// BaseNode's properties.
#[derive(Clone)]
pub(crate) struct Base {
    pub(crate) name: PhpString,
    pub(crate) parent: Option<Node>,
    pub(crate) path_separator: PhpString,
    pub(crate) normalization_closures: Vec<Closure>,
    pub(crate) normalized_types: Vec<PhpString>,
    pub(crate) final_validation_closures: Vec<Closure>,
    pub(crate) allow_overwrite: bool,
    pub(crate) required: bool,
    pub(crate) deprecation: Option<DeprecationSpec>,
    pub(crate) equivalent_values: Vec<(Value, Value)>,
    pub(crate) attributes: PhpArray,
    pub(crate) handling_placeholder: Option<Value>,
    /// The class of a PHP subclass that overrides nothing (`class BarNode
    /// extends ArrayNode {}`), when the node is one.
    pub(crate) subclass: Option<Arc<str>>,
}

/// What `setDeprecated()` stored.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DeprecationSpec {
    pub(crate) package: PhpString,
    pub(crate) version: PhpString,
    pub(crate) message: PhpString,
}

/// `getDeprecation()`'s array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deprecation {
    pub package: Vec<u8>,
    pub version: Vec<u8>,
    pub message: Vec<u8>,
}

/// The node's class and its own properties.
#[derive(Clone)]
pub(crate) enum Kind {
    /// VariableNode, or ScalarNode and its subclasses.
    Variable {
        data: VariableData,
        scalar: Option<ScalarKind>,
    },
    Array(ArrayData),
    Prototyped(ArrayData, PrototypedData),
    Custom(Arc<dyn CustomNode>),
}

/// The ScalarNode classes.
#[derive(Clone)]
pub(crate) enum ScalarKind {
    Scalar,
    Boolean { nullable: bool },
    Numeric(NumericData),
    Integer(NumericData),
    Float(NumericData),
    String,
    Enum(Box<EnumData>),
}

/// A node class written outside this crate (a PHP subclass of BaseNode):
/// its implementations of BaseNode's abstract and overridable methods.
pub trait CustomNode: Send + Sync {
    /// `static::class`.
    fn class(&self) -> &str;
    fn validate_type(&self, node: &Node, value: &Value) -> Result<(), Error>;
    fn normalize_value(&self, node: &Node, value: Value) -> Result<Value, Error>;
    fn merge_values(&self, node: &Node, left: Value, right: Value) -> Result<Value, Error>;
    fn finalize_value(&self, node: &Node, value: Value) -> Result<Value, Error>;
    fn has_default_value(&self, node: &Node) -> bool;
    fn get_default_value(&self, node: &Node) -> Result<Value, Error>;
    /// An override of `getPath()`, as a stubbed NodeInterface has.
    fn get_path(&self, _node: &Node) -> Option<PhpString> {
        None
    }
    fn allow_placeholders(&self) -> bool {
        true
    }
    fn valid_placeholder_types(&self) -> Vec<&'static str> {
        Vec::new()
    }
    /// Whether the class implements PrototypeNodeInterface.
    fn is_prototype_node(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct Placeholders {
    unique_prefixes: Vec<PhpString>,
    /// `$placeholders[$placeholder] = $values`.
    values: PhpArray,
}

thread_local! {
    static PLACEHOLDERS: RefCell<Placeholders> = RefCell::new(Placeholders::default());
}

/// `BaseNode::setPlaceholder()`: values the placeholder stands for, keyed
/// by their types.
pub fn set_placeholder(placeholder: &[u8], values: PhpArray) -> Result<(), Error> {
    if values.is_empty() {
        return Err(Error::new(
            Class::InvalidArgument,
            "At least one value must be provided.",
        ));
    }
    PLACEHOLDERS.with(|p| {
        p.borrow_mut()
            .values
            .insert(Key::from_bytes(placeholder), Value::Array(values));
    });
    Ok(())
}

/// `BaseNode::setPlaceholderUniquePrefix()`.
pub fn set_placeholder_unique_prefix(prefix: &[u8]) {
    PLACEHOLDERS.with(|p| p.borrow_mut().unique_prefixes.push(PhpString::from(prefix)));
}

/// `BaseNode::resetPlaceholders()`.
pub fn reset_placeholders() {
    PLACEHOLDERS.with(|p| *p.borrow_mut() = Placeholders::default());
}

/// `resolvePlaceholderValue()`: the values a placeholder stands for (none
/// for a value with a unique prefix), or `None` when the value is not a
/// placeholder (upstream returns the value itself).
fn resolve_placeholder_value(value: &Value) -> Option<PhpArray> {
    let Value::String(s) = value else {
        return None;
    };
    PLACEHOLDERS.with(|p| {
        let p = p.borrow();
        if let Some(Value::Array(values)) = p.values.get(&Key::from_bytes(s)) {
            return Some(values.clone());
        }
        p.unique_prefixes
            .iter()
            .any(|prefix| s.starts_with(prefix))
            .then(PhpArray::new)
    })
}

/// The type names a placeholder was registered with
/// (`array_keys(self::$placeholders[$placeholder])`).
fn placeholder_types(placeholder: &Value) -> Option<Vec<Vec<u8>>> {
    let key = Key::from_value(placeholder)?;
    PLACEHOLDERS.with(|p| match p.borrow().values.get(&key) {
        Some(Value::Array(values)) => Some(values.keys().map(php::key_bytes).collect()),
        _ => None,
    })
}

pub(crate) fn undefined_method(class: &str, method: &str) -> Error {
    Error::new(
        Class::Error,
        format!("Call to undefined method {class}::{method}()"),
    )
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({:?})", self.class(), self.get_path())
    }
}

impl Node {
    /// `parent::__construct($name, $parent, $pathSeparator)`.
    pub(crate) fn construct(
        name: &[u8],
        parent: Option<&Node>,
        path_separator: &[u8],
        kind: Kind,
    ) -> Result<Self, Error> {
        if php::contains(name, path_separator) {
            return Err(Error::new(
                Class::InvalidArgument,
                Msg::new("The name must not contain \".")
                    .b(path_separator)
                    .s("\"."),
            ));
        }
        Ok(Self(Arc::new(Mutex::new(Inner {
            base: Base {
                name: PhpString::from(name),
                parent: parent.cloned(),
                path_separator: PhpString::from(path_separator),
                normalization_closures: Vec::new(),
                normalized_types: Vec::new(),
                final_validation_closures: Vec::new(),
                allow_overwrite: true,
                required: false,
                deprecation: None,
                equivalent_values: Vec::new(),
                attributes: PhpArray::new(),
                handling_placeholder: None,
                subclass: None,
            },
            kind,
        }))))
    }

    /// A node of a [`CustomNode`] class.
    pub fn custom(
        name: &[u8],
        parent: Option<&Node>,
        path_separator: &[u8],
        class: Arc<dyn CustomNode>,
    ) -> Result<Self, Error> {
        Self::construct(name, parent, path_separator, Kind::Custom(class))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reads the node's state. The closure must not call methods of any
    /// node (they may lock this one).
    pub(crate) fn read<R>(&self, f: impl FnOnce(&Inner) -> R) -> R {
        f(&self.lock())
    }

    /// Changes the node's state, under the same rule as [`Node::read`].
    pub(crate) fn write<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        f(&mut self.lock())
    }

    pub(crate) fn kind(&self) -> Kind {
        self.read(|n| n.kind.clone())
    }

    /// `===` between two nodes: the same object.
    pub fn same(&self, other: &Node) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// PHP's `clone $node`: a new node with the same properties; child
    /// nodes, prototype and parent are shared, as PHP's shallow clone
    /// shares objects.
    pub fn clone_node(&self) -> Node {
        Node(Arc::new(Mutex::new(self.read(Inner::clone))))
    }

    /// `static::class`.
    pub fn class(&self) -> String {
        if let Some(class) = self.read(|n| n.base.subclass.clone()) {
            return class.to_string();
        }
        match self.kind() {
            Kind::Custom(c) => c.class().to_owned(),
            kind => kind_class(&kind).to_owned(),
        }
    }

    /// Makes this new node an instance of `class`, a PHP subclass of its
    /// class that overrides nothing (`class BarNode extends ArrayNode
    /// {}`). Called right after the node is constructed.
    pub fn with_class(self, class: &str) -> Self {
        self.write(|n| n.base.subclass = Some(Arc::from(class)));
        self
    }

    /// `$node instanceof $class`: its class, the classes it extends and
    /// the interfaces it implements. Class names compare without case and
    /// without a leading backslash, as PHP's do.
    pub fn is_instance_of(&self, class: &str) -> bool {
        let wanted = php::class_name_key(class);
        let (subclass, kind) = self.read(|n| (n.base.subclass.clone(), n.kind.clone()));
        let mut classes: Vec<String> = subclass.iter().map(|c| c.to_string()).collect();
        match &kind {
            Kind::Custom(c) => classes.push(c.class().to_owned()),
            kind => classes.extend(kind_ancestors(kind).iter().map(|c| (*c).to_owned())),
        }
        classes.push(BASE_NODE.to_owned());
        classes.push(NODE_INTERFACE.to_owned());
        if self.is_prototype_node() {
            classes.push(PROTOTYPE_NODE_INTERFACE.to_owned());
        }
        classes.iter().any(|c| php::class_name_key(c) == wanted)
    }

    /// `instanceof ArrayNode` (ArrayNode and PrototypedArrayNode).
    pub fn is_array_node(&self) -> bool {
        self.read(|n| matches!(n.kind, Kind::Array(_) | Kind::Prototyped(..)))
    }

    /// `instanceof PrototypedArrayNode`.
    pub fn is_prototyped_array_node(&self) -> bool {
        self.read(|n| matches!(n.kind, Kind::Prototyped(..)))
    }

    /// `instanceof VariableNode` (VariableNode, ScalarNode and its
    /// subclasses).
    pub fn is_variable_node(&self) -> bool {
        self.read(|n| matches!(n.kind, Kind::Variable { .. }))
    }

    /// `instanceof PrototypeNodeInterface`.
    pub fn is_prototype_node(&self) -> bool {
        match self.kind() {
            Kind::Custom(c) => c.is_prototype_node(),
            _ => true,
        }
    }

    pub fn set_attribute(&self, key: &[u8], value: Value) {
        self.write(|n| n.base.attributes.insert(Key::from_bytes(key), value));
    }

    /// `$this->attributes[$key] ?? $default`.
    pub fn get_attribute(&self, key: &[u8], default: Value) -> Value {
        self.read(|n| match n.base.attributes.get(&Key::from_bytes(key)) {
            Some(v) if !v.is_null() => v.clone(),
            _ => default,
        })
    }

    /// `isset($this->attributes[$key])`.
    pub fn has_attribute(&self, key: &[u8]) -> bool {
        self.read(|n| {
            n.base
                .attributes
                .get(&Key::from_bytes(key))
                .is_some_and(|v| !v.is_null())
        })
    }

    pub fn get_attributes(&self) -> PhpArray {
        self.read(|n| n.base.attributes.clone())
    }

    pub fn set_attributes(&self, attributes: PhpArray) {
        self.write(|n| n.base.attributes = attributes);
    }

    pub fn remove_attribute(&self, key: &[u8]) {
        self.write(|n| n.base.attributes.remove(&Key::from_bytes(key)));
    }

    pub fn set_info(&self, info: &[u8]) {
        self.set_attribute(b"info", Value::String(PhpString::from(info)));
    }

    /// `getInfo()`: the `info` attribute (`?string`).
    pub fn get_info(&self) -> Option<PhpString> {
        match self.get_attribute(b"info", Value::Null) {
            Value::Null => None,
            v => v.to_php_string(),
        }
    }

    /// The info when it is truthy, as `if ($hint = $this->getInfo())`.
    pub(crate) fn hint(&self) -> Option<PhpString> {
        self.get_info()
            .filter(|info| php::truthy(&Value::String(info.clone())))
    }

    /// `setExample(string|array)`.
    pub fn set_example(&self, example: Value) {
        self.set_attribute(b"example", example);
    }

    pub fn get_example(&self) -> Value {
        self.get_attribute(b"example", Value::Null)
    }

    pub fn add_equivalent_value(&self, original: Value, equivalent: Value) {
        self.write(|n| n.base.equivalent_values.push((original, equivalent)));
    }

    pub fn set_required(&self, required: bool) {
        self.write(|n| n.base.required = required);
    }

    /// `setDeprecated($package, $version, $message)`; upstream's default
    /// message is [`DEFAULT_DEPRECATION_MESSAGE`].
    pub fn set_deprecated(&self, package: &[u8], version: &[u8], message: &[u8]) {
        self.write(|n| {
            n.base.deprecation = Some(DeprecationSpec {
                package: PhpString::from(package),
                version: PhpString::from(version),
                message: PhpString::from(message),
            });
        });
    }

    pub fn set_allow_overwrite(&self, allow: bool) {
        self.write(|n| n.base.allow_overwrite = allow);
    }

    pub fn set_normalization_closures(&self, closures: Vec<Closure>) {
        self.write(|n| n.base.normalization_closures = closures);
    }

    pub fn get_normalization_closures(&self) -> Vec<Closure> {
        self.read(|n| n.base.normalization_closures.clone())
    }

    pub fn set_normalized_types(&self, types: Vec<PhpString>) {
        self.write(|n| n.base.normalized_types = types);
    }

    pub fn get_normalized_types(&self) -> Vec<PhpString> {
        self.read(|n| n.base.normalized_types.clone())
    }

    pub fn set_final_validation_closures(&self, closures: Vec<Closure>) {
        self.write(|n| n.base.final_validation_closures = closures);
    }

    pub fn is_required(&self) -> bool {
        self.read(|n| n.base.required)
    }

    pub fn is_deprecated(&self) -> bool {
        self.read(|n| n.base.deprecation.is_some())
    }

    fn not_deprecated(&self) -> Error {
        Error::new(
            Class::Logic,
            Msg::new("The node \"")
                .b(&self.get_name())
                .s("\" is not deprecated."),
        )
    }

    /// `getDeprecation($node, $path)`: the message with `%node%` and
    /// `%path%` replaced.
    pub fn get_deprecation(&self, node: &[u8], path: &[u8]) -> Result<Deprecation, Error> {
        let Some(spec) = self.read(|n| n.base.deprecation.clone()) else {
            return Err(self.not_deprecated());
        };
        Ok(Deprecation {
            package: spec.package.to_vec(),
            version: spec.version.to_vec(),
            message: php::strtr(&spec.message, &[(b"%node%", node), (b"%path%", path)]),
        })
    }

    /// `getDeprecationMessage($parent)`: the message for the path of
    /// `$parent ?? $this->parent ?? $this`, after "Since $package
    /// $version: " when either is set.
    pub fn get_deprecation_message(&self, parent: Option<&Node>) -> Result<Vec<u8>, Error> {
        let Some(spec) = self.read(|n| n.base.deprecation.clone()) else {
            return Err(self.not_deprecated());
        };
        let path = match parent.cloned().or_else(|| self.get_parent()) {
            Some(p) => p.get_path(),
            None => self.get_path(),
        };
        let mut message = php::strtr(
            &spec.message,
            &[(b"%node%", &self.get_name()), (b"%path%", &path)],
        );
        let truthy = |s: &PhpString| php::truthy(&Value::String(s.clone()));
        if truthy(&spec.package) || truthy(&spec.version) {
            let mut prefix = Msg::new("Since ")
                .b(&spec.package)
                .s(" ")
                .b(&spec.version)
                .s(": ")
                .into_bytes();
            prefix.append(&mut message);
            message = prefix;
        }
        Ok(message)
    }

    pub fn get_name(&self) -> PhpString {
        self.read(|n| n.base.name.clone())
    }

    /// `setName()`, which only PrototypeNodeInterface classes have.
    pub fn set_name(&self, name: &[u8]) -> Result<(), Error> {
        if !self.is_prototype_node() {
            return Err(undefined_method(&self.class(), "setName"));
        }
        self.write(|n| n.base.name = PhpString::from(name));
        Ok(())
    }

    pub fn get_path(&self) -> PhpString {
        if let Kind::Custom(c) = self.kind()
            && let Some(path) = c.get_path(self)
        {
            return path;
        }
        let (parent, separator, name) = self.read(|n| {
            (
                n.base.parent.clone(),
                n.base.path_separator.clone(),
                n.base.name.clone(),
            )
        });
        match parent {
            Some(parent) => {
                let mut path = parent.get_path().to_vec();
                path.extend_from_slice(&separator);
                path.extend_from_slice(&name);
                PhpString::from(path)
            }
            None => name,
        }
    }

    pub fn get_parent(&self) -> Option<Node> {
        self.read(|n| n.base.parent.clone())
    }

    /// Sets the `parent` property, which upstream's own classes assign
    /// directly.
    pub(crate) fn set_parent(&self, parent: Option<Node>) {
        self.write(|n| n.base.parent = parent);
    }

    fn set_handling_placeholder(&self, value: Option<Value>) {
        self.write(|n| n.base.handling_placeholder = value);
    }

    /// `isHandlingPlaceholder()`.
    pub(crate) fn is_handling_placeholder(&self) -> bool {
        self.read(|n| n.base.handling_placeholder.is_some())
    }

    pub(crate) fn has_final_validation_closures(&self) -> bool {
        self.read(|n| !n.base.final_validation_closures.is_empty())
    }

    /// An InvalidConfigurationException (or subclass) with the path set,
    /// and the node's info as a hint when `hint` is true.
    pub(crate) fn config_error(
        &self,
        class: Class,
        message: impl Into<Vec<u8>>,
        hint: bool,
    ) -> Error {
        let mut e = Error::new(class, message);
        if hint && let Some(info) = self.hint() {
            e.add_hint(&info);
        }
        e.set_path(self.get_path());
        e
    }

    /// `merge($leftSide, $rightSide)`.
    pub fn merge(&self, left: Value, right: Value) -> Result<Value, Error> {
        let allow_overwrite = self.read(|n| n.base.allow_overwrite);
        let left_is_empty_array = matches!(&left, Value::Array(a) if a.is_empty());
        if !allow_overwrite && !left_is_empty_array {
            return Err(Error::new(
                Class::ForbiddenOverwrite,
                Msg::new("Configuration path \"")
                    .b(&self.get_path())
                    .s("\" cannot be overwritten. You have to define all options for this path, and any of its sub-paths in one configuration section."),
            ));
        }
        if let Some(placeholders) = resolve_placeholder_value(&left) {
            for placeholder in placeholders.values() {
                self.set_handling_placeholder(Some(left.clone()));
                let result = self.merge(placeholder.clone(), right.clone());
                self.set_handling_placeholder(None);
                result?;
            }
            return Ok(right);
        }
        if let Some(placeholders) = resolve_placeholder_value(&right) {
            for placeholder in placeholders.values() {
                self.set_handling_placeholder(Some(right.clone()));
                let result = self.merge(left.clone(), placeholder.clone());
                self.set_handling_placeholder(None);
                result?;
            }
            return Ok(right);
        }
        self.do_validate_type(&left)?;
        self.do_validate_type(&right)?;
        self.merge_values(left, right)
    }

    /// `normalize($value)`.
    pub fn normalize(&self, value: Value) -> Result<Value, Error> {
        let mut value = self.pre_normalize(value);
        for closure in self.get_normalization_closures() {
            value = closure(value)?;
        }
        if let Some(placeholders) = resolve_placeholder_value(&value) {
            for placeholder in placeholders.values() {
                self.set_handling_placeholder(Some(value.clone()));
                let result = self.normalize(placeholder.clone());
                self.set_handling_placeholder(None);
                result?;
            }
            return Ok(value);
        }
        for (original, equivalent) in self.read(|n| n.base.equivalent_values.clone()) {
            if original.identical(&value) {
                value = equivalent;
            }
        }
        self.do_validate_type(&value)?;
        self.normalize_value(value)
    }

    /// `finalize($value)`.
    pub fn finalize(&self, value: Value) -> Result<Value, Error> {
        if let Some(placeholders) = resolve_placeholder_value(&value) {
            for placeholder in placeholders.values() {
                self.set_handling_placeholder(Some(value.clone()));
                let result = self.finalize(placeholder.clone());
                self.set_handling_placeholder(None);
                result?;
            }
            return Ok(value);
        }
        self.do_validate_type(&value)?;
        let mut value = self.finalize_value(value)?;
        let closures = self.read(|n| n.base.final_validation_closures.clone());
        for closure in closures {
            match closure(value.clone()) {
                Ok(v) => value = v,
                Err(e) if e.class().is_definition_exception() => {
                    if *e.class() == Class::UnsetKey && self.is_handling_placeholder() {
                        continue;
                    }
                    return Err(e);
                }
                Err(e) if e.class().is_exception() => {
                    let message = Msg::new("Invalid configuration for path \"")
                        .b(&self.get_path())
                        .s("\": ")
                        .b(e.get_message());
                    let code = e.get_code();
                    return Err(Error::with_previous(
                        Class::InvalidConfiguration,
                        message,
                        code,
                        e,
                    ));
                }
                Err(e) => return Err(e),
            }
        }
        Ok(value)
    }

    /// `doValidateType()`.
    fn do_validate_type(&self, value: &Value) -> Result<(), Error> {
        let handling = self.read(|n| n.base.handling_placeholder.clone());
        if handling.is_some() && !self.allow_placeholders() {
            return Err(self.config_error(
                Class::InvalidType,
                Msg::new("A dynamic value is not compatible with a \"")
                    .s(&self.class())
                    .s("\" node type at path \"")
                    .b(&self.get_path())
                    .s("\"."),
                false,
            ));
        }
        let Some(handling) = handling.filter(|_| !value.is_null()) else {
            return self.validate_type(value);
        };
        let Some(known_types) = placeholder_types(&handling) else {
            return Err(Error::new(
                Class::TypeError,
                "array_keys(): Argument #1 ($array) must be of type array, null given",
            ));
        };
        let valid_types = self.valid_placeholder_types();
        if !valid_types.is_empty()
            && known_types
                .iter()
                .any(|t| !valid_types.iter().any(|v| v.as_bytes() == t.as_slice()))
        {
            let list = |types: Vec<Vec<u8>>| -> Vec<u8> {
                if types.len() == 1 {
                    Msg::new("\"").b(&types[0]).s("\"").into_bytes()
                } else {
                    Msg::new("one of \"")
                        .b(&php::implode(b"\", \"", types))
                        .s("\"")
                        .into_bytes()
                }
            };
            let valid = list(valid_types.iter().map(|t| t.as_bytes().to_vec()).collect());
            return Err(self.config_error(
                Class::InvalidType,
                Msg::new("Invalid type for path \"")
                    .b(&self.get_path())
                    .s("\". Expected ")
                    .b(&valid)
                    .s(", but got ")
                    .b(&list(known_types))
                    .s("."),
                true,
            ));
        }
        self.validate_type(value)
    }

    /// `hasDefaultValue()`.
    pub fn has_default_value(&self) -> bool {
        match self.kind() {
            Kind::Variable { data, .. } => data.default_value_set,
            Kind::Array(data) => data.add_if_not_set,
            Kind::Prototyped(..) => true,
            Kind::Custom(c) => c.has_default_value(self),
        }
    }

    /// `getDefaultValue()`.
    pub fn get_default_value(&self) -> Result<Value, Error> {
        match self.kind() {
            Kind::Variable { data, .. } => variable_node::get_default_value(&data),
            Kind::Array(data) => array_node::get_default_value(self, &data),
            Kind::Prototyped(_, data) => prototyped_array_node::get_default_value(&data),
            Kind::Custom(c) => c.get_default_value(self),
        }
    }

    /// The protected `preNormalize()`.
    pub fn pre_normalize(&self, value: Value) -> Value {
        match self.kind() {
            Kind::Array(data) | Kind::Prototyped(data, _) => {
                array_node::pre_normalize(&data, value)
            }
            _ => value,
        }
    }

    /// The protected `validateType()`.
    pub fn validate_type(&self, value: &Value) -> Result<(), Error> {
        match self.kind() {
            Kind::Variable { scalar: None, .. } => Ok(()),
            Kind::Variable {
                scalar: Some(scalar),
                ..
            } => match scalar {
                ScalarKind::Scalar | ScalarKind::Numeric(_) => {
                    scalar_node::validate_type(self, value)
                }
                ScalarKind::Boolean { nullable } => {
                    boolean_node::validate_type(self, value, nullable)
                }
                ScalarKind::Integer(_) => integer_node::validate_type(self, value),
                ScalarKind::Float(_) => float_node::validate_type(self, value),
                ScalarKind::String => string_node::validate_type(self, value),
                ScalarKind::Enum(_) => enum_node::validate_type(self, value),
            },
            Kind::Array(data) | Kind::Prototyped(data, _) => {
                array_node::validate_type(self, &data, value)
            }
            Kind::Custom(c) => c.validate_type(self, value),
        }
    }

    /// The protected `normalizeValue()`.
    pub fn normalize_value(&self, value: Value) -> Result<Value, Error> {
        match self.kind() {
            Kind::Variable { .. } => Ok(value),
            Kind::Array(data) => array_node::normalize_value(self, &data, value),
            Kind::Prototyped(data, proto) => {
                prototyped_array_node::normalize_value(self, &data, &proto, value)
            }
            Kind::Custom(c) => c.normalize_value(self, value),
        }
    }

    /// The protected `mergeValues()`.
    pub fn merge_values(&self, left: Value, right: Value) -> Result<Value, Error> {
        match self.kind() {
            Kind::Variable { .. } => Ok(right),
            Kind::Array(data) => array_node::merge_values(self, &data, left, right),
            Kind::Prototyped(data, proto) => {
                prototyped_array_node::merge_values(self, &data, &proto, left, right)
            }
            Kind::Custom(c) => c.merge_values(self, left, right),
        }
    }

    /// The protected `finalizeValue()`.
    pub fn finalize_value(&self, value: Value) -> Result<Value, Error> {
        match self.kind() {
            Kind::Variable { data, scalar } => match scalar {
                Some(ScalarKind::Numeric(n) | ScalarKind::Integer(n) | ScalarKind::Float(n)) => {
                    numeric_node::finalize_value(self, &data, &n, value)
                }
                Some(ScalarKind::Enum(e)) => enum_node::finalize_value(self, &data, &e, value),
                _ => variable_node::finalize_value(self, &data, value),
            },
            Kind::Array(data) => array_node::finalize_value(self, &data, value),
            Kind::Prototyped(_, proto) => {
                prototyped_array_node::finalize_value(self, &proto, value)
            }
            Kind::Custom(c) => c.finalize_value(self, value),
        }
    }

    /// The protected `isValueEmpty()` of VariableNode and its subclasses.
    pub(crate) fn is_value_empty(&self, value: &Value) -> bool {
        match self.kind() {
            Kind::Variable { scalar: None, .. } => !php::truthy(value),
            Kind::Variable {
                scalar: Some(ScalarKind::Scalar | ScalarKind::String | ScalarKind::Enum(_)),
                ..
            } => {
                // Placeholders are assumed not to be empty, as upstream
                // says environment variables in practice are not.
                !self.is_handling_placeholder()
                    && (value.is_null() || matches!(value, Value::String(s) if s.is_empty()))
            }
            _ => false,
        }
    }

    /// The protected `allowPlaceholders()`.
    fn allow_placeholders(&self) -> bool {
        match self.kind() {
            Kind::Array(_) | Kind::Prototyped(..) => false,
            Kind::Custom(c) => c.allow_placeholders(),
            Kind::Variable { .. } => true,
        }
    }

    /// The protected `getValidPlaceholderTypes()`.
    fn valid_placeholder_types(&self) -> Vec<&'static str> {
        match self.kind() {
            Kind::Variable { scalar: None, .. } | Kind::Array(_) | Kind::Prototyped(..) => {
                Vec::new()
            }
            Kind::Variable {
                scalar: Some(scalar),
                ..
            } => match scalar {
                ScalarKind::Scalar | ScalarKind::Numeric(_) | ScalarKind::Enum(_) => {
                    vec!["bool", "int", "float", "string"]
                }
                ScalarKind::Boolean { .. } => vec!["bool"],
                ScalarKind::Integer(_) => vec!["int"],
                ScalarKind::Float(_) => vec!["float"],
                ScalarKind::String => vec!["string"],
            },
            Kind::Custom(c) => c.valid_placeholder_types(),
        }
    }

    /// Triggers the deprecation of a child that is set, as
    /// `ArrayNode::finalizeValue()` does.
    pub(crate) fn trigger_child_deprecation(&self, name: &[u8], path: &[u8]) -> Result<(), Error> {
        let d = self.get_deprecation(name, path)?;
        deprecation::trigger_deprecation(&d.package, &d.version, &d.message);
        Ok(())
    }
}

/// `setDeprecated()`'s default message.
pub const DEFAULT_DEPRECATION_MESSAGE: &str =
    "The child node \"%node%\" at path \"%path%\" is deprecated.";

pub const BASE_NODE: &str = "Symfony\\Component\\Config\\Definition\\BaseNode";
pub const NODE_INTERFACE: &str = "Symfony\\Component\\Config\\Definition\\NodeInterface";
pub const PROTOTYPE_NODE_INTERFACE: &str =
    "Symfony\\Component\\Config\\Definition\\PrototypeNodeInterface";

/// The node classes a built-in kind is an instance of, below BaseNode.
fn kind_ancestors(kind: &Kind) -> Vec<&'static str> {
    let scalar = |own: &[&'static str]| -> Vec<&'static str> {
        let mut classes = own.to_vec();
        classes.extend([scalar_node::CLASS, variable_node::CLASS]);
        classes
    };
    match kind {
        Kind::Variable { scalar: None, .. } => vec![variable_node::CLASS],
        Kind::Variable {
            scalar: Some(s), ..
        } => match s {
            ScalarKind::Scalar => scalar(&[]),
            ScalarKind::Boolean { .. } => scalar(&[boolean_node::CLASS]),
            ScalarKind::Numeric(_) => scalar(&[numeric_node::CLASS]),
            ScalarKind::Integer(_) => scalar(&[integer_node::CLASS, numeric_node::CLASS]),
            ScalarKind::Float(_) => scalar(&[float_node::CLASS, numeric_node::CLASS]),
            ScalarKind::String => scalar(&[string_node::CLASS]),
            ScalarKind::Enum(_) => scalar(&[enum_node::CLASS]),
        },
        Kind::Array(_) => vec![array_node::CLASS],
        Kind::Prototyped(..) => vec![prototyped_array_node::CLASS, array_node::CLASS],
        Kind::Custom(_) => Vec::new(),
    }
}

fn kind_class(kind: &Kind) -> &'static str {
    match kind {
        Kind::Variable { scalar: None, .. } => crate::definition::variable_node::CLASS,
        Kind::Variable {
            scalar: Some(scalar),
            ..
        } => match scalar {
            ScalarKind::Scalar => scalar_node::CLASS,
            ScalarKind::Boolean { .. } => boolean_node::CLASS,
            ScalarKind::Numeric(_) => numeric_node::CLASS,
            ScalarKind::Integer(_) => integer_node::CLASS,
            ScalarKind::Float(_) => float_node::CLASS,
            ScalarKind::String => string_node::CLASS,
            ScalarKind::Enum(_) => enum_node::CLASS,
        },
        Kind::Array(_) => array_node::CLASS,
        Kind::Prototyped(..) => prototyped_array_node::CLASS,
        Kind::Custom(_) => "",
    }
}
