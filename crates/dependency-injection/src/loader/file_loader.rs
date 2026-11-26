//! Port of Symfony\Component\DependencyInjection\Loader\FileLoader.php:
//! the base of the file loaders. Imports that may ignore a missing file
//! (`not_found`), extension configs (appended or prepended), definitions
//! with their instanceof-conditionals, and `registerClasses()`.
//!
//! `registerClasses()` finds classes the way upstream does (a glob of
//! `*.php` files under the resource, a class name from each path, PSR-4
//! style), and asks the container's [`ClassRegistry`] in place of PHP's
//! autoloader whether each class exists: a class the registry does not
//! know is "not found", one whose ancestor is missing gets the
//! `ReflectionException` message as a definition error. Its attributes
//! (`#[Exclude]`, `#[When]`, `#[WhenNot]`, `#[AsAlias]`) are the class
//! metadata's.
//!
//! [`ClassRegistry`]: crate::class_metadata::ClassRegistry
//!
//! Differences:
//!
//! - an import with `ignore_errors: not_found` ignores a
//!   `FileLocatorFileNotFoundException` from any loader of the import;
//!   upstream also checks, in the exception's trace, that the innermost
//!   `import()` was this class's (a plain config loader in between, such
//!   as a custom one, makes it rethrow). There is no trace here.
//! - upstream caches the `container.excluded` tag attributes of a source
//!   file in a static variable; they are computed again here, with the
//!   same value.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dependency-injection Loader/FileLoader.php

use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use php_value::{Key, PhpArray, Value, php_array};
use symfony_config::Error as ConfigError;
use symfony_config::file_locator::FileLocatorInterface;
use symfony_config::loader::{FileLoader as BaseFileLoader, GlobResult, LoaderInterface};

use super::{SharedContainer, di, lock};
use crate::alias::Alias;
use crate::argument::BoundArgument;
use crate::class_metadata::{AttributeMetadata, ClassMetadata, ClassRegistry};
use crate::compiler::RegisterAutoconfigureAttributesPass;
use crate::compiler::deep_clone::deep_clone;
use crate::container_builder::{TrackContents, target_parsed_name};
use crate::definition::{Definition, DefinitionError};
use crate::exception::{Error, ErrorKind, Result};
use crate::reference::{Parameter, Reference};

pub const CLASS: &str = "Symfony\\Component\\DependencyInjection\\Loader\\FileLoader";

/// `FileLoader::ANONYMOUS_ID_REGEXP`.
pub const ANONYMOUS_ID_REGEXP: &str = crate::container_builder::ANONYMOUS_ID_REGEXP;

const ATTRIBUTE: &str = "Symfony\\Component\\DependencyInjection\\Attribute\\";

/// The `$ignoreErrors` of `import()`: a boolean, or `"not_found"` to
/// ignore only a resource that is not found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreErrors {
    Bool(bool),
    NotFound,
}

impl From<bool> for IgnoreErrors {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl IgnoreErrors {
    /// A PHP value: a boolean or `"not_found"`, else upstream's
    /// `TypeError` (`static::class` is the loader's class).
    pub fn from_value(value: &Value, loader_class: &str) -> std::result::Result<Self, ConfigError> {
        match value {
            Value::Bool(b) => Ok(Self::Bool(*b)),
            Value::String(s) if s.as_bytes() == b"not_found" => Ok(Self::NotFound),
            other => Err(ConfigError::new(
                symfony_config::Class::TypeError,
                format!(
                    "Invalid argument $ignoreErrors provided to \"{loader_class}::import()\": boolean or \"not_found\" expected, \"{}\" given.",
                    other.debug_type()
                ),
            )),
        }
    }
}

/// The protected properties a loader changes while it loads.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) is_loading_instanceof: bool,
    /// `$instanceof`: ids to `ChildDefinition` values.
    pub(crate) instanceof: PhpArray,
    interfaces: Vec<String>,
    /// `$singlyImplemented`: `None` is `false`.
    singly_implemented: Vec<(String, Option<String>)>,
    aliases: Vec<(String, Alias)>,
    aliased_targets: Vec<(String, String)>,
    extension_configs: Vec<(String, Vec<PhpArray>)>,
    pub(crate) importing: i64,
}

