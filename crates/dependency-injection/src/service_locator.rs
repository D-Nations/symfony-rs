//! Port of Symfony\Component\DependencyInjection\ServiceLocator.php and
//! Argument\ServiceLocator.php, with the `ServiceLocatorTrait` of
//! symfony/service-contracts they use.
//!
//! A locator maps ids to factories ([`LocatorEntry`]) called with the
//! locator; it does not memoize. Both classes are one Rust type: the
//! `Argument\ServiceLocator` of a compiled container
//! ([`ServiceLocator::argument`]) holds a service map (id => the
//! arguments of its factory closure) instead of callables. A locator is
//! an object value ([`ServiceLocator::into_value`]).
//!
//! PHP reads the calling object from `debug_backtrace()` for the
//! not-found message. Rust has no such frame: [`ServiceLocator::get`]
//! answers as a call from no object (a function or a closure), and
//! [`ServiceLocator::get_from`] takes the caller's class ([`Caller`]).
//! The return types `getProvidedServices()` reflects from closures are
//! declared on the entry ([`LocatorEntry::returns`]).
//!
//! Upstream: symfony/dependency-injection ServiceLocator.php Argument/ServiceLocator.php
//! Upstream: symfony/service-contracts ServiceLocatorTrait.php

use std::fmt;
use std::sync::{Arc, OnceLock};

use php_value::{PhpArray, Value};

use crate::container::{Container, is_falsy_id};
use crate::container_interface::InvalidBehavior;
use crate::exception::{Error, ErrorKind, Result};
use crate::ordered_map::{OrderedMap, ThreadStack};

/// `Symfony\Component\DependencyInjection\ServiceLocator`.
pub const SERVICE_LOCATOR_CLASS: &str = "Symfony\\Component\\DependencyInjection\\ServiceLocator";

/// `Symfony\Component\DependencyInjection\Argument\ServiceLocator`.
pub const ARGUMENT_SERVICE_LOCATOR_CLASS: &str =
    "Symfony\\Component\\DependencyInjection\\Argument\\ServiceLocator";

/// A locator factory: called with the locator.
pub type LocatorFactory = Arc<dyn Fn(&ServiceLocator) -> Result<Value> + Send + Sync>;

/// The factory closure of an `Argument\ServiceLocator`, called with the
/// arguments of the service map.
pub type ArgumentFactory = Arc<dyn Fn(&[Value]) -> Result<Value> + Send + Sync>;

/// A callable of the locator, with its declared return type.
#[derive(Clone)]
pub struct LocatorEntry {
    factory: LocatorFactory,
    return_type: Option<(String, bool)>,
}

impl LocatorEntry {
    pub fn new(factory: impl Fn(&ServiceLocator) -> Result<Value> + Send + Sync + 'static) -> Self {
        Self {
            factory: Arc::new(factory),
            return_type: None,
        }
    }

    /// Declares the return type (`: string`, or `: ?string` with
    /// `allows_null`).
    #[must_use]
    pub fn returns(mut self, name: impl Into<String>, allows_null: bool) -> Self {
        self.return_type = Some((name.into(), allows_null));
        self
    }

    /// `getProvidedServices()`'s type for the callable.
    fn provided_type(&self) -> String {
        match &self.return_type {
            None => "?".to_owned(),
            Some((name, allows_null)) => {
                format!("{}{name}", if *allows_null { "?" } else { "" })
            }
        }
    }
}

#[derive(Clone)]
enum Entry {
    Callable(LocatorEntry),
    /// An `Argument\ServiceLocator` service map entry.
    Arguments(Vec<Value>),
}

#[derive(Clone)]
struct ArgumentData {
    factory: ArgumentFactory,
    service_types: Option<Vec<(String, String)>>,
}

/// The object that calls `get()`: what `debug_backtrace()` gives
/// upstream's not-found message.
#[derive(Debug, Clone)]
pub struct Caller {
    /// Its class.
    pub class: String,
    /// Whether the class implements `ServiceSubscriberInterface`.
    pub service_subscriber: bool,
}

impl Caller {
    pub fn new(class: impl Into<String>, service_subscriber: bool) -> Self {
        Self {
            class: class.into(),
            service_subscriber,
        }
    }
}

/// `ServiceLocator` (and `Argument\ServiceLocator`).
pub struct ServiceLocator {
    factories: OrderedMap<Entry>,
    loading: ThreadStack,
    provided_types: OnceLock<Vec<(String, String)>>,
    external_id: Option<String>,
    container: Option<Arc<Container>>,
    argument: Option<ArgumentData>,
}

impl fmt::Debug for ServiceLocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{:p}", self.class_name(), std::ptr::from_ref(self))
    }
}

