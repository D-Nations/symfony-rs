//! Port of Symfony\Component\Config\Definition\Builder\NodeDefinition.php,
//! with NodeParentInterface.php: the fluent interface every node
//! definition has (info, defaults, equivalent values, deprecation,
//! normalization and validation rules), and `getNode()`.
//!
//! A [`Definition<C, P>`] is a handle to one shared definition, of class
//! `C` (a marker from [`class`]) whose `end()` gives `P`. Upstream's
//! classes are the [`Builtin`]s; a PHP subclass is a [`NodeClass`]: one
//! that overrides nothing ([`NodeClass::Subclass`]) or one with its own
//! `createNode()` ([`CustomDefinition`]). Methods return a handle to the
//! same definition, so a chain and a variable kept aside see the same
//! changes, as `return $this` gives in PHP.
//!
//! `getNode()` creates nodes with the definition's runtime parent, which
//! upstream sets to the node being built (`$child->parent = $node`). A
//! definition still parented by a builder fails there with the `TypeError`
//! of the node's constructor, as in PHP, without PHP's ", called in
//! <file> on line <n>".
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/config Definition/Builder/NodeDefinition.php Definition/Builder/NodeParentInterface.php

use std::{
    fmt,
    marker::PhantomData,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

use php_value::{Key, Number, PhpArray, PhpString, Value, constants::Registry};

use crate::{
    composer,
    definition::{
        base_node::{DEFAULT_PATH_SEPARATOR, Node},
        builder::{
            array_node_definition::{self, ArrayState},
            expr_builder::{Expr, ExprBuilder, ExprType, build_expressions},
            merge_builder::MergeBuilder,
            normalization_builder::NormalizationBuilder,
            validation_builder::ValidationBuilder,
            variable_node_definition,
        },
        exception::{Class, Error},
        variable_node::DefaultValue,
    },
    php::{self, Msg},
};

pub const NODE_DEFINITION: &str = "Symfony\\Component\\Config\\Definition\\Builder\\NodeDefinition";
pub const NODE_PARENT_INTERFACE: &str =
    "Symfony\\Component\\Config\\Definition\\Builder\\NodeParentInterface";
pub const PARENT_NODE_DEFINITION_INTERFACE: &str =
    "Symfony\\Component\\Config\\Definition\\Builder\\ParentNodeDefinitionInterface";
pub const BUILDER_AWARE_INTERFACE: &str =
    "Symfony\\Component\\Config\\Definition\\Builder\\BuilderAwareInterface";
pub const NUMERIC_NODE_DEFINITION: &str =
    "Symfony\\Component\\Config\\Definition\\Builder\\NumericNodeDefinition";

/// The node definition classes of Config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Array,
    Variable,
    Scalar,
    String,
    Boolean,
    Integer,
    Float,
    Enum,
}

impl Builtin {
    /// In `NodeBuilder`'s mapping order.
    pub const ALL: [Self; 8] = [
        Self::Array,
        Self::Variable,
        Self::Scalar,
        Self::String,
        Self::Boolean,
        Self::Integer,
        Self::Float,
        Self::Enum,
    ];

    /// The class name.
    pub fn class(self) -> &'static str {
        match self {
            Self::Array => "Symfony\\Component\\Config\\Definition\\Builder\\ArrayNodeDefinition",
            Self::Variable => {
                "Symfony\\Component\\Config\\Definition\\Builder\\VariableNodeDefinition"
            }
            Self::Scalar => "Symfony\\Component\\Config\\Definition\\Builder\\ScalarNodeDefinition",
            Self::String => "Symfony\\Component\\Config\\Definition\\Builder\\StringNodeDefinition",
            Self::Boolean => {
                "Symfony\\Component\\Config\\Definition\\Builder\\BooleanNodeDefinition"
            }
            Self::Integer => {
                "Symfony\\Component\\Config\\Definition\\Builder\\IntegerNodeDefinition"
            }
            Self::Float => "Symfony\\Component\\Config\\Definition\\Builder\\FloatNodeDefinition",
            Self::Enum => "Symfony\\Component\\Config\\Definition\\Builder\\EnumNodeDefinition",
        }
    }

    /// The type `NodeBuilder` maps to it.
    pub fn type_name(self) -> &'static str {
        match self {
            Self::Array => "array",
            Self::Variable => "variable",
            Self::Scalar => "scalar",
            Self::String => "string",
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Enum => "enum",
        }
    }

    /// The class by name, compared as PHP compares class names.
    pub fn from_class(class: &str) -> Option<Self> {
        let key = php::class_name_key(class);
        Self::ALL
            .into_iter()
            .find(|b| php::class_name_key(b.class()) == key)
    }

    /// The classes and interfaces it is an instance of.
    fn ancestors(self) -> Vec<&'static str> {
        let mut classes = vec![self.class()];
        match self {
            Self::Array => {
                classes.extend([PARENT_NODE_DEFINITION_INTERFACE, BUILDER_AWARE_INTERFACE]);
            }
            Self::Variable => {}
            Self::Scalar => classes.push(Self::Variable.class()),
            Self::String | Self::Boolean | Self::Enum => {
                classes.extend([Self::Scalar.class(), Self::Variable.class()]);
            }
            Self::Integer | Self::Float => classes.extend([
                NUMERIC_NODE_DEFINITION,
                Self::Scalar.class(),
                Self::Variable.class(),
            ]),
        }
        classes.extend([NODE_DEFINITION, NODE_PARENT_INTERFACE]);
        classes
    }
}

