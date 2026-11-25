//! Build-time class metadata: what `ContainerBuilder` and the compiler
//! passes learn from PHP's reflection (`class_exists()`,
//! `\ReflectionClass`, `\ReflectionMethod`, `\ReflectionFunction`).
//!
//! A build-time port runs no PHP, so the classes a container names are
//! described as data: a [`ClassRegistry`] of [`ClassMetadata`] (kind,
//! parent, interfaces, traits, file, methods with their parameters and
//! types, attributes) and of [`FunctionMetadata`]. The converter fills it
//! from the application's sources; tests fill it with their fixture
//! classes. `ContainerBuilder::getReflectionClass()` reads it, and
//! [`FunctionReflector`] is what `getConstructor()` and
//! `getReflectionMethod()` return.
//!
//! Class and function names compare case-insensitively, as PHP's do; a
//! leading `\` is ignored. A class whose parent, interface or trait is
//! not registered fails to load, as autoloading it fails in PHP
//! ([`ClassRegistry::reflect`]).
//!
//! Upstream: none

use std::collections::HashMap;
use std::sync::Arc;

use php_value::{PhpArray, Value};

/// What a class-like declares itself as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClassKind {
    #[default]
    Class,
    Interface,
    Trait,
    Enum,
}

/// A member's visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Public,
    Protected,
    Private,
}

/// An attribute on a class, method, property or parameter: its class
/// and its arguments (named ones under string keys).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AttributeMetadata {
    pub class: String,
    pub arguments: PhpArray,
}

/// A parameter of a method or function (`\ReflectionParameter`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParameterMetadata {
    pub name: String,
    /// The declared type as PHP prints it (`?Foo`, `int|string`), if any.
    pub type_: Option<String>,
    /// The default value, when the parameter has one that is a constant
    /// expression the port can represent.
    pub default: Option<Value>,
    pub is_optional: bool,
    pub is_variadic: bool,
    pub is_by_reference: bool,
    pub attributes: Vec<AttributeMetadata>,
}

impl ParameterMetadata {
    /// A required parameter `$name` of type `type_`.
    pub fn new(name: &str, type_: Option<&str>) -> Self {
        Self {
            name: name.to_string(),
            type_: type_.map(String::from),
            ..Self::default()
        }
    }

    /// The same parameter, optional with this default.
    #[must_use]
    pub fn with_default(mut self, default: Value) -> Self {
        self.default = Some(default);
        self.is_optional = true;
        self
    }
}

/// A method (`\ReflectionMethod`), as declared by its class.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MethodMetadata {
    pub name: String,
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_abstract: bool,
    pub parameters: Vec<ParameterMetadata>,
    pub return_type: Option<String>,
    pub doc_comment: Option<String>,
    pub attributes: Vec<AttributeMetadata>,
    /// What calling it returns, for the static methods a compiler pass
    /// calls at build time (`getDefaultName()`, `getSubscribedServices()`);
    /// `None` when unknown, and the pass refuses to call it.
    pub returns: Option<Value>,
}

impl MethodMetadata {
    /// A public method without parameters.
    pub fn public(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    #[must_use]
    pub fn with_static(mut self, is_static: bool) -> Self {
        self.is_static = is_static;
        self
    }

    #[must_use]
    pub fn with_parameters(mut self, parameters: Vec<ParameterMetadata>) -> Self {
        self.parameters = parameters;
        self
    }

    /// The same method, returning `value` when called.
    #[must_use]
    pub fn with_returns(mut self, value: Value) -> Self {
        self.returns = Some(value);
        self
    }

    #[must_use]
    pub fn with_return_type(mut self, return_type: &str) -> Self {
        self.return_type = Some(return_type.to_string());
        self
    }

    pub fn is_public(&self) -> bool {
        self.visibility == Visibility::Public
    }
}

/// A property (`\ReflectionProperty`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PropertyMetadata {
    pub name: String,
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_readonly: bool,
    pub type_: Option<String>,
    pub attributes: Vec<AttributeMetadata>,
}

/// A class, interface, trait or enum (`\ReflectionClass`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClassMetadata {
    pub name: String,
    pub kind: ClassKind,
    pub is_abstract: bool,
    pub is_final: bool,
    /// Defined by PHP or an extension (no file).
    pub is_internal: bool,
    /// The file that declares it, as PHP's `getFileName()` gives it.
    pub file: Option<String>,
    pub parent: Option<String>,
    /// The interfaces it implements (an interface: those it extends),
    /// as declared.
    pub interfaces: Vec<String>,
    pub traits: Vec<String>,
    /// The methods it declares, including those its traits import.
    pub methods: Vec<MethodMetadata>,
    pub properties: Vec<PropertyMetadata>,
    pub doc_comment: Option<String>,
    pub attributes: Vec<AttributeMetadata>,
}

