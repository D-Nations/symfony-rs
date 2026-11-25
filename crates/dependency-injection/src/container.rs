//! Port of Symfony\Component\DependencyInjection\Container.php: the
//! run-time container.
//!
//! A PHP compiled container is a subclass that fills `$methodMap` and
//! `$fileMap` and defines a protected factory method per service. Here the
//! subclass is data: a [`ContainerClass`] gives the class name, the
//! `$methodMap` (id => method name) with a method table (method name =>
//! [`ServiceFactory`]), the `$fileMap` (id => file name) with a file table
//! (file name => factory, what `require $file` runs), the aliases, the
//! synthetic ids, the removed ids (`getRemovedIds()`) and the
//! `privates['service_container']` initializer. Generated code builds one
//! and calls [`Container::from_class`]. A factory gets the container, as
//! a method gets `$this`, and stores a shared service with
//! [`Container::store_service`] or [`Container::store_private`], as
//! `$this->services[$id] = ...` does.
//!
//! A container is always in an `Arc` (it is the `service_container`
//! service, an object value `get()` returns) and keeps its state behind a
//! mutex, which no method holds while it calls a factory, a service or a
//! processor. Services are php-value object values; a `null` service is
//! `None`.
//!
//! `getEnv()` looks its processors up in the
//! `container.env_var_processors_locator` service, a [`ServiceLocator`]
//! whose entries are [`EnvProcessorObject`]s (an [`EnvProcessor`] as a
//! service). The fallback `new EnvVarProcessor($this)` is the factory set
//! with [`Container::set_default_env_var_processor`]; without one, the
//! lookup fails with PHP's error for a missing class.
//!
//! Differences that cannot be kept: `$loading` and `$resolving` are kept
//! per thread, so two threads creating the same service are not a
//! circular reference; `getService()` does not store a `null` result in
//! the registry; the default parameter bag is a `ParameterBag`, not an
//! `EnvPlaceholderParameterBag`; containers are not cloneable, as
//! upstream's private `__clone()`.
//!
//! Upstream: symfony/dependency-injection Container.php

use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use php_value::{PhpArray, Value};

use crate::container_interface::{ContainerInterface, InvalidBehavior};
use crate::exception::{Error, ErrorKind, Result};
use crate::object::downcast;
use crate::ordered_map::{OrderedMap, ThreadStack};
use crate::parameter_bag::ParameterBag;
use crate::service_locator::ServiceLocator;
use crate::service_object::{
    call_method, is_resettable, is_uninitialized_lazy_object, undefined_method,
};
use crate::services_resetter::ResetMap;

/// `Symfony\Component\DependencyInjection\Container`.
pub const CONTAINER_CLASS: &str = "Symfony\\Component\\DependencyInjection\\Container";

/// The id of the container itself.
pub const SERVICE_CONTAINER: &str = "service_container";

/// The id of the env var processors locator.
pub const ENV_VAR_PROCESSORS_LOCATOR: &str = "container.env_var_processors_locator";

/// A factory method or file: creates a service of the container it gets.
pub type ServiceFactory = Arc<dyn Fn(&Container) -> Result<Value> + Send + Sync>;

/// The `privates['service_container']` initializer of a dumped container.
pub type Initializer = Box<dyn FnOnce(&Container) -> Result<()> + Send>;

/// The fallback `new EnvVarProcessor($this)`.
pub type DefaultEnvProcessor = Arc<dyn Fn(&Container) -> Arc<dyn EnvProcessor> + Send + Sync>;

/// An env var processor as `getEnv()` calls it:
/// `$processor->getEnv($prefix, $name, $getEnv)`.
pub trait EnvProcessor: Send + Sync {
    /// `getEnv($prefix, $name, $getEnv)`. `get_env` is the container's
    /// `getEnv()` closure.
    fn get_env(
        &self,
        prefix: &str,
        name: &str,
        get_env: &dyn Fn(&str) -> Result<Value>,
    ) -> Result<Value>;
}

/// An [`EnvProcessor`] as a service object.
pub struct EnvProcessorObject {
    class: String,
    processor: Arc<dyn EnvProcessor>,
}