/// `abstract class FileLoader extends BaseFileLoader`, embedded in a
/// concrete loader.
pub struct FileLoader {
    base: BaseFileLoader,
    container: SharedContainer,
    prepend: bool,
    auto_register: Mutex<bool>,
    state: Mutex<State>,
}

impl FileLoader {
    /// `parent::__construct($container, $locator, $env, $prepend)`;
    /// `this` is the embedding loader (from `Arc::new_cyclic`).
    pub fn new(
        container: SharedContainer,
        locator: Arc<dyn FileLocatorInterface>,
        env: Option<&str>,
        prepend: bool,
        this: Weak<dyn LoaderInterface>,
    ) -> Self {
        Self {
            base: BaseFileLoader::new(locator, env.map(str::as_bytes), this),
            container,
            prepend,
            auto_register: Mutex::new(true),
            state: Mutex::new(State::default()),
        }
    }

    /// The embedded `Symfony\Component\Config\Loader\FileLoader`.
    pub fn base(&self) -> &BaseFileLoader {
        &self.base
    }

    /// The protected `$container`.
    pub fn container(&self) -> &SharedContainer {
        &self.container
    }

    /// The protected `$prepend`.
    pub fn prepend(&self) -> bool {
        self.prepend
    }

    /// The protected `$env`.
    pub fn env(&self) -> Option<String> {
        self.base
            .loader()
            .env()
            .map(|e| String::from_utf8_lossy(e).into_owned())
    }

