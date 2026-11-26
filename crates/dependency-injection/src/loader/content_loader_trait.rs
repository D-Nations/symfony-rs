//! Port of Symfony\Component\DependencyInjection\Loader\ContentLoaderTrait.php:
//! turns the parsed content of a configuration file (imports,
//! parameters, extension configs, services) into the container's
//! definitions, with upstream's validation and messages.
//!
//! The trait's methods take the loader's [`FileLoader`] for `$this`; the
//! anonymous-service counter and suffix are the [`ContentLoader`]'s.
//!
//! There is no `ExpressionLanguage` component: `@=` expressions fail as
//! upstream fails when `Expression` does not exist (`\LogicException`).
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/dependency-injection Loader/ContentLoaderTrait.php

use std::sync::{Mutex, PoisonError};

use php_value::{Key, PhpArray, Value, php_array};
use symfony_config::{Class, Error as ConfigError};
use symfony_deprecation_contracts::trigger_deprecation;
use symfony_yaml::TaggedValue;

use super::{FileLoader, IgnoreErrors, cfg, lock};
use crate::alias::Alias;
use crate::argument::{
    AbstractArgument, BoundArgument, DEFAULTS_BINDING, EnvClosureArgument, INSTANCEOF_BINDING,
    IteratorArgument, SERVICE_BINDING, ServiceClosureArgument, ServiceLocatorArgument,
};
use crate::compiler::deep_clone::deep_clone;
use crate::container_builder::ContainerBuilder;
use crate::container_interface::InvalidBehavior;
use crate::definition::Definition;
use crate::exception::{Error, ErrorKind};
use crate::object::{downcast, object_value};
use crate::reference::Reference;

type CResult<T> = Result<T, ConfigError>;

/// `SERVICE_KEYWORDS`.
const SERVICE_KEYWORDS: [&str; 26] = [
    "alias",
    "parent",
    "class",
    "shared",
    "synthetic",
    "lazy",
    "public",
    "abstract",
    "deprecated",
    "factory",
    "file",
    "arguments",
    "properties",
    "configurator",
    "calls",
    "tags",
    "resource_tags",
    "decorates",
    "decoration_inner_name",
    "decoration_priority",
    "decoration_on_invalid",
    "decorates_tag",
    "autowire",
    "autoconfigure",
    "bind",
    "constructor",
];

/// `PROTOTYPE_KEYWORDS`.
const PROTOTYPE_KEYWORDS: [&str; 20] = [
    "resource",
    "namespace",
    "exclude",
    "parent",
    "shared",
    "lazy",
    "public",
    "abstract",
    "deprecated",
    "factory",
    "arguments",
    "properties",
    "configurator",
    "calls",
    "tags",
    "resource_tags",
    "autowire",
    "autoconfigure",
    "bind",
    "constructor",
];

/// `INSTANCEOF_KEYWORDS`.
const INSTANCEOF_KEYWORDS: [&str; 11] = [
    "shared",
    "lazy",
    "public",
    "properties",
    "configurator",
    "calls",
    "tags",
    "resource_tags",
    "autowire",
    "bind",
    "constructor",
];

/// `DEFAULTS_KEYWORDS`.
const DEFAULTS_KEYWORDS: [&str; 6] = [
    "public",
    "tags",
    "resource_tags",
    "autowire",
    "autoconfigure",
    "bind",
];

const EXPRESSION_LANGUAGE_MISSING: &str = "The \"@=\" expression syntax cannot be used without the ExpressionLanguage component. Try running \"composer require symfony/expression-language\".";

/// What `parseDefinition()` returns with `$return`.
pub(crate) enum Parsed {
    Definition(Definition),
    Alias(Alias),
}

impl Parsed {
    fn to_value(&self) -> Value {
        match self {
            Self::Definition(d) => d.to_value(),
            Self::Alias(a) => a.to_value(),
        }
    }
}

/// `ContentLoaderTrait`'s state: `$anonymousServicesCount` and
/// `$anonymousServicesSuffix`, and `static::class`.
pub(crate) struct ContentLoader {
    anonymous: Mutex<(i64, String)>,
    class: &'static str,
}

fn inv(message: impl Into<String>) -> ConfigError {
    cfg(Error::invalid_argument(message))
}

fn get<'a>(array: &'a PhpArray, key: &str) -> Option<&'a Value> {
    array.get(&Key::from(key))
}

/// `isset($array[$key])`.
fn isset(array: &PhpArray, key: &str) -> bool {
    get(array, key).is_some_and(|v| !v.is_null())
}

/// `$array[$key] ?? null`.
fn opt<'a>(array: &'a PhpArray, key: &str) -> Option<&'a Value> {
    get(array, key).filter(|v| !v.is_null())
}