impl ClassMetadata {
    /// A concrete class named `name`.
    pub fn class(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Self::default()
        }
    }

    /// An interface named `name`.
    pub fn interface(name: &str) -> Self {
        Self {
            name: name.to_string(),
            kind: ClassKind::Interface,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_parent(mut self, parent: &str) -> Self {
        self.parent = Some(parent.to_string());
        self
    }

    #[must_use]
    pub fn with_interfaces(mut self, interfaces: &[&str]) -> Self {
        self.interfaces = interfaces.iter().map(|s| (*s).to_string()).collect();
        self
    }

    #[must_use]
    pub fn with_abstract(mut self, is_abstract: bool) -> Self {
        self.is_abstract = is_abstract;
        self
    }

    #[must_use]
    pub fn with_file(mut self, file: &str) -> Self {
        self.file = Some(file.to_string());
        self
    }

    #[must_use]
    pub fn with_method(mut self, method: MethodMetadata) -> Self {
        self.methods.push(method);
        self
    }

    /// `isInterface()`.
    pub fn is_interface(&self) -> bool {
        self.kind == ClassKind::Interface
    }

    /// The method this class itself declares.
    pub fn declared_method(&self, name: &str) -> Option<&MethodMetadata> {
        self.methods
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(name))
    }
}

/// A function (`\ReflectionFunction`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FunctionMetadata {
    pub name: String,
    pub file: Option<String>,
    pub parameters: Vec<ParameterMetadata>,
    pub return_type: Option<String>,
    pub doc_comment: Option<String>,
    pub attributes: Vec<AttributeMetadata>,
}

/// What `getConstructor()` and `getReflectionMethod()` return
/// (`\ReflectionFunctionAbstract`).
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionReflector {
    /// A `\ReflectionMethod`: `class` is the declaring class
    /// (`$r->class`).
    Method {
        class: String,
        method: MethodMetadata,
    },
    /// A `\ReflectionFunction` of a named function.
    Function(FunctionMetadata),
    /// `static function (...$args) {}`, which upstream returns for
    /// expression factories (`\ReflectionFunction`, `is_method` false)
    /// and for `__call()`/`__callStatic()` (its `__invoke`
    /// `\ReflectionMethod` of class `Closure`, `is_method` true).
    VariadicClosure { is_method: bool },
}

impl FunctionReflector {
    /// `$r->name`.
    pub fn name(&self) -> &str {
        match self {
            Self::Method { method, .. } => &method.name,
            Self::Function(function) => &function.name,
            Self::VariadicClosure { is_method: true } => "__invoke",
            Self::VariadicClosure { is_method: false } => "{closure}",
        }
    }

    /// `$r->class` of a `\ReflectionMethod`.
    pub fn class(&self) -> Option<&str> {
        match self {
            Self::Method { class, .. } => Some(class),
            Self::VariadicClosure { is_method: true } => Some("Closure"),
            _ => None,
        }
    }

    /// `$r instanceof \ReflectionMethod`.
    pub fn is_method(&self) -> bool {
        matches!(
            self,
            Self::Method { .. } | Self::VariadicClosure { is_method: true }
        )
    }

    /// `getParameters()`.
    pub fn parameters(&self) -> Vec<ParameterMetadata> {
        match self {
            Self::Method { method, .. } => method.parameters.clone(),
            Self::Function(function) => function.parameters.clone(),
            Self::VariadicClosure { .. } => vec![ParameterMetadata {
                name: "args".to_string(),
                is_optional: true,
                is_variadic: true,
                ..ParameterMetadata::default()
            }],
        }
    }
}

/// The reflector an attribute was found on (`\Reflector`), passed to an
/// attribute autoconfigurator.
#[derive(Debug, Clone, PartialEq)]
pub enum Reflector {
    Class(Arc<ClassMetadata>),
    Method {
        class: Arc<ClassMetadata>,
        method: MethodMetadata,
    },
    Property {
        class: Arc<ClassMetadata>,
        property: PropertyMetadata,
    },
    Parameter {
        function: FunctionReflector,
        parameter: ParameterMetadata,
    },
}

fn key(name: &str) -> String {
    name.trim_start_matches('\\').to_ascii_lowercase()
}