    /// The protected `$autoRegisterAliasesForSinglyImplementedInterfaces`.
    pub fn set_auto_register_aliases_for_singly_implemented_interfaces(&self, value: bool) {
        *self
            .auto_register
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = value;
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// `setCurrentDir($dir)`.
    pub fn set_current_dir(&self, dir: &str) {
        self.base.set_current_dir(dir.as_bytes());
    }

    /// `import($resource, $type, $ignoreErrors, $sourceResource,
    /// $exclude)`.
    pub fn import(
        &self,
        resource: &Value,
        r#type: Option<&[u8]>,
        ignore_errors: IgnoreErrors,
        source_resource: Option<&[u8]>,
        exclude: &Value,
    ) -> std::result::Result<Value, ConfigError> {
        let (ignore_not_found, ignore) = match ignore_errors {
            IgnoreErrors::NotFound => (true, false),
            IgnoreErrors::Bool(b) => (false, b),
        };

        self.state().importing += 1;
        let result = self
            .base
            .import(resource, r#type, ignore, source_resource, exclude);
        // finally
        self.state().importing -= 1;
        let after = self.load_extension_configs();

        match result {
            Ok(value) => after.map(|()| value),
            Err(e) => {
                after?;
                let not_found = e.class().is_loader_load()
                    && e.get_previous().is_some_and(|p| {
                        *p.class() == symfony_config::Class::FileLocatorFileNotFound
                    });
                if ignore_not_found && not_found {
                    Ok(Value::Null)
                } else {
                    Err(e)
                }
            }
        }
    }

    /// `registerClasses($prototype, $namespace, $resource, $exclude,
    /// $source)`: registers the classes found under `resource` (a glob
    /// pattern), named with the `namespace` prefix, as services built
    /// from `prototype`. `exclude` is a pattern, an array of them, or
    /// null.
    #[expect(clippy::too_many_lines, reason = "upstream's method, step by step")]
    pub fn register_classes(
        &self,
        prototype: &Definition,
        namespace: &str,
        resource: &str,
        exclude: &Value,
        source: Option<&str>,
    ) -> Result<()> {
        if !namespace.ends_with('\\') {
            return Err(Error::invalid_argument(format!(
                "Namespace prefix must end with a \"\\\": \"{namespace}\"."
            )));
        }
        if !is_psr4_prefix(namespace.as_bytes()) {
            return Err(Error::invalid_argument(format!(
                "Namespace is not a valid PSR-4 prefix: \"{namespace}\"."
            )));
        }
        let exclude: Vec<Value> = match exclude {
            Value::Null => Vec::new(),
            Value::Array(a) => {
                // This can happen with YAML files
                if a.values().any(Value::is_null) {
                    return Err(Error::invalid_argument(
                        "The exclude list must not contain a \"null\" value.",
                    ));
                }
                // This can happen with XML files
                if a.values()
                    .any(|v| matches!(v, Value::String(s) if s.is_empty()))
                {
                    return Err(Error::invalid_argument(
                        "The exclude list must not contain an empty value.",
                    ));
                }
                a.values().cloned().collect()
            }
            Value::String(_) => vec![exclude.clone()],
            other => {
                return Err(Error::new(
                    ErrorKind::Other("TypeError".to_string()),
                    format!(
                        "{CLASS}::registerClasses(): Argument #4 ($exclude) must be of type array|string|null, {} given",
                        other.debug_type()
                    ),
                ));
            }
        };

        let autoconfigure_attributes = RegisterAutoconfigureAttributesPass::accept(prototype);
        let classes = self.find_classes(namespace, resource, exclude, source)?;
        let registry = lock(&self.container).class_registry();
        let env = self.env();

        for (class, error_message) in &classes {
            if error_message.is_none() && autoconfigure_attributes {
                let r = self.reflection_class(class)?;
                if attributes(&registry, &r, "Exclude", false).next().is_some() {
                    self.add_container_excluded_tag(class, source)?;
                    continue;
                }
                if let Some(env) = env.as_deref().filter(|e| !e.is_empty() && *e != "0") {
                    let mut excluded = true;
                    let when: Vec<_> = attributes(&registry, &r, "When", true).collect();
                    let not_when: Vec<_> = attributes(&registry, &r, "WhenNot", true).collect();

                    if !when.is_empty() && !not_when.is_empty() {
                        return Err(Error::logic(format!(
                            "The \"{class}\" class cannot have both #[When] and #[WhenNot] attributes."
                        )));
                    }

                    if when.is_empty() && not_when.is_empty() {
                        excluded = false;
                    }

                    for attribute in &when {
                        if attribute_string(attribute, 0, "env").as_deref() == Some(env) {
                            excluded = false;
                            break;
                        }
                    }

                    for attribute in &not_when {
                        excluded = attribute_string(attribute, 0, "env").as_deref() == Some(env);
                        if excluded {
                            break;
                        }
                    }

                    if excluded {
                        self.add_container_excluded_tag(class, source)?;
                        continue;
                    }
                }
            }

            let r = match error_message {
                None => Some(self.reflection_class(class)?),
                Some(_) => None,
            };

            let is_interface = r.as_ref().is_some_and(|r| r.is_interface());
            let is_abstract = r.as_ref().is_some_and(|r| r.is_abstract) || is_interface;
            let id = if is_abstract {
                format!(".abstract.{class}")
            } else {
                class.clone()
            };
            let definition = get_prototype(prototype)?;
            self.set_definition(&id, &definition)?;
            definition.set_class(Some(class));
            if let Some(message) = error_message {
                definition.add_error(DefinitionError::Text(message.clone()));

                continue;
            }
            let Some(r) = r else { continue };

            if is_abstract {
                if is_interface {
                    self.state().interfaces.push(class.clone());
                }
                definition.set_abstract(true).add_tag(
                    "container.excluded",
                    php_array!["source" => "because the class is abstract"],
                );
                continue;
            }

            let interfaces = registry.interface_names(&r);
            {
                let mut state = self.state();
                for interface in &interfaces {
                    let entry = state
                        .singly_implemented
                        .iter_mut()
                        .find(|(name, _)| name == interface);
                    match entry {
                        Some((_, value)) => {
                            if value.as_deref() != Some(class.as_str()) {
                                *value = None;
                            }
                        }
                        None => state
                            .singly_implemented
                            .push((interface.clone(), Some(class.clone()))),
                    }
                }
            }

            if !autoconfigure_attributes {
                continue;
            }
            let default_alias = (interfaces.len() == 1).then(|| interfaces[0].clone());
            for attribute in attributes(&registry, &r, "AsAlias", true) {
                let alias = attribute_string(attribute, 0, "id").or_else(|| default_alias.clone());
                let public = attribute_value(attribute, 1, "public").is_some_and(|v| v.is_truthy());
                let when: Vec<String> = match attribute_value(attribute, 2, "when") {
                    Some(Value::Array(a)) => a.values().map(string_of).collect(),
                    Some(Value::String(s)) => vec![s.to_string_lossy().into_owned()],
                    _ => Vec::new(),
                };
                let target = attribute_string(attribute, 3, "target");
                let Some(alias) = alias else {
                    return Err(Error::logic(format!(
                        "Alias cannot be automatically determined for class \"{class}\". If you have used the #[AsAlias] attribute with a class implementing multiple interfaces, add the interface you want to alias to the first parameter of #[AsAlias]."
                    )));
                };

                if !when.is_empty() && !env.as_ref().is_some_and(|e| when.contains(e)) {
                    continue;
                }
                let Some(target) = target.filter(|t| !t.is_empty() && t != "0") else {
                    let mut state = self.state();
                    if let Some((_, existing)) = state.aliases.iter().find(|(a, _)| *a == alias) {
                        return Err(Error::logic(format!(
                            "The \"{alias}\" alias has already been defined with the #[AsAlias] attribute in \"{}\".",
                            existing.id()
                        )));
                    }

                    state
                        .aliases
                        .push((alias, Alias::new(class.clone(), public)));
                    continue;
                };
                if public {
                    return Err(Error::logic(format!(
                        "#[AsAlias] attributes with a target cannot be public in \"{class}\"."
                    )));
                }
                lock(&self.container).register_alias_for_argument(
                    class,
                    &alias,
                    Some(&target),
                    None,
                )?;
                let parsed_name = target_parsed_name(&target);
                let alias = format!("{alias} ${parsed_name}");
                let mut state = self.state();
                if state.aliased_targets.iter().any(|(a, _)| *a == alias) {
                    return Err(Error::logic(format!(
                        "The \"{alias}\" alias has already been defined with the #[AsAlias] attribute in \"{class}\"."
                    )));
                }
                state.aliased_targets.push((alias, class.clone()));
            }
        }

        let aliases = self.state().aliases.clone();
        {
            let mut container = lock(&self.container);
            for (alias, definition) in aliases {
                container.set_alias(&alias, definition)?;
            }
        }

        if *self
            .auto_register
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
        {
            self.register_aliases_for_singly_implemented_interfaces()?;
        }

        Ok(())
    }

    /// `registerAliasesForSinglyImplementedInterfaces()`.
    pub fn register_aliases_for_singly_implemented_interfaces(&self) -> Result<()> {
        let (interfaces, singly_implemented, aliases) = {
            let mut state = self.state();
            state.aliased_targets.clear();
            (
                std::mem::take(&mut state.interfaces),
                std::mem::take(&mut state.singly_implemented),
                std::mem::take(&mut state.aliases),
            )
        };
        let mut container = lock(&self.container);
        for interface in &interfaces {
            let target = singly_implemented
                .iter()
                .find(|(name, _)| name == interface)
                .and_then(|(_, class)| class.clone())
                .filter(|c| !c.is_empty() && c != "0");
            if let Some(target) = target
                && !aliases.iter().any(|(a, _)| a == interface)
                && !container.has(interface)
            {
                container.set_alias(interface, target)?;
            }
        }
        Ok(())
    }

    /// `loadExtensionConfig($namespace, $config, $file)`.
    pub fn load_extension_config(&self, namespace: &str, config: PhpArray) -> Result<()> {
        if !self.prepend {
            lock(&self.container).load_from_extension(namespace, Some(config))?;

            return Ok(());
        }

        let mut state = self.state();
        if state.importing != 0 {
            match state
                .extension_configs
                .iter_mut()
                .find(|(n, _)| n == namespace)
            {
                Some((_, configs)) => configs.insert(0, config),
                None => state
                    .extension_configs
                    .push((namespace.to_string(), vec![config])),
            }

            return Ok(());
        }
        drop(state);

        lock(&self.container).prepend_extension_config(namespace, config);
        Ok(())
    }

    /// `loadExtensionConfigs()`.
    pub fn load_extension_configs(&self) -> std::result::Result<(), ConfigError> {
        let configs = {
            let mut state = self.state();
            if state.importing != 0 || state.extension_configs.is_empty() {
                return Ok(());
            }
            std::mem::take(&mut state.extension_configs)
        };

        let mut container = lock(&self.container);
        for (namespace, configs) in configs {
            for config in configs {
                container.prepend_extension_config(&namespace, config);
            }
        }
        Ok(())
    }

    /// `setDefinition($id, $definition)`: registers a definition with
    /// its instanceof-conditionals.
    pub fn set_definition(&self, id: &str, definition: &Definition) -> Result<()> {
        lock(&self.container).remove_bindings(id);

        for (_, error) in definition.tag("container.error").iter() {
            if let Some(message) = error
                .as_array()
                .and_then(|e| e.get(&Key::from("message")))
                .filter(|m| !m.is_null())
            {
                definition.add_error(DefinitionError::Text(string_of(message)));
            }
        }

        let mut state = self.state();
        if state.is_loading_instanceof {
            if !definition.is_child() {
                return Err(Error::invalid_argument(format!(
                    "Invalid type definition \"{id}\": ChildDefinition expected, \"{}\" given.",
                    Definition::CLASS
                )));
            }
            state
                .instanceof
                .insert(Key::from(id), definition.to_value());
        } else {
            let instanceof = state.instanceof.clone();
            drop(state);
            definition.set_instanceof_conditionals(instanceof);
            lock(&self.container).set_definition(id, definition.clone())?;
        }
        Ok(())
    }

    fn reflection_class(&self, class: &str) -> Result<Arc<ClassMetadata>> {
        lock(&self.container)
            .get_reflection_class(Some(class), true)?
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Other("ReflectionException".to_string()),
                    format!("Class \"{class}\" does not exist"),
                )
            })
    }

    #[expect(clippy::too_many_lines, reason = "upstream's method, step by step")]
    fn find_classes(
        &self,
        namespace: &str,
        pattern: &str,
        exclude_patterns: Vec<Value>,
        source: Option<&str>,
    ) -> Result<Vec<(String, Option<String>)>> {
        let (exclude_patterns, pattern) = {
            let container = lock(&self.container);
            let bag = container.get_parameter_bag();
            let excludes =
                bag.resolve_value(&Value::Array(PhpArray::list(exclude_patterns)), &[])?;
            let excludes = bag.unescape_value(&excludes);
            let pattern = bag.resolve_value(&Value::from(pattern), &[])?;
            (excludes, bag.unescape_value(&pattern))
        };
        let exclude_patterns: Vec<Value> = match exclude_patterns {
            Value::Array(a) => a.values().cloned().collect(),
            _ => Vec::new(),
        };

        let mut exclude_paths = PhpArray::new();
        let mut exclude_prefix: Option<Vec<u8>> = None;
        let mut exclude_pattern = Vec::new();
        for pattern in &exclude_patterns {
            exclude_pattern = glob_argument(pattern)?;
            let (paths, resource) = self
                .base
                .glob(&exclude_pattern, true, true, true, PhpArray::new())
                .map_err(di)?;
            for path in paths {
                if exclude_prefix.is_none()
                    && let GlobResult::Glob(r) = &resource
                {
                    exclude_prefix = Some(r.get_prefix().to_vec());
                }

                // normalize Windows slashes and remove trailing slashes
                let key = rtrim_slashes(&normalize(&path));
                exclude_paths.insert(Key::from_bytes(&key), Value::Bool(true));
            }
        }

        let pattern = glob_argument(&pattern)?;
        let (paths, resource) = self
            .base
            .glob(&pattern, true, false, false, exclude_paths.clone())
            .map_err(di)?;
        let mut classes: Vec<(String, Option<String>)> = Vec::new();
        let mut prefix_len: Option<usize> = None;
        for path in paths {
            if prefix_len.is_none() {
                let prefix = match &resource {
                    GlobResult::Glob(r) => r.get_prefix().to_vec(),
                    GlobResult::Existence(_) => Vec::new(),
                };
                prefix_len = Some(prefix.len());

                if let Some(exclude_prefix) = exclude_prefix.as_deref()
                    && !exclude_prefix.is_empty()
                    && exclude_prefix != b"0"
                    && !exclude_prefix.starts_with(&prefix)
                {
                    return Err(Error::invalid_argument(format!(
                        "Invalid \"exclude\" pattern when importing classes for \"{namespace}\": make sure your \"exclude\" pattern ({}) is a subset of the \"resource\" pattern ({}).",
                        String::from_utf8_lossy(&exclude_pattern),
                        String::from_utf8_lossy(&pattern)
                    )));
                }
            }

            if exclude_paths.contains_key(&Key::from_bytes(&normalize(&path))) {
                continue;
            }

            if !path.ends_with(b".php") {
                continue;
            }
            let len = prefix_len.unwrap_or(0);
            let middle = path.get(len..path.len() - 4).unwrap_or_default();
            let class = class_name(namespace, middle);

            if !is_class_name(class.as_bytes()) {
                continue;
            }

            let reflected = lock(&self.container).get_reflection_class(Some(&class), true);
            let r = match reflected {
                Ok(r) => r,
                Err(e) if e.instance_of("ReflectionException") => {
                    set(&mut classes, &class, Some(e.message()));
                    continue;
                }
                Err(e) => return Err(e),
            };
            // check to make sure the expected class exists
            let Some(r) = r else {
                return Err(Error::invalid_argument(format!(
                    "Expected to find class \"{class}\" in file \"{}\" while importing services from resource \"{}\", but it was not found! Check the namespace prefix used with the resource.",
                    String::from_utf8_lossy(&path),
                    String::from_utf8_lossy(&pattern)
                )));
            };

            if r.kind != crate::class_metadata::ClassKind::Trait {
                set(&mut classes, &class, None);
            }
        }

        // track only for new & removed files
        match resource {
            GlobResult::Glob(resource) => {
                lock(&self.container).add_resource(Arc::new(resource));
            }
            GlobResult::Existence(resources) => {
                let mut container = lock(&self.container);
                for resource in resources {
                    let path = symfony_config::resource::ResourceInterface::to_string(&resource);
                    container.file_exists(&String::from_utf8_lossy(&path), &TrackContents::No)?;
                }
            }
        }

        if let Some(prefix_len) = prefix_len {
            for (path, _) in exclude_paths.iter() {
                let path = symfony_config::php::key_bytes(path);
                let end = if path.ends_with(b".php") {
                    path.len() - 4
                } else {
                    path.len()
                };
                let middle = path.get(prefix_len..end).unwrap_or_default();
                let class = class_name(namespace, middle);
                self.add_container_excluded_tag(&class, source)?;
            }
        }

        Ok(classes)
    }

    fn add_container_excluded_tag(&self, class: &str, source: Option<&str>) -> Result<()> {
        let mut container = lock(&self.container);
        if container.has(class) {
            return Ok(());
        }

        let attributes = match source {
            Some(source) => {
                let dir = php_value::path::dirname_str(source);
                php_array![
                    "source" => format!(
                        "in \"{}/{}\"",
                        php_value::path::basename_str(&dir),
                        php_value::path::basename_str(source)
                    )
                ]
            }
            None => PhpArray::new(),
        };

        container
            .register(class, Some(class))?
            .set_abstract(true)
            .add_tag("container.excluded", attributes);
        Ok(())
    }
}

