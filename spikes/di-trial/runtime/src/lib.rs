//! What code emitted by `symfony-di-emitter` calls: the cells that hold
//! shared services, the re-entrant "configure once" guard, and placeholder
//! instances that record how each service was built.
//!
//! **Placeholders, not ports.** Every class of the demo's container is a
//! generated newtype over [`Instance`]. An instance keeps its constructor
//! arguments, factory, method calls and properties as [`Arg`] values, so
//! tests can check the wiring; it runs none of the class's code. The real
//! M3 container calls each ported class's own constructor instead.
//!
//! Upstream: none

#![forbid(unsafe_code)]

use std::{
    any::Any,
    fmt,
    sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError},
    thread::{self, ThreadId},
};

pub use php_value::{Key, PhpString, Value};

/// A built service, type-erased (what `$container->get()` returns).
pub type Service = Arc<dyn ServiceObject>;

/// A `service_closure` or iterator element: builds or fetches a service
/// when called. `None` is a reference that resolved to nothing
/// (`on-invalid="ignore_uninitialized"` to a service not yet built, or an
/// owner that has been dropped).
pub type ServiceClosure = Arc<dyn Fn() -> Option<Service> + Send + Sync>;

/// One argument, property value or method call argument.
#[derive(Clone)]
pub enum Arg {
    /// A scalar, as `XmlUtils::phpize()` reads it.
    Value(Value),
    /// A PHP array whose values may hold services (`type="collection"`).
    Array(Vec<(Option<Key>, Arg)>),
    /// A service built before this one (a constructor argument).
    Service(Service),
    /// A service reference that resolved to `null`
    /// (`on-invalid="null"`, or `ignore_uninitialized` to a service not
    /// built yet).
    Null,
    /// `type="service_closure"`: `fn () => $container->get('id')`.
    Closure(ServiceClosure),
    /// `type="iterator"` (a resolved tagged iterator): services are
    /// `Closure` elements, built when the iterator is walked; other
    /// elements are plain values.
    Iterator(Vec<(Option<Key>, Arg)>),
    /// A reference to a `lazy="true"` service: PHP injects a lazy ghost
    /// that is built on first use.
    Lazy(ServiceClosure),
    /// `%env(...)%`: evaluated at run time by the env var processors (not
    /// part of the trial).
    Env(&'static str),
    /// A string with `%env(...)%` parts inside it.
    Concat(Vec<Arg>),
    /// `type="constant"` (an enum case or class constant).
    Constant(&'static str),
}

impl fmt::Debug for Arg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Value(v) => write!(f, "{v:?}"),
            Self::Array(items) => f.debug_list().entries(items).finish(),
            Self::Service(s) => write!(f, "Service({})", describe(s)),
            Self::Null => f.write_str("Null"),
            Self::Closure(_) => f.write_str("Closure"),
            Self::Iterator(items) => write!(f, "Iterator({})", items.len()),
            Self::Lazy(_) => f.write_str("Lazy"),
            Self::Env(e) => write!(f, "Env({e})"),
            Self::Concat(parts) => f.debug_tuple("Concat").field(parts).finish(),
            Self::Constant(c) => write!(f, "Constant({c})"),
        }
    }
}

/// The service id of a placeholder, or `service_container`.
pub fn describe(service: &Service) -> &'static str {
    service.instance().map_or("service_container", |i| i.id)
}

