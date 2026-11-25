//! Service objects whose methods the run-time classes call by name.
//!
//! PHP calls methods on services: `Container::reset()` calls `reset()` on
//! every `ResetInterface` service, and `ServicesResetter` calls the
//! configured reset methods (`$service->$resetMethod()`, with
//! `method_exists()` for a `?`-prefixed one). A php-value object has no
//! methods, so a service that needs them is a [`ServiceObject`]: a class
//! name, the interfaces it implements, a method table and its public
//! properties. A lazy proxy ([`ServiceObject::lazy_proxy`]) initializes
//! its real instance on its first method call, as a ghost or virtual
//! proxy does; until then it is an uninitialized lazy object.
//!
//! [`call_method`], [`method_exists`], [`is_resettable`] and
//! [`is_uninitialized_lazy_object`] answer for any value: a
//! [`ServiceObject`], a [`Container`] or a [`ServicesResetter`] (both
//! `ResetInterface`s); any other object has no callable method.
//!
//! Upstream: none

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::{PhpArray, Value};

use crate::container::Container;
use crate::exception::{Error, ErrorKind, Result};
use crate::object::downcast;
use crate::services_resetter::ServicesResetter;

/// `Symfony\Contracts\Service\ResetInterface`.
pub const RESET_INTERFACE: &str = "Symfony\\Contracts\\Service\\ResetInterface";

/// A method: called with the object and the arguments.
pub type Method = Arc<dyn Fn(&ServiceObject, &[Value]) -> Result<Value> + Send + Sync>;

type Initializer = Box<dyn FnOnce() -> Result<ServiceObject> + Send>;

enum Lazy {
    Uninitialized(Initializer),
    Initializing,
    Initialized(Arc<ServiceObject>),
}

/// An object with a class, interfaces, methods and public properties.
pub struct ServiceObject {
    class: String,
    interfaces: Vec<String>,
    methods: Vec<(String, Option<Method>)>,
    properties: Mutex<PhpArray>,
    lazy: Option<Mutex<Lazy>>,
}

impl fmt::Debug for ServiceObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{:p}", self.class, std::ptr::from_ref(self))
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// PHP's `Error` for a call to a method the class does not have.
pub fn undefined_method(class: &str, method: &str) -> Error {
    Error::new(
        ErrorKind::Other("Error".to_owned()),
        format!("Call to undefined method {class}::{method}()"),
    )
}

impl ServiceObject {
    /// An object of `class` with no interfaces, methods or properties.
    pub fn new(class: impl Into<String>) -> Self {
        Self {
            class: class.into(),
            interfaces: Vec::new(),
            methods: Vec::new(),
            properties: Mutex::new(PhpArray::new()),
            lazy: None,
        }
    }

    /// A lazy proxy of `class`: `methods` are the class's method names,
    /// forwarded to the instance `initializer` creates on the first call.
    pub fn lazy_proxy(
        class: impl Into<String>,
        interfaces: &[&str],
        methods: &[&str],
        initializer: impl FnOnce() -> Result<ServiceObject> + Send + 'static,
    ) -> Self {
        Self {
            class: class.into(),
            interfaces: interfaces.iter().map(|&i| i.to_owned()).collect(),
            methods: methods.iter().map(|&m| (m.to_owned(), None)).collect(),
            properties: Mutex::new(PhpArray::new()),
            lazy: Some(Mutex::new(Lazy::Uninitialized(Box::new(initializer)))),
        }
    }

    /// Declares an implemented interface (or parent class).
    #[must_use]
    pub fn implements(mut self, interface: impl Into<String>) -> Self {
        self.interfaces.push(interface.into());
        self
    }

    /// Adds a method.
    #[must_use]
    pub fn method(
        mut self,
        name: impl Into<String>,
        method: impl Fn(&ServiceObject, &[Value]) -> Result<Value> + Send + Sync + 'static,
    ) -> Self {
        self.methods.push((name.into(), Some(Arc::new(method))));
        self
    }

    /// Sets a public property.
    #[must_use]
    pub fn with_property(self, name: &str, value: Value) -> Self {
        self.set_property(name, value);
        self
    }

    /// The object as a PHP value.
    pub fn into_value(self) -> Value {
        Value::Object(Arc::new(self))
    }

    pub fn class(&self) -> &str {
        &self.class
    }

    /// `$object instanceof $class`.
    pub fn instance_of(&self, class: &str) -> bool {
        let class = class.trim_start_matches('\\');
        self.class == class || self.interfaces.iter().any(|i| i == class)
    }