/// A class `NodeBuilder` can map a type to (`setNodeClass($type, $class)`).
#[derive(Clone)]
pub enum NodeClass {
    /// A class name. It names one of the [`Builtin`]s (in any case, with
    /// or without a leading backslash); any other name "does not exist".
    Name(String),
    Builtin(Builtin),
    /// A PHP subclass of a built-in class that overrides nothing.
    Subclass {
        class: Arc<str>,
        of: Builtin,
    },
    /// A PHP class that extends `NodeDefinition` with its own
    /// `createNode()`.
    Custom(Arc<dyn CustomDefinition>),
}

impl NodeClass {
    /// The class name.
    pub fn name(&self) -> String {
        match self {
            Self::Name(n) => n.clone(),
            Self::Builtin(b) => b.class().to_owned(),
            Self::Subclass { class, .. } => class.to_string(),
            Self::Custom(c) => c.class().to_owned(),
        }
    }

    /// `class_exists($class)`.
    pub(crate) fn exists(&self) -> bool {
        match self {
            Self::Name(n) => {
                Builtin::from_class(n).is_some()
                    || [NODE_DEFINITION, NUMERIC_NODE_DEFINITION]
                        .iter()
                        .any(|c| php::class_name_key(c) == php::class_name_key(n))
            }
            _ => true,
        }
    }

    /// `new $class($name)`.
    pub(crate) fn instantiate(&self, name: Option<PhpString>) -> Result<Def, Error> {
        let class = match self {
            Self::Name(n) => {
                if let Some(b) = Builtin::from_class(n) {
                    DefClass::Builtin(b)
                } else {
                    let key = php::class_name_key(n);
                    if let Some(abstract_class) = [NODE_DEFINITION, NUMERIC_NODE_DEFINITION]
                        .into_iter()
                        .find(|c| php::class_name_key(c) == key)
                    {
                        return Err(Error::new(
                            Class::Error,
                            format!("Cannot instantiate abstract class {abstract_class}"),
                        ));
                    }
                    return Err(Error::new(
                        Class::Error,
                        format!("Class \"{}\" not found", n.strip_prefix('\\').unwrap_or(n)),
                    ));
                }
            }
            Self::Builtin(b) => DefClass::Builtin(*b),
            Self::Subclass { class, of } => DefClass::Subclass {
                class: class.clone(),
                of: *of,
            },
            Self::Custom(c) => DefClass::Custom(c.clone()),
        };
        Ok(Def::new(class, name))
    }
}

impl From<&str> for NodeClass {
    fn from(name: &str) -> Self {
        Self::Name(name.to_owned())
    }
}

impl From<String> for NodeClass {
    fn from(name: String) -> Self {
        Self::Name(name)
    }
}

impl From<Builtin> for NodeClass {
    fn from(b: Builtin) -> Self {
        Self::Builtin(b)
    }
}

/// Whether an override replaced `NodeDefinition`'s own method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Call {
    /// Run `NodeDefinition`'s method (`parent::method()`).
    Parent,
    /// The override did all there is to do.
    Done,
}

/// What a definition's parent is, as an override of `setParent()` sees
/// it.
#[derive(Debug, Clone)]
pub enum Parent {
    None,
    /// A node definition (an `ArrayNodeDefinition`).
    Definition(NodeDefinition),
    /// Another object: a `NodeBuilder` or a `TreeBuilder`, by class.
    Object(String),
}

/// A PHP class that extends `NodeDefinition` directly: its `createNode()`,
/// and the public methods it overrides.
pub trait CustomDefinition: Send + Sync {
    /// `static::class`.
    fn class(&self) -> &str;

    /// Classes and interfaces it is an instance of, besides `NodeDefinition`
    /// and `NodeParentInterface`.
    fn ancestors(&self) -> Vec<String> {
        Vec::new()
    }

    /// `createNode()`. `definition` reads the protected properties.
    fn create_node(&self, definition: &NodeDefinition) -> Result<Node, Error>;

    /// An override of `setParent()`.
    fn set_parent(&self, _definition: &NodeDefinition, _parent: &Parent) -> Call {
        Call::Parent
    }

    /// An override of `setPathSeparator()`.
    fn set_path_separator(&self, _definition: &NodeDefinition, _separator: &[u8]) -> Call {
        Call::Parent
    }
}

/// A node name: `?string $name`.
pub trait IntoName {
    fn into_name(self) -> Option<PhpString>;
}

impl IntoName for &str {
    fn into_name(self) -> Option<PhpString> {
        Some(PhpString::from(self))
    }
}

impl IntoName for String {
    fn into_name(self) -> Option<PhpString> {
        Some(PhpString::from(self))
    }
}

impl IntoName for &String {
    fn into_name(self) -> Option<PhpString> {
        Some(PhpString::from(self.as_str()))
    }
}

impl IntoName for &[u8] {
    fn into_name(self) -> Option<PhpString> {
        Some(PhpString::from(self))
    }
}

impl IntoName for PhpString {
    fn into_name(self) -> Option<PhpString> {
        Some(self)
    }
}

impl IntoName for Option<&str> {
    fn into_name(self) -> Option<PhpString> {
        self.map(PhpString::from)
    }
}

impl IntoName for Option<PhpString> {
    fn into_name(self) -> Option<PhpString> {
        self
    }
}

/// The class markers of [`Definition`]: which definitions a handle may
/// hold, checked where upstream has a return type.
pub mod class {
    use super::Builtin;