/// `$classes[$class] = $value`: an existing key keeps its place.
fn set(classes: &mut Vec<(String, Option<String>)>, class: &str, value: Option<String>) {
    match classes.iter_mut().find(|(c, _)| c == class) {
        Some((_, v)) => *v = value,
        None => classes.push((class.to_string(), value)),
    }
}

/// `$namespace.ltrim(str_replace('/', '\\', $middle), '\\')`.
fn class_name(namespace: &str, middle: &[u8]) -> String {
    let replaced: Vec<u8> = middle
        .iter()
        .map(|&c| if c == b'/' { b'\\' } else { c })
        .collect();
    let start = replaced
        .iter()
        .position(|&c| c != b'\\')
        .unwrap_or(replaced.len());
    format!("{namespace}{}", String::from_utf8_lossy(&replaced[start..]))
}

/// A `glob(string $pattern)` argument: scalars convert, as PHP's coercive
/// mode does.
fn glob_argument(value: &Value) -> Result<Vec<u8>> {
    match value {
        Value::String(s) => Ok(s.as_bytes().to_vec()),
        Value::Int(_) | Value::Float(_) | Value::Bool(_) => {
            Ok(symfony_config::php::to_bytes(value))
        }
        other => Err(Error::new(
            ErrorKind::Other("TypeError".to_string()),
            format!(
                "Symfony\\Component\\Config\\Loader\\FileLoader::glob(): Argument #1 ($pattern) must be of type string, {} given",
                symfony_config::php::error_type_name(other)
            ),
        )),
    }
}

