//! Port of Symfony\Component\DependencyInjection\ContainerBuilder.php.
//!
//! The builder holds what the compiler passes and the Rust emitter read:
//! definitions, aliases, parameters, tags, autoconfiguration, extension
//! configs, env placeholders and their counters, removed ids and tracked
//! resources. The base-container behaviour the builder relies on
//! (`Container::set()`, `has()`, `get()` up to creating a service,
//! `getEnv()`, `compile()`, the parameter accessors) is written here,
//! in this module, rather than in a port of `Container.php`.
//!
//! Differences that a build-time port cannot avoid:
//!
//! - `get()` and `resolveServices()` do everything upstream does before
//!   a PHP object is created (stored services, aliases, deprecations,
//!   missing and removed ids, circular references, definition errors,
//!   synthetic and child definitions, `AbstractArgument`s, parameters);
//!   creating a service from its definition, and the run-time objects
//!   of the argument classes (closures, generators, locators), fail with
//!   an error that says so: the Rust emitter replaces them.
//!   `get('service_container')` (the builder itself) fails the same way.
//! - Reflection reads the [`ClassRegistry`] (`getReflectionClass()`,
//!   `addObjectResource()`). A class the registry does not know (a pass
//!   written in Rust, say) has no PHP file, so `addObjectResource()`
//!   tracks nothing for it.
//! - symfony-config has no `ClassExistenceResource`,
//!   `ReflectionClassResource` or `ComposerResource`: class lookups add
//!   no resource, and there is no Composer autoloader to name vendor
//!   directories, so `inVendors()` is false and every file resource is
//!   tracked.
//! - Env vars are read through an [`EnvVarResolver`] that the build
//!   sets (`EnvVarProcessor` and the processors locator); without one,
//!   resolving an env var fails.
//! - There is no `ExpressionLanguage`: no expression providers, and no
//!   `Expression` values. `setProxyInstantiator()` and
//!   `willBeAvailable()` (Composer's `InstalledVersions`) are not ported.
//! - A custom `ParameterBagInterface` cannot be passed: the bag is the
//!   crate's [`ParameterBag`].
//!
//! Upstream: symfony/dependency-injection ContainerBuilder.php

use std::collections::HashMap;
use std::sync::Arc;

use php_value::{PhpArray, PhpString, Value, array::Key};
use symfony_config::resource::{
    DirectoryResource, FileExistenceResource, FileResource, GlobResource, ResourceInterface, xxh128,
};
use symfony_deprecation_contracts::trigger_deprecation;

use crate::alias::Alias;
use crate::argument::{
    AbstractArgument, BoundArgument, EnvClosureArgument, IteratorArgument, ServiceClosureArgument,
    ServiceLocatorArgument,
};
use crate::class_metadata::{AttributeMetadata, ClassMetadata, ClassRegistry, Reflector};
use crate::compiler::{Compiler, Pass, PassConfig, PassType, ResolveEnvPlaceholdersPass};
use crate::container_interface::{CONTAINER_INTERFACE, InvalidBehavior};
use crate::definition::Definition;
use crate::exception::{Error, ErrorKind, Result};
use crate::extension::ExtensionInterface;
use crate::id_map::IdMap;
use crate::parameter_bag::{EnvPlaceholderMap, ParameterBag, escape_value};
use crate::reference::{Parameter, Reference};

/// `Symfony\Component\DependencyInjection\Container`.
pub const CONTAINER_CLASS: &str = "Symfony\\Component\\DependencyInjection\\Container";

/// `ContainerBuilder::ANONYMOUS_ID_REGEXP`.
pub const ANONYMOUS_ID_REGEXP: &str = "/^\\.\\d+_[^~]*+~[._a-zA-Z\\d]{7}$/";

/// `INTERNAL_TYPES`.
const INTERNAL_TYPES: [&str; 11] = [
    "int", "float", "string", "bool", "resource", "object", "array", "null", "callable",
    "iterable", "mixed",
];

/// The `$format` of `resolveEnvPlaceholders()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvFormat {
    /// `null`: back to `%env(NAME)%`.
    Placeholder,
    /// A `sprintf()` format of the env var name.
    Sprintf(String),
    /// `true`: the env vars' values.
    Resolve,
}

/// The `$trackContents` of `fileExists()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackContents {
    /// `false`.
    No,
    /// `true`.
    Yes,
    /// A pattern for the directory's contents (`""` and `"0"` count as
    /// `false`, as in PHP).
    Pattern(String),
}

impl TrackContents {
    fn is_truthy(&self) -> bool {
        match self {
            Self::No => false,
            Self::Yes => true,
            Self::Pattern(p) => php_value::string::truthy(p.as_bytes()),
        }
    }
}

/// Reads env vars for `getEnv()`: `$processor->getEnv($prefix, $name,
/// $getEnv)` of the processor that the prefix selects
/// (`container.env_var_processors_locator`, else `EnvVarProcessor`).
/// `prefix` is empty for a name without one; `parameters` are the
/// container's (the `env(NAME)` defaults `EnvVarProcessor` reads), and
/// `get_env` reads another env var through the builder.
pub trait EnvVarResolver: Send + Sync {
    fn get_env(
        &self,
        prefix: &str,
        name: &str,
        parameters: &ParameterBag,
        get_env: &mut dyn FnMut(&str) -> Result<Value>,
    ) -> Result<Value>;
}

/// A callable registered by `registerAttributeForAutoconfiguration()`:
/// `function (ChildDefinition $definition, $attribute, \Reflector
/// $reflector)`.
pub type AttributeConfigurator =
    Arc<dyn Fn(&Definition, &AttributeMetadata, &Reflector) -> Result<()> + Send + Sync>;

/// The third parameter of an attribute configurator, which upstream's
/// `AttributeAutoconfigurationPass` reflects to choose the reflectors the
/// configurator is called with.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum AttributeConfiguratorReflector {
    /// Two parameters or fewer: called for class attributes.
    #[default]
    None,
    /// The third parameter's declared types (`ReflectionClass`,
    /// `ReflectionMethod`, `ReflectionProperty`, `ReflectionParameter`,
    /// `Reflector`).
    Typed(Vec<String>),
    /// An untyped third parameter: its name, and the callable's file and
    /// start line.
    Untyped {
        name: String,
        file: String,
        line: i64,
    },
}

fn configurator_key(configurator: &AttributeConfigurator) -> usize {
    Arc::as_ptr(configurator).cast::<()>() as usize
}

/// The target of `setAlias()`: an id or an `Alias`.
#[derive(Debug, Clone)]
pub enum AliasTarget {
    Id(String),
    Alias(Alias),
}

impl From<&str> for AliasTarget {
    fn from(id: &str) -> Self {
        Self::Id(id.to_string())
    }
}

impl From<String> for AliasTarget {
    fn from(id: String) -> Self {
        Self::Id(id)
    }
}

impl From<Alias> for AliasTarget {
    fn from(alias: Alias) -> Self {
        Self::Alias(alias)
    }
}

impl From<&Alias> for AliasTarget {
    fn from(alias: &Alias) -> Self {
        Self::Alias(alias.clone())
    }
}

/// A shared resource, as `getResources()` lists them.
pub type Resource = Arc<dyn ResourceInterface>;

/// The services created during one `get()` (`$inlineServices`).
type InlineServices = IdMap<Value>;

fn config_error(e: &symfony_config::Error) -> Error {
    Error::new(
        ErrorKind::Other(e.class().name().to_string()),
        String::from_utf8_lossy(e.get_message()).into_owned(),
    )
}

fn object_creation_error(what: &str) -> Error {
    Error::logic(format!(
        "{what} needs PHP object creation, which this build-time port does not do; the Rust emitter replaces it."
    ))
}

/// `'.' !== ($id[0] ?? '-')`.
fn is_dotted(id: &str) -> bool {
    id.starts_with('.')
}