    pub trait Marker: 'static {
        /// The class a return type names.
        const CLASS: &'static str;
        /// Whether a definition of this built-in class (or of a subclass
        /// of it; `None` for a `CustomDefinition`) is an instance.
        fn accepts(builtin: Option<Builtin>) -> bool;
    }

    /// A class `new` can construct.
    pub trait Concrete: Marker {
        const BUILTIN: Builtin;
    }

    /// `IntegerNodeDefinition` and `FloatNodeDefinition`.
    pub trait Numeric: Marker {}

    /// `NodeDefinition`: any definition.
    pub enum Any {}
    pub enum Array {}
    pub enum Variable {}
    pub enum Scalar {}
    pub enum String {}
    pub enum Boolean {}
    pub enum Integer {}
    pub enum Float {}
    pub enum Enum {}

    impl Marker for Any {
        const CLASS: &'static str = super::NODE_DEFINITION;
        fn accepts(_: Option<Builtin>) -> bool {
            true
        }
    }

    impl Marker for Variable {
        const CLASS: &'static str =
            "Symfony\\Component\\Config\\Definition\\Builder\\VariableNodeDefinition";
        fn accepts(b: Option<Builtin>) -> bool {
            b.is_some_and(|b| b != Builtin::Array)
        }
    }

    impl Marker for Scalar {
        const CLASS: &'static str =
            "Symfony\\Component\\Config\\Definition\\Builder\\ScalarNodeDefinition";
        fn accepts(b: Option<Builtin>) -> bool {
            b.is_some_and(|b| !matches!(b, Builtin::Array | Builtin::Variable))
        }
    }

    macro_rules! exact {
        ($($marker:ident => $builtin:ident),* $(,)?) => {$(
            impl Marker for $marker {
                const CLASS: &'static str = concat!(
                    "Symfony\\Component\\Config\\Definition\\Builder\\",
                    stringify!($builtin),
                    "NodeDefinition"
                );
                fn accepts(b: Option<Builtin>) -> bool {
                    b == Some(Builtin::$builtin)
                }
            }
        )*};
    }
    exact!(
        Array => Array,
        String => String,
        Boolean => Boolean,
        Integer => Integer,
        Float => Float,
        Enum => Enum,
    );

    macro_rules! concrete {
        ($($marker:ident),* $(,)?) => {$(
            impl Concrete for $marker {
                const BUILTIN: Builtin = Builtin::$marker;
            }
        )*};
    }
    concrete!(
        Array, Variable, Scalar, String, Boolean, Integer, Float, Enum
    );

    impl Numeric for Integer {}
    impl Numeric for Float {}
}

/// The runtime class of a definition.
#[derive(Clone)]
pub(crate) enum DefClass {
    Builtin(Builtin),
    Subclass { class: Arc<str>, of: Builtin },
    Custom(Arc<dyn CustomDefinition>),
}

impl DefClass {
    pub(crate) fn name(&self) -> String {
        match self {
            Self::Builtin(b) => b.class().to_owned(),
            Self::Subclass { class, .. } => class.to_string(),
            Self::Custom(c) => c.class().to_owned(),
        }
    }

    /// The built-in class it is or extends.
    pub(crate) fn builtin(&self) -> Option<Builtin> {
        match self {
            Self::Builtin(b) | Self::Subclass { of: b, .. } => Some(*b),
            Self::Custom(_) => None,
        }
    }

    fn instance_of(&self, class: &str) -> bool {
        let wanted = php::class_name_key(class);
        let classes: Vec<String> = match self {
            Self::Builtin(b) => b.ancestors().into_iter().map(str::to_owned).collect(),
            Self::Subclass { class, of } => std::iter::once(class.to_string())
                .chain(of.ancestors().into_iter().map(str::to_owned))
                .collect(),
            Self::Custom(c) => std::iter::once(c.class().to_owned())
                .chain(c.ancestors())
                .chain([NODE_DEFINITION.to_owned(), NODE_PARENT_INTERFACE.to_owned()])
                .collect(),
        };
        classes.iter().any(|c| php::class_name_key(c) == wanted)
    }
}

/// The parent upstream's `$this->parent` holds when a node is created.
#[derive(Clone)]
pub(crate) enum RuntimeParent {
    None,
    Node(Node),
    /// A builder or a definition, by class: not a `NodeInterface`.
    Object(String),
}

impl RuntimeParent {
    fn from_parent(parent: &Parent) -> Self {
        match parent {
            Parent::None => Self::None,
            Parent::Definition(d) => Self::Object(d.class()),
            Parent::Object(class) => Self::Object(class.clone()),
        }
    }
}

/// `NumericNodeDefinition`'s properties.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct NumericState {
    pub(crate) min: Option<Number>,
    pub(crate) max: Option<Number>,
}

/// `EnumNodeDefinition`'s properties.
#[derive(Clone, Default)]
pub(crate) struct EnumState {
    pub(crate) values: Option<Vec<Value>>,
    /// `$enumFqcn`, with the class table it was found in.
    pub(crate) enum_fqcn: Option<(String, Registry)>,
}

