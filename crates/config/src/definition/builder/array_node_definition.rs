//! Port of Symfony\Component\Config\Definition\Builder\ArrayNodeDefinition.php,
//! with ParentNodeDefinitionInterface.php and BuilderAwareInterface.php:
//! the definition of an `ArrayNode` (named children) or, once it has a
//! prototype, of a `PrototypedArrayNode`; `canBeEnabled()` /
//! `canBeDisabled()`, `acceptAndWrap()`, `find()`, and the checks that
//! refuse options of the other kind of node.
//!
//! Children are keyed as PHP array keys (`$this->children[$name]`), so a
//! child named "0" is the key `0` and a second child of the same name
//! replaces the first in its place. A prototype is also a child under the
//! key `''`, as upstream's `NodeBuilder::append()` makes it.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/ArrayNodeDefinition.php Definition/Builder/ParentNodeDefinitionInterface.php Definition/Builder/BuilderAwareInterface.php

use php_value::{Key, PhpArray, PhpString, Value};

use crate::{
    definition::{
        array_node::ArrayNode,
        base_node::{BASE_NODE, Node},
        builder::{
            boolean_node_definition::BooleanNodeDefinition,
            enum_node_definition::EnumNodeDefinition,
            expr_builder::ExprType,
            float_node_definition::FloatNodeDefinition,
            integer_node_definition::IntegerNodeDefinition,
            node_builder::{Builder, BuilderParent, NodeBuilder},
            node_definition::{Def, Definition, NodeDefinition, Parent, class},
            scalar_node_definition::ScalarNodeDefinition,
            string_node_definition::StringNodeDefinition,
            variable_node_definition::VariableNodeDefinition,
        },
        exception::{Class, Error},
        prototyped_array_node::PrototypedArrayNode,
        variable_node::DefaultValue,
    },
    php::{self, Msg},
};

/// `ArrayNodeDefinition`.
pub type ArrayNodeDefinition<P = ()> = Definition<class::Array, P>;

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\Builder\\ArrayNodeDefinition";

/// `ArrayNodeDefinition`'s properties.
#[derive(Clone)]
pub(crate) struct ArrayState {
    pub(crate) perform_deep_merging: bool,
    pub(crate) ignore_extra_keys: bool,
    pub(crate) remove_extra_keys: bool,
    pub(crate) children: Vec<(Key, Def)>,
    pub(crate) prototype: Option<Def>,
    pub(crate) at_least_one: bool,
    pub(crate) allow_new_keys: bool,
    pub(crate) key: Option<PhpString>,
    pub(crate) remove_key_item: bool,
    pub(crate) add_defaults: bool,
    /// `$addDefaultChildren`: `None` is upstream's `false` (not called).
    pub(crate) add_default_children: Option<Value>,
    pub(crate) node_builder: Option<Builder>,
    pub(crate) normalize_keys: bool,
    pub(crate) allowed_types: Option<Vec<ExprType>>,
}

impl Default for ArrayState {
    fn default() -> Self {
        Self {
            perform_deep_merging: true,
            ignore_extra_keys: false,
            remove_extra_keys: true,
            children: Vec::new(),
            prototype: None,
            at_least_one: false,
            allow_new_keys: true,
            key: None,
            remove_key_item: false,
            add_defaults: false,
            add_default_children: None,
            node_builder: None,
            normalize_keys: true,
            allowed_types: None,
        }
    }
}

impl ArrayState {
    /// The value of a property that holds one.
    pub(crate) fn property(&self, name: &str) -> Option<Value> {
        Some(match name {
            "performDeepMerging" => Value::Bool(self.perform_deep_merging),
            "ignoreExtraKeys" => Value::Bool(self.ignore_extra_keys),
            "removeExtraKeys" => Value::Bool(self.remove_extra_keys),
            "atLeastOne" => Value::Bool(self.at_least_one),
            "allowNewKeys" => Value::Bool(self.allow_new_keys),
            "key" => self.key.clone().map_or(Value::Null, Value::String),
            "removeKeyItem" => Value::Bool(self.remove_key_item),
            "addDefaults" => Value::Bool(self.add_defaults),
            "addDefaultChildren" => self
                .add_default_children
                .clone()
                .unwrap_or(Value::Bool(false)),
            "normalizeKeys" => Value::Bool(self.normalize_keys),
            "allowedTypes" => match &self.allowed_types {
                None => Value::Null,
                Some(types) => Value::Array(PhpArray::list(
                    types.iter().map(|t| Value::from(t.as_str())),
                )),
            },
            _ => return None,
        })
    }