impl EnvProcessorObject {
    pub fn new(class: impl Into<String>, processor: Arc<dyn EnvProcessor>) -> Self {
        Self {
            class: class.into(),
            processor,
        }
    }

    /// The processor as an object value.
    pub fn value(class: impl Into<String>, processor: Arc<dyn EnvProcessor>) -> Value {
        Value::Object(Arc::new(Self::new(class, processor)))
    }

    pub fn processor(&self) -> &Arc<dyn EnvProcessor> {
        &self.processor
    }
}

impl fmt::Debug for EnvProcessorObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{:p}", self.class, std::ptr::from_ref(self))
    }
}

impl php_value::value::PhpObject for EnvProcessorObject {
    fn class(&self) -> &str {
        &self.class
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// What a compiled container subclass declares.
#[derive(Default)]
pub struct ContainerClass {
    /// The class name; `Container` when `None`.
    pub class: Option<String>,
    /// `$methodMap`: id => method name.
    pub method_map: Vec<(String, String)>,
    /// The factory methods, by name.
    pub methods: Vec<(String, ServiceFactory)>,
    /// `$fileMap`: id => file name.
    pub file_map: Vec<(String, String)>,
    /// What `load($file)` runs, by file name.
    pub files: Vec<(String, ServiceFactory)>,
    /// `$aliases`: alias => id.
    pub aliases: Vec<(String, String)>,
    /// `$syntheticIds`.
    pub synthetic_ids: Vec<String>,
    /// `getRemovedIds()`.
    pub removed_ids: Vec<String>,
    /// `$privates['service_container']`: run by the first `set()`.
    pub initializer: Option<Initializer>,
}

/// The registry argument of `getService()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Registry {
    /// `'services'`.
    Services,
    /// `'privates'`.
    Privates,
}

/// The `$load` argument of `getService()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Load {
    /// `false`: call the factory method.
    Method,
    /// `true`: load the factory file.
    File,
    /// A string: the message of the `RuntimeException` to throw.
    Error(String),
}

struct State {
    parameter_bag: ParameterBag,
    services: OrderedMap<Value>,
    privates: OrderedMap<Value>,
    factories: OrderedMap<ServiceFactory>,
    private_factories: OrderedMap<ServiceFactory>,
    aliases: OrderedMap<String>,
    env_cache: HashMap<String, Value>,
    compiled: bool,
    reset_map: Option<ResetMap>,
    initializer: Option<Initializer>,
}

/// `Container`.
pub struct Container {
    this: Weak<Container>,
    class: String,
    method_map: OrderedMap<String>,
    methods: HashMap<String, ServiceFactory>,
    file_map: OrderedMap<String>,
    files: HashMap<String, ServiceFactory>,
    synthetic_ids: HashSet<String>,
    removed_ids: OrderedMap<()>,
    state: Mutex<State>,
    loading: ThreadStack,
    resolving: ThreadStack,
    default_env_processor: Mutex<Option<DefaultEnvProcessor>>,
}

impl fmt::Debug for Container {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{:p}", self.class, std::ptr::from_ref(self))
    }
}

