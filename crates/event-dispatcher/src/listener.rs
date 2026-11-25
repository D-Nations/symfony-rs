//! The PHP callables EventDispatcher.php accepts, and the identity rules it
//! applies to them: `$v === $listener`, `$listener instanceof \Closure &&
//! $v == $listener`, and the lazy `[\Closure $factory, 'method']` form.
//!
//! Closure `==` was checked against PHP 8.4.27 in the oracle: two closures
//! are equal only when they are the same closure, or when both were made
//! from the same method of the same object (`$o->m(...)`, `$o(...)` and
//! `$o->__invoke(...)`). Anonymous closures from one declaration are never
//! equal, whatever they capture.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/event-dispatcher EventDispatcher.php

use std::{any::Any, fmt, sync::Arc};

use symfony_event_dispatcher_contracts::{EventObject, Throwable};

use crate::EventDispatcherInterface;

/// An object whose methods a listener names: the `$object` in PHP's
/// `[$object, 'method']`, an invokable object, or an event subscriber.
///
/// PHP objects are mutated through shared handles, so methods take `&self`;
/// keep mutable state behind a `Mutex` or atomics.
pub trait Callable: Any + Send + Sync {
    /// The PHP class name, used in error messages.
    fn class(&self) -> &'static str;

    /// Calls `method` as PHP calls `[$this, $method]($event, $eventName,
    /// $dispatcher)`. `__invoke` is the invokable form. For a method the
    /// class does not have, return [`UndefinedMethod::error`].
    fn call(
        &self,
        method: &str,
        event: &mut dyn EventObject,
        event_name: &str,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable>;
}

impl dyn Callable {
    pub fn downcast_ref<T: Callable>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }
}

/// A shared PHP object handle. Identity (`===`) is the allocation.
pub type Object = Arc<dyn Callable>;

/// The `\Closure` of a lazy listener, usually a service locator's getter.
/// It runs at most once per stored listener in PHP; with concurrent first
/// dispatches it can run once per thread, so it must be idempotent.
/// Identity (`===`) is the allocation; clones are the same closure.
#[derive(Clone)]
pub struct Factory {
    create: Arc<dyn Fn() -> Object + Send + Sync>,
    name: Option<&'static str>,
}

impl Factory {
    pub fn new(create: impl Fn() -> Object + Send + Sync + 'static) -> Self {
        Self {
            create: Arc::new(create),
            name: None,
        }
    }

    /// A factory carrying `#[\Closure(name: Foo::class)]`, as the container
    /// writes it: the class it returns, so debug tools can describe the
    /// listener without creating the service.
    pub fn named(name: &'static str, create: impl Fn() -> Object + Send + Sync + 'static) -> Self {
        Self {
            create: Arc::new(create),
            name: Some(name),
        }
    }

    pub fn name(&self) -> Option<&'static str> {
        self.name
    }

    /// `$factory()`.
    pub fn create(&self) -> Object {
        (self.create)()
    }

    fn same(&self, other: &Self) -> bool {
        same(&self.create, &other.create)
    }
}

type ClosureFn = dyn Fn(&mut dyn EventObject, &str, &dyn EventDispatcherInterface) -> Result<(), Throwable>
    + Send
    + Sync;

/// PHP's `Error` for `Call to undefined method Class::method()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndefinedMethod {
    pub class: &'static str,
    pub method: String,
}

impl UndefinedMethod {
    pub fn error(class: &'static str, method: &str) -> Throwable {
        Box::new(Self {
            class,
            method: method.to_owned(),
        })
    }
}

impl fmt::Display for UndefinedMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Call to undefined method {}::{}()",
            self.class, self.method
        )
    }
}

impl std::error::Error for UndefinedMethod {}

/// A PHP `\Closure`.
#[derive(Clone)]
pub struct Closure {
    function: Arc<ClosureFn>,
    /// Set for `$object->method(...)` and `\Closure::fromCallable([$object,
    /// 'method'])`, which compare equal (`==`) to each other.
    bound: Option<(Object, &'static str)>,
}

impl Closure {
    /// An anonymous closure (`static function ($event, $eventName,
    /// $dispatcher) {}`). It is equal only to itself.
    pub fn new(
        function: impl Fn(
            &mut dyn EventObject,
            &str,
            &dyn EventDispatcherInterface,
        ) -> Result<(), Throwable>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self {
            function: Arc::new(function),
            bound: None,
        }
    }

    /// `$object->method(...)`; `$object(...)` is `from_method(object,
    /// "__invoke")`.
    pub fn from_method(object: Object, method: &'static str) -> Self {
        let target = object.clone();
        Self {
            function: Arc::new(move |event, name, dispatcher| {
                target.call(method, event, name, dispatcher)
            }),
            bound: Some((object, method)),
        }
    }

    pub fn call(
        &self,
        event: &mut dyn EventObject,
        event_name: &str,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        (self.function)(event, event_name, dispatcher)
    }

    fn identical(&self, other: &Self) -> bool {
        same(&self.function, &other.function)
    }