/// The classes and functions a build knows.
#[derive(Debug, Clone, Default)]
pub struct ClassRegistry {
    classes: HashMap<String, Arc<ClassMetadata>>,
    functions: HashMap<String, Arc<FunctionMetadata>>,
}

impl ClassRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares a class (replacing one of the same name).
    pub fn add_class(&mut self, class: ClassMetadata) -> &mut Self {
        self.classes.insert(key(&class.name), Arc::new(class));
        self
    }

    /// Declares a function.
    pub fn add_function(&mut self, function: FunctionMetadata) -> &mut Self {
        self.functions
            .insert(key(&function.name), Arc::new(function));
        self
    }

    /// The class-like named `name`, without loading its ancestors.
    pub fn class(&self, name: &str) -> Option<Arc<ClassMetadata>> {
        self.classes.get(&key(name)).cloned()
    }

    /// `function_exists($name)`, with the function.
    pub fn function(&self, name: &str) -> Option<Arc<FunctionMetadata>> {
        self.functions.get(&key(name)).cloned()
    }

    /// `new \ReflectionClass($name)` after the autoloader ran: the
    /// class, `Ok(None)` when it does not exist, or the
    /// `\ReflectionException` message when an ancestor is missing
    /// (`ClassExistenceResource`'s `Class "%s" not found while loading
    /// "%s".`).
    pub fn reflect(&self, name: &str) -> Result<Option<Arc<ClassMetadata>>, String> {
        let Some(class) = self.class(name) else {
            return Ok(None);
        };
        let mut pending = vec![class.clone()];
        let mut seen = Vec::new();
        while let Some(current) = pending.pop() {
            if seen.contains(&key(&current.name)) {
                continue;
            }
            seen.push(key(&current.name));
            let ancestors = current
                .parent
                .iter()
                .chain(&current.interfaces)
                .chain(&current.traits);
            for ancestor in ancestors {
                match self.class(ancestor) {
                    Some(found) => pending.push(found),
                    None => {
                        return Err(format!(
                            "Class \"{}\" not found while loading \"{}\".",
                            ancestor.trim_start_matches('\\'),
                            class.name
                        ));
                    }
                }
            }
        }
        Ok(Some(class))
    }

    /// `$r->hasMethod($name)` and `$r->getMethod($name)`: the method with
    /// its declaring class, searched in the class, its parents, then the
    /// interfaces (abstract methods an abstract class inherits).
    pub fn find_method(
        &self,
        class: &ClassMetadata,
        name: &str,
    ) -> Option<(String, MethodMetadata)> {
        let mut current = Some(Arc::new(class.clone()));
        while let Some(c) = current {
            if let Some(method) = c.declared_method(name) {
                return Some((c.name.clone(), method.clone()));
            }
            current = c.parent.as_deref().and_then(|p| self.class(p));
        }
        self.interface_names(class)
            .into_iter()
            .find_map(|interface| {
                let i = self.class(&interface)?;
                i.declared_method(name).map(|m| (i.name.clone(), m.clone()))
            })
    }

    /// `getInterfaceNames()`: every interface the class implements,
    /// inherited ones included.
    pub fn interface_names(&self, class: &ClassMetadata) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        let mut pending: Vec<String> = Vec::new();
        let mut current = Some(Arc::new(class.clone()));
        while let Some(c) = current {
            pending.extend(c.interfaces.iter().cloned());
            current = c.parent.as_deref().and_then(|p| self.class(p));
        }
        pending.reverse();
        while let Some(name) = pending.pop() {
            let Some(interface) = self.class(&name) else {
                continue;
            };
            if names
                .iter()
                .any(|n| n.eq_ignore_ascii_case(&interface.name))
            {
                continue;
            }
            names.push(interface.name.clone());
            for parent in interface.interfaces.iter().rev() {
                pending.push(parent.clone());
            }
        }
        names
    }

    /// `is_a($class, $parent, true)`: the same class, a subclass, or an
    /// implementation.
    pub fn is_a(&self, class: &str, parent: &str) -> bool {
        if key(class) == key(parent) {
            return true;
        }
        let Some(metadata) = self.class(class) else {
            return false;
        };
        let mut current = metadata.parent.clone();
        while let Some(name) = current {
            if key(&name) == key(parent) {
                return true;
            }
            current = self.class(&name).and_then(|c| c.parent.clone());
        }
        self.interface_names(&metadata)
            .iter()
            .any(|i| key(i) == key(parent))
    }
}