/// `NodeDefinition`'s properties, and those of its subclasses.
pub(crate) struct State {
    pub(crate) class: DefClass,
    pub(crate) name: Option<PhpString>,
    pub(crate) parent: RuntimeParent,
    pub(crate) normalization: Option<NormalizationBuilder>,
    pub(crate) validation: Option<ValidationBuilder>,
    /// `$defaultValue`, which stays uninitialized until set.
    pub(crate) default_value: Option<DefaultValue>,
    pub(crate) default: bool,
    pub(crate) required: bool,
    /// `[package, version, message]`.
    pub(crate) deprecation: Option<[PhpString; 3]>,
    pub(crate) merge: Option<MergeBuilder>,
    pub(crate) allow_empty_value: bool,
    pub(crate) null_equivalent: Value,
    pub(crate) true_equivalent: Value,
    pub(crate) false_equivalent: Value,
    pub(crate) path_separator: PhpString,
    pub(crate) attributes: PhpArray,
    /// `ArrayNodeDefinition`'s properties.
    pub(crate) array: Option<Box<ArrayState>>,
    pub(crate) numeric: NumericState,
    pub(crate) enumeration: EnumState,
}

/// A shared definition.
#[derive(Clone)]
pub(crate) struct Def(Arc<Mutex<State>>);

impl Def {
    /// `new $class($name)`: `NodeDefinition`'s constructor and those of the
    /// subclasses that set equivalent values.
    pub(crate) fn new(class: DefClass, name: Option<PhpString>) -> Self {
        let builtin = class.builtin();
        let (null_equivalent, true_equivalent) = match builtin {
            Some(Builtin::Array) => (Value::Array(PhpArray::new()), Value::Array(PhpArray::new())),
            Some(Builtin::String) => (Value::from(""), Value::Bool(true)),
            Some(Builtin::Boolean) => (Value::Bool(true), Value::Bool(true)),
            _ => (Value::Null, Value::Bool(true)),
        };
        Self(Arc::new(Mutex::new(State {
            array: (builtin == Some(Builtin::Array)).then(|| Box::new(ArrayState::default())),
            class,
            name,
            parent: RuntimeParent::None,
            normalization: None,
            validation: None,
            default_value: None,
            default: false,
            required: false,
            deprecation: None,
            merge: None,
            allow_empty_value: true,
            null_equivalent,
            true_equivalent,
            false_equivalent: Value::Bool(false),
            path_separator: PhpString::from(DEFAULT_PATH_SEPARATOR),
            attributes: PhpArray::new(),
            numeric: NumericState::default(),
            enumeration: EnumState::default(),
        })))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reads the state. The closure must not touch other definitions or
    /// nodes (they may lock this one).
    pub(crate) fn read<R>(&self, f: impl FnOnce(&State) -> R) -> R {
        f(&self.lock())
    }

    /// Changes the state, under the same rule as [`Def::read`].
    pub(crate) fn write<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut self.lock())
    }

    pub(crate) fn same(&self, other: &Def) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn class(&self) -> DefClass {
        self.read(|s| s.class.clone())
    }

    pub(crate) fn builtin(&self) -> Option<Builtin> {
        self.read(|s| s.class.builtin())
    }

    pub(crate) fn class_name(&self) -> String {
        self.read(|s| s.class.name())
    }

    pub(crate) fn is_array(&self) -> bool {
        self.builtin() == Some(Builtin::Array)
    }

    fn custom(&self) -> Option<Arc<dyn CustomDefinition>> {
        match self.class() {
            DefClass::Custom(c) => Some(c),
            _ => None,
        }
    }

    /// `$node->setParent($parent)`, through an override when the class
    /// has one.
    pub(crate) fn set_parent(&self, parent: &Parent) {
        if let Some(c) = self.custom()
            && c.set_parent(&NodeDefinition::from_def(self.clone(), ()), parent) == Call::Done
        {
            return;
        }
        let parent = RuntimeParent::from_parent(parent);
        self.write(|s| s.parent = parent);
    }

    /// `$this->parent = $node`, as upstream assigns it directly.
    pub(crate) fn set_parent_node(&self, node: Option<Node>) {
        self.write(|s| {
            s.parent = match node {
                Some(n) => RuntimeParent::Node(n),
                None => RuntimeParent::None,
            }
        });
    }

    /// `setPathSeparator($separator)`: on the children first, for a
    /// `ParentNodeDefinitionInterface`.
    pub(crate) fn set_path_separator(&self, separator: &[u8]) {
        if let Some(c) = self.custom()
            && c.set_path_separator(&NodeDefinition::from_def(self.clone(), ()), separator)
                == Call::Done
        {
            return;
        }
        let children = self.read(|s| {
            s.array.as_ref().map(|a| {
                a.children
                    .iter()
                    .map(|(_, d)| d.clone())
                    .collect::<Vec<_>>()
            })
        });
        for child in children.into_iter().flatten() {
            child.set_path_separator(separator);
        }
        self.write(|s| s.path_separator = PhpString::from(separator));
    }

    /// The runtime parent as `?NodeInterface`, or the `TypeError` of
    /// `constructor` (the node class whose constructor declares
    /// `$parent`).
    pub(crate) fn parent_node(&self, constructor: &str) -> Result<Option<Node>, Error> {
        match self.read(|s| s.parent.clone()) {
            RuntimeParent::None => Ok(None),
            RuntimeParent::Node(n) => Ok(Some(n)),
            RuntimeParent::Object(class) => Err(Error::new(
                Class::TypeError,
                format!(
                    "{constructor}::__construct(): Argument #2 ($parent) must be of type ?Symfony\\Component\\Config\\Definition\\NodeInterface, {class} given"
                ),
            )),
        }
    }

    /// `(string) $this->name`.
    pub(crate) fn name_bytes(&self) -> Vec<u8> {
        self.read(|s| s.name.as_ref().map(|n| n.to_vec()).unwrap_or_default())
    }

    /// `getNode($forceRootNode)`.
    pub(crate) fn get_node(&self, force_root_node: bool) -> Result<Node, Error> {
        if force_root_node {
            self.write(|s| s.parent = RuntimeParent::None);
        }

        if let Some(before) = self.read(|s| s.normalization.as_ref().map(|n| n.before.clone())) {
            let mut allowed_types: Vec<Option<ExprType>> = Vec::new();
            for expr in &before {
                // A closure has no `allowedTypes`: PHP warns and reads
                // null, which is what a second `getNode()` sees.
                let t = match expr {
                    Expr::Builder(e) => Some(e.allowed_types().ok_or_else(|| {
                        Error::new(
                            Class::Error,
                            "Typed property Symfony\\Component\\Config\\Definition\\Builder\\ExprBuilder::$allowedTypes must not be accessed before initialization",
                        )
                    })?),
                    Expr::Closure(_) => None,
                };
                if !allowed_types.contains(&t) {
                    allowed_types.push(t);
                }
            }
            let built = build_expressions(before);
            self.write(|s| {
                if let Some(n) = s.normalization.as_mut() {
                    n.before = built;
                    n.declared_types = allowed_types;
                }
            });
        }

        if let Some(rules) = self.read(|s| s.validation.as_ref().map(|v| v.rules.clone())) {
            let built = build_expressions(rules);
            self.write(|s| {
                if let Some(v) = s.validation.as_mut() {
                    v.rules = built;
                }
            });
        }

        let node = self.create_node()?;
        if node.is_base_node() {
            node.set_attributes(self.read(|s| s.attributes.clone()));
        }
        Ok(node)
    }

    /// `createNode()`.
    fn create_node(&self) -> Result<Node, Error> {
        match self.class() {
            DefClass::Custom(c) => c.create_node(&NodeDefinition::from_def(self.clone(), ())),
            class => match class.builtin() {
                Some(Builtin::Array) | None => array_node_definition::create_node(self),
                Some(builtin) => variable_node_definition::create_node(self, builtin),
            },
        }
    }

    /// The closures of the normalization rules, once built.
    pub(crate) fn normalization_closures(&self) -> Option<Vec<crate::definition::Closure>> {
        self.read(|s| s.normalization.as_ref().map(|n| closures(&n.before)))
    }

    /// The closures of the validation rules, once built.
    pub(crate) fn validation_closures(&self) -> Option<Vec<crate::definition::Closure>> {
        self.read(|s| s.validation.as_ref().map(|v| closures(&v.rules)))
    }

    /// `$this->deprecation`, when set.
    pub(crate) fn deprecation(&self) -> Option<[PhpString; 3]> {
        self.read(|s| s.deprecation.clone())
    }

    /// The equivalent values (`null`, `true`, `false`).
    pub(crate) fn equivalents(&self) -> [Value; 3] {
        self.read(|s| {
            [
                s.null_equivalent.clone(),
                s.true_equivalent.clone(),
                s.false_equivalent.clone(),
            ]
        })
    }
}