/// How PHP obtains the object.
#[derive(Clone, Debug)]
pub enum Factory {
    /// `new Class(...$arguments)`.
    Constructor,
    /// `function(...$arguments)`, a PHP function such as `current`.
    Function(&'static str),
    /// `Class::method(...$arguments)`.
    Static {
        class: &'static str,
        method: &'static str,
    },
    /// `$service->method(...$arguments)`.
    Service { service: Arg, method: &'static str },
}

/// A recorded `$service->method(...$arguments)` (a `<call>` or the
/// `<configurator>`).
#[derive(Clone, Debug)]
pub struct Call {
    pub method: &'static str,
    /// A wither (`returns-clone`): PHP keeps the object the call returns.
    pub returns_clone: bool,
    pub arguments: Vec<Arg>,
    /// `Some` for a configurator: the service or class it runs on.
    pub on: Option<Arg>,
}

/// A placeholder object: what PHP would have built, recorded.
#[derive(Debug)]
pub struct Instance {
    pub id: &'static str,
    pub class: &'static str,
    pub factory: Factory,
    pub arguments: Vec<Arg>,
    calls: Mutex<Vec<Call>>,
    properties: Mutex<Vec<(&'static str, Arg)>>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Instance {
    pub fn new(
        id: &'static str,
        class: &'static str,
        factory: Factory,
        arguments: Vec<Arg>,
    ) -> Self {
        Self {
            id,
            class,
            factory,
            arguments,
            calls: Mutex::new(Vec::new()),
            properties: Mutex::new(Vec::new()),
        }
    }

    /// `$service->method(...$arguments)`.
    pub fn call(&self, method: &'static str, arguments: Vec<Arg>) {
        lock(&self.calls).push(Call {
            method,
            returns_clone: false,
            arguments,
            on: None,
        });
    }

    /// `$service = $service->method(...$arguments)`. A placeholder records
    /// it on itself; a ported class returns a new value.
    pub fn call_wither(&self, method: &'static str, arguments: Vec<Arg>) {
        lock(&self.calls).push(Call {
            method,
            returns_clone: true,
            arguments,
            on: None,
        });
    }

    /// The configurator: `$configurator->method($service)`.
    pub fn configure(&self, on: Arg, method: &'static str) {
        lock(&self.calls).push(Call {
            method,
            returns_clone: false,
            arguments: Vec::new(),
            on: Some(on),
        });
    }

    /// `$service->name = $value`.
    pub fn set_property(&self, name: &'static str, value: Arg) {
        lock(&self.properties).push((name, value));
    }

    pub fn calls(&self) -> Vec<Call> {
        lock(&self.calls).clone()
    }

    pub fn properties(&self) -> Vec<(&'static str, Arg)> {
        lock(&self.properties).clone()
    }
}

/// Implemented by every generated placeholder type, and by the generated
/// request scope (the `service_container` service).
pub trait ServiceObject: Any + Send + Sync {
    /// The recorded placeholder; `None` for the container itself.
    fn instance(&self) -> Option<&Instance>;

    /// For downcasting to the generated type.
    fn into_any(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}

/// Declares a placeholder type for one PHP class.
#[macro_export]
macro_rules! placeholder {
    ($name:ident) => {
        #[derive(Debug)]
        pub struct $name(pub $crate::Instance);

        impl $crate::ServiceObject for $name {
            fn instance(&self) -> Option<&$crate::Instance> {
                Some(&self.0)
            }

            fn into_any(
                self: ::std::sync::Arc<Self>,
            ) -> ::std::sync::Arc<dyn ::std::any::Any + Send + Sync> {
                self
            }
        }
    };
}

/// The slot of one shared service (`$this->services[$id]`).
///
/// The lock is not held while the service is built, so a service that is
/// reached again while it is being built (a circular reference through a
/// method call, which PHP allows) does not deadlock: as in the PHP
/// container that `PhpDumper` writes, the instance stored first wins.
pub struct ServiceCell<T> {
    slot: Mutex<Option<Arc<T>>>,
}

impl<T> Default for ServiceCell<T> {
    fn default() -> Self {
        Self {
            slot: Mutex::new(None),
        }
    }
}

impl<T> ServiceCell<T> {
    pub fn get(&self) -> Option<Arc<T>> {
        lock(&self.slot).clone()
    }

    #[expect(clippy::similar_names, reason = "names as in the PHP source")]
    pub fn get_or_build(&self, build: impl FnOnce() -> Arc<T>) -> Arc<T> {
        if let Some(s) = self.get() {
            return s;
        }
        let built = build();
        lock(&self.slot).get_or_insert(built).clone()
    }
}

#[derive(Default)]
enum ConfigureState {
    #[default]
    NotStarted,
    Running(ThreadId),
    Done,
}

/// Runs a shared service's method calls and configurator once, after the
/// service is stored. The thread that is running them gets the service
/// back unconfigured if it reaches it again (PHP's order: store, then
/// call); other threads wait until the calls are done.
#[derive(Default)]
pub struct Configure {
    state: Mutex<ConfigureState>,
    done: Condvar,
}

impl Configure {
    pub fn run(&self, calls: impl FnOnce()) {
        let me = thread::current().id();
        let mut state = lock(&self.state);
        loop {
            match *state {
                ConfigureState::Done => return,
                ConfigureState::Running(t) if t == me => return,
                ConfigureState::Running(_) => {
                    state = self
                        .done
                        .wait(state)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                ConfigureState::NotStarted => break,
            }
        }
        *state = ConfigureState::Running(me);
        drop(state);
        calls();
        *lock(&self.state) = ConfigureState::Done;
        self.done.notify_all();
    }
}

/// Erases a typed service.
pub fn erase<T: ServiceObject>(service: Arc<T>) -> Service {
    service
}