    /// PHPUnit's comparison of the properties.
    pub(crate) fn equals(&self, other: &Self) -> bool {
        let optional_defs = |a: &Option<Def>, b: &Option<Def>| match (a, b) {
            (None, None) => true,
            (Some(x), Some(y)) => super::node_definition::defs_equal(x, y),
            _ => false,
        };
        let builders = match (&self.node_builder, &other.node_builder) {
            (None, None) => true,
            (Some(x), Some(y)) => x.equals(y),
            _ => false,
        };
        self.perform_deep_merging == other.perform_deep_merging
            && self.ignore_extra_keys == other.ignore_extra_keys
            && self.remove_extra_keys == other.remove_extra_keys
            && self.at_least_one == other.at_least_one
            && self.allow_new_keys == other.allow_new_keys
            && self.key == other.key
            && self.remove_key_item == other.remove_key_item
            && self.add_defaults == other.add_defaults
            && match (&self.add_default_children, &other.add_default_children) {
                (None, None) => true,
                (Some(x), Some(y)) => x.identical(y),
                _ => false,
            }
            && self.normalize_keys == other.normalize_keys
            && self.allowed_types == other.allowed_types
            && self.children.len() == other.children.len()
            && self
                .children
                .iter()
                .zip(&other.children)
                .all(|((ka, a), (kb, b))| ka == kb && super::node_definition::defs_equal(a, b))
            && optional_defs(&self.prototype, &other.prototype)
            && builders
    }
}

fn state<R>(def: &Def, f: impl FnOnce(&mut ArrayState) -> R) -> R {
    def.write(|s| f(s.array.get_or_insert_with(Default::default)))
}

/// `append($node)`: `$this->children[$node->name ?? ''] =
/// $node->setParent($this)`.
pub(crate) fn append(parent: &Def, node: &Def) {
    node.set_parent(&Parent::Definition(NodeDefinition::from_def(
        parent.clone(),
        (),
    )));
    let key = Key::from_bytes(&node.name_bytes());
    let node = node.clone();
    state(parent, |a| {
        match a.children.iter_mut().find(|(k, _)| *k == key) {
            Some((_, slot)) => *slot = node,
            None => a.children.push((key, node)),
        }
    });
}

/// `setBuilder($builder)`.
pub(crate) fn set_builder(def: &Def, builder: Builder) {
    state(def, |a| a.node_builder = Some(builder));
}

/// `getNodeBuilder()`: the definition's `NodeBuilder` (a new one at first),
/// parented by the definition.
fn get_node_builder(def: &Def) -> Builder {
    let builder = state(def, |a| {
        a.node_builder.get_or_insert_with(Builder::new).clone()
    });
    builder.set_parent(Some(def.clone()));
    builder
}

/// `int|string|array|null $children` as PHP's coercive typing gives it:
/// a bool is an int, a float in the int range an int (truncated, with an
/// `E_DEPRECATED` for a fraction that is not reproduced), another float
/// (out of range, INF, NAN) a string.
fn default_children_argument(children: Value) -> Result<Value, Error> {
    Ok(match children {
        Value::Bool(b) => Value::Int(i64::from(b)),
        Value::Float(f) if php::float_fits_int(f) => Value::Int(f as i64),
        Value::Float(f) => Value::from(php::float_string(f)),
        Value::Object(o) => {
            return Err(Error::new(
                Class::TypeError,
                format!(
                    "{CLASS}::addDefaultChildrenIfNoneSet(): Argument #1 ($children) must be of type array|string|int|null, {} given",
                    o.class()
                ),
            ));
        }
        other => other,
    })
}

fn enabled(value: bool) -> Value {
    let mut a = PhpArray::new();
    a.insert("enabled", Value::Bool(value));
    Value::Array(a)
}