/// The service-id check of `setDefinition()` and `setAlias()`: empty,
/// ending in `\`, or holding NUL, CR, LF or `'`.
fn is_invalid_id(id: &str) -> bool {
    id.is_empty() || id.ends_with('\\') || id.contains(['\0', '\r', '\n', '\''])
}

fn push_unique(list: &mut Vec<String>, item: &str) {
    if !list.iter().any(|i| i == item) {
        list.push(item.to_string());
    }
}

/// `ContainerBuilder`.
pub struct ContainerBuilder {
    // Container
    parameter_bag: ParameterBag,
    services: IdMap<Value>,
    privates: IdMap<Value>,
    loading: Vec<String>,
    resolving: Vec<String>,
    env_cache: IdMap<Value>,
    compiled: bool,

    // ContainerBuilder
    extensions: IdMap<Arc<dyn ExtensionInterface>>,
    definitions: IdMap<Definition>,
    alias_definitions: IdMap<Alias>,
    resources: IdMap<Resource>,
    extension_configs: IdMap<Vec<PhpArray>>,
    compiler: Compiler,
    track_resources: bool,
    used_tags: Vec<String>,
    env_placeholders: EnvPlaceholderMap,
    env_counters: IdMap<i64>,
    autoconfigured_instanceof: IdMap<Definition>,
    autoconfigured_attributes: IdMap<Vec<AttributeConfigurator>>,
    removed_ids: IdMap<bool>,
    removed_binding_ids: Vec<i64>,
    class_reflectors: HashMap<String, Option<Arc<ClassMetadata>>>,
    classes: Arc<ClassRegistry>,
    env_var_resolver: Option<Arc<dyn EnvVarResolver>>,
    /// `MergeExtensionConfigurationContainerBuilder`'s `$extensionClass`.
    merge_extension_class: Option<String>,
    /// The third parameter of each attribute configurator, by its address.
    attribute_configurator_reflectors: HashMap<usize, AttributeConfiguratorReflector>,
}

impl std::fmt::Debug for ContainerBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContainerBuilder")
            .field("definitions", &self.definitions)
            .field("aliases", &self.alias_definitions)
            .field("parameters", self.parameter_bag.all())
            .field("compiled", &self.compiled)
            .finish_non_exhaustive()
    }
}

impl Default for ContainerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerBuilder {
    /// `new ContainerBuilder()`: with an `EnvPlaceholderParameterBag`.
    pub fn new() -> Self {
        let mut bag = ParameterBag::default();
        if let Ok(env_bag) = ParameterBag::env_placeholder(&PhpArray::new()) {
            bag = env_bag;
        }
        Self::with_parameter_bag(bag)
    }

    /// `new ContainerBuilder($parameterBag)`.
    pub fn with_parameter_bag(parameter_bag: ParameterBag) -> Self {
        let mut builder = Self {
            parameter_bag,
            services: IdMap::new(),
            privates: IdMap::new(),
            loading: Vec::new(),
            resolving: Vec::new(),
            env_cache: IdMap::new(),
            compiled: false,
            extensions: IdMap::new(),
            definitions: IdMap::new(),
            alias_definitions: IdMap::new(),
            resources: IdMap::new(),
            extension_configs: IdMap::new(),
            compiler: Compiler::new(),
            track_resources: true,
            used_tags: Vec::new(),
            env_placeholders: EnvPlaceholderMap::new(),
            env_counters: IdMap::new(),
            autoconfigured_instanceof: IdMap::new(),
            autoconfigured_attributes: IdMap::new(),
            removed_ids: IdMap::new(),
            removed_binding_ids: Vec::new(),
            class_reflectors: HashMap::new(),
            classes: Arc::new(ClassRegistry::new()),
            env_var_resolver: None,
            merge_extension_class: None,
            attribute_configurator_reflectors: HashMap::new(),
        };
        let container = Definition::with_class(CONTAINER_INTERFACE);
        container.set_synthetic(true).set_public(true);
        builder.definitions.insert("service_container", container);
        builder
    }

    /// `new MergeExtensionConfigurationContainerBuilder($extension,
    /// $parameterBag)`: the container an extension loads into, which
    /// refuses the methods that would have no effect from an extension
    /// (`registerExtension()` is not refused: it cannot fail here).
    pub fn for_extension(extension_class: &str, parameter_bag: ParameterBag) -> Self {
        let mut builder = Self::with_parameter_bag(parameter_bag);
        builder.merge_extension_class = Some(extension_class.to_string());
        builder
    }

    /// `$container instanceof MergeExtensionConfigurationContainerBuilder`:
    /// the extension class it was created for.
    pub fn merge_extension_class(&self) -> Option<&str> {
        self.merge_extension_class.as_deref()
    }

    // ----- build-time inputs of this port -----

    /// The class metadata that stands for PHP's reflection.
    pub fn class_registry(&self) -> Arc<ClassRegistry> {
        self.classes.clone()
    }

    /// Sets the class metadata.
    pub fn set_class_registry(&mut self, classes: Arc<ClassRegistry>) {
        self.classes = classes;
        self.class_reflectors.clear();
    }

    /// Sets what reads env vars for `getEnv()`.
    pub fn set_env_var_resolver(&mut self, resolver: Arc<dyn EnvVarResolver>) {
        self.env_var_resolver = Some(resolver);
    }

    // ----- Container -----

    /// `isCompiled()`.
    pub fn is_compiled(&self) -> bool {
        self.compiled
    }

    /// `getParameterBag()`.
    pub fn get_parameter_bag(&self) -> &ParameterBag {
        &self.parameter_bag
    }

    /// `getParameterBag()`, to change it.
    pub fn get_parameter_bag_mut(&mut self) -> &mut ParameterBag {
        &mut self.parameter_bag
    }

    /// `getParameter($name)`.
    pub fn get_parameter(&self, name: &str) -> Result<Value> {
        self.parameter_bag.get(name)
    }

    /// `hasParameter($name)`.
    pub fn has_parameter(&self, name: &str) -> bool {
        self.parameter_bag.has(name)
    }

    /// `setParameter($name, $value)`.
    pub fn set_parameter(&mut self, name: &str, value: impl Into<Value>) -> Result<()> {
        self.parameter_bag.set(name, value.into())
    }

    /// `initialized($id)`.
    pub fn initialized(&self, id: &str) -> bool {
        id != "service_container" && self.services.contains_key(id)
    }

    /// `Container::has($id)`.
    fn container_has(&self, id: &str) -> bool {
        self.services.contains_key(id) || id == "service_container"
    }