/// `(string) $value` for a scalar.
fn s(value: &Value) -> String {
    match value {
        Value::Array(_) => "Array".to_string(),
        other => other
            .to_php_string()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

fn key_string(key: &Key) -> String {
    String::from_utf8_lossy(&symfony_config::php::key_bytes(key)).into_owned()
}

fn key_value(key: &Key) -> Value {
    Value::from(key.clone())
}

fn tagged(value: &Value) -> Option<&TaggedValue> {
    downcast::<TaggedValue>(value)
}

/// `array_merge($a, $b)`.
fn array_merge(a: &PhpArray, b: &PhpArray) -> PhpArray {
    let mut out = PhpArray::new();
    for (k, v) in a.iter().chain(b.iter()) {
        match k {
            Key::Int(_) => {
                let _ = out.push(v.clone());
            }
            Key::Str(_) => out.insert(k.clone(), v.clone()),
        }
    }
    out
}

/// `is_scalar($value ?? '')`.
fn is_scalar_or_null(value: &Value) -> bool {
    matches!(
        value,
        Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_)
    )
}

/// `sprintf($format, $arg)`.
fn sprintf(format: &str, arg: &str) -> CResult<String> {
    let bytes = symfony_config::php::sprintf(format.as_bytes(), &[Value::from(arg)])?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn type_error(message: String) -> ConfigError {
    ConfigError::new(Class::TypeError, message)
}

fn array_argument(value: &Value, method: &str) -> CResult<PhpArray> {
    match value {
        Value::Array(a) => Ok(a.clone()),
        other => Err(type_error(format!(
            "{}::{method}(): Argument #1 must be of type array, {} given",
            Definition::CLASS,
            other.debug_type()
        ))),
    }
}

impl ContentLoader {
    pub(crate) fn new(class: &'static str) -> Self {
        Self {
            anonymous: Mutex::new((0, String::new())),
            class,
        }
    }

    fn anonymous(&self) -> std::sync::MutexGuard<'_, (i64, String)> {
        self.anonymous
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// `loadContent($content, $path)`.
    pub(crate) fn load_content(
        &self,
        fl: &FileLoader,
        content: &PhpArray,
        path: &str,
    ) -> CResult<()> {
        // imports
        self.parse_imports(fl, content, path)?;

        // parameters
        if isset(content, "parameters") {
            let Some(Value::Array(parameters)) = get(content, "parameters") else {
                return Err(inv(format!(
                    "The \"parameters\" key should contain an array in \"{path}\"."
                )));
            };

            for (key, value) in parameters.iter() {
                let value = self.resolve_services(fl, value, path, true)?;
                lock(fl.container())
                    .set_parameter(&key_string(key), value)
                    .map_err(cfg)?;
            }
        }

        // extensions
        self.load_from_extensions(fl, content)?;

        // services
        {
            let container = lock(fl.container());
            let project_dir = if container.has_parameter("kernel.project_dir") {
                container.get_parameter("kernel.project_dir").map_err(cfg)?
            } else {
                Value::Null
            };
            // hashing the path relative to the project keeps the ids
            // independent from where the project is
            let relative = match &project_dir {
                Value::String(dir) if path.as_bytes().starts_with(dir.as_bytes()) => {
                    &path[dir.as_bytes().len()..]
                }
                _ => path,
            };
            *self.anonymous() = (
                0,
                format!("~{}", ContainerBuilder::hash(&Value::from(relative))),
            );
        }
        fl.set_current_dir(&php_value::path::dirname_str(path));
        let result = self.parse_definitions(fl, content, path, true);
        // finally
        fl.state().instanceof = PhpArray::new();
        let after = fl.register_aliases_for_singly_implemented_interfaces();
        result?;
        after.map_err(cfg)
    }

    fn parse_imports(&self, fl: &FileLoader, content: &PhpArray, file: &str) -> CResult<()> {
        if !isset(content, "imports") {
            return Ok(());
        }

        let Some(Value::Array(imports)) = get(content, "imports") else {
            return Err(inv(format!(
                "The \"imports\" key should contain an array in \"{file}\"."
            )));
        };

        let default_directory = php_value::path::dirname_str(file);
        for import in imports.values() {
            let import = match import {
                Value::Array(a) => a.clone(),
                other => php_array!["resource" => other.clone()],
            };
            if !isset(&import, "resource") {
                return Err(inv(format!(
                    "An import should provide a resource in \"{file}\"."
                )));
            }

            fl.set_current_dir(&default_directory);
            let r#type = opt(&import, "type").map(|t| s(t).into_bytes());
            let ignore_errors = match opt(&import, "ignore_errors") {
                Some(value) => IgnoreErrors::from_value(value, self.class)?,
                None => IgnoreErrors::Bool(false),
            };
            fl.import(
                get(&import, "resource").unwrap_or(&Value::Null),
                r#type.as_deref(),
                ignore_errors,
                Some(file.as_bytes()),
                &Value::Null,
            )?;
        }
        Ok(())
    }

    fn parse_definitions(
        &self,
        fl: &FileLoader,
        content: &PhpArray,
        file: &str,
        track_bindings: bool,
    ) -> CResult<()> {
        if !isset(content, "services") {
            return Ok(());
        }

        let Some(Value::Array(services)) = get(content, "services") else {
            return Err(inv(format!(
                "The \"services\" key should contain an array in \"{file}\"."
            )));
        };
        let mut services = services.clone();

        if let Some(instanceof) = services.remove(&Key::from("_instanceof")) {
            let Value::Array(instanceof) = instanceof else {
                return Err(inv(format!(
                    "Service \"_instanceof\" key must be an array, \"{}\" given in \"{file}\".",
                    instanceof.debug_type()
                )));
            };
            {
                let mut state = fl.state();
                state.instanceof = PhpArray::new();
                state.is_loading_instanceof = true;
            }
            for (id, service) in instanceof.iter() {
                let id = key_string(id);
                if !service.is_truthy() || !matches!(service, Value::Array(_)) {
                    return Err(inv(format!(
                        "Type definition \"{id}\" must be a non-empty array within \"_instanceof\" in \"{file}\"."
                    )));
                }
                self.parse_definition(
                    fl,
                    &id,
                    service,
                    file,
                    &PhpArray::new(),
                    false,
                    track_bindings,
                )?;
            }
        }

        fl.state().is_loading_instanceof = false;
        let defaults = self.parse_defaults(fl, &mut services, file)?;
        for (id, service) in services.iter() {
            self.parse_definition(
                fl,
                &key_string(id),
                service,
                file,
                &defaults,
                false,
                track_bindings,
            )?;
        }
        Ok(())
    }

    fn parse_defaults(
        &self,
        fl: &FileLoader,
        services: &mut PhpArray,
        file: &str,
    ) -> CResult<PhpArray> {
        let Some(defaults) = services.remove(&Key::from("_defaults")) else {
            return Ok(PhpArray::new());
        };

        let Value::Array(mut defaults) = defaults else {
            return Err(inv(format!(
                "Service \"_defaults\" key must be an array, \"{}\" given in \"{file}\".",
                defaults.debug_type()
            )));
        };

        for key in defaults.keys() {
            let key = key_string(key);
            if !DEFAULTS_KEYWORDS.contains(&key.as_str()) {
                return Err(inv(format!(
                    "The configuration key \"{key}\" cannot be used to define a default value in \"{file}\". Allowed keys are \"{}\".",
                    DEFAULTS_KEYWORDS.join("\", \"")
                )));
            }
        }

        for r#type in ["tags", "resource_tags"] {
            if !isset(&defaults, r#type) {
                continue;
            }

            let Some(Value::Array(tags)) = get(&defaults, r#type) else {
                return Err(inv(format!(
                    "Parameter \"{type}\" in \"_defaults\" must be an array in \"{file}\"."
                )));
            };

            for tag in tags.values() {
                let (name, tag) = normalize_tag(tag).ok_or_else(|| {
                    inv(format!(
                        "A \"{type}\" entry in \"_defaults\" is missing a \"name\" key in \"{file}\"."
                    ))
                })?;

                let name = match name {
                    Value::String(n) if !n.is_empty() => n.to_string_lossy().into_owned(),
                    _ => {
                        return Err(inv(format!(
                            "The tag name in \"_defaults\" must be a non-empty string in \"{file}\"."
                        )));
                    }
                };

                let message = format!(
                    "Tag \"{}\", attribute \"%s\" in \"_defaults\" must be of a scalar-type in \"{}\".",
                    name.replace('%', "%%"),
                    file.replace('%', "%%")
                );
                validate_attributes(&message, &tag, &[])?;
            }
        }

        if isset(&defaults, "bind") {
            let Some(Value::Array(bind)) = get(&defaults, "bind").cloned() else {
                return Err(inv(format!(
                    "Parameter \"bind\" in \"_defaults\" must be an array in \"{file}\"."
                )));
            };

            let resolved = self.resolve_services(fl, &Value::Array(bind.clone()), file, false)?;
            let mut bind = bind;
            if let Value::Array(resolved) = resolved {
                for (argument, value) in resolved.iter() {
                    bind.insert(
                        argument.clone(),
                        BoundArgument::new(
                            value.clone(),
                            true,
                            DEFAULTS_BINDING,
                            Some(file.to_string()),
                        )
                        .to_value(),
                    );
                }
            }
            defaults.insert(Key::from("bind"), Value::Array(bind));
        }

        Ok(defaults)
    }

    /// `parseDefinition($id, $service, $file, $defaults, $return,
    /// $trackBindings)`.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub(crate) fn parse_definition(
        &self,
        fl: &FileLoader,
        id: &str,
        service: &Value,
        file: &str,
        defaults: &PhpArray,
        r#return: bool,
        track_bindings: bool,
    ) -> CResult<Option<Parsed>> {
        if id.starts_with('_')
            && id[1..]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(inv(format!(
                "Service names that start with an underscore are reserved. Rename the \"{id}\" service or define it in XML instead."
            )));
        }

        if let Value::String(target) = service
            && target.as_bytes().starts_with(b"@")
        {
            let alias = Alias::new(
                String::from_utf8_lossy(&target.as_bytes()[1..]).into_owned(),
                false,
            );

            if let Some(public) = opt(defaults, "public") {
                alias.set_public(public.is_truthy());
            }

            return alias_result(fl, id, alias, r#return);
        }

        let mut service = match service {
            Value::Array(a) if is_using_short_syntax(a) => {
                php_array!["arguments" => Value::Array(a.clone())]
            }
            Value::Array(a) => a.clone(),
            Value::Null => PhpArray::new(),
            other => {
                return Err(inv(format!(
                    "A service definition must be an array or a string starting with \"@\" but \"{}\" found for service \"{id}\" in \"{file}\".",
                    other.debug_type()
                )));
            }
        };

        if isset(&service, "stack") {
            let Some(Value::Array(frames)) = get(&service, "stack").cloned() else {
                return Err(inv(format!(
                    "A stack must be an array of definitions, \"array\" given for service \"{id}\" in \"{file}\"."
                )));
            };

            let mut stack = PhpArray::new();

            for (k, frame) in frames.iter() {
                let mut frame = frame.clone();
                if let Value::Array(a) = &frame
                    && a.len() == 1
                    && let Some((key, value)) = a.first()
                    && !SERVICE_KEYWORDS.contains(&key_string(key).as_str())
                {
                    frame = Value::Array(php_array![
                        "class" => key_value(key),
                        "arguments" => value.clone(),
                    ]);
                }

                if let Value::Array(a) = &frame
                    && isset(a, "stack")
                {
                    return Err(inv(format!(
                        "Service stack \"{id}\" cannot contain another stack in \"{file}\"."
                    )));
                }

                let frame_id = format!("{id}\" at index \"{}", key_string(k));
                let definition =
                    self.parse_definition(fl, &frame_id, &frame, file, defaults, true, true)?;

                if let Some(Parsed::Definition(d)) = &definition {
                    let instanceof = fl.state().instanceof.clone();
                    d.set_instanceof_conditionals(instanceof);
                }

                stack.insert(
                    k.clone(),
                    definition.as_ref().map_or(Value::Null, Parsed::to_value),
                );
            }

            let allowed = [
                "stack",
                "public",
                "deprecated",
                "decorates",
                "decorates_tag",
                "decoration_inner_name",
                "decoration_priority",
                "decoration_on_invalid",
            ];
            let diff: Vec<String> = service
                .keys()
                .map(key_string)
                .filter(|k| !allowed.contains(&k.as_str()))
                .collect();
            if !diff.is_empty() {
                return Err(inv(format!(
                    "Invalid attribute \"{}\"; supported ones are \"public\", \"deprecated\", \"decorates\", \"decorates_tag\" and \"decoration_*\" for service \"{id}\" in \"{file}\".",
                    diff.join("\", \"")
                )));
            }

            let field = |key: &str| get(&service, key).cloned().unwrap_or_default();
            service = php_array![
                "parent" => "",
                "arguments" => Value::Array(stack),
                "tags" => Value::Array(PhpArray::list([Value::from("container.stack")])),
                "public" => field("public"),
                "deprecated" => field("deprecated"),
                "decorates" => field("decorates"),
                "decorates_tag" => field("decorates_tag"),
                "decoration_inner_name" => field("decoration_inner_name"),
                "decoration_priority" => field("decoration_priority"),
                "decoration_on_invalid" => field("decoration_on_invalid"),
            ];
        }

        let mut definition = None;
        if let Some(first) = get(&service, "0").and_then(Definition::from_value) {
            // array_shift(): the first element out, the int keys renumbered
            let mut rest = PhpArray::new();
            for (k, v) in service.iter().skip(1) {
                match k {
                    Key::Int(_) => {
                        let _ = rest.push(v.clone());
                    }
                    Key::Str(_) => rest.insert(k.clone(), v.clone()),
                }
            }
            service = rest;
            definition = Some(first);
        }
        let r#return = definition.is_some() || r#return;

        if isset(&service, "from_callable") {
            for key in [
                "alias",
                "parent",
                "synthetic",
                "factory",
                "file",
                "arguments",
                "properties",
                "configurator",
                "calls",
            ] {
                if isset(&service, "factory") {
                    return Err(inv(format!(
                        "The configuration key \"{key}\" is unsupported for the service \"{id}\" when using \"from_callable\" in \"{file}\"."
                    )));
                }

                if isset(&service, key) {
                    trigger_deprecation(
                        b"symfony/dependency-injection",
                        b"8.1",
                        format!(
                            "Configuring the \"{key}\" key for the service \"{id}\" when using \"from_callable\" is deprecated and will throw an \"InvalidArgumentException\" in 9.0."
                        )
                        .as_bytes(),
                    );
                }
            }

            if !isset(&service, "class") {
                service.insert(Key::from("class"), Value::from("Closure"));
            }
            if !matches!(get(&service, "class"), Some(Value::String(c)) if c.as_bytes() == b"Closure")
            {
                service.insert(Key::from("lazy"), Value::Bool(true));
            }

            service.insert(
                Key::from("factory"),
                Value::Array(PhpArray::list([
                    Value::from("Closure"),
                    Value::from("fromCallable"),
                ])),
            );
            let callable = get(&service, "from_callable").cloned().unwrap_or_default();
            service.insert(
                Key::from("arguments"),
                Value::Array(PhpArray::list([callable])),
            );
            service.remove(&Key::from("from_callable"));
        }

        self.check_definition(fl, id, &service, file)?;

        if let Some(target) = opt(&service, "alias") {
            let alias = Alias::new(s(target), false);

            if let Some(public) = opt(&service, "public") {
                alias.set_public(public.is_truthy());
            } else if let Some(public) = opt(defaults, "public") {
                alias.set_public(public.is_truthy());
            }

            for (key, value) in service.iter() {
                let key = key_string(key);
                if !["alias", "public", "deprecated"].contains(&key.as_str()) {
                    return Err(inv(format!(
                        "The configuration key \"{key}\" is unsupported for the service \"{id}\" which is defined as an alias in \"{file}\". Allowed configuration keys for service aliases are \"alias\", \"public\" and \"deprecated\"."
                    )));
                }

                if key == "deprecated" {
                    let (package, version, message) = deprecation(value, file)?;
                    alias
                        .set_deprecated(&package, &version, &message)
                        .map_err(cfg)?;
                }
            }

            return alias_result(fl, id, alias, r#return);
        }

        let mut changes = Vec::new();
        let definition = match definition {
            Some(definition) => {
                changes = definition.changes();
                definition
            }
            None if fl.state().is_loading_instanceof => Definition::child(""),
            None => match opt(&service, "parent") {
                Some(parent) => {
                    let parent = s(parent);
                    if let Some(stripped) = parent.strip_prefix('@') {
                        return Err(inv(format!(
                            "The value of the \"parent\" option for the \"{id}\" service must be the id of the service without the \"@\" prefix (replace \"{parent}\" with \"{stripped}\")."
                        )));
                    }

                    Definition::child(&parent)
                }
                None => Definition::default(),
            },
        };

        if let Some(public) = opt(defaults, "public") {
            definition.set_public(public.is_truthy());
        }
        if let Some(autowire) = opt(defaults, "autowire") {
            definition.set_autowired(autowire.is_truthy());
        }
        if let Some(autoconfigure) = opt(defaults, "autoconfigure") {
            definition.set_autoconfigured(autoconfigure.is_truthy());
        }

        definition.set_changes(changes);

        if let Some(class) = opt(&service, "class") {
            definition.set_class(Some(&s(class)));
        }

        if let Some(shared) = opt(&service, "shared") {
            definition.set_shared(shared.is_truthy());
        }

        if let Some(synthetic) = opt(&service, "synthetic") {
            definition.set_synthetic(synthetic.is_truthy());
        }

        if let Some(lazy) = opt(&service, "lazy") {
            definition.set_lazy(lazy.is_truthy());
            if let Value::String(interface) = lazy {
                definition.add_tag(
                    "proxy",
                    php_array!["interface" => Value::String(interface.clone())],
                );
            }
        }

        if let Some(public) = opt(&service, "public") {
            definition.set_public(public.is_truthy());
        }

        if let Some(r#abstract) = opt(&service, "abstract") {
            definition.set_abstract(r#abstract.is_truthy());
        }

        if let Some(deprecated) = opt(&service, "deprecated") {
            let (package, version, message) = deprecation(deprecated, file)?;
            definition
                .set_deprecated(&package, &version, &message)
                .map_err(cfg)?;
        }

        if let Some(factory) = opt(&service, "factory") {
            definition.set_factory(self.parse_callable(fl, factory, "factory", id, file)?);
        }

        if let Some(constructor) = opt(&service, "constructor") {
            if !definition.factory().is_null() {
                return Err(cfg(Error::logic(format!(
                    "The \"{id}\" service cannot declare a factory as well as a constructor."
                ))));
            }

            definition.set_factory(Value::Array(PhpArray::list([
                Value::Null,
                constructor.clone(),
            ])));
        }

        if let Some(f) = opt(&service, "file") {
            definition.set_file(Some(&s(f)));
        }

        if let Some(arguments) = opt(&service, "arguments") {
            let arguments = self.resolve_services(fl, arguments, file, false)?;
            definition.set_arguments(array_argument(&arguments, "setArguments")?);
        }

        if let Some(properties) = opt(&service, "properties") {
            let properties = self.resolve_services(fl, properties, file, false)?;
            definition.set_properties(array_argument(&properties, "setProperties")?);
        }

        if let Some(configurator) = opt(&service, "configurator") {
            definition.set_configurator(self.parse_callable(
                fl,
                configurator,
                "configurator",
                id,
                file,
            )?);
        }

        if let Some(calls) = opt(&service, "calls") {
            let Value::Array(calls) = calls else {
                return Err(inv(format!(
                    "Parameter \"calls\" must be an array for service \"{id}\" in \"{file}\"."
                )));
            };

            for (k, call) in calls.iter() {
                let string_key = matches!(k, Key::Str(_));
                let call_tag = tagged(call);
                if !matches!(call, Value::Array(_)) && (!string_key || call_tag.is_none()) {
                    let given = match call_tag {
                        Some(t) => format!("!{}", t.get_tag()),
                        None => call.debug_type(),
                    };
                    return Err(inv(format!(
                        "Invalid method call for service \"{id}\": expected map or array, \"{given}\" given in \"{file}\"."
                    )));
                }

                if string_key {
                    return Err(inv(format!(
                        "Invalid method call for service \"{id}\", did you forget a leading dash before \"{}: ...\" in \"{file}\"?",
                        key_string(k)
                    )));
                }
                let Value::Array(call) = call else { continue };

                let (method, args, returns_clone) = if let Some(Value::String(method)) =
                    opt(call, "method")
                {
                    (
                        Value::String(method.clone()),
                        get(call, "arguments")
                            .filter(|v| !v.is_null())
                            .cloned()
                            .unwrap_or(Value::Array(PhpArray::new())),
                        opt(call, "returns_clone").is_some_and(Value::is_truthy),
                    )
                } else if call.len() == 1
                    && let Some((Key::Str(method), args)) = call.first()
                {
                    let method = Value::String(method.clone());
                    match tagged(args) {
                        Some(t) => {
                            if t.get_tag() != "returns_clone" {
                                return Err(inv(format!(
                                    "Unsupported tag \"!{}\", did you mean \"!returns_clone\" for service \"{id}\" in \"{file}\"?",
                                    t.get_tag()
                                )));
                            }

                            (method, t.get_value().clone(), true)
                        }
                        None => (method, args.clone(), false),
                    }
                } else if !get(call, "0").is_some_and(Value::is_truthy) {
                    return Err(inv(format!(
                        "Invalid call for service \"{id}\": the method must be defined as the first index of an array or as the only key of a map in \"{file}\"."
                    )));
                } else {
                    (
                        get(call, "0").cloned().unwrap_or_default(),
                        opt(call, "1")
                            .cloned()
                            .unwrap_or(Value::Array(PhpArray::new())),
                        opt(call, "2").is_some_and(Value::is_truthy),
                    )
                };

                let Value::Array(args) = args else {
                    return Err(inv(format!(
                        "The second parameter for function call \"{}\" must be an array of its arguments for service \"{id}\" in \"{file}\".",
                        s(&method)
                    )));
                };

                let args = self.resolve_services(fl, &Value::Array(args), file, false)?;
                definition
                    .add_method_call(
                        &s(&method),
                        array_argument(&args, "addMethodCall")?,
                        returns_clone,
                    )
                    .map_err(cfg)?;
            }
        }

        for r#type in ["tags", "resource_tags"] {
            let mut tags = match opt(&service, r#type) {
                None => PhpArray::new(),
                Some(Value::Array(tags)) => tags.clone(),
                Some(_) => {
                    return Err(inv(format!(
                        "Parameter \"{type}\" must be an array for service \"{id}\" in \"{file}\"."
                    )));
                }
            };

            if let Some(Value::Array(default_tags)) = opt(defaults, r#type) {
                tags = array_merge(&tags, default_tags);
            }

            for tag in tags.values() {
                let (name, tag) = normalize_tag(tag).ok_or_else(|| {
                    inv(format!(
                        "A \"{type}\" entry is missing a \"name\" key for service \"{id}\" in \"{file}\"."
                    ))
                })?;

                let name = match name {
                    Value::String(n) if !n.is_empty() => n.to_string_lossy().into_owned(),
                    _ => {
                        return Err(inv(format!(
                            "The tag name for service \"{id}\" in \"{file}\" must be a non-empty string."
                        )));
                    }
                };

                let message = format!(
                    "A \"{}\" attribute must be of a scalar-type for service \"{}\", tag \"%s\", attribute \"{}\" in \"{}\".",
                    id.replace('%', "%%"),
                    name.replace('%', "%%"),
                    r#type,
                    file.replace('%', "%%")
                );
                validate_attributes(&message, &tag, &[])?;

                match r#type {
                    "tags" => definition.add_tag(&name, tag),
                    _ => definition.add_resource_tag(&name, tag),
                };
            }
        }

        if let Some(decorates) = opt(&service, "decorates") {
            let decorates = s(decorates);
            if let Some(stripped) = decorates.strip_prefix('@') {
                return Err(inv(format!(
                    "The value of the \"decorates\" option for the \"{id}\" service must be the id of the service without the \"@\" prefix (replace \"{decorates}\" with \"{stripped}\")."
                )));
            }

            let invalid_behavior = decoration_on_invalid(&service, id, file)?;

            let rename_id = opt(&service, "decoration_inner_name").map(s);
            let priority = opt(&service, "decoration_priority").map_or(0, int);

            definition
                .set_decorated_service(
                    Some(&decorates),
                    rename_id.as_deref(),
                    priority,
                    invalid_behavior,
                )
                .map_err(cfg)?;
        }

        if let Some(decorates_tag) = opt(&service, "decorates_tag") {
            if isset(&service, "decorates") {
                return Err(inv(format!(
                    "A service cannot have both \"decorates\" and \"decorates_tag\" attributes on service \"{id}\" in \"{file}\"."
                )));
            }

            let priority = opt(&service, "decoration_priority")
                .cloned()
                .unwrap_or(Value::Int(0));

            let invalid_behavior = decoration_on_invalid(&service, id, file)?;

            let mut tag_attributes = php_array![
                "decorates_tag" => decorates_tag.clone(),
                "priority" => priority,
            ];

            if invalid_behavior != InvalidBehavior::Exception {
                tag_attributes.insert(
                    Key::from("on_invalid"),
                    Value::Int(invalid_behavior.as_int()),
                );
            }

            definition.add_resource_tag("container.tag_decorator", tag_attributes);
        }

        if let Some(autowire) = opt(&service, "autowire") {
            definition.set_autowired(autowire.is_truthy());
        }

        if isset(defaults, "bind") || isset(&service, "bind") {
            // deep clone, to avoid multiple process of the same instance in the passes
            let mut bindings = definition.bindings();
            if let Some(Value::Array(default_bind)) = opt(defaults, "bind")
                && let Value::Array(cloned) =
                    deep_clone(&Value::Array(default_bind.clone())).map_err(cfg)?
            {
                for (k, v) in cloned.iter() {
                    if !bindings.contains_key(k) {
                        bindings.insert(k.clone(), v.clone());
                    }
                }
            }

            if let Some(bind) = opt(&service, "bind") {
                let Value::Array(_) = bind else {
                    return Err(inv(format!(
                        "Parameter \"bind\" must be an array for service \"{id}\" in \"{file}\"."
                    )));
                };

                if let Value::Array(resolved) = self.resolve_services(fl, bind, file, false)? {
                    bindings = array_merge(&bindings, &resolved);
                }
                let binding_type = if fl.state().is_loading_instanceof {
                    INSTANCEOF_BINDING
                } else {
                    SERVICE_BINDING
                };
                let keys: Vec<Key> = bindings.keys().cloned().collect();
                for argument in keys {
                    let value = bindings.get(&argument).cloned().unwrap_or_default();
                    if BoundArgument::from_value(&value).is_none() {
                        bindings.insert(
                            argument,
                            BoundArgument::new(
                                value,
                                track_bindings,
                                binding_type,
                                Some(file.to_string()),
                            )
                            .to_value(),
                        );
                    }
                }
            }

            definition.set_bindings(&bindings);
        }

        if let Some(autoconfigure) = opt(&service, "autoconfigure") {
            definition.set_autoconfigured(autoconfigure.is_truthy());
        }

        if service.contains_key(&Key::from("namespace"))
            && !service.contains_key(&Key::from("resource"))
        {
            return Err(inv(format!(
                "A \"resource\" attribute must be set when the \"namespace\" attribute is set for service \"{id}\" in \"{file}\"."
            )));
        }

        if r#return {
            if service.contains_key(&Key::from("resource")) {
                return Err(inv(format!(
                    "Invalid \"resource\" attribute found for service \"{id}\" in \"{file}\"."
                )));
            }

            return Ok(Some(Parsed::Definition(definition)));
        }

        if let Some(resource) = get(&service, "resource") {
            let Value::String(resource) = resource else {
                return Err(inv(format!(
                    "A \"resource\" attribute must be of type string for service \"{id}\" in \"{file}\"."
                )));
            };
            let exclude = opt(&service, "exclude").cloned().unwrap_or_default();
            let namespace = opt(&service, "namespace").map_or_else(|| id.to_string(), s);
            fl.register_classes(
                &definition,
                &namespace,
                &resource.to_string_lossy(),
                &exclude,
                Some(file),
            )
            .map_err(cfg)?;
        } else {
            fl.set_definition(id, &definition).map_err(cfg)?;
        }

        Ok(None)
    }

    /// `parseCallable($callable, $parameter, $id, $file)`.
    fn parse_callable(
        &self,
        fl: &FileLoader,
        callable: &Value,
        parameter: &str,
        id: &str,
        file: &str,
    ) -> CResult<Value> {
        if Reference::from_value(callable).is_some() {
            return Ok(Value::Array(PhpArray::list([
                callable.clone(),
                Value::from("__invoke"),
            ])));
        }

        if let Value::String(string) = callable {
            let bytes = string.as_bytes();
            if bytes.starts_with(b"@=") {
                if parameter != "factory" {
                    return Err(inv(format!(
                        "Using expressions in \"{parameter}\" for the \"{id}\" service is not supported in \"{file}\"."
                    )));
                }
                return Err(cfg(Error::new(
                    ErrorKind::Other("LogicException".to_string()),
                    EXPRESSION_LANGUAGE_MISSING,
                )));
            }

            if bytes.first() == Some(&b'@') {
                if !bytes.contains(&b':') {
                    return Ok(Value::Array(PhpArray::list([
                        self.resolve_services(fl, callable, file, false)?,
                        Value::from("__invoke"),
                    ])));
                }

                let callable = string.to_string_lossy();
                return Err(inv(format!(
                    "The value of the \"{parameter}\" option for the \"{id}\" service must be the id of the service without the \"@\" prefix (replace \"{callable}\" with \"{}\" in \"{file}\").",
                    &callable[1..]
                )));
            }

            return Ok(callable.clone());
        }

        if let Value::Array(array) = callable {
            if isset(array, "0") && isset(array, "1") {
                let first = get(array, "0").cloned().unwrap_or_default();
                return Ok(Value::Array(PhpArray::list([
                    self.resolve_services(fl, &first, file, false)?,
                    get(array, "1").cloned().unwrap_or_default(),
                ])));
            }

            if parameter == "factory" && isset(array, "1") && opt(array, "0").is_none() {
                return Ok(callable.clone());
            }

            return Err(inv(format!(
                "Parameter \"{parameter}\" must contain an array with two elements for service \"{id}\" in \"{file}\"."
            )));
        }

        Err(inv(format!(
            "Parameter \"{parameter}\" must be a string or an array for service \"{id}\" in \"{file}\"."
        )))
    }

    /// `resolveServices($value, $file, $isParameter)`.
    #[allow(clippy::too_many_lines)]
    fn resolve_services(
        &self,
        fl: &FileLoader,
        value: &Value,
        file: &str,
        is_parameter: bool,
    ) -> CResult<Value> {
        if let Some(tagged_value) = tagged(value) {
            let argument = tagged_value.get_value();
            let tag = tagged_value.get_tag();

            if tag == "closure" {
                let argument = self.resolve_services(fl, argument, file, is_parameter)?;

                let definition = Definition::with_class("Closure");
                definition
                    .set_factory(Value::Array(PhpArray::list([
                        Value::from("Closure"),
                        Value::from("fromCallable"),
                    ])))
                    .add_argument(argument);
                return Ok(definition.to_value());
            }
            if tag == "iterator" {
                let Value::Array(_) = argument else {
                    return Err(inv(format!(
                        "\"!iterator\" tag only accepts sequences in \"{file}\"."
                    )));
                };
                let argument = self.resolve_services(fl, argument, file, is_parameter)?;

                return Ok(IteratorArgument::new(array_argument(
                    &argument,
                    "IteratorArgument::__construct",
                )?)
                .to_value());
            }
            if tag == "service_closure" {
                let argument = self.resolve_services(fl, argument, file, is_parameter)?;

                return Ok(ServiceClosureArgument::new(argument).to_value());
            }
            if tag == "env_closure" {
                let (env_expr, default, stringable, valid_keys) = match argument {
                    Value::Array(a) => (
                        get(a, "0").cloned().unwrap_or_default(),
                        get(a, "1").cloned().unwrap_or_default(),
                        get(a, "2")
                            .filter(|v| !v.is_null())
                            .cloned()
                            .unwrap_or(Value::Bool(true)),
                        a.keys().all(|k| matches!(k, Key::Int(0..=2))),
                    ),
                    other => (other.clone(), Value::Null, Value::Bool(false), true),
                };

                let (Value::String(env_expr), Value::Bool(stringable), true) =
                    (&env_expr, &stringable, valid_keys)
                else {
                    return Err(inv(format!(
                        "\"!env_closure\" tag only accepts a string value or an array [value, default, stringable] in \"{file}\"."
                    )));
                };

                let mut env_expr = Value::String(env_expr.clone());
                match lock(fl.container())
                    .get_parameter_bag()
                    .resolve_value(&env_expr, &[])
                {
                    Ok(resolved) => env_expr = resolved,
                    Err(e) if e.is(&ErrorKind::ParameterNotFound) => {}
                    Err(e) => return Err(cfg(e)),
                }

                let argument =
                    EnvClosureArgument::new(s(&env_expr), default, *stringable).map_err(cfg)?;
                return Ok(object_value(argument));
            }
            if tag == "service_locator" {
                let Value::Array(_) = argument else {
                    return Err(inv(format!(
                        "\"!service_locator\" tag only accepts maps in \"{file}\"."
                    )));
                };

                let argument = self.resolve_services(fl, argument, file, is_parameter)?;

                return Ok(ServiceLocatorArgument::new(array_argument(
                    &argument,
                    "ServiceLocatorArgument::__construct",
                )?)
                .to_value());
            }
            if tag == "tagged_iterator" || tag == "tagged_locator" {
                let for_locator = tag == "tagged_locator";

                let iterator = match argument {
                    Value::Array(a) if get(a, "tag").is_some_and(Value::is_truthy) => {
                        let supported_keys = [
                            "tag",
                            "index_by",
                            "default_index_method",
                            "default_priority_method",
                            "exclude",
                            "exclude_self",
                        ];
                        let diff: Vec<String> = a
                            .keys()
                            .filter(|k| {
                                !matches!(k, Key::Str(_))
                                    || !supported_keys.contains(&key_string(k).as_str())
                            })
                            .map(key_string)
                            .collect();
                        if !diff.is_empty() {
                            return Err(inv(format!(
                                "\"!{tag}\" tag contains unsupported key \"{}\"; supported ones are \"{}\".",
                                diff.join("\", \""),
                                supported_keys.join("\", \"")
                            )));
                        }

                        let tag_name = s(get(a, "tag").unwrap_or(&Value::Null));
                        let index_by = opt(a, "index_by").map(s);
                        let exclude: Vec<String> = match opt(a, "exclude") {
                            None => Vec::new(),
                            Some(Value::Array(e)) => e.values().map(s).collect(),
                            Some(other) => vec![s(other)],
                        };
                        let exclude_self = opt(a, "exclude_self").is_none_or(Value::is_truthy);

                        if a.contains_key(&Key::from("default_index_method"))
                            || a.contains_key(&Key::from("default_priority_method"))
                        {
                            let index_method = opt(a, "default_index_method").map(s);
                            let priority_method = opt(a, "default_priority_method").map(s);
                            IteratorArgument::tagged_legacy(
                                &tag_name,
                                index_by.as_deref(),
                                index_method.as_deref(),
                                for_locator,
                                priority_method.as_deref(),
                                exclude,
                                exclude_self,
                            )
                        } else {
                            IteratorArgument::tagged(
                                &tag_name,
                                index_by.as_deref(),
                                for_locator,
                                exclude,
                                exclude_self,
                            )
                        }
                    }
                    Value::String(t) if argument.is_truthy() => IteratorArgument::tagged(
                        &t.to_string_lossy(),
                        None,
                        for_locator,
                        Vec::new(),
                        true,
                    ),
                    _ => {
                        return Err(inv(format!(
                            "\"!{tag}\" tags only accept a non empty string or an array with a key \"tag\" in \"{file}\"."
                        )));
                    }
                };

                if for_locator {
                    return Ok(ServiceLocatorArgument::from_tagged(iterator).to_value());
                }

                return Ok(iterator.to_value());
            }
            if tag == "service" {
                if is_parameter {
                    return Err(inv(format!(
                        "Using an anonymous service in a parameter is not allowed in \"{file}\"."
                    )));
                }

                let (is_loading_instanceof, instanceof) = {
                    let mut state = fl.state();
                    let saved = (
                        state.is_loading_instanceof,
                        std::mem::take(&mut state.instanceof),
                    );
                    state.is_loading_instanceof = false;
                    saved
                };

                let class = match argument {
                    Value::Array(a) => opt(a, "class").map(s).unwrap_or_default(),
                    _ => String::new(),
                };
                let short = class.rsplit('\\').next().unwrap_or_default();
                let id = {
                    let mut anonymous = self.anonymous();
                    anonymous.0 += 1;
                    format!(".{}_{short}{}", anonymous.0, anonymous.1)
                };
                self.parse_definition(fl, &id, argument, file, &PhpArray::new(), false, true)?;

                if !lock(fl.container()).has_definition(&id) {
                    return Err(inv(format!(
                        "Creating an alias using the tag \"!service\" is not allowed in \"{file}\"."
                    )));
                }

                {
                    let mut state = fl.state();
                    state.is_loading_instanceof = is_loading_instanceof;
                    state.instanceof = instanceof;
                }

                return Ok(Reference::new(id).into());
            }
            if tag == "abstract" {
                return Ok(AbstractArgument::new(&s(argument)).to_value());
            }

            return Err(inv(format!("Unsupported tag \"!{tag}\".")));
        }

        match value {
            Value::Array(array) => {
                let mut out = PhpArray::new();
                for (k, v) in array.iter() {
                    out.insert(k.clone(), self.resolve_services(fl, v, file, is_parameter)?);
                }
                Ok(Value::Array(out))
            }
            Value::String(string) if string.as_bytes().starts_with(b"@=") => {
                if is_parameter {
                    return Err(inv(format!(
                        "Using expressions in parameters is not allowed in \"{file}\"."
                    )));
                }

                Err(cfg(Error::new(
                    ErrorKind::Other("LogicException".to_string()),
                    EXPRESSION_LANGUAGE_MISSING,
                )))
            }
            Value::String(string) if string.as_bytes().starts_with(b"@") => {
                let bytes = string.as_bytes();
                if bytes.starts_with(b"@>") {
                    let mut rest = vec![b'@'];
                    rest.extend_from_slice(&bytes[2..]);
                    let argument =
                        self.resolve_services(fl, &Value::string(rest), file, is_parameter)?;

                    return Ok(ServiceClosureArgument::new(argument).to_value());
                }
                let (id, behavior) = if bytes.starts_with(b"@@") {
                    (&bytes[1..], None)
                } else if bytes.starts_with(b"@!") {
                    (&bytes[2..], Some(InvalidBehavior::IgnoreUninitialized))
                } else if bytes.starts_with(b"@?") {
                    (&bytes[2..], Some(InvalidBehavior::Ignore))
                } else {
                    (&bytes[1..], Some(InvalidBehavior::Exception))
                };

                Ok(match behavior {
                    Some(behavior) => {
                        Reference::with_behavior(String::from_utf8_lossy(id).into_owned(), behavior)
                            .into()
                    }
                    None => Value::string(id.to_vec()),
                })
            }
            other => Ok(other.clone()),
        }
    }

    #[expect(clippy::unused_self, reason = "a method of the trait upstream")]
    fn load_from_extensions(&self, fl: &FileLoader, content: &PhpArray) -> CResult<()> {
        for (namespace, values) in content.iter() {
            let namespace = key_string(namespace);
            if ["imports", "parameters", "services"].contains(&namespace.as_str()) {
                continue;
            }
            if let Some(env) = namespace.strip_prefix("when@") {
                let mut container = lock(fl.container());
                let mut known = PhpArray::new();
                if container.has_parameter(".container.known_envs")
                    && let Value::Array(envs) = container
                        .get_parameter(".container.known_envs")
                        .map_err(cfg)?
                {
                    // array_flip()
                    for v in envs.values() {
                        known.insert(Key::from_bytes(s(v).as_bytes()), Value::Bool(true));
                    }
                }
                let key = Key::from_bytes(env.as_bytes());
                if !known.contains_key(&key) {
                    known.insert(key, Value::Bool(true));
                }
                let envs = PhpArray::list(known.keys().map(key_value));
                container
                    .set_parameter(".container.known_envs", Value::Array(envs))
                    .map_err(cfg)?;
                continue;
            }

            let values = match values {
                Value::Array(a) => a.clone(),
                _ => PhpArray::new(),
            };

            fl.load_extension_config(&namespace, values).map_err(cfg)?;
        }

        fl.load_extension_configs()
    }

    #[expect(clippy::unused_self, reason = "a method of the trait upstream")]
    fn check_definition(
        &self,
        fl: &FileLoader,
        id: &str,
        definition: &PhpArray,
        file: &str,
    ) -> CResult<()> {
        let keywords: &[&str] = if fl.state().is_loading_instanceof {
            &INSTANCEOF_KEYWORDS
        } else if isset(definition, "resource") || isset(definition, "namespace") {
            &PROTOTYPE_KEYWORDS
        } else {
            &SERVICE_KEYWORDS
        };

        for key in definition.keys() {
            let key = key_string(key);
            if !keywords.contains(&key.as_str()) {
                return Err(inv(format!(
                    "The configuration key \"{key}\" is unsupported for definition \"{id}\" in \"{file}\". Allowed configuration keys are \"{}\".",
                    keywords.join("\", \"")
                )));
            }
        }
        Ok(())
    }
}