fn is_label_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x7f
}

fn is_label_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x7f
}

/// `/^(?:[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*+\\\\)++$/`.
fn is_psr4_prefix(namespace: &[u8]) -> bool {
    let Some(body) = namespace.strip_suffix(b"\\") else {
        return false;
    };
    body.split(|&b| b == b'\\').all(|segment| {
        segment.first().is_some_and(|&b| is_label_start(b))
            && segment.iter().all(|&b| is_label_char(b))
    })
}

/// `/^[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*+(?:\\\\[a-zA-Z_\x7f-\xff][a-zA-Z0-9_\x7f-\xff]*+)*+$/`.
fn is_class_name(class: &[u8]) -> bool {
    class.split(|&b| b == b'\\').all(|segment| {
        segment.first().is_some_and(|&b| is_label_start(b))
            && segment.iter().all(|&b| is_label_char(b))
    })
}

fn normalize(path: &[u8]) -> Vec<u8> {
    path.iter()
        .map(|&c| if c == b'\\' { b'/' } else { c })
        .collect()
}

fn rtrim_slashes(path: &[u8]) -> Vec<u8> {
    let mut end = path.len();
    while end > 0 && path[end - 1] == b'/' {
        end -= 1;
    }
    path[..end].to_vec()
}

fn string_of(value: &Value) -> String {
    value
        .to_php_string()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `$r->getAttributes(Attribute\$short::class[, IS_INSTANCEOF])`.
fn attributes<'a>(
    registry: &'a ClassRegistry,
    class: &'a ClassMetadata,
    short: &'a str,
    instance_of: bool,
) -> impl Iterator<Item = &'a AttributeMetadata> + 'a {
    let wanted = format!("{ATTRIBUTE}{short}");
    class.attributes.iter().filter(move |a| {
        a.class
            .trim_start_matches('\\')
            .eq_ignore_ascii_case(&wanted)
            || (instance_of && registry.is_a(&a.class, &wanted))
    })
}