impl<P: Clone> Definition<class::Array, P> {
    /// `acceptAndWrap($allowedTypes, $key)`: a value of one of the types
    /// becomes `[$key ?? 0 => $v]` before normalization. Only int,
    /// string, bool, null and backed-enum are allowed types.
    pub fn accept_and_wrap(
        &self,
        allowed_types: &[ExprType],
        key: Option<&[u8]>,
    ) -> Result<Self, Error> {
        let types = allowed_types.to_vec();
        state(&self.def, |a| a.allowed_types = Some(types));
        let key = key.map_or(Key::Int(0), Key::from_bytes);
        for t in allowed_types {
            let test: fn(&Value) -> bool = match t {
                ExprType::Int => |v| matches!(v, Value::Int(_)),
                ExprType::String => |v| matches!(v, Value::String(_)),
                ExprType::Bool => |v| matches!(v, Value::Bool(_)),
                ExprType::Null => Value::is_null,
                ExprType::BackedEnum => |v| {
                    v.as_object()
                        .and_then(|o| o.downcast_ref::<php_value::object::EnumCase>())
                        .is_some_and(|c| c.value().is_some())
                },
                ExprType::Any | ExprType::Array => {
                    return Err(Error::new(
                        Class::Other {
                            class: "UnhandledMatchError".to_owned(),
                            exception: false,
                        },
                        format!("Unhandled match case '{}'", t.as_str()),
                    ));
                }
            };
            let key = key.clone();
            self.before_normalization()
                .if_true_with(test)
                .then(move |v| {
                    let mut a = PhpArray::new();
                    a.insert(key.clone(), v);
                    Ok(Value::Array(a))
                });
        }
        Ok(self.clone())
    }

    /// `setBuilder($builder)` (BuilderAwareInterface): the builder that
    /// `children()` and `prototype()` use.
    pub fn set_builder<Q: Clone>(&self, builder: &NodeBuilder<Q>) {
        set_builder(&self.def, builder.builder.clone());
    }

    /// `children()`: the builder that adds children.
    pub fn children(&self) -> NodeBuilder<Self> {
        NodeBuilder::from_builder(get_node_builder(&self.def), self.clone())
    }

    /// `prototype($type)`: sets the prototype of the elements.
    pub fn prototype(&self, type_name: &str) -> Result<NodeDefinition<Self>, Error> {
        let builder = NodeBuilder::from_builder(get_node_builder(&self.def), self.clone());
        let node = builder
            .node(None::<&str>, type_name)?
            .set_parent(self.clone());
        let prototype = node.def.clone();
        state(&self.def, |a| a.prototype = Some(prototype));
        Ok(node)
    }

    fn typed_prototype<D: class::Marker>(
        &self,
        type_name: &str,
        method: &str,
    ) -> Result<Definition<D, Self>, Error> {
        self.prototype(type_name)?
            .returned(&format!("{CLASS}::{method}"))
    }

    pub fn variable_prototype(&self) -> Result<VariableNodeDefinition<Self>, Error> {
        self.typed_prototype("variable", "variablePrototype")
    }

    pub fn scalar_prototype(&self) -> Result<ScalarNodeDefinition<Self>, Error> {
        self.typed_prototype("scalar", "scalarPrototype")
    }

    pub fn string_prototype(&self) -> Result<StringNodeDefinition<Self>, Error> {
        self.typed_prototype("string", "stringPrototype")
    }

    pub fn boolean_prototype(&self) -> Result<BooleanNodeDefinition<Self>, Error> {
        self.typed_prototype("boolean", "booleanPrototype")
    }

    pub fn integer_prototype(&self) -> Result<IntegerNodeDefinition<Self>, Error> {
        self.typed_prototype("integer", "integerPrototype")
    }

    pub fn float_prototype(&self) -> Result<FloatNodeDefinition<Self>, Error> {
        self.typed_prototype("float", "floatPrototype")
    }

    /// `arrayPrototype()`, whose return type is `self`.
    pub fn array_prototype(&self) -> Result<ArrayNodeDefinition<Self>, Error> {
        self.typed_prototype("array", "arrayPrototype")
    }

    pub fn enum_prototype(&self) -> Result<EnumNodeDefinition<Self>, Error> {
        self.typed_prototype("enum", "enumPrototype")
    }

    /// `addDefaultsIfNotSet()` (concrete nodes only).
    pub fn add_defaults_if_not_set(&self) -> Self {
        state(&self.def, |a| a.add_defaults = true);
        self.clone()
    }

    /// `addDefaultChildrenIfNoneSet($children)` (prototype nodes only):
    /// the number of children, a child name or the names; `null` is
    /// upstream's default.
    pub fn add_default_children_if_none_set(
        &self,
        children: impl Into<Value>,
    ) -> Result<Self, Error> {
        let children = default_children_argument(children.into())?;
        state(&self.def, |a| a.add_default_children = Some(children));
        Ok(self.clone())
    }