impl php_value::value::PhpObject for Container {
    fn class(&self) -> &str {
        &self.class
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

fn not_a_service(value: Value) -> Option<Value> {
    match value {
        Value::Null => None,
        v => Some(v),
    }
}

/// PHP's falsy strings: `''` and `'0'`.
pub(crate) fn is_falsy_id(id: &str) -> bool {
    id.is_empty() || id == "0"
}

impl Container {
    /// `new Container($parameterBag)`.
    pub fn new(parameter_bag: Option<ParameterBag>) -> Arc<Self> {
        Self::from_class(parameter_bag, ContainerClass::default())
    }

    /// A compiled container subclass: `new $class($parameterBag)`.
    pub fn from_class(parameter_bag: Option<ParameterBag>, class: ContainerClass) -> Arc<Self> {
        let parameter_bag = parameter_bag.unwrap_or_default();
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            class: class.class.unwrap_or_else(|| CONTAINER_CLASS.to_owned()),
            method_map: class.method_map.into_iter().collect(),
            methods: class.methods.into_iter().collect(),
            file_map: class.file_map.into_iter().collect(),
            files: class.files.into_iter().collect(),
            synthetic_ids: class.synthetic_ids.into_iter().collect(),
            removed_ids: class.removed_ids.into_iter().map(|id| (id, ())).collect(),
            state: Mutex::new(State {
                parameter_bag,
                services: OrderedMap::new(),
                privates: OrderedMap::new(),
                factories: OrderedMap::new(),
                private_factories: OrderedMap::new(),
                aliases: class.aliases.into_iter().collect(),
                env_cache: HashMap::new(),
                compiled: false,
                reset_map: None,
                initializer: class.initializer,
            }),
            loading: ThreadStack::default(),
            resolving: ThreadStack::default(),
            default_env_processor: Mutex::new(None),
        })
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The class name.
    pub fn class(&self) -> &str {
        &self.class
    }

    /// The container as an object value (`$this`).
    pub fn to_value(&self) -> Value {
        self.this
            .upgrade()
            .map_or(Value::Null, |this| Value::Object(this))
    }

    /// Whether `value` is this container (`$value === $this`).
    pub fn is_self(&self, value: &Value) -> bool {
        downcast::<Container>(value).is_some_and(|c| std::ptr::eq(c, self))
    }

    /// `compile()`: resolves the parameters and freezes the bag.
    pub fn compile(&self) -> Result<()> {
        let mut state = self.state();
        state.parameter_bag.resolve()?;
        let bag = &state.parameter_bag;
        let frozen = ParameterBag::frozen(
            bag.all().clone(),
            bag.all_deprecated().to_vec(),
            bag.all_non_empty().to_vec(),
        );
        state.parameter_bag = frozen;
        state.compiled = true;
        Ok(())
    }

    /// `isCompiled()`.
    pub fn is_compiled(&self) -> bool {
        self.state().compiled
    }

    /// `getParameterBag()`, as a copy.
    pub fn get_parameter_bag(&self) -> ParameterBag {
        self.state().parameter_bag.clone()
    }

    /// Reads the parameter bag.
    pub fn with_parameter_bag<R>(&self, f: impl FnOnce(&ParameterBag) -> R) -> R {
        f(&self.state().parameter_bag)
    }

    /// Changes the parameter bag (`getParameterBag()->...`).
    pub fn with_parameter_bag_mut<R>(&self, f: impl FnOnce(&mut ParameterBag) -> R) -> R {
        f(&mut self.state().parameter_bag)
    }

    /// `getParameter($name)`.
    pub fn get_parameter(&self, name: &str) -> Result<Value> {
        self.state().parameter_bag.get(name)
    }

    /// `hasParameter($name)`.
    pub fn has_parameter(&self, name: &str) -> bool {
        self.state().parameter_bag.has(name)
    }

    /// `setParameter($name, $value)`.
    pub fn set_parameter(&self, name: &str, value: Value) -> Result<()> {
        self.state().parameter_bag.set(name, value)
    }

    /// `set($id, $service)`. Setting a synthetic service to `None` resets
    /// it.
    pub fn set(&self, id: &str, service: Option<Value>) -> Result<()> {
        // Runs the internal initializer; used by the dumped container to
        // include always-needed files.
        let initializer = self.state().initializer.take();
        if let Some(initialize) = initializer {
            initialize(self)?;
        }

        if id == SERVICE_CONTAINER {
            return Err(Error::invalid_argument(
                "You cannot set service \"service_container\".",
            ));
        }

        let mut state = self.state();
        if !(self.file_map.contains_key(id) || self.method_map.contains_key(id)) {
            if self.synthetic_ids.contains(id) || !self.removed_ids.contains_key(id) {
                // no-op
            } else if service.is_none() {
                return Err(Error::invalid_argument(format!(
                    "The \"{id}\" service is private, you cannot unset it."
                )));
            } else {
                return Err(Error::invalid_argument(format!(
                    "The \"{id}\" service is private, you cannot replace it."
                )));
            }
        } else if state.services.contains_key(id) {
            return Err(Error::invalid_argument(format!(
                "The \"{id}\" service is already initialized, you cannot replace it."
            )));
        }

        state.aliases.remove(id);

        match service {
            None => {
                state.services.remove(id);
            }
            Some(service) => state.services.insert(id, service),
        }
        Ok(())
    }

    /// `has($id)`.
    pub fn has(&self, id: &str) -> bool {
        let state = self.state();
        let id = state.aliases.get(id).map_or(id, String::as_str);
        if state.services.contains_key(id) {
            return true;
        }
        if id == SERVICE_CONTAINER {
            return true;
        }
        self.file_map.contains_key(id) || self.method_map.contains_key(id)
    }

    /// `get($id, $invalidBehavior)`.
    pub fn get(&self, id: &str, invalid_behavior: InvalidBehavior) -> Result<Option<Value>> {
        let (id, factory) = {
            let state = self.state();
            if let Some(service) = state.services.get(id) {
                return Ok(Some(service.clone()));
            }
            let id = state
                .aliases
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.to_owned());
            if let Some(service) = state.services.get(&id) {
                return Ok(Some(service.clone()));
            }
            if id == SERVICE_CONTAINER {
                return Ok(not_a_service(self.to_value()));
            }
            let factory = state.factories.get(&id).cloned();
            (id, factory)
        };
        match factory {
            Some(factory) => factory(self).map(not_a_service),
            None => self.make(&id, invalid_behavior),
        }
    }