fn closures(exprs: &[Expr]) -> Vec<crate::definition::Closure> {
    exprs
        .iter()
        .filter_map(|e| match e {
            Expr::Closure(c) => Some(c.clone()),
            Expr::Builder(_) => None,
        })
        .collect()
}

/// A handle that can be a definition's parent (`NodeParentInterface`).
pub trait ParentHandle: Clone {
    /// What the parent is, as [`Parent`].
    fn as_parent(&self) -> Parent;
}

impl ParentHandle for () {
    fn as_parent(&self) -> Parent {
        Parent::None
    }
}

impl<C: class::Marker, P: Clone> ParentHandle for Definition<C, P> {
    fn as_parent(&self) -> Parent {
        Parent::Definition(NodeDefinition::from_def(self.def.clone(), ()))
    }
}

/// A handle to a node definition of class `C`, whose `end()` gives `P`.
pub struct Definition<C, P = ()> {
    pub(crate) def: Def,
    pub(crate) parent: P,
    class: PhantomData<fn() -> C>,
}

/// `NodeDefinition`: a definition of any class.
pub type NodeDefinition<P = ()> = Definition<class::Any, P>;

impl<C, P: Clone> Clone for Definition<C, P> {
    fn clone(&self) -> Self {
        Self {
            def: self.def.clone(),
            parent: self.parent.clone(),
            class: PhantomData,
        }
    }
}

impl<C, P> fmt::Debug for Definition<C, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (class, name) = self.def.read(|s| (s.class.name(), s.name.clone()));
        write!(f, "{class}({name:?})")
    }
}

impl<C: class::Concrete> Definition<C, ()> {
    /// `new XNodeDefinition($name)`.
    pub fn new(name: impl IntoName) -> Self {
        Self::from_def(
            Def::new(DefClass::Builtin(C::BUILTIN), name.into_name()),
            (),
        )
    }
}

impl NodeDefinition {
    /// `new $class($name)` for any definition class.
    pub fn of_class(class: &NodeClass, name: impl IntoName) -> Result<Self, Error> {
        Ok(Self::from_def(class.instantiate(name.into_name())?, ()))
    }
}

fn invalid_definition(message: impl Into<Vec<u8>>) -> Error {
    Error::new(Class::InvalidDefinition, message)
}

impl<C: class::Marker, P: Clone> Definition<C, P> {
    pub(crate) fn from_def(def: Def, parent: P) -> Self {
        Self {
            def,
            parent,
            class: PhantomData,
        }
    }