impl php_value::value::PhpObject for ServiceLocator {
    fn class(&self) -> &str {
        self.class_name()
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// `preg_replace('/service "\.service_locator\.[^"]++"/', $what, $message)`.
fn replace_locator_ids(message: &str, what: &str) -> String {
    const NEEDLE: &str = "service \".service_locator.";
    let mut out = String::with_capacity(message.len());
    let mut rest = message;
    while let Some(i) = rest.find(NEEDLE) {
        let after = &rest[i + NEEDLE.len()..];
        match after.find('"') {
            Some(end) if end > 0 => {
                out.push_str(&rest[..i]);
                out.push_str(what);
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&rest[..=i]);
                rest = &rest[i + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `preg_replace('/([^\\\\]++\\\\)++/', '', $class)`: the short name.
fn short_class(class: &str) -> &str {
    class.rsplit('\\').next().unwrap_or(class)
}

impl ServiceLocator {
    /// `new ServiceLocator($factories)`.
    pub fn new(factories: Vec<(String, LocatorEntry)>) -> Self {
        Self {
            factories: factories
                .into_iter()
                .map(|(id, f)| (id, Entry::Callable(f)))
                .collect(),
            loading: ThreadStack::default(),
            provided_types: OnceLock::new(),
            external_id: None,
            container: None,
            argument: None,
        }
    }

    /// `new Argument\ServiceLocator($factory, $serviceMap, $serviceTypes)`.
    pub fn argument(
        factory: ArgumentFactory,
        service_map: Vec<(String, Vec<Value>)>,
        service_types: Option<Vec<(String, String)>>,
    ) -> Self {
        Self {
            factories: service_map
                .into_iter()
                .map(|(id, args)| (id, Entry::Arguments(args)))
                .collect(),
            loading: ThreadStack::default(),
            provided_types: OnceLock::new(),
            external_id: None,
            container: None,
            argument: Some(ArgumentData {
                factory,
                service_types,
            }),
        }
    }

    /// The locator as an object value.
    pub fn into_value(self) -> Value {
        Value::Object(Arc::new(self))
    }

    fn class_name(&self) -> &'static str {
        if self.argument.is_some() {
            ARGUMENT_SERVICE_LOCATOR_CLASS
        } else {
            SERVICE_LOCATOR_CLASS
        }
    }

    /// `has($id)`.
    pub fn has(&self, id: &str) -> bool {
        self.factories.contains_key(id)
    }

    /// `get($id)`, called from no object.
    pub fn get(&self, id: &str) -> Result<Value> {
        self.get_from(id, None)
    }

    /// `get($id)`, called from an object of the caller's class.
    pub fn get_from(&self, id: &str, caller: Option<&Caller>) -> Result<Value> {
        if let Some(argument) = &self.argument {
            match self.factories.get(id) {
                Some(Entry::Arguments(args)) if args.len() == 1 => return Ok(args[0].clone()),
                Some(Entry::Arguments(args)) if args.len() > 1 => {
                    return (argument.factory)(args);
                }
                _ => {
                    let caller = Caller::new(ARGUMENT_SERVICE_LOCATOR_CLASS, false);
                    return self.base_get(id, Some(&caller));
                }
            }
        }
        self.base_get(id, caller)
    }

    /// `ServiceLocator::get()`.
    fn base_get(&self, id: &str, caller: Option<&Caller>) -> Result<Value> {
        let Some(external_id) = self.external_id.as_deref().filter(|e| !is_falsy_id(e)) else {
            return self.do_get(id, caller);
        };
        match self.do_get(id, caller) {
            Err(mut e) if e.is(&ErrorKind::Runtime) => {
                let what = format!("service \"{id}\" required by \"{external_id}\"");
                let original = e.message();
                let mut message = replace_locator_ids(&original, &what);
                if message == original {
                    message = format!("Cannot resolve {what}: {message}");
                }
                e.set_message(message);
                Err(e)
            }
            other => other,
        }
    }

    /// `ServiceLocatorTrait::get()`.
    fn do_get(&self, id: &str, caller: Option<&Caller>) -> Result<Value> {
        let Some(entry) = self.factories.get(id) else {
            return Err(self.create_not_found_exception(id, caller)?);
        };

        if self.loading.contains(id) {
            let loading = self.loading.current();
            let start = loading.iter().position(|i| i == id).unwrap_or(0);
            let mut path = loading[start..].to_vec();
            path.push(id.to_owned());
            return Err(Error::service_circular_reference(id, path));
        }

        let entry = entry.clone();
        self.loading.push(id);
        let result = match &entry {
            Entry::Callable(callable) => (callable.factory)(self),
            Entry::Arguments(_) => Err(Error::new(
                ErrorKind::Other("Error".to_owned()),
                "Array callback must have exactly two elements",
            )),
        };
        self.loading.remove(id);
        result
    }

    /// `__invoke($id)`.
    pub fn invoke(&self, id: &str) -> Result<Option<Value>> {
        if self.factories.contains_key(id) {
            let caller = Caller::new(self.class_name(), false);
            self.get_from(id, Some(&caller)).map(Some)
        } else {
            Ok(None)
        }
    }

    /// `withContext($externalId, $container)`: a copy that names its
    /// owner in its errors.
    #[must_use]
    pub fn with_context(&self, external_id: &str, container: Arc<Container>) -> Self {
        Self {
            factories: self.factories.clone(),
            loading: ThreadStack::default(),
            provided_types: self.provided_types.clone(),
            external_id: Some(external_id.to_owned()),
            container: Some(container),
            argument: self.argument.clone(),
        }
    }

    /// `count()`.
    pub fn count(&self) -> usize {
        self.get_provided_services().len()
    }

    /// `getIterator()`: the services, created as the iteration reaches
    /// them.
    pub fn iter(&self) -> impl Iterator<Item = Result<(String, Value)>> + '_ {
        let caller = Caller::new(self.class_name(), false);
        self.get_provided_services()
            .into_iter()
            .map(move |(id, _)| self.get_from(&id, Some(&caller)).map(|v| (id, v)))
    }

    /// `getProvidedServices()`: id => type (`?` when unknown).
    pub fn get_provided_services(&self) -> Vec<(String, String)> {
        self.provided_types
            .get_or_init(|| {
                if let Some(argument) = &self.argument {
                    if let Some(types) = &argument.service_types {
                        return types.clone();
                    }
                    return self
                        .factories
                        .keys()
                        .map(|id| (id.to_owned(), "?".to_owned()))
                        .collect();
                }
                self.factories
                    .iter()
                    .map(|(id, entry)| {
                        let ty = match entry {
                            Entry::Callable(callable) => callable.provided_type(),
                            Entry::Arguments(_) => "?".to_owned(),
                        };
                        (id.to_owned(), ty)
                    })
                    .collect()
            })
            .clone()
    }

    fn create_not_found_exception(&self, id: &str, caller: Option<&Caller>) -> Result<Error> {
        if let Some(last) = self.loading.last() {
            let msg = format!(
                "The service \"{last}\" has a dependency on a non-existent service \"{id}\". This locator {}",
                self.format_alternatives(None, "and")
            );
            let source = (!is_falsy_id(&last)).then_some(last);
            return Ok(Error::service_not_found(id, source, Vec::new(), Some(msg)));
        }

        let mut class = caller;
        let external_id = self
            .external_id
            .as_deref()
            .filter(|e| !is_falsy_id(e))
            .or(caller.map(|c| c.class.as_str()))
            .filter(|e| !is_falsy_id(e));

        let mut msg = vec![format!("Service \"{id}\" not found:")];

        match &self.container {
            None => class = None,
            Some(container) => {
                if container.has(id) || container.is_removed_id(id) {
                    msg.push("even though it exists in the app's container,".to_owned());
                } else {
                    match container.get(id, InvalidBehavior::Exception) {
                        Ok(_) => class = None,
                        Err(e) if e.is(&ErrorKind::ServiceNotFound) => {
                            if e.alternatives().is_empty() {
                                class = None;
                            } else {
                                msg.push(format!(
                                    "did you mean {}? Anyway,",
                                    self.format_alternatives(Some(e.alternatives().to_vec()), "or")
                                ));
                            }
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
        }

        match external_id {
            Some(external_id) => msg.push(format!(
                "the container inside \"{external_id}\" is a smaller service locator that {}",
                self.format_alternatives(None, "and")
            )),
            None => msg.push(format!(
                "the current service locator {}",
                self.format_alternatives(None, "and")
            )),
        }

        match class {
            None => {}
            Some(caller) if caller.service_subscriber => msg.push(format!(
                "Unless you need extra laziness, try using dependency injection instead. Otherwise, you need to declare it using \"{}::getSubscribedServices()\".",
                short_class(&caller.class)
            )),
            Some(_) => msg.push("Try using dependency injection instead.".to_owned()),
        }

        let source = self.loading.last().filter(|l| !is_falsy_id(l));
        Ok(Error::service_not_found(
            id,
            source,
            Vec::new(),
            Some(msg.join(" ")),
        ))
    }

    fn format_alternatives(&self, alternatives: Option<Vec<String>>, separator: &str) -> String {
        let known = alternatives.is_none();
        let mut alternatives = if let Some(alternatives) = alternatives {
            alternatives
        } else {
            let keys: Vec<String> = self.factories.keys().map(str::to_owned).collect();
            if keys.is_empty() {
                return "is empty...".to_owned();
            }
            keys
        };
        let plural = if alternatives.len() > 1 { "s" } else { "" };
        let last = alternatives.pop().unwrap_or_default();
        let listed = if alternatives.is_empty() {
            format!("\"{last}\"")
        } else {
            format!("\"{}\" {separator} \"{last}\"", alternatives.join("\", \""))
        };
        if known {
            format!("only knows about the {listed} service{plural}.")
        } else {
            listed
        }
    }
}

/// PSR-11's `get($id)`/`has($id)`.
impl psr_container::ContainerInterface for ServiceLocator {
    fn get(&self, id: &[u8]) -> std::result::Result<Value, psr_container::Throwable> {
        let id = String::from_utf8_lossy(id);
        ServiceLocator::get(self, &id).map_err(|e| Arc::new(e) as _)
    }

    fn has(&self, id: &[u8]) -> bool {
        std::str::from_utf8(id).is_ok_and(|id| ServiceLocator::has(self, id))
    }
}