    /// `requiresAtLeastOneElement()` (prototype nodes only).
    pub fn requires_at_least_one_element(&self) -> Self {
        state(&self.def, |a| a.at_least_one = true);
        self.clone()
    }

    pub fn disallow_new_keys_in_subsequent_configs(&self) -> Self {
        state(&self.def, |a| a.allow_new_keys = false);
        self.clone()
    }

    /// `fixXmlConfig($singular, $plural)`.
    pub fn fix_xml_config(&self, singular: impl AsRef<[u8]>, plural: Option<&[u8]>) -> Self {
        self.def.write(|s| {
            s.normalization
                .get_or_insert_with(Default::default)
                .remap(singular.as_ref(), plural);
        });
        self.clone()
    }

    /// `useAttributeAsKey($name, $removeKeyItem)` (prototype nodes only).
    pub fn use_attribute_as_key(&self, name: impl AsRef<[u8]>, remove_key_item: bool) -> Self {
        let name = PhpString::from(name.as_ref());
        state(&self.def, |a| {
            a.key = Some(name);
            a.remove_key_item = remove_key_item;
        });
        self.clone()
    }

    /// `canBeUnset($allow)`.
    pub fn can_be_unset(&self, allow: bool) -> Self {
        self.def.write(|s| {
            s.merge
                .get_or_insert_with(Default::default)
                .allow_unset(allow);
        });
        self.clone()
    }

    /// `canBeEnabled($info)`: an "enabled" boolean, false by default and
    /// true once anything is configured.
    pub fn can_be_enabled(&self, info: Option<&str>) -> Result<Self, Error> {
        let disabled_node = self
            .attribute(b"auto_enable", Value::Bool(true))
            .add_defaults_if_not_set()
            .treat_false_like(enabled(false))
            .treat_true_like(enabled(true))
            .treat_null_like(enabled(true))
            .before_normalization()
            .if_array()
            .then(|v| match v {
                Value::Array(mut a) => {
                    let key = Key::from("enabled");
                    if a.get(&key).is_none_or(Value::is_null) {
                        a.insert(key, Value::Bool(true));
                    }
                    Ok(Value::Array(a))
                }
                other => Ok(other),
            })
            .end()?
            .children()
            .boolean_node("enabled")?
            .default_false()?;
        if let Some(info) = info.filter(|i| php::truthy(&Value::from(*i))) {
            disabled_node.info(info);
        }
        Ok(self.clone())
    }

    /// `canBeDisabled($info)`: an "enabled" boolean, true by default.
    pub fn can_be_disabled(&self, info: Option<&str>) -> Result<Self, Error> {
        let enabled_node = self
            .attribute(b"auto_enable", Value::Bool(true))
            .add_defaults_if_not_set()
            .treat_false_like(enabled(false))
            .treat_true_like(enabled(true))
            .treat_null_like(enabled(true))
            .children()
            .boolean_node("enabled")?
            .default_true()?;
        if let Some(info) = info.filter(|i| php::truthy(&Value::from(*i))) {
            enabled_node.info(info);
        }
        Ok(self.clone())
    }

    pub fn perform_no_deep_merging(&self) -> Self {
        state(&self.def, |a| a.perform_deep_merging = false);
        self.clone()
    }

    /// `ignoreExtraKeys($remove)`.
    pub fn ignore_extra_keys(&self, remove: bool) -> Self {
        state(&self.def, |a| {
            a.ignore_extra_keys = true;
            a.remove_extra_keys = remove;
        });
        self.clone()
    }

    /// `normalizeKeys($bool)`.
    pub fn normalize_keys(&self, normalize: bool) -> Self {
        state(&self.def, |a| a.normalize_keys = normalize);
        self.clone()
    }

    /// `append($node)` (`ParentNodeDefinitionInterface`).
    pub fn append<D: class::Marker, Q: Clone>(&self, node: &Definition<D, Q>) -> Self {
        append(&self.def, &node.def);
        self.clone()
    }

    /// `getChildNodeDefinitions()`.
    pub fn get_child_node_definitions(&self) -> Vec<(Key, NodeDefinition)> {
        state(&self.def, |a| a.children.clone())
            .into_iter()
            .map(|(k, d)| (k, NodeDefinition::from_def(d, ())))
            .collect()
    }