    /// `$object->$name`.
    pub fn property(&self, name: &str) -> Option<Value> {
        lock(&self.properties)
            .get(&php_value::Key::from_bytes(name.as_bytes()))
            .cloned()
    }

    /// `$object->$name = $value`.
    pub fn set_property(&self, name: &str, value: Value) {
        lock(&self.properties).insert(php_value::Key::from_bytes(name.as_bytes()), value);
    }

    /// `method_exists($object, $name)` (case-insensitive, as PHP).
    pub fn method_exists(&self, name: &str) -> bool {
        self.methods
            .iter()
            .any(|(m, _)| m.eq_ignore_ascii_case(name))
    }

    /// Whether this is a lazy proxy whose instance is not created yet.
    pub fn is_uninitialized_lazy_object(&self) -> bool {
        self.lazy
            .as_ref()
            .is_some_and(|l| !matches!(&*lock(l), Lazy::Initialized(_)))
    }

    /// The real instance of a lazy proxy, created on first use.
    fn instance(&self, lazy: &Mutex<Lazy>) -> Result<Arc<ServiceObject>> {
        let state = std::mem::replace(&mut *lock(lazy), Lazy::Initializing);
        match state {
            Lazy::Initialized(real) => {
                *lock(lazy) = Lazy::Initialized(real.clone());
                Ok(real)
            }
            Lazy::Uninitialized(initializer) => match initializer() {
                Ok(real) => {
                    let real = Arc::new(real);
                    *lock(lazy) = Lazy::Initialized(real.clone());
                    Ok(real)
                }
                Err(e) => {
                    *lock(lazy) = Lazy::Initializing;
                    Err(e)
                }
            },
            Lazy::Initializing => Err(Error::new(
                ErrorKind::Other("Error".to_owned()),
                format!(
                    "Lazy object of class {} is already being initialized",
                    self.class
                ),
            )),
        }
    }

    /// `$object->$name(...$args)`.
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value> {
        if let Some(lazy) = &self.lazy {
            if !self.method_exists(name) {
                return Err(undefined_method(&self.class, name));
            }
            return self.instance(lazy)?.call(name, args);
        }
        match self
            .methods
            .iter()
            .find(|(m, _)| m.eq_ignore_ascii_case(name))
        {
            Some((_, Some(method))) => method(self, args),
            _ => Err(undefined_method(&self.class, name)),
        }
    }
}

impl php_value::value::PhpObject for ServiceObject {
    fn class(&self) -> &str {
        &self.class
    }

    fn properties(&self) -> PhpArray {
        lock(&self.properties).clone()
    }
}

/// `$value->$name(...$args)` on a service.
pub fn call_method(value: &Value, name: &str, args: &[Value]) -> Result<Value> {
    let Value::Object(object) = value else {
        return Err(Error::new(
            ErrorKind::Other("Error".to_owned()),
            format!(
                "Call to a member function {name}() on {}",
                value.debug_type()
            ),
        ));
    };
    if let Some(service) = downcast::<ServiceObject>(value) {
        return service.call(name, args);
    }
    if name.eq_ignore_ascii_case("reset") {
        if let Some(container) = downcast::<Container>(value) {
            container.reset();
            return Ok(Value::Null);
        }
        if let Some(resetter) = downcast::<ServicesResetter>(value) {
            resetter.reset()?;
            return Ok(Value::Null);
        }
    }
    Err(undefined_method(object.class(), name))
}

/// `method_exists($value, $name)`.
pub fn method_exists(value: &Value, name: &str) -> bool {
    if let Some(service) = downcast::<ServiceObject>(value) {
        return service.method_exists(name);
    }
    name.eq_ignore_ascii_case("reset")
        && (downcast::<Container>(value).is_some() || downcast::<ServicesResetter>(value).is_some())
}

/// `$value instanceof ResetInterface`.
pub fn is_resettable(value: &Value) -> bool {
    if let Some(service) = downcast::<ServiceObject>(value) {
        return service.instance_of(RESET_INTERFACE);
    }
    downcast::<Container>(value).is_some() || downcast::<ServicesResetter>(value).is_some()
}

/// `isUninitializedLazyObject()` of `Container` and `ServicesResetter`.
pub fn is_uninitialized_lazy_object(value: &Value) -> bool {
    downcast::<ServiceObject>(value).is_some_and(ServiceObject::is_uninitialized_lazy_object)
}