    /// `setParent($parent)`: the same definition, ending to `parent`.
    pub fn set_parent<Q: ParentHandle>(&self, parent: Q) -> Definition<C, Q> {
        self.def.set_parent(&parent.as_parent());
        Definition::from_def(self.def.clone(), parent)
    }

    /// `info($info)`.
    pub fn info(&self, info: impl AsRef<[u8]>) -> Self {
        self.attribute(b"info", Value::from(PhpString::from(info.as_ref())))
    }

    /// `example(string|array $example)`; a scalar is cast to a string,
    /// as PHP's coercive typing does.
    pub fn example(&self, example: impl Into<Value>) -> Result<Self, Error> {
        let example = match example.into() {
            v @ (Value::String(_) | Value::Array(_)) => v,
            v @ (Value::Bool(_) | Value::Int(_) | Value::Float(_)) => {
                Value::String(v.to_php_string().unwrap_or_else(|| PhpString::from("")))
            }
            other => {
                return Err(Error::new(
                    Class::TypeError,
                    format!(
                        "{NODE_DEFINITION}::example(): Argument #1 ($example) must be of type array|string, {} given",
                        php::error_type_name(&other)
                    ),
                ));
            }
        };
        Ok(self.attribute(b"example", example))
    }

    /// `docUrl($uri, $package)`: `{package}`, `{version:major}` and
    /// `{version:minor}` replaced, the version being Composer's for the
    /// package ([`crate::composer`]).
    pub fn doc_url(&self, uri: impl AsRef<[u8]>, package: Option<&str>) -> Result<Self, Error> {
        let mut major = Vec::new();
        let mut minor = Vec::new();
        if let Some(p) = package.filter(|p| php::truthy(&Value::from(*p))) {
            let version = composer::get_version(p)?.unwrap_or_default();
            if let Some((ma, mi)) = leading_version(version.as_bytes()) {
                major = ma.to_vec();
                minor = mi.to_vec();
            }
        }
        let package = package.unwrap_or("");
        let url = php::strtr(
            uri.as_ref(),
            &[
                (b"{package}", package.as_bytes()),
                (b"{version:major}", &major),
                (b"{version:minor}", &minor),
            ],
        );
        Ok(self.attribute(b"docUrl", Value::from(PhpString::from(url))))
    }

    /// `attribute($key, $value)`.
    pub fn attribute(&self, key: impl AsRef<[u8]>, value: impl Into<Value>) -> Self {
        let value = value.into();
        self.def.write(|s| {
            s.attributes.insert(Key::from_bytes(key.as_ref()), value);
        });
        self.clone()
    }

    /// `end()`: the parent.
    pub fn end(&self) -> P {
        self.parent.clone()
    }

    /// `getNode($forceRootNode)`.
    pub fn get_node(&self, force_root_node: bool) -> Result<Node, Error> {
        self.def.get_node(force_root_node)
    }

    /// `defaultValue($value)`. `ArrayNodeDefinition` and
    /// `BooleanNodeDefinition` first set the null equivalent from it.
    pub fn default_value(&self, value: impl Into<DefaultValue>) -> Result<Self, Error> {
        let value = value.into();
        let is_null = matches!(&value, DefaultValue::Value(Value::Null));
        match self.def.builtin() {
            Some(Builtin::Array) => self.def.write(|s| {
                s.null_equivalent = if is_null {
                    Value::Null
                } else {
                    Value::Array(PhpArray::new())
                };
            }),
            Some(Builtin::Boolean) => self.def.write(|s| {
                s.null_equivalent = if is_null {
                    Value::Null
                } else {
                    Value::Bool(true)
                };
            }),
            _ => {}
        }
        if self.def.read(|s| s.required) {
            return Err(self.required_and_default());
        }
        self.def.write(|s| {
            s.default = true;
            s.default_value = Some(value);
        });
        Ok(self.clone())
    }

    fn required_and_default(&self) -> Error {
        invalid_definition(
            Msg::new("The node \"")
                .b(&self.def.name_bytes())
                .s("\" cannot be required and have a default value."),
        )
    }

    /// `isRequired()`.
    pub fn is_required(&self) -> Result<Self, Error> {
        if self.def.read(|s| s.default) {
            return Err(self.required_and_default());
        }
        self.def.write(|s| s.required = true);
        Ok(self.clone())
    }

    /// `setDeprecated($package, $version, $message)`; upstream's default
    /// message is [`crate::definition::DEFAULT_DEPRECATION_MESSAGE`].
    pub fn set_deprecated(
        &self,
        package: impl AsRef<[u8]>,
        version: impl AsRef<[u8]>,
        message: impl AsRef<[u8]>,
    ) -> Self {
        let deprecation = [
            PhpString::from(package.as_ref()),
            PhpString::from(version.as_ref()),
            PhpString::from(message.as_ref()),
        ];
        self.def.write(|s| s.deprecation = Some(deprecation));
        self.clone()
    }

    pub fn treat_null_like(&self, value: impl Into<Value>) -> Self {
        let value = value.into();
        self.def.write(|s| s.null_equivalent = value);
        self.clone()
    }

    pub fn treat_true_like(&self, value: impl Into<Value>) -> Self {
        let value = value.into();
        self.def.write(|s| s.true_equivalent = value);
        self.clone()
    }

    pub fn treat_false_like(&self, value: impl Into<Value>) -> Self {
        let value = value.into();
        self.def.write(|s| s.false_equivalent = value);
        self.clone()
    }

    pub fn default_null(&self) -> Result<Self, Error> {
        self.default_value(Value::Null)
    }