    /// `Container::get($id, $invalidBehavior)` with `make()`: the builder
    /// has no factories, file map or method map.
    fn container_get(
        &mut self,
        id: &str,
        invalid_behavior: InvalidBehavior,
    ) -> Result<Option<Value>> {
        if let Some(service) = self.services.get(id) {
            return Ok(Some(service.clone()));
        }
        if id == "service_container" {
            return Err(object_creation_error(
                "Getting \"service_container\" (the container itself)",
            ));
        }
        if self.loading.iter().any(|l| l == id) {
            let mut path = self.loading.clone();
            path.push(id.to_string());
            return Err(Error::service_circular_reference(id, path));
        }
        if invalid_behavior == InvalidBehavior::Exception {
            if id.is_empty() {
                return Err(Error::service_not_found(id, None, Vec::new(), None));
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
            for known in self.get_service_ids() {
                if known.is_empty() || known.starts_with('.') {
                    continue;
                }
                let lev = php_value::string::levenshtein(id.as_bytes(), known.as_bytes());
                #[expect(clippy::cast_precision_loss, reason = "PHP's `strlen($id) / 3`")]
                let limit = id.len() as f64 / 3.0;
                #[expect(clippy::cast_precision_loss, reason = "PHP's int and float comparison")]
                if lev as f64 <= limit || known.contains(id) {
                    alternatives.push(known);
                }
            }
            return Err(Error::service_not_found(id, None, alternatives, None));
        }
        Ok(None)
    }

    // ----- resources -----

    /// `setResourceTracking($track)`.
    pub fn set_resource_tracking(&mut self, track: bool) {
        self.track_resources = track;
    }

    /// `isTrackingResources()`.
    pub fn is_tracking_resources(&self) -> bool {
        self.track_resources
    }

    /// `registerExtension($extension)`.
    pub fn register_extension(&mut self, extension: Arc<dyn ExtensionInterface>) {
        self.extensions.insert(extension.get_alias(), extension);
    }

    /// `getExtension($name)`.
    pub fn get_extension(&self, name: &str) -> Result<Arc<dyn ExtensionInterface>> {
        self.extensions.get(name).cloned().ok_or_else(|| {
            Error::logic(format!("Container extension \"{name}\" is not registered."))
        })
    }

    /// `getExtensions()`.
    pub fn get_extensions(&self) -> &IdMap<Arc<dyn ExtensionInterface>> {
        &self.extensions
    }

    /// `hasExtension($name)`.
    pub fn has_extension(&self, name: &str) -> bool {
        self.extensions.contains_key(name)
    }

    /// `getResources()`.
    pub fn get_resources(&self) -> Vec<Resource> {
        self.resources.values().cloned().collect()
    }

    /// `addResource($resource)`: keyed by its string form.
    pub fn add_resource(&mut self, resource: Resource) -> &mut Self {
        if !self.track_resources {
            return self;
        }
        let key = String::from_utf8_lossy(&resource.to_string()).into_owned();
        self.resources.insert(key, resource);
        self
    }

    /// `setResources($resources)`.
    pub fn set_resources(&mut self, resources: IdMap<Resource>) -> &mut Self {
        if self.track_resources {
            self.resources = resources;
        }
        self
    }

    /// `addObjectResource($object)` for the class `class`: the files of
    /// its interfaces, its class chain and its traits.
    pub fn add_object_resource(&mut self, class: &str) -> Result<&mut Self> {
        if !self.track_resources {
            return Ok(self);
        }
        let Some(metadata) = self.classes.class(class) else {
            return Ok(self);
        };
        for interface in self.classes.interface_names(&metadata) {
            if let Some(file) = self.classes.class(&interface).and_then(|i| i.file.clone())
                && path_exists(&file)
            {
                self.file_exists(&file, &TrackContents::Yes)?;
            }
        }
        let mut current = Some(metadata);
        while let Some(c) = current {
            if let Some(file) = &c.file
                && path_exists(file)
            {
                self.file_exists(file, &TrackContents::Yes)?;
            }
            for name in &c.traits {
                self.add_object_resource(name)?;
            }
            current = c.parent.as_deref().and_then(|p| self.classes.class(p));
        }
        Ok(self)
    }

    /// `getReflectionClass($class, $throw)`: the class's metadata after
    /// resolving parameters in its name; `None` for a falsy name, an
    /// internal type or a class that does not exist. A class whose
    /// ancestor is missing fails with a `ReflectionException` when
    /// `throw` is set.
    pub fn get_reflection_class(
        &mut self,
        class: Option<&str>,
        throw: bool,
    ) -> Result<Option<Arc<ClassMetadata>>> {
        let Some(class) = class else {
            return Ok(None);
        };
        let resolved = self.parameter_bag.resolve_value(&Value::from(class), &[])?;
        if !resolved.is_truthy() {
            return Ok(None);
        }
        let class = resolved
            .to_php_string()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if INTERNAL_TYPES.contains(&class.as_str()) {
            return Ok(None);
        }
        let reflector = if let Some(Some(cached)) = self.class_reflectors.get(&class) {
            Some(cached.clone())
        } else {
            match self.classes.reflect(&class) {
                Ok(found) => found,
                Err(message) if throw => {
                    return Err(Error::new(
                        ErrorKind::Other("ReflectionException".to_string()),
                        message,
                    ));
                }
                Err(_) => None,
            }
        };
        if self.track_resources {
            self.class_reflectors.insert(class, reflector.clone());
        }
        Ok(reflector)
    }

    /// `fileExists($path, $trackContents)`.
    pub fn file_exists(&mut self, path: &str, track_contents: &TrackContents) -> Result<bool> {
        let exists = path_exists(path);
        if !self.track_resources {
            return Ok(exists);
        }
        if !exists {
            self.add_resource(Arc::new(FileExistenceResource::new(path.as_bytes())));
            return Ok(false);
        }
        let is_dir = php_value::path::to_path(path.as_bytes()).is_some_and(|p| p.is_dir());
        if is_dir {
            if track_contents.is_truthy() {
                let pattern = match track_contents {
                    TrackContents::Pattern(p) => Some(p.as_bytes()),
                    _ => None,
                };
                let resource = DirectoryResource::new(path.as_bytes(), pattern)
                    .map_err(|e| config_error(&e))?;
                self.add_resource(Arc::new(resource));
            } else {
                let resource =
                    GlobResource::new(path.as_bytes(), b"/*", false, false, PhpArray::new())
                        .map_err(|e| config_error(&e))?;
                self.add_resource(Arc::new(resource));
            }
        } else if track_contents.is_truthy() {
            let resource = FileResource::new(path.as_bytes()).map_err(|e| config_error(&e))?;
            self.add_resource(Arc::new(resource));
        }
        Ok(true)
    }

    // ----- extensions and passes -----

    /// `loadFromExtension($extension, $values)`.
    pub fn load_from_extension(
        &mut self,
        extension: &str,
        values: Option<PhpArray>,
    ) -> Result<&mut Self> {
        if self.compiled {
            return Err(Error::bad_method_call(
                "Cannot load from an extension on a compiled container.",
            ));
        }
        let namespace = self.get_extension(extension)?.get_alias();
        self.extension_configs
            .get_or_insert_with(&namespace, Vec::new)
            .push(values.unwrap_or_default());
        Ok(self)
    }

    /// `addCompilerPass($pass, $type, $priority)`.
    pub fn add_compiler_pass(
        &mut self,
        pass: Pass,
        type_: PassType,
        priority: i64,
    ) -> Result<&mut Self> {
        let class = pass.class();
        if let Some(extension_class) = &self.merge_extension_class {
            return Err(Error::logic(format!(
                "You cannot add compiler pass \"{class}\" from extension \"{extension_class}\". Compiler passes must be registered before the container is compiled."
            )));
        }
        self.compiler.add_pass(pass, type_, priority);
        self.add_object_resource(&class)?;
        Ok(self)
    }

    /// `getCompilerPassConfig()`.
    pub fn get_compiler_pass_config(&mut self) -> &mut PassConfig {
        self.compiler.get_pass_config_mut()
    }

    /// `getCompiler()`.
    pub fn get_compiler(&mut self) -> &mut Compiler {
        &mut self.compiler
    }

    // ----- services -----

    /// `set($id, $service)`: `None` unsets it.
    pub fn set(&mut self, id: &str, service: Option<Value>) -> Result<()> {
        if self.compiled
            && let Some(definition) = self.definitions.get(id)
            && !definition.is_synthetic()
        {
            // setting a synthetic service on a compiled container is alright
            return Err(Error::bad_method_call(format!(
                "Setting service \"{id}\" for an unknown or non-synthetic service definition on a compiled container is not allowed."
            )));
        }
        self.definitions.remove(id);
        self.alias_definitions.remove(id);
        self.removed_ids.remove(id);

        // Container::set()
        if id == "service_container" {
            return Err(Error::invalid_argument(
                "You cannot set service \"service_container\".",
            ));
        }
        match service {
            None => {
                self.services.remove(id);
            }
            Some(service) => {
                self.services.insert(id, service);
            }
        }
        Ok(())
    }

    /// `removeDefinition($id)`.
    pub fn remove_definition(&mut self, id: &str) {
        if self.definitions.remove(id).is_some() && !is_dotted(id) {
            self.removed_ids.insert(id, true);
        }
    }

    /// `has($id)`.
    pub fn has(&self, id: &str) -> bool {
        self.definitions.contains_key(id)
            || self.alias_definitions.contains_key(id)
            || self.container_has(id)
    }

    /// `get($id, $invalidBehavior)`.
    pub fn get(&mut self, id: &str, invalid_behavior: InvalidBehavior) -> Result<Option<Value>> {
        if self.compiled && self.removed_ids.contains_key(id) {
            return if invalid_behavior.as_int() <= InvalidBehavior::Exception.as_int() {
                self.container_get(id, InvalidBehavior::Exception)
            } else {
                Ok(None)
            };
        }
        self.do_get(id, invalid_behavior, None, false)
    }

    fn do_get(
        &mut self,
        id: &str,
        invalid_behavior: InvalidBehavior,
        inline_services: Option<&mut InlineServices>,
        is_constructor_argument: bool,
    ) -> Result<Option<Value>> {
        let mut fresh = InlineServices::new();
        let (inline_services, is_constructor_argument) = match inline_services {
            Some(inline) => (inline, is_constructor_argument),
            None => (&mut fresh, true),
        };
        if let Some(service) = inline_services.get(id) {
            return Ok(Some(service.clone()));
        }
        let found = if invalid_behavior == InvalidBehavior::IgnoreUninitialized {
            match self.privates.get(id) {
                Some(service) => Ok(Some(service.clone())),
                None => self.container_get(id, invalid_behavior),
            }
        } else {
            match self.privates.get(id) {
                Some(service) => Ok(Some(service.clone())),
                None => self.container_get(id, InvalidBehavior::Null),
            }
        };
        match found {
            Ok(Some(service)) => return Ok(Some(service)),
            Ok(None) if invalid_behavior == InvalidBehavior::IgnoreUninitialized => {
                return Ok(None);
            }
            Ok(None) => {}
            Err(e) if e.is(&ErrorKind::ServiceCircularReference) && !is_constructor_argument => {}
            Err(e) => return Err(e),
        }

        if !self.definitions.contains_key(id)
            && let Some(alias) = self.alias_definitions.get(id).cloned()
        {
            if let Some(deprecation) = alias.deprecation(id) {
                trigger_deprecation(
                    deprecation.package.as_bytes(),
                    deprecation.version.as_bytes(),
                    deprecation.message.as_bytes(),
                );
            }
            return self.do_get(
                &alias.id(),
                invalid_behavior,
                Some(inline_services),
                is_constructor_argument,
            );
        }

        let definition = match self.get_definition(id) {
            Ok(definition) => definition,
            Err(e) => {
                if invalid_behavior.as_int() > InvalidBehavior::Exception.as_int() {
                    return Ok(None);
                }
                return Err(e);
            }
        };

        if definition.has_errors()
            && let Some(first) = definition.errors().into_iter().next()
        {
            return Err(Error::runtime(first));
        }

        if is_constructor_argument {
            self.loading.push(id.to_string());
        }
        let result = self.create_service(&definition, inline_services, Some(id));
        if is_constructor_argument && let Some(at) = self.loading.iter().rposition(|l| l == id) {
            self.loading.remove(at);
        }
        result.map(Some)
    }

    /// `createService()` up to creating the object.
    #[expect(
        clippy::unused_self,
        reason = "upstream's method; the part that reads the builder creates the object"
    )]
    fn create_service(
        &mut self,
        definition: &Definition,
        inline_services: &mut InlineServices,
        id: Option<&str>,
    ) -> Result<Value> {
        // `"\0".spl_object_id($definition)`
        let inline_key = match definition.to_value() {
            Value::Object(object) => format!("\0{:p}", Arc::as_ptr(&object).cast::<()>()),
            _ => String::from("\0"),
        };
        if id.is_none()
            && let Some(service) = inline_services.get(&inline_key)
        {
            return Ok(service.clone());
        }
        let shown = id.unwrap_or("");
        if definition.is_child() {
            return Err(Error::runtime(format!(
                "Constructing service \"{shown}\" from a parent definition is not supported at build time."
            )));
        }
        if definition.is_synthetic() {
            return Err(Error::runtime(format!(
                "You have requested a synthetic service (\"{shown}\"). The DIC does not know how to construct this service."
            )));
        }
        if let Some(deprecation) = definition.deprecation(shown) {
            trigger_deprecation(
                deprecation.package.as_bytes(),
                deprecation.version.as_bytes(),
                deprecation.message.as_bytes(),
            );
        }
        Err(object_creation_error(&format!(
            "Creating the service \"{shown}\""
        )))
    }

    /// `resolveServices($value)`.
    pub fn resolve_services(&mut self, value: &Value) -> Result<Value> {
        let mut inline = InlineServices::new();
        self.do_resolve_services(value, &mut inline, false)
    }

    fn do_resolve_services(
        &mut self,
        value: &Value,
        inline_services: &mut InlineServices,
        is_constructor_argument: bool,
    ) -> Result<Value> {
        if let Value::Array(array) = value {
            let mut out = array.clone();
            for (k, v) in array.iter() {
                let resolved =
                    self.do_resolve_services(v, inline_services, is_constructor_argument)?;
                out.insert(k.clone(), resolved);
            }
            return Ok(Value::Array(out));
        }
        if ServiceClosureArgument::from_value(value).is_some() {
            return Err(object_creation_error(
                "Resolving a ServiceClosureArgument (a closure)",
            ));
        }
        if EnvClosureArgument::from_value(value).is_some() {
            return Err(object_creation_error(
                "Resolving an EnvClosureArgument (an EnvClosure)",
            ));
        }
        if IteratorArgument::from_value(value).is_some() {
            return Err(object_creation_error(
                "Resolving an IteratorArgument (a RewindableGenerator)",
            ));
        }
        if ServiceLocatorArgument::from_value(value).is_some() {
            return Err(object_creation_error(
                "Resolving a ServiceLocatorArgument (a ServiceLocator)",
            ));
        }
        if let Some(reference) = Reference::from_value(value) {
            return Ok(self
                .do_get(
                    reference.id(),
                    reference.invalid_behavior(),
                    Some(inline_services),
                    is_constructor_argument,
                )?
                .unwrap_or_default());
        }
        if let Some(definition) = Definition::from_value(value) {
            return self.create_service(&definition, inline_services, None);
        }
        if let Some(parameter) = Parameter::from_value(value) {
            return self.get_parameter(parameter.id());
        }
        if let Some(argument) = AbstractArgument::from_value(value) {
            return Err(Error::runtime(argument.text_with_context()));
        }
        Ok(value.clone())
    }

    /// `merge($container)`.
    pub fn merge(&mut self, container: &ContainerBuilder) -> Result<()> {
        if self.compiled {
            return Err(Error::bad_method_call(
                "Cannot merge on a compiled container.",
            ));
        }
        for (id, definition) in container.get_definitions().iter() {
            if !definition.has_tag("container.excluded") || !self.has(id) {
                self.set_definition(id, definition.clone())?;
            }
        }
        let aliases: Vec<(String, AliasTarget)> = container
            .get_aliases()
            .iter()
            .map(|(id, alias)| (id.to_string(), AliasTarget::Alias(alias.clone())))
            .collect();
        self.add_aliases(aliases)?;
        let other_bag = container.get_parameter_bag();
        self.parameter_bag.add(other_bag.all())?;
        for (name, deprecated) in other_bag.all_deprecated() {
            self.parameter_bag.deprecate(
                name,
                &deprecated.package,
                &deprecated.version,
                Some(&deprecated.message),
            )?;
        }
        for (name, message) in other_bag.all_non_empty() {
            self.parameter_bag.cannot_be_empty(name, message)?;
        }

        if self.track_resources {
            for resource in container.get_resources() {
                self.add_resource(resource);
            }
        }

        let names: Vec<String> = self.extensions.keys().map(String::from).collect();
        for name in names {
            let other = container
                .extension_configs
                .get(&name)
                .cloned()
                .unwrap_or_default();
            self.extension_configs
                .get_or_insert_with(&name, Vec::new)
                .extend(other);
        }

        let env_placeholders =
            if self.parameter_bag.is_env_placeholder() && other_bag.is_env_placeholder() {
                let placeholders = other_bag.get_env_placeholders();
                self.parameter_bag.merge_env_placeholders(other_bag);
                placeholders
            } else {
                EnvPlaceholderMap::new()
            };

        for (env, count) in container.env_counters.iter() {
            if *count == 0 && !env_placeholders.contains_key(env) {
                continue;
            }
            *self.env_counters.get_or_insert_with(env, || 0) += count;
        }

        for (interface, child_definition) in container.get_autoconfigured_instanceof().iter() {
            if self.autoconfigured_instanceof.contains_key(interface) {
                return Err(Error::invalid_argument(format!(
                    "\"{interface}\" has already been autoconfigured and merge() does not support merging autoconfiguration for the same class/interface."
                )));
            }
            self.autoconfigured_instanceof
                .insert(interface, child_definition.clone());
        }

        for (attribute, configurators) in container.get_attribute_autoconfigurators().iter() {
            self.autoconfigured_attributes
                .get_or_insert_with(attribute, Vec::new)
                .extend(configurators.iter().cloned());
        }
        for (key, reflector) in &container.attribute_configurator_reflectors {
            self.attribute_configurator_reflectors
                .entry(*key)
                .or_insert_with(|| reflector.clone());
        }
        Ok(())
    }

    /// `getExtensionConfig($name)` (an unknown name gets an empty list).
    pub fn get_extension_config(&mut self, name: &str) -> Vec<PhpArray> {
        self.extension_configs
            .get_or_insert_with(name, Vec::new)
            .clone()
    }

    /// `prependExtensionConfig($name, $config)`.
    pub fn prepend_extension_config(&mut self, name: &str, config: PhpArray) {
        self.extension_configs
            .get_or_insert_with(name, Vec::new)
            .insert(0, config);
    }

    /// `deprecateParameter($name, $package, $version, $message)`;
    /// upstream's default message when `None`.
    pub fn deprecate_parameter(
        &mut self,
        name: &str,
        package: &str,
        version: &str,
        message: Option<&str>,
    ) -> Result<()> {
        self.parameter_bag
            .deprecate(name, package, version, message)
    }

    /// `parameterCannotBeEmpty($name, $message)`.
    pub fn parameter_cannot_be_empty(&mut self, name: &str, message: &str) -> Result<()> {
        self.parameter_bag.cannot_be_empty(name, message)
    }

    /// `compile($resolveEnvPlaceholders)`.
    pub fn compile(&mut self, resolve_env_placeholders: bool) -> Result<()> {
        if let Some(extension_class) = &self.merge_extension_class {
            return Err(Error::logic(format!(
                "Cannot compile the container in extension \"{extension_class}\"."
            )));
        }
        if self.track_resources {
            for pass in self.compiler.get_pass_config().get_passes() {
                self.add_object_resource(&pass.class())?;
            }
        }
        if resolve_env_placeholders && self.parameter_bag.is_env_placeholder() {
            self.compiler.add_pass(
                Pass::new(ResolveEnvPlaceholdersPass::default()),
                PassType::AfterRemoving,
                -1000,
            );
        }

        let passes = self.compiler.get_pass_config().get_passes();
        let result = Compiler::run(&passes, self);
        self.compiler.get_service_reference_graph_mut().clear();
        result?;

        if self.track_resources {
            let lazy: Vec<Option<String>> = self
                .definitions
                .values()
                .filter(|d| d.is_lazy())
                .map(Definition::class)
                .collect();
            for class in lazy {
                self.get_reflection_class(class.as_deref(), true)?;
            }
        }

        self.extension_configs.clear();

        if self.parameter_bag.is_env_placeholder() {
            let env_placeholders = self.parameter_bag.get_env_placeholders();
            if resolve_env_placeholders {
                let escaped = escape_parameters(self.parameter_bag.all());
                let resolved = self.resolve_env_placeholders(
                    Value::Array(escaped),
                    &EnvFormat::Resolve,
                    None,
                )?;
                let parameters = match resolved {
                    Value::Array(a) => a,
                    _ => PhpArray::new(),
                };
                self.parameter_bag = ParameterBag::new(&parameters)?;
            }
            self.env_placeholders = env_placeholders;
        }

        // Container::compile()
        self.parameter_bag.resolve()?;
        self.parameter_bag = ParameterBag::frozen(
            self.parameter_bag.all().clone(),
            self.parameter_bag.all_deprecated().to_vec(),
            self.parameter_bag.all_non_empty().to_vec(),
        );
        self.compiled = true;

        let mut removed = Vec::new();
        for (id, definition) in self.definitions.iter() {
            if !is_dotted(id) && definition.is_private() {
                removed.push(id.to_string());
            }
        }
        for (id, alias) in self.alias_definitions.iter() {
            if !self.definitions.contains_key(id) && !is_dotted(id) && alias.is_private() {
                removed.push(id.to_string());
            }
        }
        for id in removed {
            self.removed_ids.insert(id, true);
        }
        Ok(())
    }

    /// `getServiceIds()`.
    pub fn get_service_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        for id in self
            .definitions
            .keys()
            .chain(self.alias_definitions.keys())
            .chain(std::iter::once("service_container"))
            .chain(self.services.keys())
        {
            push_unique(&mut ids, id);
        }
        ids
    }

    /// `getRemovedIds()`.
    pub fn get_removed_ids(&self) -> &IdMap<bool> {
        &self.removed_ids
    }

    // ----- aliases -----

    /// `addAliases($aliases)`.
    pub fn add_aliases<I, T>(&mut self, aliases: I) -> Result<()>
    where
        I: IntoIterator<Item = (String, T)>,
        T: Into<AliasTarget>,
    {
        for (alias, id) in aliases {
            self.set_alias(&alias, id)?;
        }
        Ok(())
    }

    /// `setAliases($aliases)`.
    pub fn set_aliases<I, T>(&mut self, aliases: I) -> Result<()>
    where
        I: IntoIterator<Item = (String, T)>,
        T: Into<AliasTarget>,
    {
        self.alias_definitions.clear();
        self.add_aliases(aliases)
    }

    /// `setAlias($alias, $id)`.
    pub fn set_alias(&mut self, alias: &str, id: impl Into<AliasTarget>) -> Result<Alias> {
        if is_invalid_id(alias) {
            return Err(Error::invalid_argument(format!(
                "Invalid alias id: \"{alias}\"."
            )));
        }
        let id = match id.into() {
            AliasTarget::Id(id) => Alias::new(id, false),
            AliasTarget::Alias(alias) => alias,
        };
        if alias == id.id() {
            return Err(Error::invalid_argument(format!(
                "An alias cannot reference itself, got a circular reference on \"{alias}\"."
            )));
        }
        self.definitions.remove(alias);
        self.removed_ids.remove(alias);
        self.alias_definitions.insert(alias, id.clone());
        Ok(id)
    }

    /// `removeAlias($alias)`.
    pub fn remove_alias(&mut self, alias: &str) {
        if self.alias_definitions.remove(alias).is_some() && !is_dotted(alias) {
            self.removed_ids.insert(alias, true);
        }
    }

    /// `hasAlias($id)`.
    pub fn has_alias(&self, id: &str) -> bool {
        self.alias_definitions.contains_key(id)
    }

    /// `getAliases()`.
    pub fn get_aliases(&self) -> &IdMap<Alias> {
        &self.alias_definitions
    }

    /// `getAlias($id)`.
    pub fn get_alias(&self, id: &str) -> Result<Alias> {
        self.alias_definitions.get(id).cloned().ok_or_else(|| {
            Error::invalid_argument(format!("The service alias \"{id}\" does not exist."))
        })
    }

    // ----- definitions -----

    /// `register($id, $class)`.
    pub fn register(&mut self, id: &str, class: Option<&str>) -> Result<Definition> {
        self.set_definition(id, Definition::new(class, PhpArray::new()))
    }

    /// `registerChild($id, $parent)`.
    pub fn register_child(&mut self, id: &str, parent: &str) -> Result<Definition> {
        self.set_definition(id, Definition::child(parent))
    }

    /// `autowire($id, $class)`.
    pub fn autowire(&mut self, id: &str, class: Option<&str>) -> Result<Definition> {
        let definition = Definition::new(class, PhpArray::new());
        definition.set_autowired(true);
        self.set_definition(id, definition)
    }

    /// `addDefinitions($definitions)`.
    pub fn add_definitions<I>(&mut self, definitions: I) -> Result<()>
    where
        I: IntoIterator<Item = (String, Definition)>,
    {
        for (id, definition) in definitions {
            self.set_definition(&id, definition)?;
        }
        Ok(())
    }

    /// `setDefinitions($definitions)`.
    pub fn set_definitions<I>(&mut self, definitions: I) -> Result<()>
    where
        I: IntoIterator<Item = (String, Definition)>,
    {
        self.definitions.clear();
        self.add_definitions(definitions)
    }

    /// `getDefinitions()`.
    pub fn get_definitions(&self) -> &IdMap<Definition> {
        &self.definitions
    }

    /// `setDefinition($id, $definition)`.
    pub fn set_definition(&mut self, id: &str, definition: Definition) -> Result<Definition> {
        if self.compiled {
            return Err(Error::bad_method_call(
                "Adding definition to a compiled container is not allowed.",
            ));
        }
        if is_invalid_id(id) {
            return Err(Error::invalid_argument(format!(
                "Invalid service id: \"{id}\"."
            )));
        }
        self.alias_definitions.remove(id);
        self.removed_ids.remove(id);
        self.definitions.insert(id, definition.clone());
        Ok(definition)
    }

    /// `hasDefinition($id)`.
    pub fn has_definition(&self, id: &str) -> bool {
        self.definitions.contains_key(id)
    }

    /// `getDefinition($id)`.
    pub fn get_definition(&self, id: &str) -> Result<Definition> {
        self.definitions
            .get(id)
            .cloned()
            .ok_or_else(|| Error::service_not_found(id, None, Vec::new(), None))
    }

    /// `findDefinition($id)`: through aliases, failing on a cycle.
    pub fn find_definition(&self, id: &str) -> Result<Definition> {
        let mut id = id.to_string();
        let mut seen: Vec<String> = Vec::new();
        while let Some(alias) = self.alias_definitions.get(&id) {
            id = alias.id();
            if let Some(at) = seen.iter().position(|s| *s == id) {
                let mut path = seen[at..].to_vec();
                path.push(id.clone());
                return Err(Error::service_circular_reference(id, path));
            }
            seen.push(id.clone());
        }
        self.get_definition(&id)
    }

    // ----- tags -----

    /// `findTaggedServiceIds($name, $throwOnAbstract)`.
    pub fn find_tagged_service_ids(
        &mut self,
        name: &str,
        throw_on_abstract: bool,
    ) -> Result<IdMap<PhpArray>> {
        self.used_tags.push(name.to_string());
        let mut tags = IdMap::new();
        for (id, definition) in self.definitions.iter() {
            if definition.has_tag(name) && !definition.has_tag("container.excluded") {
                if throw_on_abstract && definition.is_abstract() {
                    return Err(Error::invalid_argument(format!(
                        "The service \"{id}\" tagged \"{name}\" must not be abstract."
                    )));
                }
                tags.insert(id, definition.tag(name));
            }
        }
        Ok(tags)
    }

    /// `findTaggedResourceIds($tagName, $throwOnAbstract)`.
    pub fn find_tagged_resource_ids(
        &mut self,
        tag_name: &str,
        throw_on_abstract: bool,
    ) -> Result<IdMap<PhpArray>> {
        self.used_tags.push(tag_name.to_string());
        let mut tags = IdMap::new();
        let definitions: Vec<(String, Definition)> = self
            .definitions
            .iter()
            .map(|(id, d)| (id.to_string(), d.clone()))
            .collect();
        for (id, definition) in definitions {
            if !definition.has_tag(tag_name) {
                continue;
            }
            if !definition.has_tag("container.excluded") {
                return Err(Error::invalid_argument(format!(
                    "The resource \"{id}\" tagged \"{tag_name}\" is missing the \"container.excluded\" tag; did you mean to use \"resource_tags\" instead of \"tags\"?"
                )));
            }
            let class = match definition.class() {
                Some(class) => self.parameter_bag.resolve_value(&Value::from(class), &[])?,
                None => Value::Null,
            };
            let class_name = class
                .to_php_string()
                .map(|s| s.to_string_lossy().into_owned());
            if class.is_truthy() && throw_on_abstract && definition.is_abstract() {
                let r = self.get_reflection_class(class_name.as_deref(), false)?;
                if r.is_some_and(|r| r.is_abstract || r.is_interface()) {
                    // an abstract class or interface matches autoconfiguration rules on behalf
                    // of its subtypes but is not a resource itself: skip it, don't throw
                    continue;
                }
            }
            if !class.is_truthy() || throw_on_abstract && definition.is_abstract() {
                return Err(Error::invalid_argument(format!(
                    "The resource \"{id}\" tagged \"{tag_name}\" must have a class and not be abstract."
                )));
            }
            if definition.class() != class_name {
                definition.set_class(class_name.as_deref());
            }
            tags.insert(id, definition.tag(tag_name));
        }
        Ok(tags)
    }

    /// `findTags()`.
    pub fn find_tags(&self) -> Vec<String> {
        let mut tags = Vec::new();
        for definition in self.definitions.values() {
            for key in definition.tags().keys() {
                push_unique(&mut tags, &String::from_utf8_lossy(&key.bytes()));
            }
        }
        tags
    }

    /// `findUnusedTags()`.
    pub fn find_unused_tags(&self) -> Vec<String> {
        self.find_tags()
            .into_iter()
            .filter(|t| !self.used_tags.contains(t))
            .collect()
    }

    // ----- autoconfiguration -----

    /// `registerForAutoconfiguration($interface)`: the same
    /// `ChildDefinition('')` on each call.
    pub fn register_for_autoconfiguration(&mut self, interface: &str) -> Definition {
        self.autoconfigured_instanceof
            .get_or_insert_with(interface, || Definition::child(""))
            .clone()
    }

    /// `registerAttributeForAutoconfiguration($attributeClass,
    /// $configurator)`.
    pub fn register_attribute_for_autoconfiguration(
        &mut self,
        attribute_class: &str,
        configurator: AttributeConfigurator,
    ) {
        self.autoconfigured_attributes
            .get_or_insert_with(attribute_class, Vec::new)
            .push(configurator);
    }

    /// `registerAliasForArgument($id, $type, $name, $target)`.
    pub fn register_alias_for_argument(
        &mut self,
        id: &str,
        type_: &str,
        name: Option<&str>,
        target: Option<&str>,
    ) -> Result<Alias> {
        let name = name.unwrap_or(id);
        let parsed_name = target_parsed_name(name);
        let target = target.unwrap_or(name);

        if !parsed_name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f)
        {
            let for_service = if id == name {
                id.to_string()
            } else {
                format!(" for service \"{id}\"")
            };
            let format = format!(
                "Invalid argument name \"%s\"{for_service}: the first character must be a letter."
            );
            let message = symfony_config::php::sprintf(format.as_bytes(), &[Value::from(name)])
                .map_err(|e| config_error(&e))?;
            return Err(Error::invalid_argument(
                String::from_utf8_lossy(&message).into_owned(),
            ));
        }

        if parsed_name != target {
            self.set_alias(
                &format!(".{type_} ${target}"),
                format!("{type_} ${parsed_name}"),
            )?;
        }
        self.set_alias(&format!("{type_} ${parsed_name}"), id)
    }

    /// `getAutoconfiguredInstanceof()`.
    pub fn get_autoconfigured_instanceof(&self) -> &IdMap<Definition> {
        &self.autoconfigured_instanceof
    }

    /// `getAttributeAutoconfigurators()`.
    pub fn get_attribute_autoconfigurators(&self) -> &IdMap<Vec<AttributeConfigurator>> {
        &self.autoconfigured_attributes
    }

    /// `registerAttributeForAutoconfiguration()` for a configurator with a
    /// third parameter, which upstream reflects.
    pub fn register_attribute_for_autoconfiguration_with_reflector(
        &mut self,
        attribute_class: &str,
        configurator: AttributeConfigurator,
        reflector: AttributeConfiguratorReflector,
    ) {
        self.attribute_configurator_reflectors
            .insert(configurator_key(&configurator), reflector);
        self.register_attribute_for_autoconfiguration(attribute_class, configurator);
    }

    /// The third parameter of a registered attribute configurator.
    pub fn attribute_configurator_reflector(
        &self,
        configurator: &AttributeConfigurator,
    ) -> AttributeConfiguratorReflector {
        self.attribute_configurator_reflectors
            .get(&configurator_key(configurator))
            .cloned()
            .unwrap_or_default()
    }

    // ----- env vars -----

    /// `resolveEnvPlaceholders($value, $format, $usedEnvs)`: env
    /// placeholders in strings, and in array keys and values, replaced
    /// as `format` says. The env vars found are added to `used_envs`.
    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    pub fn resolve_env_placeholders(
        &mut self,
        mut value: Value,
        format: &EnvFormat,
        mut used_envs: Option<&mut Vec<String>>,
    ) -> Result<Value> {
        if let Some(extension_class) = &self.merge_extension_class
            && *format == EnvFormat::Resolve
            && let Value::String(s) = &value
        {
            // MergeExtensionConfigurationContainerBuilder::resolveEnvPlaceholders()
            let resolved = self.parameter_bag.resolve_value(&value, &[])?;
            if self.parameter_bag.is_env_placeholder() {
                let haystack = match &resolved {
                    Value::String(r) => r.as_bytes().to_ascii_lowercase(),
                    _ => s.as_bytes().to_ascii_lowercase(),
                };
                for (env, placeholders) in self.parameter_bag.get_env_placeholders().iter() {
                    if !env.contains(':') {
                        continue;
                    }
                    for placeholder in placeholders {
                        let needle = placeholder.to_ascii_lowercase();
                        if haystack
                            .windows(needle.len().max(1))
                            .any(|w| w == needle.as_bytes())
                        {
                            return Err(Error::runtime(format!(
                                "Using a cast in \"env({env})\" is incompatible with resolution at compile time in \"{extension_class}\". The logic in the extension should be moved to a compiler pass, or an env parameter with no cast should be used instead."
                            )));
                        }
                    }
                }
            }
            value = resolved;
        }
        let mut value = if *format == EnvFormat::Resolve {
            self.parameter_bag.resolve_value(&value, &[])?
        } else {
            value
        };
        if let Some(definition) = Definition::from_value(&value)
            && let Some(object) = definition.to_value().as_object()
        {
            value = Value::Array(object.properties());
        }

        if let Value::Array(array) = &value {
            let mut result = PhpArray::new();
            for (k, v) in array.iter() {
                let key = match k {
                    Key::Str(s) => {
                        let resolved = self.resolve_env_placeholders(
                            Value::String(s.clone()),
                            format,
                            used_envs.as_deref_mut(),
                        )?;
                        Key::from_value(&resolved).ok_or_else(|| {
                            Error::new(
                                ErrorKind::Other("TypeError".to_string()),
                                format!(
                                    "Cannot access offset of type {} on array",
                                    resolved.debug_type()
                                ),
                            )
                        })?
                    }
                    Key::Int(_) => k.clone(),
                };
                let v =
                    self.resolve_env_placeholders(v.clone(), format, used_envs.as_deref_mut())?;
                result.insert(key, v);
            }
            return Ok(Value::Array(result));
        }

        let Value::String(text) = &value else {
            return Ok(value);
        };
        if text.len() < 38 || !contains_ignore_case(text.as_bytes(), b"env_") {
            return Ok(value);
        }
        let env_placeholders = if self.parameter_bag.is_env_placeholder() {
            self.parameter_bag.get_env_placeholders()
        } else {
            self.env_placeholders.clone()
        };

        let used_placeholders = find_placeholders(text.as_bytes());
        let mut value = value.clone();
        'envs: for (env, placeholders) in env_placeholders.iter() {
            for placeholder in placeholders {
                if !used_placeholders.contains(&placeholder.as_bytes()) {
                    continue;
                }
                let resolved = match format {
                    EnvFormat::Resolve => {
                        let env_value = self.get_env(env)?;
                        escape_value(&env_value)
                    }
                    EnvFormat::Placeholder => sprintf_env(b"%%env(%s)%%", env)?,
                    EnvFormat::Sprintf(f) => sprintf_env(f.as_bytes(), env)?,
                };
                let mut completed = false;
                let current = match &value {
                    Value::String(s) => s.clone(),
                    _ => break 'envs,
                };
                if current.as_bytes() == placeholder.as_bytes() {
                    value = resolved;
                    completed = true;
                } else {
                    let replacement = match &resolved {
                        Value::String(s) => s.clone(),
                        Value::Int(_) | Value::Float(_) => {
                            resolved.to_php_string().unwrap_or_default()
                        }
                        other => {
                            let shown = self.resolve_env_placeholders(
                                Value::String(current.clone()),
                                &EnvFormat::Placeholder,
                                None,
                            )?;
                            return Err(Error::runtime(format!(
                                "A string value must be composed of strings and/or numbers, but found parameter \"env({env})\" of type \"{}\" inside string value \"{}\".",
                                other.debug_type(),
                                shown
                                    .to_php_string()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_default()
                            )));
                        }
                    };
                    value = Value::String(PhpString::from(replace_ignore_case(
                        current.as_bytes(),
                        placeholder.as_bytes(),
                        replacement.as_bytes(),
                    )));
                }
                if let Some(used) = used_envs.as_deref_mut() {
                    push_unique(used, env);
                }
                *self.env_counters.get_or_insert_with(env, || 0) += 1;
                if completed {
                    break 'envs;
                }
            }
        }
        Ok(value)
    }

    /// `getEnvCounters()`: every env var with a placeholder counts, if
    /// only as 0.
    pub fn get_env_counters(&mut self) -> IdMap<i64> {
        let env_placeholders = if self.parameter_bag.is_env_placeholder() {
            self.parameter_bag.get_env_placeholders()
        } else {
            self.env_placeholders.clone()
        };
        for env in env_placeholders.keys() {
            self.env_counters.get_or_insert_with(env, || 0);
        }
        self.env_counters.clone()
    }

    /// `log($pass, $message)`; `pass_class` is `$pass::class`.
    pub fn log(&mut self, pass_class: &str, message: &str) -> Result<()> {
        let resolved =
            self.resolve_env_placeholders(Value::from(message), &EnvFormat::Placeholder, None)?;
        let message = resolved
            .to_php_string()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.compiler.log(pass_class, &message);
        Ok(())
    }

    /// `getRemovedBindingIds()`.
    pub fn get_removed_binding_ids(&self) -> &[i64] {
        &self.removed_binding_ids
    }

    /// `removeBindings($id)`.
    pub fn remove_bindings(&mut self, id: &str) {
        let Some(definition) = self.definitions.get(id) else {
            return;
        };
        for (_, binding) in definition.bindings().iter() {
            let Some(binding) = BoundArgument::from_value(binding) else {
                continue;
            };
            let binding_id = binding.values().get(&Key::Int(1)).map_or(0, php_int);
            if !self.removed_binding_ids.contains(&binding_id) {
                self.removed_binding_ids.push(binding_id);
            }
        }
    }

    /// `getServiceConditionals($value)`: the ids of the
    /// `IGNORE_ON_INVALID_REFERENCE` references in a value.
    pub fn get_service_conditionals(value: &Value) -> Vec<String> {
        conditionals(value, InvalidBehavior::Ignore)
    }

    /// `getInitializedConditionals($value)`: the ids of the
    /// `IGNORE_ON_UNINITIALIZED_REFERENCE` references in a value.
    pub fn get_initialized_conditionals(value: &Value) -> Vec<String> {
        conditionals(value, InvalidBehavior::IgnoreUninitialized)
    }

    /// `hash($value)`: 7 characters of the base64 of the xxh128 of the
    /// serialized value, `/` and `+` as `.` and `_`.
    pub fn hash(value: &Value) -> String {
        let hex = xxh128(php_value::serialize::serialize(value).as_bytes());
        let raw: Vec<u8> = hex
            .chunks(2)
            .filter_map(|pair| {
                std::str::from_utf8(pair)
                    .ok()
                    .and_then(|p| u8::from_str_radix(p, 16).ok())
            })
            .collect();
        php_value::base64::encode(&raw)
            .chars()
            .take(7)
            .map(|c| match c {
                '/' => '.',
                '+' => '_',
                c => c,
            })
            .collect()
    }

    /// `getEnv($name)`: `Container::getEnv()` (cached, circular uses
    /// refused) through the [`EnvVarResolver`], then the builder's
    /// placeholder handling.
    pub fn get_env(&mut self, name: &str) -> Result<Value> {
        let env_name = format!("env({name})");
        if self.resolving.contains(&env_name) {
            return Err(Error::parameter_circular_reference(self.resolving.clone()));
        }
        let value = if let Some(cached) = self.env_cache.get(name) {
            cached.clone()
        } else {
            let resolver = self.env_var_resolver.clone().ok_or_else(|| {
                Error::logic(format!(
                    "Resolving the env var \"{name}\" needs an env var processor; this build has none (set one with set_env_var_resolver())."
                ))
            })?;
            let (prefix, local_name) = match name.find(':') {
                Some(i) => (&name[..i], &name[i + 1..]),
                None => ("", name),
            };
            self.resolving.push(env_name.clone());
            let result = {
                let parameters = self.parameter_bag.clone();
                resolver.get_env(prefix, local_name, &parameters, &mut |n: &str| {
                    self.get_env(n)
                })
            };
            self.unresolve(&env_name);
            let value = result?;
            self.env_cache.insert(name, value.clone());
            value
        };

        let Value::String(text) = &value else {
            return Ok(value);
        };
        if !self.parameter_bag.is_env_placeholder() {
            return Ok(value);
        }
        let text = text.to_string_lossy().into_owned();
        let env_placeholders = self.parameter_bag.get_env_placeholders();
        if env_placeholders
            .get(name)
            .is_some_and(|ps| ps.contains(&text))
        {
            let bag = ParameterBag::new(self.parameter_bag.all())?;
            return Ok(bag.unescape_value(&bag.get(&env_name)?));
        }
        for (env, placeholders) in env_placeholders.iter() {
            if placeholders.contains(&text) {
                return self.get_env(env);
            }
        }
        self.resolving.push(env_name.clone());
        let escaped = self.parameter_bag.escape_value(&value);
        let result = self.resolve_env_placeholders(escaped, &EnvFormat::Resolve, None);
        self.unresolve(&env_name);
        Ok(self.parameter_bag.unescape_value(&result?))
    }

    fn unresolve(&mut self, env_name: &str) {
        if let Some(at) = self.resolving.iter().rposition(|r| r == env_name) {
            self.resolving.remove(at);
        }
    }
}