    /// `get($id)` with the default behaviour, for a service that must
    /// exist.
    pub fn get_service_value(&self, id: &str) -> Result<Value> {
        Ok(self
            .get(id, InvalidBehavior::Exception)?
            .unwrap_or(Value::Null))
    }

    /// `make()`: creates a service.
    fn make(&self, id: &str, invalid_behavior: InvalidBehavior) -> Result<Option<Value>> {
        if self.loading.contains(id) {
            let mut path = self.loading.current();
            path.push(id.to_owned());
            return Err(Error::service_circular_reference(id, path));
        }

        self.loading.push(id);

        let result = match (self.file_map.get(id), self.method_map.get(id)) {
            _ if !(self.file_map.contains_key(id) || self.method_map.contains_key(id)) => None,
            _ if invalid_behavior == InvalidBehavior::IgnoreUninitialized => Some(Ok(None)),
            (Some(file), _) => Some(self.load(file).map(not_a_service)),
            (None, Some(method)) => Some(self.call_factory_method(method).map(not_a_service)),
            (None, None) => None,
        };

        self.loading.remove(id);

        match result {
            Some(Ok(service)) => return Ok(service),
            Some(Err(e)) => {
                let mut state = self.state();
                state.services.remove(id);
                state.privates.remove(id);
                return Err(e);
            }
            None => {}
        }

        if invalid_behavior == InvalidBehavior::Exception {
            if is_falsy_id(id) {
                return Err(Error::service_not_found(id, None, Vec::new(), None));
            }
            if self.synthetic_ids.contains(id) {
                return Err(Error::service_not_found(
                    id,
                    None,
                    Vec::new(),
                    Some(format!(
                        "The \"{id}\" service is synthetic, it needs to be set at boot time before it can be used."
                    )),
                ));
            }
            if self.removed_ids.contains_key(id) {
                return Err(Error::service_not_found(
                    id,
                    None,
                    Vec::new(),
                    Some(format!(
                        "The \"{id}\" service or alias has been removed or inlined when the container was compiled. You should either make it public, or stop using the container directly and use dependency injection instead."
                    )),
                ));
            }

            let mut alternatives = Vec::new();
            for known_id in self.get_service_ids() {
                if known_id.is_empty() || known_id.starts_with('.') {
                    continue;
                }
                let lev = php_value::string::levenshtein(id.as_bytes(), known_id.as_bytes());
                #[expect(clippy::cast_precision_loss, reason = "PHP's float division")]
                let close = lev as f64 <= id.len() as f64 / 3.0;
                if close || known_id.contains(id) {
                    alternatives.push(known_id);
                }
            }

            return Err(Error::service_not_found(id, None, alternatives, None));
        }

        Ok(None)
    }