    pub fn default_true(&self) -> Result<Self, Error> {
        self.default_value(Value::Bool(true))
    }

    pub fn default_false(&self) -> Result<Self, Error> {
        self.default_value(Value::Bool(false))
    }

    /// `beforeNormalization()`: a new expression run before the value is
    /// normalized.
    pub fn before_normalization(&self) -> ExprBuilder<Self> {
        let expr = self.def.write(|s| {
            s.normalization
                .get_or_insert_with(Default::default)
                .before()
        });
        ExprBuilder::new(expr, self.clone())
    }

    /// `cannotBeEmpty()`, which `BooleanNodeDefinition` and
    /// `NumericNodeDefinition` refuse.
    pub fn cannot_be_empty(&self) -> Result<Self, Error> {
        match self.def.builtin() {
            Some(Builtin::Boolean) => Err(invalid_definition(
                "->cannotBeEmpty() is not applicable to BooleanNodeDefinition.",
            )),
            Some(Builtin::Integer | Builtin::Float) => Err(invalid_definition(
                "->cannotBeEmpty() is not applicable to NumericNodeDefinition.",
            )),
            _ => {
                self.def.write(|s| s.allow_empty_value = false);
                Ok(self.clone())
            }
        }
    }

    /// `validate()`: a new expression run when the value is finalized.
    pub fn validate(&self) -> ExprBuilder<Self> {
        let expr = self
            .def
            .write(|s| s.validation.get_or_insert_with(Default::default).rule());
        ExprBuilder::new(expr, self.clone())
    }

    /// `cannotBeOverwritten($deny)`.
    pub fn cannot_be_overwritten(&self, deny: bool) -> Self {
        self.def.write(|s| {
            s.merge
                .get_or_insert_with(Default::default)
                .deny_overwrite(deny);
        });
        self.clone()
    }

    /// `setPathSeparator($separator)`.
    pub fn set_path_separator(&self, separator: impl AsRef<[u8]>) -> Self {
        self.def.set_path_separator(separator.as_ref());
        self.clone()
    }

    /// `static::class`.
    pub fn class(&self) -> String {
        self.def.class_name()
    }

    /// `instanceof $class`.
    pub fn is_instance_of(&self, class: &str) -> bool {
        self.def.read(|s| s.class.instance_of(class))
    }

    /// `===`: the same definition.
    pub fn same<D, Q>(&self, other: &Definition<D, Q>) -> bool {
        self.def.same(&other.def)
    }

    /// PHPUnit's `assertEquals()` of two definitions: the same class and
    /// equal properties; parents, closures and nodes compare by identity.
    pub fn equals<D, Q>(&self, other: &Definition<D, Q>) -> bool {
        defs_equal(&self.def, &other.def)
    }

    /// The protected `$name`.
    pub fn name(&self) -> Option<PhpString> {
        self.def.read(|s| s.name.clone())
    }

    /// The protected `$pathSeparator`.
    pub fn path_separator(&self) -> PhpString {
        self.def.read(|s| s.path_separator.clone())
    }

    /// The protected `$parent` as the `?NodeInterface $parent` argument
    /// of `constructor`'s constructor (for a [`CustomDefinition`]).
    pub fn parent_node(&self, constructor: &str) -> Result<Option<Node>, Error> {
        self.def.parent_node(constructor)
    }

    /// The same definition as a `NodeDefinition`.
    pub fn erase(&self) -> NodeDefinition<P> {
        Definition::from_def(self.def.clone(), self.parent.clone())
    }

    /// The same definition as class `D`, when it is an instance of it.
    pub fn downcast<D: class::Marker>(&self) -> Option<Definition<D, P>> {
        D::accepts(self.def.builtin())
            .then(|| Definition::from_def(self.def.clone(), self.parent.clone()))
    }

    /// The same definition as the return type `D` of `method`, or PHP's
    /// `TypeError` for it.
    pub(crate) fn returned<D: class::Marker>(
        &self,
        method: &str,
    ) -> Result<Definition<D, P>, Error> {
        self.downcast().ok_or_else(|| {
            Error::new(
                Class::TypeError,
                format!(
                    "{method}(): Return value must be of type {}, {} returned",
                    D::CLASS,
                    self.class()
                ),
            )
        })
    }

    /// `ReflectionProperty::getValue()` of a property that holds a value
    /// (the builders, children and prototype are read through their own
    /// methods).
    pub fn property(&self, name: &str) -> Result<Value, Error> {
        self.def.property(name)
    }
}

/// `preg_match('/^(\d+)\.(\d+)\.(\d+)/', $version, $m)`: `$m[1]`, `$m[2]`.
fn leading_version(version: &[u8]) -> Option<(&[u8], &[u8])> {
    let digits = |s: &[u8]| s.iter().take_while(|b| b.is_ascii_digit()).count();
    let a = digits(version);
    if a == 0 || version.get(a) != Some(&b'.') {
        return None;
    }
    let rest = &version[a + 1..];
    let b = digits(rest);
    if b == 0 || rest.get(b) != Some(&b'.') {
        return None;
    }
    if digits(&rest[b + 1..]) == 0 {
        return None;
    }
    Some((&version[..a], &rest[..b]))
}

fn uninitialized(class: &str, property: &str) -> Error {
    Error::new(
        Class::Error,
        format!("Typed property {class}::${property} must not be accessed before initialization"),
    )
}