fn path_exists(path: &str) -> bool {
    php_value::path::to_path(path.as_bytes()).is_some_and(|p| p.exists())
}

/// `(int) $value`.
fn php_int(value: &Value) -> i64 {
    match value {
        Value::Int(i) => *i,
        Value::Bool(b) => i64::from(*b),
        #[expect(clippy::cast_possible_truncation, reason = "PHP's (int) cast")]
        Value::Float(f) => *f as i64,
        Value::String(s) => php_value::numeric::string_to_int(s.as_bytes()),
        _ => 0,
    }
}

fn conditionals(value: &Value, behavior: InvalidBehavior) -> Vec<String> {
    let mut services = Vec::new();
    if let Value::Array(array) = value {
        for v in array.values() {
            for s in conditionals(v, behavior) {
                push_unique(&mut services, &s);
            }
        }
    } else if let Some(reference) = Reference::from_value(value)
        && reference.invalid_behavior() == behavior
    {
        services.push(reference.id().to_string());
    }
    services
}

/// `escapeParameters($parameters)`.
fn escape_parameters(parameters: &PhpArray) -> PhpArray {
    let mut out = PhpArray::new();
    for (k, v) in parameters.iter() {
        let escaped = match v {
            Value::Array(a) => Value::Array(escape_parameters(a)),
            Value::String(s) => Value::String(PhpString::from(
                String::from_utf8_lossy(s.as_bytes()).replace('%', "%%"),
            )),
            other => other.clone(),
        };
        out.insert(k.clone(), escaped);
    }
    out
}

