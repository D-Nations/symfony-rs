//! Port of Symfony\Component\DependencyInjection\ServicesResetter.php and
//! ServicesResetterInterface.php.
//!
//! The resettable services are a [`RewindableGenerator`] of `id =>
//! service` (what the compiled container passes; an `\ArrayIterator` is
//! [`RewindableGenerator::from_pairs`]). `$resetMap` is a [`ResetMap`]:
//! PHP's `\WeakMap` of non-shared instances, holding weak references so
//! a dropped instance leaves it. Methods are called through
//! [`crate::service_object::call_method`].
//!
//! Upstream: symfony/dependency-injection ServicesResetter.php ServicesResetterInterface.php

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use php_value::value::PhpObject;
use php_value::{Object, PhpArray, Value};

use crate::argument::RewindableGenerator;
use crate::exception::{Error, Result};
use crate::service_object::{call_method, is_uninitialized_lazy_object, method_exists};

/// `Symfony\Component\DependencyInjection\ServicesResetter`.
pub const SERVICES_RESETTER_CLASS: &str =
    "Symfony\\Component\\DependencyInjection\\ServicesResetter";

/// `Symfony\Component\DependencyInjection\ServicesResetterInterface`.
pub const SERVICES_RESETTER_INTERFACE: &str =
    "Symfony\\Component\\DependencyInjection\\ServicesResetterInterface";

/// `ServicesResetterInterface` (a `ResetInterface`).
pub trait ServicesResetterInterface: Send + Sync {
    /// `reset()`.
    fn reset(&self) -> Result<()>;
}

type Entries = Vec<(Weak<dyn PhpObject>, Vec<String>)>;

/// A `\WeakMap` of objects to their reset methods.
#[derive(Clone, Default)]
pub struct ResetMap(Arc<Mutex<Entries>>);

impl fmt::Debug for ResetMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WeakMap({})", self.count())
    }
}

impl ResetMap {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Entries> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `$map[$object] = $methods`.
    pub fn insert(&self, object: &Object, methods: Vec<String>) {
        let weak = Arc::downgrade(object);
        let mut entries = self.lock();
        entries.retain(|(w, _)| w.strong_count() > 0);
        match entries.iter_mut().find(|(w, _)| Weak::ptr_eq(w, &weak)) {
            Some(entry) => entry.1 = methods,
            None => entries.push((weak, methods)),
        }
    }

    /// `count($map)`: the live objects.
    pub fn count(&self) -> usize {
        self.lock()
            .iter()
            .filter(|(w, _)| w.strong_count() > 0)
            .count()
    }

    /// The live objects with their methods.
    pub fn entries(&self) -> Vec<(Object, Vec<String>)> {
        self.lock()
            .iter()
            .filter_map(|(w, m)| w.upgrade().map(|o| (o, m.clone())))
            .collect()
    }

    /// Whether both handles are the same map (`===`).
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// `ServicesResetter`.
pub struct ServicesResetter {
    resettable_services: RewindableGenerator,
    reset_methods: Vec<(String, Vec<String>)>,
    reset_map: Option<ResetMap>,
}

impl fmt::Debug for ServicesResetter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ServicesResetter#{:p}", std::ptr::from_ref(self))
    }
}

impl PhpObject for ServicesResetter {
    fn class(&self) -> &str {
        SERVICES_RESETTER_CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// A PHP array key as `$id`.
fn key_string(key: &Value) -> String {
    match key {
        Value::Int(i) => i.to_string(),
        Value::String(s) => s.to_string_lossy().into_owned(),
        other => other.debug_type(),
    }
}

impl ServicesResetter {
    /// `new ServicesResetter($resettableServices, $resetMethods,
    /// $resetMap)`: `reset_methods` is id => methods (a string is a
    /// one-method list, as `(array)` makes it).
    pub fn new(
        resettable_services: RewindableGenerator,
        reset_methods: Vec<(String, Vec<String>)>,
        reset_map: Option<ResetMap>,
    ) -> Self {
        Self {
            resettable_services,
            reset_methods,
            reset_map,
        }
    }

    /// The resetter as an object value.
    pub fn into_value(self) -> Value {
        Value::Object(Arc::new(self))
    }

    /// `reset()`: every reset method runs; the first error is returned
    /// after all ran.
    pub fn reset(&self) -> Result<()> {
        let mut throwable: Option<Error> = None;
        for item in self.resettable_services.get_iterator() {
            let (id, service) = item?;
            let id = key_string(&id);
            let methods = self
                .reset_methods
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, m)| m.clone())
                .unwrap_or_default();
            reset_instance(&service, &methods, &mut throwable);
        }

        if let Some(reset_map) = &self.reset_map {
            for (service, methods) in reset_map.entries() {
                reset_instance(&Value::Object(service), &methods, &mut throwable);
            }
        }

        match throwable {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

impl ServicesResetterInterface for ServicesResetter {
    fn reset(&self) -> Result<()> {
        ServicesResetter::reset(self)
    }
}

fn reset_instance(service: &Value, methods: &[String], throwable: &mut Option<Error>) {
    if is_uninitialized_lazy_object(service) {
        return;
    }

    for reset_method in methods {
        let mut method = reset_method.as_str();
        if let Some(optional) = method.strip_prefix('?') {
            method = optional;
            if !method_exists(service, method) {
                continue;
            }
        }
        if let Err(e) = call_method(service, method, &[]) {
            // failing to reset one service should not prevent resetting
            // the remaining ones
            throwable.get_or_insert(e);
        }
    }
}