    fn call_factory_method(&self, method: &str) -> Result<Value> {
        match self.methods.get(method) {
            Some(factory) => factory(self),
            None => Err(undefined_method(&self.class, method)),
        }
    }

    /// `initialized($id)`.
    pub fn initialized(&self, id: &str) -> bool {
        let state = self.state();
        let id = state.aliases.get(id).map_or(id, String::as_str);
        if id == SERVICE_CONTAINER {
            return false;
        }
        state.services.contains_key(id)
    }

    /// `reset()`: resets the `ResetInterface` services that are not
    /// uninitialized lazy objects, ignoring their errors, then forgets
    /// every service.
    pub fn reset(&self) {
        let services: Vec<Value> = {
            let state = self.state();
            let mut seen: HashSet<&str> = HashSet::new();
            let mut values = Vec::new();
            for (id, service) in state.services.iter().chain(state.privates.iter()) {
                if seen.insert(id) {
                    values.push(service.clone());
                }
            }
            values
        };

        for service in &services {
            if is_resettable(service) && !is_uninitialized_lazy_object(service) {
                // A failing reset does not stop the others.
                let _ = call_method(service, "reset", &[]);
            }
        }

        let mut state = self.state();
        state.env_cache.clear();
        state.services.clear();
        state.factories.clear();
        state.private_factories.clear();
        state.privates.clear();
        // Non-shared tracked instances are not reset here because
        // Container::reset() tears down the entire container.
        // ServicesResetter::reset() handles calling reset methods on
        // tracked instances during the request lifecycle.
        state.reset_map = None;
    }

    /// `resetEnvCache()`.
    pub fn reset_env_cache(&self) {
        self.state().env_cache.clear();
    }

    /// `getServiceIds()`.
    pub fn get_service_ids(&self) -> Vec<String> {
        let state = self.state();
        let mut seen = HashSet::new();
        std::iter::once(SERVICE_CONTAINER)
            .chain(self.file_map.keys())
            .chain(self.method_map.keys())
            .chain(state.aliases.keys())
            .chain(state.services.keys())
            .filter(|id| seen.insert(*id))
            .map(str::to_owned)
            .collect()
    }

    /// `getRemovedIds()`: the ids, in order.
    pub fn get_removed_ids(&self) -> Vec<String> {
        self.removed_ids.keys().map(str::to_owned).collect()
    }

    /// `isset($this->getRemovedIds()[$id])`.
    pub fn is_removed_id(&self, id: &str) -> bool {
        self.removed_ids.contains_key(id)
    }

