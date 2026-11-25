//! Port of Symfony\Component\Config\Definition\Builder\NodeBuilder.php:
//! creates child definitions by type, from a mapping of types to classes
//! that `setNodeClass()` extends or overrides.
//!
//! A PHP subclass of `NodeBuilder` is a [`NodeBuilderClass`]: its class
//! name, and its override of the protected `getNodeClass()`. Each
//! `ArrayNodeDefinition` a builder creates gets a copy of the builder
//! (PHP's `clone $this`), so the mapping and the class reach its
//! children.
//!
//! A definition made by a builder without a parent has a null parent in
//! PHP, so its `end()` gives null; here its `end()` gives the builder,
//! the type being fixed when the builder method returns.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/NodeBuilder.php

use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use crate::definition::{
    builder::{
        array_node_definition::{self, ArrayNodeDefinition},
        boolean_node_definition::BooleanNodeDefinition,
        enum_node_definition::EnumNodeDefinition,
        float_node_definition::FloatNodeDefinition,
        integer_node_definition::IntegerNodeDefinition,
        node_definition::{
            Builtin, Def, Definition, IntoName, NodeClass, NodeDefinition, Parent, ParentHandle,
            class,
        },
        scalar_node_definition::ScalarNodeDefinition,
        string_node_definition::StringNodeDefinition,
        variable_node_definition::VariableNodeDefinition,
    },
    exception::{Class, Error},
};

pub const CLASS: &str = "Symfony\\Component\\Config\\Definition\\Builder\\NodeBuilder";

/// A PHP subclass of `NodeBuilder`.
pub trait NodeBuilderClass: Send + Sync {
    /// `static::class`.
    fn class(&self) -> &str;

    /// An override of the protected `getNodeClass($type)`; `None` runs
    /// `NodeBuilder`'s own (`parent::getNodeClass($type)`).
    fn get_node_class(&self, _type_name: &str) -> Option<NodeClass> {
        None
    }
}

#[derive(Clone)]
struct BuilderState {
    parent: Option<Def>,
    node_mapping: Vec<(String, NodeClass)>,
    class: Option<Arc<dyn NodeBuilderClass>>,
}

/// A shared `NodeBuilder`.
#[derive(Clone)]
pub(crate) struct Builder(Arc<Mutex<BuilderState>>);

impl Builder {
    /// `new NodeBuilder()`.
    pub(crate) fn new() -> Self {
        Self::of_class(None)
    }

    fn of_class(class: Option<Arc<dyn NodeBuilderClass>>) -> Self {
        Self(Arc::new(Mutex::new(BuilderState {
            parent: None,
            node_mapping: Builtin::ALL
                .into_iter()
                .map(|b| (b.type_name().to_owned(), NodeClass::Builtin(b)))
                .collect(),
            class,
        })))
    }

    fn lock(&self) -> MutexGuard<'_, BuilderState> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// PHP's `clone $builder`.
    fn clone_object(&self) -> Self {
        Self(Arc::new(Mutex::new(self.lock().clone())))
    }

    pub(crate) fn set_parent(&self, parent: Option<Def>) {
        self.lock().parent = parent;
    }

    fn parent(&self) -> Option<Def> {
        self.lock().parent.clone()
    }

    fn class_name(&self) -> String {
        match &self.lock().class {
            Some(c) => c.class().to_owned(),
            None => CLASS.to_owned(),
        }
    }

    /// PHPUnit's comparison of two builders.
    pub(crate) fn equals(&self, other: &Builder) -> bool {
        if Arc::ptr_eq(&self.0, &other.0) {
            return true;
        }
        let (a, b) = (self.lock().clone(), other.lock().clone());
        let parents = match (&a.parent, &b.parent) {
            (None, None) => true,
            (Some(x), Some(y)) => x.same(y),
            _ => false,
        };
        parents
            && self.class_name() == other.class_name()
            && a.node_mapping.len() == b.node_mapping.len()
            && a.node_mapping
                .iter()
                .zip(&b.node_mapping)
                .all(|((ta, ca), (tb, cb))| ta == tb && ca.name() == cb.name())
    }
}