/// The `foreach ($tags as $tag)` normalization of a tag: its name and
/// attributes; `None` when the name is missing.
fn normalize_tag(tag: &Value) -> Option<(Value, PhpArray)> {
    let mut tag = match tag {
        Value::Array(a) => a.clone(),
        other => php_array!["name" => other.clone()],
    };

    if tag.len() == 1
        && let Some((key, Value::Array(attributes))) = tag.first()
    {
        return Some((key_value(key), attributes.clone()));
    }

    let name = opt(&tag, "name")?.clone();
    tag.remove(&Key::from("name"));
    Some((name, tag))
}

/// `validateAttributes($message, $attributes, $path)`.
fn validate_attributes(message: &str, attributes: &PhpArray, path: &[String]) -> CResult<()> {
    for (name, value) in attributes.iter() {
        let mut full = path.to_vec();
        full.push(key_string(name));
        if let Value::Array(nested) = value {
            validate_attributes(message, nested, &full)?;
        } else if !is_scalar_or_null(value) {
            return Err(inv(sprintf(message, &full.join("."))?));
        }
    }
    Ok(())
}

/// `isUsingShortSyntax($service)`.
fn is_using_short_syntax(service: &PhpArray) -> bool {
    for key in service.keys() {
        if let Key::Str(key) = key {
            let key = key.as_bytes();
            if key.is_empty() || (key[0] != b'$' && !key.contains(&b'\\')) {
                return false;
            }
        }
    }

    true
}

