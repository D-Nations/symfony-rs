//! Port of Symfony\Component\HttpKernel\Controller\ControllerResolver.php
//! and Attribute/AsController.php.
//!
//! PHP's resolver reads the `_controller` request attribute (a
//! `Class::method` string, a class or function name, an array `[class or
//! object, method]`, an object, a closure), instantiates classes with
//! `new $class()`, asks `is_callable()` and, for the error messages and
//! `checkController()`, reflection. Nothing is reflected here at run time:
//! the PHP program is declared instead, in a [`ClassRegistry`]:
//!
//! - each [`Class`]: its name, kind (class, abstract class, interface,
//!   trait), parent, interfaces, attributes (as names) and [`Method`]s,
//!   each with its visibility, staticness and body (a [`Controller`]; an
//!   abstract method has none). The constructor is the method named
//!   `__construct`: its body's declared parameters give its arity, and a
//!   body that fails is a constructor that throws;
//! - each [`Function`]: its name, attributes and body.
//!
//! Instances of declared classes are [`Instance`] objects, closures are
//! [`ClosureObject`]s (what `\ReflectionFunction` reports of them, and the
//! controller they run). [`ControllerResolver::resolve`] returns PHP's
//! callable (the same value, or the array with the instantiated object)
//! together with the [`Controller`] the kernel calls; the
//! [`ControllerResolverInterface`] gives the latter.
//!
//! Class and function names are looked up as PHP does: ASCII
//! case-insensitively, one leading backslash ignored. `stdClass` is
//! declared in every registry; `Closure`'s own methods are known, and
//! `bind`, `bindTo`, `call` and `fromCallable` resolve but are not run
//! (their controller throws a `\LogicException`).
//!
//! The `\ArgumentCountError` for a constructor that needs arguments names
//! the PHP file and line that ran `new` (`ControllerResolver.php`, line
//! 152): the file is [`ControllerResolver::with_source_file`]
//! ([`DEFAULT_SOURCE_FILE`] by default; PLAN.md, "Decisions for the
//! owner").
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-kernel Controller/ControllerResolver.php Attribute/AsController.php

use std::{collections::HashMap, fmt, sync::Arc};

use php_value::{Key, Object, PhpArray, PhpObject, PhpString, Value, string::levenshtein};
use psr_log::LoggerInterface;
use symfony_http_foundation::{
    exception::{Class as FoundationClass, Error as FoundationError},
    request::Request,
    request_stack::RequestHandle,
};

use crate::{
    container_controller_resolver::{ControllerContainer, instantiate_controller},
    controller::{Controller, ControllerResolverInterface, ControllerResult},
    exception::{PhpClass, PhpException, Throwable, instance_of, message_of},
};

/// `Symfony\Component\HttpKernel\Attribute\AsController`.
pub const AS_CONTROLLER: &str = "Symfony\\Component\\HttpKernel\\Attribute\\AsController";

/// `Symfony\Component\HttpKernel\Controller\ControllerResolver`, the scope
/// `new $class()` runs in.
pub const CONTROLLER_RESOLVER: &str =
    "Symfony\\Component\\HttpKernel\\Controller\\ControllerResolver";

/// The file a `\ArgumentCountError` from `new $class()` names, unless
/// [`ControllerResolver::with_source_file`] gives another.
pub const DEFAULT_SOURCE_FILE: &str =
    "vendor/symfony/http-kernel/Controller/ControllerResolver.php";

/// The line of `ControllerResolver.php` (v8.1.11) that runs `new $class()`.
const NEW_LINE: i64 = 152;

/// `stdClass`, declared in every registry.
const STD_CLASS: &str = "stdClass";

/// `Closure`'s methods, as `get_class_methods()` lists them, and whether
/// each is static. `__invoke` is callable but not listed.
const CLOSURE_METHODS: [(&str, bool); 4] = [
    ("bind", true),
    ("bindTo", false),
    ("call", false),
    ("fromCallable", true),
];

/// A method's visibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Protected,
    Private,
}

impl Visibility {
    /// The keyword.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Protected => "protected",
            Self::Private => "private",
        }
    }
}

/// What a declared class is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    AbstractClass,
    Interface,
    Trait,
}

impl ClassKind {
    /// How `new` names a kind it cannot instantiate.
    const fn not_instantiable(self) -> Option<&'static str> {
        match self {
            Self::Class => None,
            Self::AbstractClass => Some("abstract class"),
            Self::Interface => Some("interface"),
            Self::Trait => Some("trait"),
        }
    }

    /// `class_exists()`: classes and abstract classes, not interfaces or
    /// traits.
    const fn is_class(self) -> bool {
        matches!(self, Self::Class | Self::AbstractClass)
    }
}

/// A method body that reads `$this`: the object, then the arguments.
type ThisCallable = dyn Fn(&Value, &[Value]) -> Result<ControllerResult, Throwable> + Send + Sync;

#[derive(Clone)]
struct ThisBody(Arc<ThisCallable>);

impl fmt::Debug for ThisBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ThisBody")
    }
}

/// A declared method.
#[derive(Debug, Clone)]
pub struct Method {
    name: String,
    visibility: Visibility,
    is_static: bool,
    attributes: Vec<String>,
    /// `None` for an abstract method.
    body: Option<Controller>,
    /// What runs when the method is called on an object, given `$this`.
    this_body: Option<ThisBody>,
}

impl Method {
    /// A public instance method.
    pub fn new(name: impl Into<String>, body: Controller) -> Self {
        Self {
            name: name.into(),
            visibility: Visibility::Public,
            is_static: false,
            attributes: Vec::new(),
            body: Some(body),
            this_body: None,
        }
    }

    /// A public abstract method (an interface's methods are abstract).
    pub fn abstract_method(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            visibility: Visibility::Public,
            is_static: false,
            attributes: Vec::new(),
            body: None,
            this_body: None,
        }
    }

    /// With a visibility.
    #[must_use]
    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    /// With a body that reads `$this`: called on an object (`[$object,
    /// 'method']`, an invokable object, or a closure made of either), the
    /// kernel runs `function($this, $arguments)`; what reflection reports
    /// (name, parameters, attributes) stays the declared body's, which
    /// runs when there is no object.
    #[must_use]
    pub fn with_this(
        mut self,
        function: impl Fn(&Value, &[Value]) -> Result<ControllerResult, Throwable>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.this_body = Some(ThisBody(Arc::new(function)));
        self
    }

    /// A static method.
    #[must_use]
    pub fn into_static(mut self) -> Self {
        self.is_static = true;
        self
    }

    /// With attributes (their class names, as `getName()` gives them).
    #[must_use]
    pub fn with_attributes<S: Into<String>>(
        mut self,
        attributes: impl IntoIterator<Item = S>,
    ) -> Self {
        self.attributes = attributes.into_iter().map(Into::into).collect();
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn is_static(&self) -> bool {
        self.is_static
    }

    pub fn is_abstract(&self) -> bool {
        self.body.is_none()
    }

    pub fn attributes(&self) -> &[String] {
        &self.attributes
    }

    /// The body; `None` for an abstract method.
    pub fn body(&self) -> Option<&Controller> {
        self.body.as_ref()
    }
}