/// A `NodeBuilder` whose `end()` gives `P`.
pub struct NodeBuilder<P = ()> {
    pub(crate) builder: Builder,
    parent: P,
}

impl<P: Clone> Clone for NodeBuilder<P> {
    fn clone(&self) -> Self {
        Self {
            builder: self.builder.clone(),
            parent: self.parent.clone(),
        }
    }
}

impl<P> fmt::Debug for NodeBuilder<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.builder.class_name())
    }
}

impl Default for NodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeBuilder {
    /// `new NodeBuilder()`.
    pub fn new() -> Self {
        Self::from_builder(Builder::new(), ())
    }

    /// `new $subclass()`: a `NodeBuilder` of a PHP subclass.
    pub fn with_class(class: Arc<dyn NodeBuilderClass>) -> Self {
        Self::from_builder(Builder::of_class(Some(class)), ())
    }
}

/// What can be a `NodeBuilder`'s parent: an `ArrayNodeDefinition`, or none.
pub trait BuilderParent: ParentHandle {
    fn parent_definition(&self) -> Option<NodeDefinition>;
}

impl BuilderParent for () {
    fn parent_definition(&self) -> Option<NodeDefinition> {
        None
    }
}

impl<P: Clone> ParentHandle for NodeBuilder<P> {
    fn as_parent(&self) -> Parent {
        Parent::Object(self.builder.class_name())
    }
}

impl<P: Clone> NodeBuilder<P> {
    pub(crate) fn from_builder(builder: Builder, parent: P) -> Self {
        Self { builder, parent }
    }

    /// `setParent($parent)`: the same builder, ending to `parent`.
    pub fn set_parent<Q: BuilderParent>(&self, parent: Q) -> NodeBuilder<Q> {
        self.builder
            .set_parent(parent.parent_definition().map(|d| d.def));
        NodeBuilder::from_builder(self.builder.clone(), parent)
    }

    /// `static::class`.
    pub fn class(&self) -> String {
        self.builder.class_name()
    }

    /// `instanceof $class`.
    pub fn is_instance_of(&self, class: &str) -> bool {
        let wanted = crate::php::class_name_key(class);
        [
            self.class(),
            CLASS.to_owned(),
            super::node_definition::NODE_PARENT_INTERFACE.to_owned(),
        ]
        .iter()
        .any(|c| crate::php::class_name_key(c) == wanted)
    }

    /// `===`: the same builder.
    pub fn same<Q>(&self, other: &NodeBuilder<Q>) -> bool {
        Arc::ptr_eq(&self.builder.0, &other.builder.0)
    }

    fn typed<D: class::Marker>(
        &self,
        name: &str,
        type_name: &str,
        method: &str,
    ) -> Result<Definition<D, Self>, Error> {
        self.node(name, type_name)?
            .returned(&format!("{CLASS}::{method}"))
    }

    /// `arrayNode($name, $singular)`: with a singular name, the parent
    /// also remaps it to `$name` in XML.
    pub fn array_node(
        &self,
        name: &str,
        singular: Option<&str>,
    ) -> Result<ArrayNodeDefinition<Self>, Error> {
        if let Some(singular) = singular {
            let Some(parent) = self.builder.parent().filter(Def::is_array) else {
                return Err(Error::new(
                    Class::Other {
                        class: "LogicException".to_owned(),
                        exception: true,
                    },
                    "The parent node must be an ArrayNodeDefinition when setting the singular name.",
                ));
            };
            NodeDefinition::from_def(parent, ())
                .downcast::<class::Array>()
                .map(|p| p.fix_xml_config(singular, Some(name.as_bytes())));
        }
        self.typed(name, "array", "arrayNode")
    }