    /// `camelize($id)`.
    pub fn camelize(id: &str) -> String {
        let mut spaced = Vec::with_capacity(id.len() * 2);
        for &b in id.as_bytes() {
            match b {
                b'_' => spaced.push(b' '),
                b'.' | b'\\' => spaced.extend_from_slice(b"_ "),
                _ => spaced.push(b),
            }
        }
        // ucwords() with its default delimiters.
        let mut out = Vec::with_capacity(spaced.len());
        let mut word_start = true;
        for b in spaced {
            let delimiter = matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 0x0c | 0x0b);
            out.push(if word_start {
                b.to_ascii_uppercase()
            } else {
                b
            });
            word_start = delimiter;
        }
        out.retain(|&b| b != b' ');
        String::from_utf8_lossy(&out).into_owned()
    }

    /// `underscore($id)`.
    pub fn underscore(id: &str) -> String {
        let s: Vec<u8> = id
            .bytes()
            .map(|b| if b == b'_' { b'.' } else { b })
            .collect();

        // '/([A-Z]+)([A-Z][a-z])/' => '\1_\2'
        let mut first = Vec::with_capacity(s.len() + 4);
        let mut p = 0;
        while p < s.len() {
            let mut e = p;
            while e < s.len() && s[e].is_ascii_uppercase() {
                e += 1;
            }
            if e - p >= 2 && e < s.len() && s[e].is_ascii_lowercase() {
                first.extend_from_slice(&s[p..e - 1]);
                first.push(b'_');
                first.extend_from_slice(&s[e - 1..=e]);
                p = e + 1;
            } else {
                first.push(s[p]);
                p += 1;
            }
        }

        // '/([a-z\d])([A-Z])/' => '\1_\2'
        let mut second = Vec::with_capacity(first.len() + 4);
        let mut p = 0;
        while p < first.len() {
            let b = first[p];
            if (b.is_ascii_lowercase() || b.is_ascii_digit())
                && first.get(p + 1).is_some_and(u8::is_ascii_uppercase)
            {
                second.push(b);
                second.push(b'_');
                second.push(first[p + 1]);
                p += 2;
            } else {
                second.push(b);
                p += 1;
            }
        }

        second.make_ascii_lowercase();
        String::from_utf8_lossy(&second).into_owned()
    }

    /// `load($file)`: runs the factory of a file.
    pub fn load(&self, file: &str) -> Result<Value> {
        match self.files.get(file) {
            Some(factory) => factory(self),
            None => Err(Error::new(
                ErrorKind::Other("Error".to_owned()),
                format!("Failed opening required '{file}'"),
            )),
        }
    }

    /// Sets the fallback processor `getEnv()` uses when the locator has
    /// none for a prefix (`new EnvVarProcessor($this)`).
    pub fn set_default_env_var_processor(&self, processor: DefaultEnvProcessor) {
        *self
            .default_env_processor
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(processor);
    }

    /// `getEnv($name)`: an environment variable through the env var
    /// processors.
    pub fn get_env(&self, name: &str) -> Result<Value> {
        let env_name = format!("env({name})");
        if self.resolving.contains(&env_name) {
            return Err(Error::parameter_circular_reference(
                self.resolving.current(),
            ));
        }
        if let Some(value) = self.state().env_cache.get(name) {
            return Ok(value.clone());
        }
        if !self.has(ENV_VAR_PROCESSORS_LOCATOR) {
            self.set(
                ENV_VAR_PROCESSORS_LOCATOR,
                Some(ServiceLocator::new(Vec::new()).into_value()),
            )?;
        }
        let processors = self.get_service_value(ENV_VAR_PROCESSORS_LOCATOR)?;
        let Some(processors) = downcast::<ServiceLocator>(&processors) else {
            return Err(undefined_method(&processors.debug_type(), "has"));
        };

        let (mut prefix, local_name, has_prefix) = match name.find(':') {
            Some(i) => (&name[..i], &name[i + 1..], true),
            None => ("string", name, false),
        };

        let processor: Arc<dyn EnvProcessor> = if processors.has(prefix) {
            let processor = processors.get(prefix)?;
            match downcast::<EnvProcessorObject>(&processor) {
                Some(p) => p.processor.clone(),
                None => return Err(undefined_method(&processor.debug_type(), "getEnv")),
            }
        } else {
            let default = self
                .default_env_processor
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            match default {
                Some(make) => make(self),
                None => {
                    return Err(Error::new(
                        ErrorKind::Other("Error".to_owned()),
                        "Class \"Symfony\\Component\\DependencyInjection\\EnvVarProcessor\" not found",
                    ));
                }
            }
        };
        if !has_prefix {
            prefix = "";
        }

        self.resolving.push(&env_name);
        let result = processor.get_env(prefix, local_name, &|n: &str| self.get_env(n));
        self.resolving.remove(&env_name);
        let value = result?;
        self.state()
            .env_cache
            .insert(name.to_owned(), value.clone());
        Ok(value)
    }

    /// `getService($registry, $id, $method, $load)`, for generated code.
    pub fn get_service(
        &self,
        registry: Option<Registry>,
        id: &str,
        method: Option<&str>,
        load: &Load,
    ) -> Result<Value> {
        if id == SERVICE_CONTAINER {
            return Ok(self.to_value());
        }
        if let Load::Error(message) = load {
            return Err(Error::runtime(message.clone()));
        }
        let Some(method) = method else {
            return Ok(match registry {
                Some(registry) => self.registry_get(registry, id).unwrap_or(Value::Null),
                None => Value::Null,
            });
        };
        let create = |c: &Self| match load {
            Load::File => c.load(method),
            _ => c.call_factory_method(method),
        };
        if let Some(registry) = registry {
            if let Some(service) = self.registry_get(registry, id) {
                return Ok(service);
            }
            let service = create(self)?;
            if !matches!(service, Value::Null) {
                let mut state = self.state();
                let map = match registry {
                    Registry::Services => &mut state.services,
                    Registry::Privates => &mut state.privates,
                };
                map.insert(id, service.clone());
            }
            return Ok(service);
        }
        if *load == Load::Method {
            return self.call_factory_method(method);
        }
        let factory = {
            let state = self.state();
            state
                .factories
                .get(id)
                .or_else(|| state.private_factories.get(id))
                .cloned()
        };
        match factory {
            Some(factory) => factory(self),
            None => self.load(method),
        }
    }

    fn registry_get(&self, registry: Registry, id: &str) -> Option<Value> {
        let state = self.state();
        let map = match registry {
            Registry::Services => &state.services,
            Registry::Privates => &state.privates,
        };
        map.get(id).filter(|v| !matches!(v, Value::Null)).cloned()
    }

    /// `trackForReset($instance, $resetMethods)`, for generated code.
    pub fn track_for_reset(&self, instance: Value, reset_methods: Vec<String>) -> Value {
        if let Value::Object(object) = &instance {
            self.get_reset_map().insert(object, reset_methods);
        }
        instance
    }

    /// `getResetMap()`, for generated code.
    pub fn get_reset_map(&self) -> ResetMap {
        self.state()
            .reset_map
            .get_or_insert_with(ResetMap::new)
            .clone()
    }

    /// `$this->services[$id] ?? null`, for generated code.
    pub fn shared_service(&self, id: &str) -> Option<Value> {
        self.state().services.get(id).cloned()
    }

    /// `$this->services[$id] = $service`, for generated code.
    pub fn store_service(&self, id: &str, service: Value) -> Value {
        self.state().services.insert(id, service.clone());
        service
    }

    /// `$this->privates[$id] ?? null`, for generated code.
    pub fn private_service(&self, id: &str) -> Option<Value> {
        self.state().privates.get(id).cloned()
    }

    /// `$this->privates[$id] = $service`, for generated code.
    pub fn store_private(&self, id: &str, service: Value) -> Value {
        self.state().privates.insert(id, service.clone());
        service
    }

    /// The ids in `$this->privates`.
    pub fn private_ids(&self) -> Vec<String> {
        self.state().privates.keys().map(str::to_owned).collect()
    }

    /// `$this->factories[$id] = $factory`, for generated code.
    pub fn set_factory(&self, id: &str, factory: ServiceFactory) {
        self.state().factories.insert(id, factory);
    }

    /// `$this->factories['service_container'][$id] = $factory`, for
    /// generated code.
    pub fn set_private_factory(&self, id: &str, factory: ServiceFactory) {
        self.state().private_factories.insert(id, factory);
    }

    /// `$this->syntheticIds`.
    pub fn is_synthetic_id(&self, id: &str) -> bool {
        self.synthetic_ids.contains(id)
    }

    /// `ReverseContainer`'s lookup: `array_search($service, $this->services,
    /// true) ?: array_search($service, $this->privates, true) ?: null`.
    pub(crate) fn find_service_id(&self, service: &Value) -> Option<String> {
        let state = self.state();
        let search = |map: &OrderedMap<Value>| {
            map.iter()
                .find(|(_, v)| v.identical(service))
                .map(|(id, _)| id.to_owned())
                .filter(|id| !is_falsy_id(id))
        };
        search(&state.services).or_else(|| search(&state.privates))
    }
}