/// The `deprecated` option: package, version and message, or the
/// missing-attribute errors.
fn deprecation(value: &Value, file: &str) -> CResult<(String, String, String)> {
    let deprecation = match value {
        Value::Array(a) => a.clone(),
        other => php_array!["message" => other.clone()],
    };

    let Some(package) = opt(&deprecation, "package") else {
        return Err(inv(format!(
            "Missing attribute \"package\" of the \"deprecated\" option in \"{file}\"."
        )));
    };

    let Some(version) = opt(&deprecation, "version") else {
        return Err(inv(format!(
            "Missing attribute \"version\" of the \"deprecated\" option in \"{file}\"."
        )));
    };

    Ok((
        s(package),
        s(version),
        opt(&deprecation, "message").map(s).unwrap_or_default(),
    ))
}

/// The `match ($decorationOnInvalid)` of `decorates` and `decorates_tag`.
fn decoration_on_invalid(service: &PhpArray, id: &str, file: &str) -> CResult<InvalidBehavior> {
    let value = if service.contains_key(&Key::from("decoration_on_invalid")) {
        get(service, "decoration_on_invalid")
            .cloned()
            .unwrap_or_default()
    } else {
        Value::from("exception")
    };
    match &value {
        Value::String(v) if v.as_bytes() == b"exception" => Ok(InvalidBehavior::Exception),
        Value::Int(1) => Ok(InvalidBehavior::Exception),
        Value::String(v) if v.as_bytes() == b"ignore" => Ok(InvalidBehavior::Ignore),
        Value::Int(3) => Ok(InvalidBehavior::Ignore),
        Value::Null | Value::Int(2) => Ok(InvalidBehavior::Null),
        Value::String(v) if v.as_bytes() == b"null" => Err(inv(format!(
            "Invalid value \"null\" for attribute \"decoration_on_invalid\" on service \"{id}\". Did you mean null (without quotes) in \"{file}\"?"
        ))),
        other => Err(inv(format!(
            "Invalid value \"{}\" for attribute \"decoration_on_invalid\" on service \"{id}\". Did you mean \"exception\", \"ignore\" or \"null\" in \"{file}\"?",
            s(other)
        ))),
    }
}

/// `(int) $value`, PHP's `int` coercion of a parameter.
fn int(value: &Value) -> i64 {
    match value {
        Value::Int(i) => *i,
        #[allow(clippy::cast_possible_truncation)]
        Value::Float(f) => *f as i64,
        Value::Bool(b) => i64::from(*b),
        Value::String(s) => std::str::from_utf8(s.as_bytes())
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0),
        _ => 0,
    }
}

fn alias_result(
    fl: &FileLoader,
    id: &str,
    alias: Alias,
    r#return: bool,
) -> CResult<Option<Parsed>> {
    if r#return {
        return Ok(Some(Parsed::Alias(alias)));
    }
    lock(fl.container()).set_alias(id, alias).map_err(cfg)?;
    Ok(None)
}