    pub fn scalar_node(&self, name: &str) -> Result<ScalarNodeDefinition<Self>, Error> {
        self.typed(name, "scalar", "scalarNode")
    }

    pub fn boolean_node(&self, name: &str) -> Result<BooleanNodeDefinition<Self>, Error> {
        self.typed(name, "boolean", "booleanNode")
    }

    pub fn integer_node(&self, name: &str) -> Result<IntegerNodeDefinition<Self>, Error> {
        self.typed(name, "integer", "integerNode")
    }

    pub fn float_node(&self, name: &str) -> Result<FloatNodeDefinition<Self>, Error> {
        self.typed(name, "float", "floatNode")
    }

    pub fn enum_node(&self, name: &str) -> Result<EnumNodeDefinition<Self>, Error> {
        self.typed(name, "enum", "enumNode")
    }

    pub fn variable_node(&self, name: &str) -> Result<VariableNodeDefinition<Self>, Error> {
        self.typed(name, "variable", "variableNode")
    }

    pub fn string_node(&self, name: &str) -> Result<StringNodeDefinition<Self>, Error> {
        self.typed(name, "string", "stringNode")
    }

    /// `end()`: the parent.
    pub fn end(&self) -> P {
        self.parent.clone()
    }

    /// `node($name, $type)`: a definition of the class mapped to the type,
    /// appended.
    pub fn node(
        &self,
        name: impl IntoName,
        type_name: &str,
    ) -> Result<NodeDefinition<Self>, Error> {
        let class = self.get_node_class(type_name)?;
        let def = class.instantiate(name.into_name())?;
        self.append_def(&def);
        Ok(NodeDefinition::from_def(def, self.clone()))
    }

    /// `append($node)`.
    #[expect(
        clippy::return_self_not_must_use,
        reason = "a PHP fluent call: it changes the shared builder and returns $this"
    )]
    pub fn append<D: class::Marker, Q: Clone>(&self, node: &Definition<D, Q>) -> Self {
        self.append_def(&node.def);
        self.clone()
    }

    fn append_def(&self, node: &Def) {
        if node.is_array() {
            let builder = self.builder.clone_object();
            builder.set_parent(None);
            array_node_definition::set_builder(node, builder);
        }
        if let Some(parent) = self.builder.parent() {
            array_node_definition::append(&parent, node);
            // This builder becomes the node's parent, for the fluent
            // interface.
            node.set_parent(&self.as_parent());
        }
    }

    /// `setNodeClass($type, $class)`.
    #[expect(
        clippy::return_self_not_must_use,
        reason = "a PHP fluent call: it changes the shared builder and returns $this"
    )]
    pub fn set_node_class(&self, type_name: &str, class: impl Into<NodeClass>) -> Self {
        let type_name = type_name.to_ascii_lowercase();
        let class = class.into();
        let mut state = self.builder.lock();
        match state.node_mapping.iter_mut().find(|(t, _)| *t == type_name) {
            Some((_, slot)) => *slot = class,
            None => state.node_mapping.push((type_name, class)),
        }
        drop(state);
        self.clone()
    }

    /// The protected `getNodeClass($type)`.
    pub(crate) fn get_node_class(&self, type_name: &str) -> Result<NodeClass, Error> {
        let hook = self.builder.lock().class.clone();
        if let Some(class) = hook.and_then(|c| c.get_node_class(type_name)) {
            return Ok(class);
        }
        let type_name = type_name.to_ascii_lowercase();
        let class = self
            .builder
            .lock()
            .node_mapping
            .iter()
            .find(|(t, _)| *t == type_name)
            .map(|(_, c)| c.clone());
        let Some(class) = class else {
            return Err(Error::new(
                Class::Runtime,
                format!("The node type \"{type_name}\" is not registered."),
            ));
        };
        if !class.exists() {
            return Err(Error::new(
                Class::Runtime,
                format!("The node class \"{}\" does not exist.", class.name()),
            ));
        }
        Ok(class)
    }
}