    /// `find($nodePath)`: the definition at a path of child names joined
    /// by the path separator.
    pub fn find(&self, node_path: impl AsRef<[u8]>) -> Result<NodeDefinition, Error> {
        find(&self.def, node_path.as_ref()).map(|d| NodeDefinition::from_def(d, ()))
    }
}

impl<P: Clone> BuilderParent for Definition<class::Array, P> {
    fn parent_definition(&self) -> Option<NodeDefinition> {
        Some(NodeDefinition::from_def(self.def.clone(), ()))
    }
}

/// `strpos($haystack, $needle)`: an empty needle is found at 0.
fn strpos(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn find(def: &Def, node_path: &[u8]) -> Result<Def, Error> {
    let separator = def.read(|s| s.path_separator.clone());
    let position = strpos(node_path, &separator);
    let first = match position {
        Some(p) => &node_path[..p],
        None => node_path,
    };
    let key = Key::from_bytes(first);
    let child = state(def, |a| {
        a.children
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, d)| d.clone())
    });
    let Some(child) = child else {
        return Err(Error::new(
            Class::Runtime,
            Msg::new("Node with name \"")
                .b(first)
                .s("\" does not exist in the current node \"")
                .b(&def.name_bytes())
                .s("\"."),
        ));
    };
    let Some(p) = position else {
        return Ok(child);
    };
    if !child.is_array() {
        return Err(crate::definition::base_node::undefined_method(
            &child.class_name(),
            "find",
        ));
    }
    find(&child, &node_path[p + separator.len()..])
}

fn invalid_definition(message: Msg) -> Error {
    Error::new(Class::InvalidDefinition, message)
}

/// `validateConcreteNode()`.
fn validate_concrete_node(def: &Def, path: &[u8]) -> Result<(), Error> {
    let (key, allow_empty, at_least_one, default, add_default_children) = def.read(|s| {
        let a = s.array.as_deref().cloned().unwrap_or_default();
        (
            a.key.is_some(),
            s.allow_empty_value,
            a.at_least_one,
            s.default,
            a.add_default_children.is_some(),
        )
    });
    let refuse = |method: &str| {
        Err(invalid_definition(
            Msg::new("->")
                .s(method)
                .s("() is not applicable to concrete nodes at path \"")
                .b(path)
                .s("\"."),
        ))
    };
    if key {
        return refuse("useAttributeAsKey");
    }
    if !allow_empty {
        return refuse("cannotBeEmpty");
    }
    if at_least_one {
        return refuse("requiresAtLeastOneElement");
    }
    if default {
        return refuse("defaultValue");
    }
    if add_default_children {
        return refuse("addDefaultChildrenIfNoneSet");
    }
    Ok(())
}

/// `validatePrototypeNode()`.
fn validate_prototype_node(def: &Def, path: &[u8]) -> Result<(), Error> {
    let (add_defaults, add_default_children, default, has_key) = def.read(|s| {
        let a = s.array.as_deref().cloned().unwrap_or_default();
        (
            a.add_defaults,
            a.add_default_children,
            s.default,
            a.key.is_some(),
        )
    });
    if add_defaults {
        return Err(invalid_definition(
            Msg::new("->addDefaultsIfNotSet() is not applicable to prototype nodes at path \"")
                .b(path)
                .s("\"."),
        ));
    }
    if let Some(children) = add_default_children {
        if default {
            return Err(invalid_definition(
                Msg::new(
                    "A default value and default children might not be used together at path \"",
                )
                .b(path)
                .s("\"."),
            ));
        }
        let unnamed = matches!(children, Value::Null) || matches!(children, Value::Int(n) if n > 0);
        if has_key && unnamed {
            return Err(invalid_definition(
                Msg::new("->addDefaultChildrenIfNoneSet() should set default children names as ->useAttributeAsKey() is used at path \"")
                    .b(path)
                    .s("\"."),
            ));
        }
        if !has_key && matches!(children, Value::String(_) | Value::Array(_)) {
            return Err(invalid_definition(
                Msg::new("->addDefaultChildrenIfNoneSet() might not set default children names as ->useAttributeAsKey() is not used at path \"")
                    .b(path)
                    .s("\"."),
            ));
        }
    }
    Ok(())
}