/// A declared class, abstract class, interface or trait.
#[derive(Debug, Clone)]
pub struct Class {
    name: String,
    kind: ClassKind,
    parent: Option<String>,
    interfaces: Vec<String>,
    attributes: Vec<String>,
    methods: Vec<Method>,
}

impl Class {
    /// A class of the given kind. An anonymous class is a class named as
    /// PHP names it (`Parent@anonymous`, a NUL, then its file, line and
    /// number).
    pub fn of_kind(kind: ClassKind, name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind,
            parent: None,
            interfaces: Vec::new(),
            attributes: Vec::new(),
            methods: Vec::new(),
        }
    }

    /// A class.
    pub fn new(name: impl Into<String>) -> Self {
        Self::of_kind(ClassKind::Class, name)
    }

    /// `extends $parent`.
    #[must_use]
    pub fn extends(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(parent.into());
        self
    }

    /// `implements ...` (for an interface, `extends ...`).
    #[must_use]
    pub fn implements<S: Into<String>>(mut self, interfaces: impl IntoIterator<Item = S>) -> Self {
        self.interfaces = interfaces.into_iter().map(Into::into).collect();
        self
    }

    /// With attributes (their class names).
    #[must_use]
    pub fn with_attributes<S: Into<String>>(
        mut self,
        attributes: impl IntoIterator<Item = S>,
    ) -> Self {
        self.attributes = attributes.into_iter().map(Into::into).collect();
        self
    }

    /// With a method, after the others.
    #[must_use]
    pub fn with_method(mut self, method: Method) -> Self {
        self.methods.push(method);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> ClassKind {
        self.kind
    }

    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }

    pub fn interfaces(&self) -> &[String] {
        &self.interfaces
    }

    pub fn attributes(&self) -> &[String] {
        &self.attributes
    }

    pub fn methods(&self) -> &[Method] {
        &self.methods
    }
}

/// A declared function.
#[derive(Debug, Clone)]
pub struct Function {
    name: String,
    attributes: Vec<String>,
    body: Controller,
}

impl Function {
    /// The function `name` (with its namespace).
    pub fn new(name: impl Into<String>, body: Controller) -> Self {
        Self {
            name: name.into(),
            attributes: Vec::new(),
            body,
        }
    }