    /// For `$object->method(...)`: the object's class and the method.
    #[cfg(feature = "debug")]
    pub(crate) fn bound_method(&self) -> Option<(&'static str, &'static str)> {
        self.bound
            .as_ref()
            .map(|(object, method)| (object.class(), *method))
    }

    /// PHP 8.4 `$a == $b` for closures. Method names are case-insensitive
    /// in PHP, so the bound method compares that way.
    fn equal(&self, other: &Self) -> bool {
        if self.identical(other) {
            return true;
        }
        match (&self.bound, &other.bound) {
            (Some((a, m)), Some((b, n))) => same(a, b) && m.eq_ignore_ascii_case(n),
            _ => false,
        }
    }
}

/// A listener as `addListener()` accepts it.
#[derive(Clone)]
pub enum Listener {
    /// A `\Closure`.
    Closure(Closure),
    /// `[$object, 'method']`.
    Method(Object, &'static str),
    /// An invokable object, `$object`.
    Invokable(Object),
    /// `[$factory, 'method']`, or `[$factory]` with method `__invoke`: the
    /// factory is called the first time the listener is needed, and the
    /// listener becomes `[$factory(), 'method']`. `getListeners()` never
    /// returns this form.
    Lazy(Factory, &'static str),
}

impl Listener {
    pub fn closure(
        function: impl Fn(
            &mut dyn EventObject,
            &str,
            &dyn EventDispatcherInterface,
        ) -> Result<(), Throwable>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self::Closure(Closure::new(function))
    }

    pub fn method(object: &Object, method: &'static str) -> Self {
        Self::Method(object.clone(), method)
    }

    pub fn lazy(
        factory: impl Fn() -> Object + Send + Sync + 'static,
        method: &'static str,
    ) -> Self {
        Self::Lazy(Factory::new(factory), method)
    }

    /// The object of `[$object, 'method']` or of an invokable listener.
    pub fn object(&self) -> Option<&Object> {
        match self {
            Self::Method(object, _) | Self::Invokable(object) => Some(object),
            Self::Closure(_) | Self::Lazy(..) => None,
        }
    }

    /// The method of `[$object, 'method']` (PHP `$listener[1]`).
    pub fn method_name(&self) -> Option<&'static str> {
        match self {
            Self::Method(_, method) | Self::Lazy(_, method) => Some(method),
            Self::Closure(_) | Self::Invokable(_) => None,
        }
    }

    /// PHP `$a === $b`.
    pub fn identical(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Closure(a), Self::Closure(b)) => a.identical(b),
            (Self::Method(a, m), Self::Method(b, n)) => same(a, b) && m == n,
            (Self::Invokable(a), Self::Invokable(b)) => same(a, b),
            (Self::Lazy(a, m), Self::Lazy(b, n)) => a.same(b) && m == n,
            _ => false,
        }
    }

    /// The test EventDispatcher.php uses to find a listener:
    /// `$v === $listener || ($listener instanceof \Closure && $v == $listener)`.
    pub(crate) fn matches(&self, listener: &Self) -> bool {
        if self.identical(listener) {
            return true;
        }
        match (self, listener) {
            (Self::Closure(v), Self::Closure(l)) => v.equal(l),
            _ => false,
        }
    }

    /// `$listener[0] = $listener[0](); $listener[1] ??= '__invoke';`
    pub(crate) fn resolve(&self) -> Option<Self> {
        match self {
            Self::Lazy(factory, method) => Some(Self::Method(factory.create(), method)),
            _ => None,
        }
    }

    /// `$listener($event, $eventName, $dispatcher)`.
    pub fn call(
        &self,
        event: &mut dyn EventObject,
        event_name: &str,
        dispatcher: &dyn EventDispatcherInterface,
    ) -> Result<(), Throwable> {
        match self {
            Self::Closure(closure) => closure.call(event, event_name, dispatcher),
            Self::Method(object, method) => object.call(method, event, event_name, dispatcher),
            Self::Invokable(object) => object.call("__invoke", event, event_name, dispatcher),
            Self::Lazy(factory, method) => {
                factory.create().call(method, event, event_name, dispatcher)
            }
        }
    }
}

/// `assertSame()` on callables is `===`.
impl PartialEq for Listener {
    fn eq(&self, other: &Self) -> bool {
        self.identical(other)
    }
}

impl From<Closure> for Listener {
    fn from(closure: Closure) -> Self {
        Self::Closure(closure)
    }
}

impl fmt::Debug for Listener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closure(Closure {
                bound: Some((object, method)),
                ..
            }) => write!(f, "{}::{method}(...)", object.class()),
            Self::Closure(closure) => write!(
                f,
                "closure@{:p}",
                Arc::as_ptr(&closure.function).cast::<()>()
            ),
            Self::Method(object, method) => write!(f, "[{}, {method:?}]", object.class()),
            Self::Invokable(object) => write!(f, "{}", object.class()),
            Self::Lazy(_, method) => write!(f, "[lazy, {method:?}]"),
        }
    }
}

/// Identity of two handles, ignoring vtables (`===` on PHP objects).
fn same<T: ?Sized>(a: &Arc<T>, b: &Arc<T>) -> bool {
    std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
}