/// `createNode()`.
pub(crate) fn create_node(def: &Def) -> Result<Node, Error> {
    let parent = def.parent_node(BASE_NODE)?;
    let (name, separator, array, default, default_value, allow_empty, required) = def.read(|s| {
        (
            s.name.as_ref().map(|n| n.to_vec()).unwrap_or_default(),
            s.path_separator.clone(),
            s.array.as_deref().cloned().unwrap_or_default(),
            s.default,
            s.default_value.clone(),
            s.allow_empty_value,
            s.required,
        )
    });

    let node = match &array.prototype {
        None => {
            let node = ArrayNode::with_separator(&name, parent.as_ref(), &separator)?;
            validate_concrete_node(def, &node.get_path())?;
            node.set_add_if_not_set(array.add_defaults)?;
            for (_, child) in &array.children {
                child.set_parent_node(Some(node.clone()));
                node.add_child(&child.get_node(false)?)?;
            }
            node
        }
        Some(prototype) => {
            let node = PrototypedArrayNode::with_separator(&name, parent.as_ref(), &separator)?;
            validate_prototype_node(def, &node.get_path())?;
            if let Some(key) = &array.key {
                node.set_key_attribute(key, array.remove_key_item)?;
            }
            if array.at_least_one || !allow_empty {
                node.set_min_number_of_elements(1)?;
            }
            if default {
                match default_value {
                    Some(DefaultValue::Value(Value::Null)) => node.set_null_as_default()?,
                    Some(DefaultValue::Value(Value::Array(a))) => {
                        node.set_default_value(Value::Array(a))?;
                    }
                    _ => {
                        let mut message = node.get_path().to_vec();
                        message.extend_from_slice(
                            b": the default value of an array node has to be an array or null.",
                        );
                        return Err(Error::new(Class::InvalidArgument, message));
                    }
                }
            }
            if let Some(children) = &array.add_default_children {
                node.set_add_children_if_none_set(children.clone())?;
                if prototype.is_array()
                    && prototype.read(|s| s.array.as_ref().is_none_or(|a| a.prototype.is_none()))
                {
                    state(prototype, |a| a.add_defaults = true);
                }
            }
            prototype.set_parent_node(Some(node.clone()));
            node.set_prototype(&prototype.get_node(false)?)?;
            node
        }
    };

    node.set_allow_new_keys(array.allow_new_keys)?;
    let [null, truthy, falsy] = def.equivalents();
    node.add_equivalent_value(Value::Null, null);
    node.add_equivalent_value(Value::Bool(true), truthy.clone());
    node.add_equivalent_value(Value::Bool(false), falsy.clone());
    node.set_perform_deep_merging(array.perform_deep_merging)?;
    node.set_required(required);
    node.set_ignore_extra_keys(array.ignore_extra_keys, array.remove_extra_keys)?;
    node.set_normalize_keys(array.normalize_keys)?;
    if let Some([package, version, message]) = def.deprecation() {
        node.set_deprecated(&package, &version, &message);
    }

    let mut normalized_types: Vec<Option<ExprType>> = array
        .allowed_types
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(Some)
        .collect();
    let normalization = def.read(|s| {
        s.normalization
            .as_ref()
            .map(|n| (n.declared_types.clone(), n.remappings.clone()))
    });
    if let Some((declared_types, remappings)) = normalization {
        if normalized_types.is_empty() {
            normalized_types = declared_types;
        }
        node.set_normalization_closures(def.normalization_closures().unwrap_or_default());
        node.set_xml_remappings(remappings)?;
    }
    normalized_types.push(Some(ExprType::Array));
    for equivalent in [&truthy, &falsy] {
        if matches!(equivalent, Value::Array(a) if !a.is_empty()) {
            normalized_types.push(Some(ExprType::Bool));
        }
    }
    let mut unique: Vec<PhpString> = Vec::new();
    // A null type (from a second getNode(), see NormalizationBuilder) is
    // dropped: the node's types are strings.
    for t in normalized_types.into_iter().flatten() {
        let t = PhpString::from(t.as_str());
        if !unique.contains(&t) {
            unique.push(t);
        }
    }
    node.set_normalized_types(unique);

    if let Some(merge) = def.read(|s| s.merge) {
        node.set_allow_overwrite(merge.allow_overwrite);
        node.set_allow_false(merge.allow_false)?;
    }
    if let Some(closures) = def.validation_closures() {
        node.set_final_validation_closures(closures);
    }
    Ok(node)
}