impl ContainerInterface for Container {
    fn set(&self, id: &str, service: Option<Value>) -> Result<()> {
        Container::set(self, id, service)
    }

    fn get(&self, id: &str, invalid_behavior: InvalidBehavior) -> Result<Option<Value>> {
        Container::get(self, id, invalid_behavior)
    }

    fn has(&self, id: &str) -> bool {
        Container::has(self, id)
    }

    fn initialized(&self, id: &str) -> bool {
        Container::initialized(self, id)
    }

    fn get_parameter(&self, name: &str) -> Result<Value> {
        Container::get_parameter(self, name)
    }

    fn has_parameter(&self, name: &str) -> bool {
        Container::has_parameter(self, name)
    }

    fn set_parameter(&self, name: &str, value: Value) -> Result<()> {
        Container::set_parameter(self, name, value)
    }
}

/// PSR-11's `get($id)`/`has($id)`.
impl psr_container::ContainerInterface for Container {
    fn get(&self, id: &[u8]) -> std::result::Result<Value, psr_container::Throwable> {
        let id = String::from_utf8_lossy(id);
        self.get_service_value(&id).map_err(|e| Arc::new(e) as _)
    }

    fn has(&self, id: &[u8]) -> bool {
        std::str::from_utf8(id).is_ok_and(|id| Container::has(self, id))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::service_locator::LocatorEntry;

    /// Answers `prefix|name`, or resolves `env(<name>)` again for `loop`.
    struct Echo(Arc<AtomicUsize>);

    impl EnvProcessor for Echo {
        fn get_env(
            &self,
            prefix: &str,
            name: &str,
            get_env: &dyn Fn(&str) -> Result<Value>,
        ) -> Result<Value> {
            self.0.fetch_add(1, Ordering::SeqCst);
            if name == "loop" {
                return get_env("echo:loop");
            }
            Ok(Value::from(format!("{prefix}|{name}")))
        }
    }

    fn container_with_echo(calls: &Arc<AtomicUsize>) -> Arc<Container> {
        let c = Container::new(None);
        let echo: Arc<dyn EnvProcessor> = Arc::new(Echo(calls.clone()));
        let locator = ServiceLocator::new(vec![
            (
                "echo".to_owned(),
                LocatorEntry::new({
                    let echo = echo.clone();
                    move |_| Ok(EnvProcessorObject::value("Echo", echo.clone()))
                }),
            ),
            (
                "string".to_owned(),
                LocatorEntry::new(move |_| Ok(EnvProcessorObject::value("Echo", echo.clone()))),
            ),
        ]);
        c.set(ENV_VAR_PROCESSORS_LOCATOR, Some(locator.into_value()))
            .unwrap();
        c
    }

    #[test]
    fn get_env_routes_prefixes_and_caches() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = container_with_echo(&calls);
        assert!(
            c.get_env("echo:FOO")
                .unwrap()
                .identical(&Value::from("echo|FOO"))
        );
        // No prefix: the "string" processor, called with an empty prefix.
        assert!(c.get_env("FOO").unwrap().identical(&Value::from("|FOO")));
        assert!(c.get_env("FOO").unwrap().identical(&Value::from("|FOO")));
        assert_eq!(2, calls.load(Ordering::SeqCst));
        c.reset_env_cache();
        c.get_env("FOO").unwrap();
        assert_eq!(3, calls.load(Ordering::SeqCst));
    }

    #[test]
    fn get_env_detects_circular_references() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = container_with_echo(&calls);
        let e = c.get_env("echo:loop").unwrap_err();
        assert!(e.is(&ErrorKind::ParameterCircularReference));
        assert_eq!(vec!["env(echo:loop)".to_owned()], e.parameters());
    }

    #[test]
    fn get_env_without_the_default_processor() {
        let c = Container::new(None);
        let e = c.get_env("FOO").unwrap_err();
        assert_eq!(
            "Class \"Symfony\\Component\\DependencyInjection\\EnvVarProcessor\" not found",
            e.message()
        );
        // The empty locator was set on the way.
        assert!(c.has(ENV_VAR_PROCESSORS_LOCATOR));
    }
}