    /// With attributes (their class names).
    #[must_use]
    pub fn with_attributes<S: Into<String>>(
        mut self,
        attributes: impl IntoIterator<Item = S>,
    ) -> Self {
        self.attributes = attributes.into_iter().map(Into::into).collect();
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attributes(&self) -> &[String] {
        &self.attributes
    }

    pub fn body(&self) -> &Controller {
        &self.body
    }
}

/// A declaration the registry refuses (PHP would not compile it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationError(String);

impl fmt::Display for DeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DeclarationError {}

/// An instance of a declared class.
#[derive(Debug)]
pub struct Instance {
    class: Arc<Class>,
}

impl Instance {
    /// The class.
    pub fn class_decl(&self) -> &Arc<Class> {
        &self.class
    }
}

impl PhpObject for Instance {
    fn class(&self) -> &str {
        &self.class.name
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// A `\Closure`: what `\ReflectionFunction` reports of it, and the
/// controller it runs.
#[derive(Debug)]
pub struct ClosureObject {
    name: String,
    called_class: Option<String>,
    anonymous: bool,
    attributes: Vec<String>,
    controller: Controller,
}

impl ClosureObject {
    /// A closure as reflection describes it: its `name`, its
    /// `getClosureCalledClass()`, `isAnonymous()` and attributes. The
    /// controller takes the name `ArgumentMetadataFactory` gives it.
    pub fn new<S: Into<String>>(
        name: impl Into<String>,
        called_class: Option<String>,
        anonymous: bool,
        attributes: impl IntoIterator<Item = S>,
        controller: Controller,
    ) -> Self {
        let name = name.into();
        let pretty = match &called_class {
            Some(class) if !anonymous => format!("{class}::{name}"),
            _ => name.clone(),
        };
        Self {
            name,
            called_class,
            anonymous,
            attributes: attributes.into_iter().map(Into::into).collect(),
            controller: controller.with_name(pretty),
        }
    }

    /// `static function () {}`: anonymous, named as its controller is
    /// (PHP 8.4 names closures `{closure:file:line}`).
    pub fn anonymous<S: Into<String>>(
        controller: Controller,
        attributes: impl IntoIterator<Item = S>,
    ) -> Self {
        let name = controller.name().to_owned();
        Self::new(name, None, true, attributes, controller)
    }

    /// `function_name(...)`.
    pub fn from_function(function: &Function) -> Self {
        Self::new(
            function.name.clone(),
            None,
            false,
            function.attributes.clone(),
            function.body.clone(),
        )
    }

    /// As a value.
    pub fn object(self) -> Value {
        Value::Object(Arc::new(self))
    }

    /// `\ReflectionFunction::$name`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `getClosureCalledClass()?->name`.
    pub fn called_class(&self) -> Option<&str> {
        self.called_class.as_deref()
    }

    /// `isAnonymous()`.
    pub fn is_anonymous(&self) -> bool {
        self.anonymous
    }

    /// The attributes' names.
    pub fn attributes(&self) -> &[String] {
        &self.attributes
    }

    /// What calling the closure runs.
    pub fn controller(&self) -> &Controller {
        &self.controller
    }
}

impl PhpObject for ClosureObject {
    fn class(&self) -> &str {
        php_value::object::Closure::CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// The names `new`, `class_exists()` and the others look up: one leading
/// backslash dropped, ASCII lower case.
fn lookup_key(name: &[u8]) -> Vec<u8> {
    name.strip_prefix(b"\\")
        .unwrap_or(name)
        .to_ascii_lowercase()
}

fn is_closure_class(name: &[u8]) -> bool {
    lookup_key(name) == b"closure"
}

/// One method in a class's method table, with the class declaring it.
#[derive(Clone)]
struct Entry {
    lower: Vec<u8>,
    method: Method,
    declaring: Arc<Class>,
}

/// The declared PHP program: classes and functions.
#[derive(Debug)]
pub struct ClassRegistry {
    classes: Vec<Arc<Class>>,
    class_index: HashMap<Vec<u8>, usize>,
    functions: Vec<Arc<Function>>,
    function_index: HashMap<Vec<u8>, usize>,
}

impl Default for ClassRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// What a callable resolves to.
#[derive(Clone)]
enum Target {
    Closure(Controller),
    Method {
        called: Arc<Class>,
        method: Method,
        /// `$this`: the object a non-static method is called on.
        this: Option<Value>,
    },
    /// `__call` / `__callStatic` with the name called.
    Magic {
        called: Arc<Class>,
        magic: Method,
        name: PhpString,
    },
    Function(Arc<Function>),
    /// One of `Closure`'s own methods.
    ClosureMethod(&'static str),
}

/// An object's class, as the resolver sees it.
enum ObjectClass<'a> {
    Declared(Arc<Class>),
    Closure(Closure<'a>),
    /// A class the registry does not declare: no methods.
    Other(&'a str),
}

enum Closure<'a> {
    Declared(&'a ClosureObject),
    Value(&'a php_value::object::Closure),
}

impl Closure<'_> {
    fn controller(&self) -> Controller {
        match self {
            Self::Declared(c) => c.controller.clone(),
            Self::Value(c) => {
                let c = (*c).clone();
                Controller::new(move |arguments| Ok(ControllerResult::Value(c.call(arguments))))
            }
        }
    }
}

impl ClassRegistry {
    /// A registry with `stdClass`.
    pub fn new() -> Self {
        let mut registry = Self {
            classes: Vec::new(),
            class_index: HashMap::new(),
            functions: Vec::new(),
            function_index: HashMap::new(),
        };
        registry.insert_class(Class::new(STD_CLASS));
        registry
    }

    fn insert_class(&mut self, class: Class) {
        self.class_index.insert(
            class.name.to_ascii_lowercase().into_bytes(),
            self.classes.len(),
        );
        self.classes.push(Arc::new(class));
    }

    /// Declares a class. Its parent and interfaces must be declared
    /// first; a class (not abstract) must implement every abstract method
    /// it inherits.
    pub fn declare_class(&mut self, class: Class) -> Result<(), DeclarationError> {
        let refuse = |message: String| Err(DeclarationError(message));
        let name = &class.name;
        if name.is_empty() || name.starts_with('\\') {
            return refuse(format!("Invalid class name \"{name}\""));
        }
        let key = name.to_ascii_lowercase().into_bytes();
        if self.class_index.contains_key(&key) || is_closure_class(&key) {
            return refuse(format!(
                "Cannot declare class {name}, because the name is already in use"
            ));
        }
        if let Some(parent) = &class.parent {
            match self.class(parent.as_bytes()) {
                None => return refuse(format!("Class \"{parent}\" not found (parent of {name})")),
                Some(p) if !p.kind.is_class() || class.kind == ClassKind::Interface => {
                    return refuse(format!("{name} cannot extend {}", p.name));
                }
                Some(_) => {}
            }
        }
        for interface in &class.interfaces {
            match self.class(interface.as_bytes()) {
                Some(i) if i.kind == ClassKind::Interface => {}
                _ => {
                    return refuse(format!(
                        "{name} cannot implement {interface}: not an interface"
                    ));
                }
            }
        }
        let mut seen = Vec::new();
        for method in &class.methods {
            let lower = method.name.to_ascii_lowercase();
            if seen.contains(&lower) {
                return refuse(format!("Cannot redeclare {name}::{}()", method.name));
            }
            seen.push(lower);
            if method.is_abstract() && matches!(class.kind, ClassKind::Class | ClassKind::Trait) {
                return refuse(format!(
                    "{name}::{}() is abstract in a class that is not",
                    method.name
                ));
            }
        }
        let class = Arc::new(class);
        if class.kind == ClassKind::Class
            && let Some(missing) = self
                .table(&class)
                .into_iter()
                .find(|e| e.method.is_abstract())
        {
            return refuse(format!(
                "{} must implement the abstract method {}::{}()",
                class.name, missing.declaring.name, missing.method.name
            ));
        }
        self.class_index.insert(key, self.classes.len());
        self.classes.push(class);
        Ok(())
    }

    /// Declares a function.
    pub fn declare_function(&mut self, function: Function) -> Result<(), DeclarationError> {
        let name = &function.name;
        if name.is_empty() || name.starts_with('\\') {
            return Err(DeclarationError(format!(
                "Invalid function name \"{name}\""
            )));
        }
        let key = name.to_ascii_lowercase().into_bytes();
        if self.function_index.contains_key(&key) {
            return Err(DeclarationError(format!(
                "Cannot redeclare function {name}()"
            )));
        }
        self.function_index.insert(key, self.functions.len());
        self.functions.push(Arc::new(function));
        Ok(())
    }

    /// The class (abstract class, interface, trait) named `name`.
    pub fn class(&self, name: &[u8]) -> Option<&Arc<Class>> {
        self.class_index
            .get(&lookup_key(name))
            .and_then(|&i| self.classes.get(i))
    }

    /// The function named `name` (`function_exists()`).
    pub fn function(&self, name: &[u8]) -> Option<&Arc<Function>> {
        self.function_index
            .get(&lookup_key(name))
            .and_then(|&i| self.functions.get(i))
    }

    /// An object of the class `name`, as a successful `new` gives it
    /// (the constructor is not run); `None` for a name that is not a
    /// declared class, or an abstract class, interface or trait.
    pub fn new_instance(&self, name: &[u8]) -> Option<Value> {
        let class = self.class(name).filter(|c| c.kind == ClassKind::Class)?;
        Some(Value::Object(Arc::new(Instance {
            class: class.clone(),
        })))
    }

    /// `class_exists($name)`.
    fn class_exists(&self, name: &[u8]) -> bool {
        is_closure_class(name) || self.class(name).is_some_and(|c| c.kind.is_class())
    }

    /// The class's method table, in PHP's order: its own methods, then
    /// what its parent's table adds, then its interfaces'.
    fn table(&self, class: &Arc<Class>) -> Vec<Entry> {
        let mut entries: Vec<Entry> = class
            .methods
            .iter()
            .map(|m| Entry {
                lower: m.name.to_ascii_lowercase().into_bytes(),
                method: m.clone(),
                declaring: class.clone(),
            })
            .collect();
        let inherited = class
            .parent
            .iter()
            .chain(&class.interfaces)
            .filter_map(|name| self.class(name.as_bytes()));
        for ancestor in inherited {
            for entry in self.table(ancestor) {
                if !entries.iter().any(|e| e.lower == entry.lower) {
                    entries.push(entry);
                }
            }
        }
        entries
    }

    /// The method `name` of `class`. A parent's private method is found
    /// only with `parent_private` (`method_exists()` on an object and
    /// reflection see it, `method_exists()` on a class name does not).
    fn find_method(&self, class: &Arc<Class>, name: &[u8], parent_private: bool) -> Option<Entry> {
        let lower = name.to_ascii_lowercase();
        self.table(class).into_iter().find(|e| {
            e.lower == lower
                && (parent_private
                    || Arc::ptr_eq(&e.declaring, class)
                    || e.method.visibility != Visibility::Private)
        })
    }

    /// The class and its ancestors (parents, interfaces), lower case.
    fn ancestry(&self, class: &Arc<Class>) -> Vec<Vec<u8>> {
        let mut names = vec![class.name.to_ascii_lowercase().into_bytes()];
        for name in class.parent.iter().chain(&class.interfaces) {
            if let Some(ancestor) = self.class(name.as_bytes()) {
                names.extend(self.ancestry(ancestor));
            }
        }
        names
    }

    fn object_class<'a>(&self, object: &'a Object) -> ObjectClass<'a> {
        if let Some(instance) = object.downcast_ref::<Instance>() {
            return ObjectClass::Declared(instance.class.clone());
        }
        if let Some(closure) = object.downcast_ref::<ClosureObject>() {
            return ObjectClass::Closure(Closure::Declared(closure));
        }
        if let Some(closure) = object.downcast_ref::<php_value::object::Closure>() {
            return ObjectClass::Closure(Closure::Value(closure));
        }
        match self.class(object.class().as_bytes()) {
            Some(class) => ObjectClass::Declared(class.clone()),
            None => ObjectClass::Other(object.class()),
        }
    }

    /// `is_callable([$class, $method])` and `is_callable('Class::method')`,
    /// from outside the class.
    fn static_callable(&self, class: &[u8], method: &[u8]) -> Option<Target> {
        if is_closure_class(class) {
            return CLOSURE_METHODS
                .iter()
                .find(|(name, is_static)| {
                    *is_static && name.as_bytes().eq_ignore_ascii_case(method)
                })
                .map(|(name, _)| Target::ClosureMethod(name));
        }
        let class = self.class(class)?;
        match self.find_method(class, method, true) {
            Some(e) if e.method.visibility == Visibility::Public => {
                (e.method.is_static && !e.method.is_abstract()).then(|| Target::Method {
                    called: class.clone(),
                    method: e.method,
                    this: None,
                })
            }
            _ => self.magic(class, b"__callStatic", method),
        }
    }

    /// `is_callable([$object, $method])`, from outside the class.
    fn instance_callable(&self, object: &Object, method: &[u8]) -> Option<Target> {
        match self.object_class(object) {
            ObjectClass::Closure(closure) => {
                if method.eq_ignore_ascii_case(b"__invoke") {
                    return Some(Target::Closure(closure.controller()));
                }
                CLOSURE_METHODS
                    .iter()
                    .find(|(name, _)| name.as_bytes().eq_ignore_ascii_case(method))
                    .map(|(name, _)| Target::ClosureMethod(name))
            }
            ObjectClass::Declared(class) => match self.find_method(&class, method, true) {
                Some(e) if e.method.visibility == Visibility::Public && !e.method.is_abstract() => {
                    let this = (!e.method.is_static).then(|| Value::Object(object.clone()));
                    Some(Target::Method {
                        called: class,
                        method: e.method,
                        this,
                    })
                }
                _ => self.magic(&class, b"__call", method),
            },
            ObjectClass::Other(_) => None,
        }
    }

    fn magic(&self, class: &Arc<Class>, magic: &[u8], name: &[u8]) -> Option<Target> {
        let entry = self.find_method(class, magic, true)?;
        (!entry.method.is_abstract()).then(|| Target::Magic {
            called: class.clone(),
            magic: entry.method,
            name: PhpString::from(name),
        })
    }

    /// `is_callable($value)`, from outside any class.
    fn callable(&self, value: &Value) -> Option<Target> {
        match value {
            Value::String(s) => {
                let s = s.as_bytes();
                match find(s, b"::") {
                    Some(at) => self.static_callable(&s[..at], &s[at + 2..]),
                    None => self.function(s).cloned().map(Target::Function),
                }
            }
            Value::Array(array) => {
                if array.len() != 2 {
                    return None;
                }
                let Some(Value::String(method)) = array.get(&Key::Int(1)) else {
                    return None;
                };
                match array.get(&Key::Int(0))? {
                    Value::String(class) => {
                        self.static_callable(class.as_bytes(), method.as_bytes())
                    }
                    Value::Object(object) => self.instance_callable(object, method.as_bytes()),
                    _ => None,
                }
            }
            Value::Object(object) => match self.object_class(object) {
                ObjectClass::Closure(closure) => Some(Target::Closure(closure.controller())),
                ObjectClass::Declared(class) => self
                    .find_method(&class, b"__invoke", true)
                    .filter(|e| !e.method.is_abstract())
                    .map(|e| Target::Method {
                        called: class,
                        this: (!e.method.is_static).then(|| value.clone()),
                        method: e.method,
                    }),
                ObjectClass::Other(_) => None,
            },
            _ => None,
        }
    }

    /// `\Closure::fromCallable($callable)`, from outside any class: `None`
    /// when the value is not callable (PHP throws a `\TypeError`) or is
    /// one of `Closure`'s own methods.
    pub fn closure_from_callable(&self, callable: &Value) -> Option<Value> {
        if let Value::Object(object) = callable
            && let ObjectClass::Closure(_) = self.object_class(object)
        {
            return Some(callable.clone());
        }
        let closure = match self.callable(callable)? {
            Target::Closure(_) | Target::ClosureMethod(_) => return None,
            Target::Function(f) => ClosureObject::from_function(&f),
            Target::Method {
                called,
                method,
                this,
            } => {
                let controller = controller_of(&Target::Method {
                    called: called.clone(),
                    method: method.clone(),
                    this,
                });
                ClosureObject::new(
                    method.name.clone(),
                    Some(called.name.clone()),
                    false,
                    method.attributes.clone(),
                    controller,
                )
            }
            Target::Magic {
                called,
                magic,
                name,
            } => {
                // The trampoline has the magic method's attributes.
                let (class, method) = (called.name.clone(), name.to_string_lossy().into_owned());
                let attributes = magic.attributes.clone();
                let controller = controller_of(&Target::Magic {
                    called,
                    magic,
                    name,
                });
                ClosureObject::new(method, Some(class), false, attributes, controller)
            }
        };
        Some(closure.object())
    }

    /// `get_class_methods($objectOrClass)` from outside the class, without
    /// the names starting with `__`.
    fn methods_without_magic(&self, class: &ObjectClass<'_>) -> Vec<PhpString> {
        let names: Vec<PhpString> = match class {
            ObjectClass::Declared(class) => self
                .table(class)
                .into_iter()
                .filter(|e| e.method.visibility == Visibility::Public)
                .map(|e| PhpString::from(e.method.name))
                .collect(),
            ObjectClass::Closure(_) => CLOSURE_METHODS
                .iter()
                .map(|(name, _)| PhpString::from(*name))
                .collect(),
            ObjectClass::Other(_) => Vec::new(),
        };
        names
            .into_iter()
            .filter(|name| !name.as_bytes().starts_with(b"__"))
            .collect()
    }
}

/// The controller the kernel calls for a target.
fn controller_of(target: &Target) -> Controller {
    match target {
        Target::Closure(controller) => controller.clone(),
        Target::Method {
            called,
            method,
            this,
        } => match &method.body {
            Some(body) => {
                let body = match (this, &method.this_body) {
                    (Some(this), Some(ThisBody(function))) => {
                        let (this, function) = (this.clone(), function.clone());
                        body.with_function(move |arguments| function(&this, arguments))
                    }
                    _ => body.clone(),
                };
                body.with_name(format!("{}::{}", called.name, method.name))
            }
            // Not reached: abstract methods are not callable.
            None => Controller::new(|_| Ok(ControllerResult::Value(Value::Null))),
        },
        Target::Magic {
            called,
            magic,
            name,
        } => {
            let body = magic.body.clone();
            let called_name = name.clone();
            Controller::new(move |arguments| match &body {
                Some(body) => body.call(&[
                    Value::String(called_name.clone()),
                    Value::Array(PhpArray::list(arguments.iter().cloned())),
                ]),
                None => Ok(ControllerResult::Value(Value::Null)),
            })
            .with_name(format!("{}::{}", called.name, name.to_string_lossy()))
        }
        Target::Function(function) => function.body.clone().with_name(&function.name),
        Target::ClosureMethod(name) => {
            let name = *name;
            // What PHP checks before running it: the argument count.
            let (expected, exactly) = match name {
                "bind" => (2, false),
                "fromCallable" => (1, true),
                _ => (1, false),
            };
            Controller::new(move |arguments| {
                let given = arguments.len();
                if given < expected || (exactly && given != expected) {
                    let arity = if exactly { "exactly" } else { "at least" };
                    let plural = if expected == 1 { "" } else { "s" };
                    return Err(PhpException::new(
                        PhpClass::ArgumentCountError,
                        format!("Closure::{name}() expects {arity} {expected} argument{plural}, {given} given"),
                    )
                    .throwable());
                }
                Err(PhpException::new(
                    PhpClass::LogicException,
                    format!("Closure::{name}() is not run as a controller by this port."),
                )
                .throwable())
            })
            .with_name(format!("Closure::{name}"))
        }
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

fn concat(parts: &[&[u8]]) -> PhpString {
    PhpString::from(parts.concat())
}

fn error(class: PhpClass, message: impl Into<PhpString>) -> Throwable {
    PhpException::new(class, message).throwable()
}

/// `catch (\Error|\LogicException $e)`.
fn is_error_or_logic(e: &Throwable) -> bool {
    instance_of(e.as_ref(), "Error") || instance_of(e.as_ref(), "LogicException")
}

/// PHP's callable and the controller the kernel calls.
#[derive(Debug, Clone)]
pub struct ResolvedController {
    /// What `getController()` returns: the `_controller` value, or the
    /// array with the instantiated object, or what `createController()`
    /// made of a string.
    pub callable: Value,
    /// What calling it runs, named as `ArgumentMetadataFactory` names it.
    pub controller: Controller,
}

/// `ControllerResolver`: the controller named by the `_controller` request
/// attribute.
pub struct ControllerResolver {
    classes: Arc<ClassRegistry>,
    logger: Option<Arc<dyn LoggerInterface>>,
    allowed_types: Vec<PhpString>,
    allowed_attributes: Vec<PhpString>,
    source_file: PhpString,
    /// Set for a `ContainerControllerResolver`, whose
    /// `instantiateController()` asks the container first.
    pub(crate) container: Option<ControllerContainer>,
}

impl fmt::Debug for ControllerResolver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControllerResolver")
            .field("allowed_types", &self.allowed_types)
            .field("allowed_attributes", &self.allowed_attributes)
            .finish_non_exhaustive()
    }
}

/// What `checkController()` tests with `is_a()`.
enum ClassArg<'a> {
    Value(&'a Value),
    Name(PhpString),
}

impl ControllerResolver {
    /// `new ControllerResolver($logger)` over the declared program.
    pub fn new(classes: Arc<ClassRegistry>, logger: Option<Arc<dyn LoggerInterface>>) -> Self {
        Self {
            classes,
            logger,
            allowed_types: Vec::new(),
            allowed_attributes: vec![PhpString::from(AS_CONTROLLER)],
            source_file: PhpString::from(DEFAULT_SOURCE_FILE),
            container: None,
        }
    }

    /// With the path of `ControllerResolver.php` that a constructor's
    /// `\ArgumentCountError` names.
    #[must_use]
    pub fn with_source_file(mut self, file: impl Into<PhpString>) -> Self {
        self.source_file = file.into();
        self
    }

    /// The declared program.
    pub fn classes(&self) -> &Arc<ClassRegistry> {
        &self.classes
    }

    /// `allowControllers($types, $attributes)`.
    pub fn allow_controllers<T: Into<PhpString>, A: Into<PhpString>>(
        &mut self,
        types: impl IntoIterator<Item = T>,
        attributes: impl IntoIterator<Item = A>,
    ) {
        for t in types {
            let t = t.into();
            if !self.allowed_types.contains(&t) {
                self.allowed_types.push(t);
            }
        }
        for a in attributes {
            let a = a.into();
            if !self.allowed_attributes.contains(&a) {
                self.allowed_attributes.push(a);
            }
        }
    }

    /// `getController($request)`: `None` is PHP's `false`.
    pub fn resolve(
        &self,
        request: &RequestHandle,
    ) -> Result<Option<ResolvedController>, Throwable> {
        let controller = request.with(|r| r.attributes().get("_controller", Value::Null));
        if !controller.is_truthy() {
            if let Some(logger) = &self.logger {
                logger.warning(
                    "Unable to look for the controller as the \"_controller\" parameter is missing.",
                    Vec::new(),
                );
            }
            return Ok(None);
        }

        match &controller {
            Value::Array(array) => {
                let mut array = array.clone();
                let class = match (array.get(&Key::Int(0)), array.get(&Key::Int(1))) {
                    (Some(Value::String(class)), Some(method)) if !method.is_null() => {
                        Some(class.clone())
                    }
                    _ => None,
                };
                if let Some(class) = class {
                    match self.instantiate(class.as_bytes()) {
                        Ok(object) => array.insert(Key::Int(0), object),
                        Err(e) if is_error_or_logic(&e) => {
                            let original = Value::Array(array);
                            return match self.classes.callable(&original) {
                                Some(target) => self.check(request, original, &target).map(Some),
                                None => Err(e),
                            };
                        }
                        Err(e) => return Err(e),
                    }
                }
                let controller = Value::Array(array);
                self.callable_or_error(request, controller).map(Some)
            }
            Value::Object(_) => self
                .callable_or_error(request, controller.clone())
                .map(Some),
            _ => {
                // A scalar: the string the typed parameters coerce it to.
                let name = controller.to_php_string().unwrap_or_default();
                if let Some(function) = self.classes.function(name.as_bytes()) {
                    let target = Target::Function(function.clone());
                    return self.check(request, controller.clone(), &target).map(Some);
                }
                let callable = match self.create_controller(&name) {
                    Ok(callable) => callable,
                    Err(e) if instance_of(e.as_ref(), "InvalidArgumentException") => {
                        let message = concat(&[
                            &Self::not_callable_prefix(request)?,
                            message_of(e.as_ref()).as_bytes(),
                        ]);
                        return Err(
                            PhpException::new(PhpClass::InvalidArgumentException, message)
                                .with_previous(e)
                                .throwable(),
                        );
                    }
                    Err(e) => return Err(e),
                };
                self.callable_or_error(request, callable).map(Some)
            }
        }
    }

    /// `if (!\is_callable($c)) { throw ... } return $this->checkController(...)`.
    fn callable_or_error(
        &self,
        request: &RequestHandle,
        callable: Value,
    ) -> Result<ResolvedController, Throwable> {
        if let Some(target) = self.classes.callable(&callable) {
            return self.check(request, callable, &target);
        }
        let prefix = Self::not_callable_prefix(request)?;
        let message = self.controller_error(&callable)?;
        Err(error(
            PhpClass::InvalidArgumentException,
            concat(&[&prefix, message.as_bytes()]),
        ))
    }

    /// `The controller for URI "%s" is not callable: `.
    fn not_callable_prefix(request: &RequestHandle) -> Result<Vec<u8>, Throwable> {
        let path = request
            .with_mut(Request::get_path_info)
            .map_err(|e| Arc::new(e) as Throwable)?;
        Ok([
            b"The controller for URI \"".as_slice(),
            path.as_bytes(),
            b"\" is not callable: ",
        ]
        .concat())
    }

    /// `createController($controller)`.
    fn create_controller(&self, controller: &PhpString) -> Result<Value, Throwable> {
        let bytes = controller.as_bytes();
        let Some(at) = find(bytes, b"::") else {
            let object = self.instantiate(bytes)?;
            if self.classes.callable(&object).is_none() {
                return Err(error(
                    PhpClass::InvalidArgumentException,
                    self.controller_error(&object)?,
                ));
            }
            return Ok(object);
        };
        let (class, method) = (&bytes[..at], &bytes[at + 2..]);
        match self.instantiate(class) {
            Ok(object) => {
                let callable = Value::Array(PhpArray::list([
                    object,
                    Value::String(PhpString::from(method)),
                ]));
                if self.classes.callable(&callable).is_none() {
                    return Err(error(
                        PhpClass::InvalidArgumentException,
                        self.controller_error(&callable)?,
                    ));
                }
                Ok(callable)
            }
            Err(e) if is_error_or_logic(&e) => {
                // `(new \ReflectionMethod($class, $method))->isStatic()`
                let is_static = self
                    .classes
                    .class(class)
                    .and_then(|c| self.classes.find_method(c, method, true))
                    .is_some_and(|m| m.method.is_static);
                if !is_static {
                    return Err(e);
                }
                let callable = Value::String(concat(&[class, b"::", method]));
                // The `callable` return type.
                if self.classes.callable(&callable).is_none() {
                    return Err(error(
                        PhpClass::TypeError,
                        format!(
                            "{CONTROLLER_RESOLVER}::createController(): Return value must be of type callable, string returned"
                        ),
                    ));
                }
                Ok(callable)
            }
            Err(e) => Err(e),
        }
    }

    /// `$this->instantiateController($class)`: the container's override
    /// when there is one.
    fn instantiate(&self, name: &[u8]) -> Result<Value, Throwable> {
        match &self.container {
            Some(container) => instantiate_controller(self, container, name),
            None => self.instantiate_class(name),
        }
    }

    /// `ControllerResolver::instantiateController($class)`: `new $class()`
    /// from this class's scope.
    pub(crate) fn instantiate_class(&self, name: &[u8]) -> Result<Value, Throwable> {
        if is_closure_class(name) {
            return Err(error(
                PhpClass::Error,
                "Instantiation of class Closure is not allowed",
            ));
        }
        let Some(class) = self.classes.class(name) else {
            return Err(error(
                PhpClass::Error,
                concat(&[b"Class \"", name, b"\" not found"]),
            ));
        };
        if let Some(kind) = class.kind.not_instantiable() {
            return Err(error(
                PhpClass::Error,
                format!("Cannot instantiate {kind} {}", class.name),
            ));
        }
        if let Some(constructor) = self.classes.find_method(class, b"__construct", true) {
            let declaring = &constructor.declaring.name;
            if constructor.method.visibility != Visibility::Public {
                return Err(error(
                    PhpClass::Error,
                    format!(
                        "Call to {} {declaring}::__construct() from scope {CONTROLLER_RESOLVER}",
                        constructor.method.visibility.as_str()
                    ),
                ));
            }
            if let Some(body) = &constructor.method.body {
                let parameters = body.parameters();
                let positional = parameters.iter().filter(|p| !p.variadic).count();
                let required = parameters
                    .iter()
                    .filter(|p| !p.variadic && p.default.is_none())
                    .count();
                if required > 0 {
                    let arity = if positional == required {
                        "exactly"
                    } else {
                        "at least"
                    };
                    return Err(error(
                        PhpClass::ArgumentCountError,
                        concat(&[
                            format!(
                                "Too few arguments to function {}::__construct(), 0 passed in ",
                                php_value::value::debug_class_name(declaring)
                            )
                            .as_bytes(),
                            self.source_file.as_bytes(),
                            format!(" on line {NEW_LINE} and {arity} {required} expected")
                                .as_bytes(),
                        ]),
                    ));
                }
                body.call(&[])?;
            }
        }
        Ok(Value::Object(Arc::new(Instance {
            class: class.clone(),
        })))
    }

    /// `getControllerError($callable)`.
    fn controller_error(&self, callable: &Value) -> Result<PhpString, Throwable> {
        let exploded;
        let callable = match callable {
            Value::String(s) => match find(s.as_bytes(), b"::") {
                Some(at) => {
                    let s = s.as_bytes();
                    exploded = Value::Array(PhpArray::list([
                        Value::String(PhpString::from(&s[..at])),
                        Value::String(PhpString::from(&s[at + 2..])),
                    ]));
                    &exploded
                }
                None => {
                    return Ok(concat(&[
                        b"Function \"",
                        s.as_bytes(),
                        b"\" does not exist.",
                    ]));
                }
            },
            other => other,
        };

        if let Value::Object(object) = callable {
            let methods = self
                .classes
                .methods_without_magic(&self.classes.object_class(object));
            let mut message = format!(
                "Controller class \"{}\" cannot be called without a method name. You need to implement \"__invoke\"",
                callable.debug_type()
            )
            .into_bytes();
            if !methods.is_empty() {
                message.extend_from_slice(b" or use one of the available methods: \"");
                message.extend_from_slice(&join(&methods));
                message.push(b'"');
            }
            message.push(b'.');
            return Ok(PhpString::from(message));
        }

        let Value::Array(array) = callable else {
            return Ok(PhpString::from(format!(
                "Invalid type for controller given, expected string, array or object, got \"{}\".",
                callable.debug_type()
            )));
        };

        let set = |key| array.get(&Key::Int(key)).filter(|v| !v.is_null());
        let (Some(controller), Some(method)) = (set(0), set(1)) else {
            return Ok(PhpString::from(
                "Invalid array callable, expected [controller, method].",
            ));
        };
        if array.len() != 2 {
            return Ok(PhpString::from(
                "Invalid array callable, expected [controller, method].",
            ));
        }

        if let Value::String(class) = controller
            && !self.classes.class_exists(class.as_bytes())
        {
            return Ok(concat(&[
                b"Class \"",
                class.as_bytes(),
                b"\" does not exist.",
            ]));
        }

        let class_name = match controller {
            Value::Object(_) => PhpString::from(controller.debug_type()),
            other => other.to_php_string().unwrap_or_default(),
        };

        let (class, method, exists) = self.method_exists(controller, method)?;
        if exists {
            return Ok(concat(&[
                b"Method \"",
                method.as_bytes(),
                b"\" on class \"",
                class_name.as_bytes(),
                b"\" should be public and non-abstract.",
            ]));
        }

        if let (Value::String(_), ObjectClass::Other("")) = (controller, &class) {
            return Err(error(
                PhpClass::TypeError,
                "get_class_methods(): Argument #1 ($object_or_class) must be an object or a valid class name, string given",
            ));
        }
        let collection = self.classes.methods_without_magic(&class);

        Ok(expected_method(
            method.as_bytes(),
            class_name.as_bytes(),
            &collection,
        ))
    }

    /// `method_exists($controller, $method)`: the controller's class, the
    /// method name as a string, and the answer.
    fn method_exists<'a>(
        &self,
        controller: &'a Value,
        method: &Value,
    ) -> Result<(ObjectClass<'a>, PhpString, bool), Throwable> {
        let class = match controller {
            Value::Object(object) => self.classes.object_class(object),
            Value::String(name) => match self.classes.class(name.as_bytes()) {
                Some(class) => ObjectClass::Declared(class.clone()),
                None if is_closure_class(name.as_bytes()) => ObjectClass::Other("Closure"),
                None => ObjectClass::Other(""),
            },
            other => {
                return Err(error(
                    PhpClass::TypeError,
                    format!(
                        "method_exists(): Argument #1 ($object_or_class) must be of type object|string, {} given",
                        other.value_name()
                    ),
                ));
            }
        };
        let Some(method) = method.to_php_string_or_stringable() else {
            return Err(error(
                PhpClass::TypeError,
                format!(
                    "method_exists(): Argument #2 ($method) must be of type string, {} given",
                    method.value_name()
                ),
            ));
        };
        let exists = match &class {
            ObjectClass::Declared(c) => self
                .classes
                .find_method(c, method.as_bytes(), matches!(controller, Value::Object(_)))
                .is_some(),
            ObjectClass::Closure(_) => closure_has_method(method.as_bytes()),
            ObjectClass::Other(class) => {
                is_closure_class(class.as_bytes()) && closure_has_method(method.as_bytes())
            }
        };
        Ok((class, method, exists))
    }

    /// `checkController($request, $controller)`.
    fn check(
        &self,
        request: &RequestHandle,
        callable: Value,
        target: &Target,
    ) -> Result<ResolvedController, Throwable> {
        let resolved = ResolvedController {
            controller: controller_of(target),
            callable,
        };
        let check = request.with(|r| {
            r.attributes()
                .get("_check_controller_is_allowed", Value::Bool(false))
        });
        if !check.is_truthy() {
            return Ok(resolved);
        }

        let (class, name, attributes) = self.subject(&resolved.callable)?;

        if let Some(class) = &class
            && self
                .allowed_types
                .iter()
                .any(|t| self.is_a(class, t.as_bytes()))
        {
            return Ok(resolved);
        }

        if attributes.iter().any(|a| {
            self.allowed_attributes
                .iter()
                .any(|x| x.as_bytes() == a.as_bytes())
        }) {
            return Ok(resolved);
        }

        let name = self.rewrite_anonymous(name.as_bytes());
        let message = concat(&[
            b"Callable \"",
            &name,
            format!("()\" is not allowed as a controller. Did you miss tagging it with \"#[AsController]\" or registering its type with \"{CONTROLLER_RESOLVER}::allowControllers()\"?").as_bytes(),
        ]);
        Err(Arc::new(FoundationError::new(
            FoundationClass::BadRequest,
            message,
        )))
    }

    /// What `checkController()` tests: the class for `is_a()`, the name
    /// for the message, the attributes (the class's for an array or an
    /// object, the function's or closure's otherwise).
    fn subject<'a>(
        &self,
        callable: &'a Value,
    ) -> Result<(Option<ClassArg<'a>>, PhpString, Vec<String>), Throwable> {
        Ok(match callable {
            Value::Array(array) => {
                let class = array.get(&Key::Int(0)).unwrap_or(&Value::Null);
                let method = array
                    .get(&Key::Int(1))
                    .and_then(Value::to_php_string_or_stringable)
                    .unwrap_or_default();
                // `\is_string($class) ? $class : $class::class`
                let class_name = match class {
                    Value::Object(object) => PhpString::from(object.class()),
                    other => other.to_php_string().unwrap_or_default(),
                };
                let attributes = self.class_attributes(class);
                (
                    Some(ClassArg::Value(class)),
                    concat(&[class_name.as_bytes(), b"::", method.as_bytes()]),
                    attributes,
                )
            }
            Value::Object(object)
                if !matches!(self.classes.object_class(object), ObjectClass::Closure(_)) =>
            {
                let name = concat(&[object.class().as_bytes(), b"::__invoke"]);
                (
                    Some(ClassArg::Value(callable)),
                    name,
                    self.class_attributes(callable),
                )
            }
            Value::Object(object) => {
                let (name, called, anonymous, attributes) = match self.classes.object_class(object)
                {
                    ObjectClass::Closure(Closure::Declared(c)) => (
                        c.name.clone(),
                        c.called_class.clone(),
                        c.anonymous,
                        c.attributes.clone(),
                    ),
                    _ => (
                        crate::controller::CLOSURE_NAME.to_owned(),
                        None,
                        true,
                        Vec::new(),
                    ),
                };
                if anonymous {
                    let closure = PhpString::from(php_value::object::Closure::CLASS);
                    (Some(ClassArg::Name(closure.clone())), closure, attributes)
                } else if let Some(class) = called {
                    let name = PhpString::from(format!("{class}::{name}"));
                    (
                        Some(ClassArg::Name(PhpString::from(class))),
                        name,
                        attributes,
                    )
                } else {
                    (None, PhpString::from(name), attributes)
                }
            }
            other => {
                // `new \ReflectionFunction($controller)`: a function name.
                let s = other.to_php_string().unwrap_or_default();
                let Some(function) = self
                    .classes
                    .function(s.as_bytes())
                    .filter(|_| find(s.as_bytes(), b"::").is_none())
                else {
                    return Err(error(
                        PhpClass::ReflectionException,
                        concat(&[b"Function ", s.as_bytes(), b"() does not exist"]),
                    ));
                };
                (
                    None,
                    PhpString::from(function.name.as_str()),
                    function.attributes.clone(),
                )
            }
        })
    }

    /// `(new \ReflectionClass($class))->getAttributes()`, by name.
    fn class_attributes(&self, class: &Value) -> Vec<String> {
        let class = match class {
            Value::Object(object) => match self.classes.object_class(object) {
                ObjectClass::Declared(class) => Some(class),
                _ => None,
            },
            Value::String(name) => self.classes.class(name.as_bytes()).cloned(),
            _ => None,
        };
        class.map(|c| c.attributes.clone()).unwrap_or_default()
    }

    /// `is_a($class, $type, true)`.
    fn is_a(&self, class: &ClassArg<'_>, type_name: &[u8]) -> bool {
        let ancestry = match class {
            ClassArg::Value(Value::Object(object)) => match self.classes.object_class(object) {
                ObjectClass::Declared(class) => self.classes.ancestry(&class),
                ObjectClass::Closure(_) => vec![b"closure".to_vec()],
                ObjectClass::Other(name) => vec![name.to_ascii_lowercase().into_bytes()],
            },
            ClassArg::Value(Value::String(name)) | ClassArg::Name(name) => {
                self.name_ancestry(name.as_bytes())
            }
            ClassArg::Value(_) => Vec::new(),
        };
        let wanted = lookup_key(type_name);
        (self.classes.class(&wanted).is_some() || wanted == b"closure")
            && ancestry.contains(&wanted)
    }

    fn name_ancestry(&self, name: &[u8]) -> Vec<Vec<u8>> {
        if is_closure_class(name) {
            return vec![b"closure".to_vec()];
        }
        self.classes
            .class(name)
            .map(|c| self.classes.ancestry(c))
            .unwrap_or_default()
    }

    /// The `preg_replace_callback()` naming anonymous classes
    /// `Parent@anonymous` in the message: the pattern
    /// `[a-zA-Z_\x7f-\xff][\\a-zA-Z0-9_\x7f-\xff]*+@anonymous\x00.*?\.php(?:0x?|:[0-9]++\$)?[0-9a-fA-F]++`,
    /// matched by hand.
    fn rewrite_anonymous(&self, name: &[u8]) -> Vec<u8> {
        if find(name, b"@anonymous").is_none() {
            return name.to_vec();
        }
        let first = |c: u8| c.is_ascii_alphabetic() || c == b'_' || c >= 0x7f;
        let rest = |c: u8| first(c) || c.is_ascii_digit() || c == b'\\';
        let mut out = Vec::with_capacity(name.len());
        let mut copied = 0;
        let mut s = 0;
        while s < name.len() {
            if !first(name[s]) {
                s += 1;
                continue;
            }
            let mut k = s + 1;
            while k < name.len() && rest(name[k]) {
                k += 1;
            }
            let Some(end) = name[k..]
                .starts_with(b"@anonymous\0")
                .then(|| anonymous_tail(name, k + b"@anonymous\0".len()))
                .flatten()
            else {
                s = k.max(s + 1);
                continue;
            };
            let matched = &name[s..end];
            out.extend_from_slice(&name[copied..s]);
            match self.classes.class(matched) {
                Some(class) => {
                    let base = class
                        .parent
                        .as_deref()
                        .or(class.interfaces.first().map(String::as_str))
                        .unwrap_or("class");
                    out.extend_from_slice(base.as_bytes());
                    out.extend_from_slice(b"@anonymous");
                }
                None => out.extend_from_slice(matched),
            }
            copied = end;
            s = end;
        }
        out.extend_from_slice(&name[copied..]);
        out
    }
}

/// `Expected method "%s" on class "%s"`, with the alternatives or the
/// methods available.
fn expected_method(method: &[u8], class_name: &[u8], collection: &[PhpString]) -> PhpString {
    let mut alternatives: Vec<PhpString> = collection
        .iter()
        .filter(|item| {
            // `$lev <= \strlen($method) / 3`, in integers.
            let lev = levenshtein(method, item.as_bytes());
            3 * lev <= method.len() || find_or_empty(item.as_bytes(), method)
        })
        .cloned()
        .collect();
    // asort(): method names are identifiers, never numeric strings, so
    // PHP compares them byte by byte; the sort is stable.
    alternatives.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

    let mut message = [
        b"Expected method \"".as_slice(),
        method,
        b"\" on class \"",
        class_name,
        b"\"",
    ]
    .concat();
    if alternatives.is_empty() {
        message.extend_from_slice(b". Available methods: \"");
        message.extend_from_slice(&join(collection));
        message.extend_from_slice(b"\".");
    } else {
        message.extend_from_slice(b", did you mean \"");
        message.extend_from_slice(&join(&alternatives));
        message.extend_from_slice(b"\"?");
    }
    PhpString::from(message)
}

/// Whether `Closure` has the method (`method_exists()`): `__invoke`, its
/// private `__construct` (which `get_class_methods()` does not list from
/// outside the class) and its public methods.
fn closure_has_method(method: &[u8]) -> bool {
    method.eq_ignore_ascii_case(b"__invoke")
        || method.eq_ignore_ascii_case(b"__construct")
        || CLOSURE_METHODS
            .iter()
            .any(|(name, _)| name.as_bytes().eq_ignore_ascii_case(method))
}

/// `str_contains($haystack, $needle)`: always true for an empty needle.
fn find_or_empty(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || find(haystack, needle).is_some()
}

/// `implode('", "', $names)`.
fn join(names: &[PhpString]) -> Vec<u8> {
    let parts: Vec<&[u8]> = names.iter().map(PhpString::as_bytes).collect();
    parts.join(&b"\", \""[..])
}

/// After `@anonymous\0`: `.*?\.php(?:0x?|:[0-9]++\$)?[0-9a-fA-F]++`; the end
/// of the match.
fn anonymous_tail(name: &[u8], from: usize) -> Option<usize> {
    let hex = |at: usize| {
        let n = name.get(at..).map_or(0, |t| {
            t.iter().take_while(|c| c.is_ascii_hexdigit()).count()
        });
        (n > 0).then_some(at + n)
    };
    let mut e = from;
    loop {
        if name.get(e..).is_some_and(|t| t.starts_with(b".php")) {
            let p = e + 4;
            let rest = &name[p..];
            let digits = rest
                .get(1..)
                .map_or(0, |t| t.iter().take_while(|c| c.is_ascii_digit()).count());
            let found = rest
                .starts_with(b"0x")
                .then(|| hex(p + 2))
                .flatten()
                .or_else(|| rest.starts_with(b"0").then(|| hex(p + 1)).flatten())
                .or_else(|| {
                    (rest.starts_with(b":") && digits > 0 && rest.get(1 + digits) == Some(&b'$'))
                        .then(|| hex(p + 2 + digits))
                        .flatten()
                })
                .or_else(|| hex(p));
            if found.is_some() {
                return found;
            }
        }
        match name.get(e) {
            None | Some(b'\n') => return None,
            Some(_) => e += 1,
        }
    }
}

impl ControllerResolverInterface for ControllerResolver {
    fn get_controller(&self, request: &RequestHandle) -> Result<Option<Controller>, Throwable> {
        Ok(self.resolve(request)?.map(|r| r.controller))
    }
}