/// A constructor argument of an attribute, by position or name.
fn attribute_value(attribute: &AttributeMetadata, position: i64, name: &str) -> Option<Value> {
    attribute
        .arguments
        .get(&Key::from(name))
        .or_else(|| attribute.arguments.get(&Key::Int(position)))
        .cloned()
}

fn attribute_string(attribute: &AttributeMetadata, position: i64, name: &str) -> Option<String> {
    attribute_value(attribute, position, name)
        .filter(|v| !v.is_null())
        .map(|v| string_of(&v))
}

/// The prototype's clone: `clone $prototype`, with a deep copy of each
/// part that holds objects compiler passes change.
fn get_prototype(prototype: &Definition) -> Result<Definition> {
    let definition = prototype.clone_object();
    let changes = definition.changes();

    let arguments = Value::Array(prototype.arguments());
    if needs_deep_clone(&arguments)
        && let Value::Array(a) = deep_clone(&arguments)?
    {
        definition.set_arguments(a);
    }
    let properties = Value::Array(prototype.properties());
    if needs_deep_clone(&properties)
        && let Value::Array(a) = deep_clone(&properties)?
    {
        definition.set_properties(a);
    }
    let calls = Value::Array(prototype.method_calls());
    if needs_deep_clone(&calls)
        && let Value::Array(a) = deep_clone(&calls)?
    {
        definition.set_method_calls(&a)?;
    }
    let configurator = prototype.configurator();
    if needs_deep_clone(&configurator) {
        definition.set_configurator(deep_clone(&configurator)?);
    }
    let factory = prototype.factory();
    if needs_deep_clone(&factory) {
        definition.set_factory(deep_clone(&factory)?);
    }
    let bindings = Value::Array(prototype.bindings());
    if needs_deep_clone(&bindings)
        && let Value::Array(a) = deep_clone(&bindings)?
    {
        definition.set_bindings(&a);
    }
    // the setters above record changes the clone already has
    definition.set_changes(changes);

    Ok(definition)
}

/// `needsDeepClone($value)`: whether a part of the prototype holds
/// objects that compiler passes mutate.
fn needs_deep_clone(value: &Value) -> bool {
    let value = match BoundArgument::from_value(value) {
        // bindings track their usage by identifier, so the very same
        // instance can be shared
        Some(bound) => bound
            .values()
            .get(&Key::Int(0))
            .cloned()
            .unwrap_or_default(),
        None => value.clone(),
    };

    match &value {
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_) => false,
        Value::Object(_)
            if Reference::from_value(&value).is_some()
                || Parameter::from_value(&value).is_some()
                || crate::object::downcast::<php_value::object::EnumCase>(&value).is_some() =>
        {
            false
        }
        Value::Object(_) => true,
        Value::Array(a) => a.values().any(needs_deep_clone),
    }
}