fn number_value(n: Option<Number>) -> Value {
    match n {
        None => Value::Null,
        Some(Number::Int(i)) => Value::Int(i),
        Some(Number::Float(f)) => Value::Float(f),
    }
}

fn bytes(s: &PhpString) -> Value {
    Value::String(s.clone())
}

impl Def {
    fn property(&self, name: &str) -> Result<Value, Error> {
        let class = self.class_name();
        let builtin = self.builtin();
        let not_a_value = || {
            Error::new(
                Class::Error,
                format!(
                    "Property {class}::${name} holds an object; the port reads it with its own method"
                ),
            )
        };
        self.read(|s| {
            let common = match name {
                "name" => Some(s.name.as_ref().map_or(Value::Null, bytes)),
                "default" => Some(Value::Bool(s.default)),
                "required" => Some(Value::Bool(s.required)),
                "allowEmptyValue" => Some(Value::Bool(s.allow_empty_value)),
                "nullEquivalent" => Some(s.null_equivalent.clone()),
                "trueEquivalent" => Some(s.true_equivalent.clone()),
                "falseEquivalent" => Some(s.false_equivalent.clone()),
                "pathSeparator" => Some(bytes(&s.path_separator)),
                "attributes" => Some(Value::Array(s.attributes.clone())),
                "deprecation" => Some(Value::Array(match &s.deprecation {
                    None => PhpArray::new(),
                    Some([package, version, message]) => {
                        let mut a = PhpArray::new();
                        a.insert("package", bytes(package));
                        a.insert("version", bytes(version));
                        a.insert("message", bytes(message));
                        a
                    }
                })),
                "defaultValue" => Some(match &s.default_value {
                    None => return Err(uninitialized(NODE_DEFINITION, "defaultValue")),
                    Some(DefaultValue::Value(v)) => v.clone(),
                    Some(DefaultValue::Closure(_)) => return Err(not_a_value()),
                }),
                "parent" | "normalization" | "validation" | "merge" => {
                    return Err(not_a_value());
                }
                _ => None,
            };
            if let Some(v) = common {
                return Ok(v);
            }
            let own = match (builtin, &s.array) {
                (Some(Builtin::Array), Some(a)) => a.property(name),
                (Some(Builtin::Integer | Builtin::Float), _) => match name {
                    "min" => Some(number_value(s.numeric.min)),
                    "max" => Some(number_value(s.numeric.max)),
                    _ => None,
                },
                (Some(Builtin::Enum), _) => match name {
                    "values" => Some(match &s.enumeration.values {
                        Some(v) => Value::Array(PhpArray::list(v.iter().cloned())),
                        None => {
                            return Err(uninitialized(Builtin::Enum.class(), "values"));
                        }
                    }),
                    "enumFqcn" => Some(match &s.enumeration.enum_fqcn {
                        Some((f, _)) => Value::from(f.as_str()),
                        None => {
                            return Err(uninitialized(Builtin::Enum.class(), "enumFqcn"));
                        }
                    }),
                    _ => None,
                },
                _ => None,
            };
            own.ok_or_else(|| {
                Error::new(
                    Class::Other {
                        class: "ReflectionException".to_owned(),
                        exception: true,
                    },
                    format!("Property {class}::${name} does not exist"),
                )
            })
        })
    }
}

/// PHPUnit's comparison of two definition objects.
pub(crate) fn defs_equal(a: &Def, b: &Def) -> bool {
    if a.same(b) {
        return true;
    }
    let (ca, cb) = (a.class(), b.class());
    if php::class_name_key(&ca.name()) != php::class_name_key(&cb.name()) {
        return false;
    }
    let scalars_equal = a.read(|x| {
        b.read(|y| {
            x.name == y.name
                && parents_equal(&x.parent, &y.parent)
                && x.default == y.default
                && default_values_equal(x.default_value.as_ref(), y.default_value.as_ref())
                && x.required == y.required
                && x.deprecation == y.deprecation
                && x.allow_empty_value == y.allow_empty_value
                && x.null_equivalent.identical(&y.null_equivalent)
                && x.true_equivalent.identical(&y.true_equivalent)
                && x.false_equivalent.identical(&y.false_equivalent)
                && x.path_separator == y.path_separator
                && Value::Array(x.attributes.clone()).identical(&Value::Array(y.attributes.clone()))
                && x.normalization.is_some() == y.normalization.is_some()
                && x.validation.is_some() == y.validation.is_some()
                && x.merge == y.merge
                && x.numeric == y.numeric
                && x.array.is_some() == y.array.is_some()
        })
    });
    if !scalars_equal {
        return false;
    }
    let arrays = (
        a.read(|s| s.array.as_deref().cloned()),
        b.read(|s| s.array.as_deref().cloned()),
    );
    match arrays {
        (Some(x), Some(y)) => x.equals(&y),
        _ => true,
    }
}

fn parents_equal(a: &RuntimeParent, b: &RuntimeParent) -> bool {
    match (a, b) {
        (RuntimeParent::None, RuntimeParent::None) => true,
        (RuntimeParent::Node(x), RuntimeParent::Node(y)) => x.same(y),
        (RuntimeParent::Object(x), RuntimeParent::Object(y)) => x == y,
        _ => false,
    }
}

fn default_values_equal(a: Option<&DefaultValue>, b: Option<&DefaultValue>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(DefaultValue::Value(x)), Some(DefaultValue::Value(y))) => x.identical(y),
        (Some(DefaultValue::Closure(x)), Some(DefaultValue::Closure(y))) => Arc::ptr_eq(x, y),
        _ => false,
    }
}