/// `sprintf($format, $env)`.
fn sprintf_env(format: &[u8], env: &str) -> Result<Value> {
    symfony_config::php::sprintf(format, &[Value::from(env)])
        .map(|s| Value::String(PhpString::from(s)))
        .map_err(|e| config_error(&e))
}

/// `Target::getParsedName()`: `lcfirst(str_replace(' ', '',
/// ucwords(preg_replace('/[^a-zA-Z0-9\x7f-\xff]++/', ' ', $name))))`.
fn target_parsed_name(name: &str) -> String {
    let mut out = Vec::with_capacity(name.len());
    let mut upper_next = true;
    for &b in name.as_bytes() {
        if b.is_ascii_alphanumeric() || b >= 0x7f {
            out.push(if upper_next {
                b.to_ascii_uppercase()
            } else {
                b
            });
            upper_next = false;
        } else {
            upper_next = true;
        }
    }
    if let Some(first) = out.first_mut() {
        first.make_ascii_lowercase();
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn contains_ignore_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w.eq_ignore_ascii_case(needle))
}

/// `str_ireplace($search, $replace, $subject)`.
fn replace_ignore_case(subject: &[u8], search: &[u8], replace: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if !search.is_empty()
            && subject.len() - i >= search.len()
            && subject[i..i + search.len()].eq_ignore_ascii_case(search)
        {
            out.extend_from_slice(replace);
            i += search.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

/// `preg_match_all('/env_[a-f0-9]{16}_\w+_[a-f0-9]{32}/Ui', $value)`:
/// the matches, in order (`U` makes `\w+` lazy).
fn find_placeholders(text: &[u8]) -> Vec<&[u8]> {
    let hex = |b: &u8| b.is_ascii_hexdigit();
    let word = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
    let mut found = Vec::new();
    let mut i = 0;
    'scan: while i + 4 <= text.len() {
        let start = i;
        let rest = &text[i..];
        if rest.len() > 4 + 16
            && rest[..4].eq_ignore_ascii_case(b"env_")
            && rest[4..20].iter().all(hex)
            && rest[20] == b'_'
        {
            // `\w+?` then `_` and 32 hex digits.
            let mut k = 21;
            while k < rest.len() && word(&rest[k]) {
                let end = k + 1;
                if rest.len() >= end + 33
                    && rest[end] == b'_'
                    && rest[end + 1..end + 33].iter().all(hex)
                {
                    found.push(&text[start..start + end + 33]);
                    i = start + end + 33;
                    continue 'scan;
                }
                k += 1;
            }
        }
        i += 1;
    }
    found
}
