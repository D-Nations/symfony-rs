//! Port of Symfony\Component\DependencyInjection\Compiler\CheckTypeDeclarationsPass.php.
//!
//! Checks whether injected values are compatible with the type
//! declarations of constructors, factories and method calls. Reflection
//! reads the container's class registry: a parameter's declared type is
//! parsed from its metadata (`?Foo`, `Foo|int`, `Foo&Bar`,
//! `(Foo&Bar)|null`), and "loaded" classes (`class_exists($class,
//! false)`, used when `autoload` is off) are the registered ones.
//!
//! What PHP knows without the registry is a small table of the internal
//! and component classes the pass itself names (`Closure`,
//! `RewindableGenerator`, `ServiceLocator`, `EnvClosure`, the
//! `Traversable` family, `stdClass`, `WeakMap`, ...); a registered class
//! wins over the table. A class that declares `__toString()` is
//! `Stringable`, as in PHP. `is_callable()` on a string knows the
//! registered functions and public static methods only.
//!
//! The port has no `ExpressionLanguage` and no `Expression` values, so
//! the expression branch of `checkType()` does not exist.
//!
//! Upstream: symfony/dependency-injection Compiler/CheckTypeDeclarationsPass.php

use std::collections::HashSet;

use php_value::{PhpArray, Value, array::Key};

use super::abstract_recursive_pass::{
    AbstractRecursivePass, RecursivePassState, recursive_process,
};
use super::compiler_pass_interface::CompilerPassInterface;
use crate::argument::{
    EnvClosureArgument, IteratorArgument, ServiceClosureArgument, ServiceLocatorArgument,
};
use crate::class_metadata::{ClassKind, ClassRegistry, FunctionReflector, ParameterMetadata};
use crate::container_builder::{ContainerBuilder, EnvFormat};
use crate::definition::Definition;
use crate::exception::{Error, ErrorKind, Result};
use crate::reference::{Parameter, Reference};

const SCALAR_TYPES: [&str; 4] = ["int", "float", "bool", "string"];

const BUILTIN_TYPES: [&str; 8] = [
    "array", "bool", "callable", "float", "int", "iterable", "object", "string",
];

const SERVICE_LOCATOR: &str = "Symfony\\Component\\DependencyInjection\\ServiceLocator";
const REWINDABLE_GENERATOR: &str =
    "Symfony\\Component\\DependencyInjection\\Argument\\RewindableGenerator";
const ENV_CLOSURE: &str = "Symfony\\Component\\DependencyInjection\\Argument\\EnvClosure";
const CONTAINER: &str = "Symfony\\Component\\DependencyInjection\\Container";
const RUNTIME_EXCEPTION: &str =
    "Symfony\\Component\\DependencyInjection\\Exception\\RuntimeException";

/// The classes PHP knows without the registry: their parents and
/// interfaces, and whether they have `__invoke()`.
const KNOWN_CLASSES: &[(&str, &[&str], bool)] = &[
    ("stdClass", &[], false),
    ("Closure", &[], true),
    ("Traversable", &[], false),
    ("Iterator", &["Traversable"], false),
    ("IteratorAggregate", &["Traversable"], false),
    ("Generator", &["Iterator"], false),
    ("Countable", &[], false),
    ("ArrayAccess", &[], false),
    ("Stringable", &[], false),
    (
        "WeakMap",
        &["ArrayAccess", "Countable", "IteratorAggregate"],
        false,
    ),
    ("DateTimeInterface", &[], false),
    ("DateTime", &["DateTimeInterface"], false),
    ("DateTimeImmutable", &["DateTimeInterface"], false),
    ("Psr\\Container\\ContainerInterface", &[], false),
    (
        "Symfony\\Contracts\\Service\\ServiceProviderInterface",
        &["Psr\\Container\\ContainerInterface"],
        false,
    ),
    (
        "Symfony\\Contracts\\Service\\ServiceCollectionInterface",
        &[
            "Symfony\\Contracts\\Service\\ServiceProviderInterface",
            "Countable",
            "IteratorAggregate",
        ],
        false,
    ),
    ("Symfony\\Contracts\\Service\\ResetInterface", &[], false),
    (
        "Symfony\\Component\\DependencyInjection\\ContainerInterface",
        &["Psr\\Container\\ContainerInterface"],
        false,
    ),
    (
        CONTAINER,
        &[
            "Symfony\\Component\\DependencyInjection\\ContainerInterface",
            "Symfony\\Contracts\\Service\\ResetInterface",
        ],
        false,
    ),
    (
        SERVICE_LOCATOR,
        &["Symfony\\Contracts\\Service\\ServiceCollectionInterface"],
        true,
    ),
    (
        REWINDABLE_GENERATOR,
        &["IteratorAggregate", "Countable"],
        false,
    ),
    (ENV_CLOSURE, &["Stringable"], true),
];

fn key(name: &str) -> String {
    name.trim_start_matches('\\').to_ascii_lowercase()
}

fn known_class(name: &str) -> Option<&'static (&'static str, &'static [&'static str], bool)> {
    let name = key(name);
    KNOWN_CLASSES.iter().find(|(n, _, _)| key(n) == name)
}

/// `class_exists($class, false) || interface_exists($class, false)`.
fn is_loaded(registry: &ClassRegistry, class: &str) -> bool {
    registry
        .class(class)
        .is_some_and(|c| c.kind != ClassKind::Trait)
        || (registry.class(class).is_none() && known_class(class).is_some())
}

/// Every parent and interface of `class`, inherited ones included
/// (`Stringable` for a class that declares `__toString()`).
fn ancestors(registry: &ClassRegistry, class: &str) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::from([key(class)]);
    let mut found = Vec::new();
    let mut pending = vec![class.to_string()];
    while let Some(name) = pending.pop() {
        let mut direct: Vec<String> = Vec::new();
        if let Some(metadata) = registry.class(&name) {
            direct.extend(metadata.parent.iter().cloned());
            direct.extend(metadata.interfaces.iter().cloned());
            if metadata.declared_method("__toString").is_some() {
                direct.push("Stringable".to_string());
            }
        } else if let Some((_, parents, _)) = known_class(&name) {
            direct.extend(parents.iter().map(|p| (*p).to_string()));
        }
        for ancestor in direct {
            if seen.insert(key(&ancestor)) {
                found.push(ancestor.clone());
                pending.push(ancestor);
            }
        }
    }
    found
}

/// `is_a($class, $type, true)`.
fn is_a(registry: &ClassRegistry, class: &str, type_: &str) -> bool {
    key(class) == key(type_) || is_subclass_of(registry, class, type_)
}

/// `is_subclass_of($class, $type)`.
fn is_subclass_of(registry: &ClassRegistry, class: &str, type_: &str) -> bool {
    let type_ = key(type_);
    ancestors(registry, class).iter().any(|a| key(a) == type_)
}

/// `method_exists($class, $method)`.
fn method_exists(registry: &ClassRegistry, class: &str, method: &str) -> bool {
    if let Some(metadata) = registry.class(class) {
        return registry.find_method(&metadata, method).is_some();
    }
    method.eq_ignore_ascii_case("__invoke")
        && known_class(class).is_some_and(|(_, _, invoke)| *invoke)
}

/// `is_callable($value)` for a value that is not a closure or an array.
fn is_callable(registry: &ClassRegistry, value: &Value) -> bool {
    let Some(name) = value.as_string() else {
        return false;
    };
    let name = name.to_string_lossy();
    if let Some((class, method)) = name.split_once("::") {
        return registry.class(class).is_some_and(|metadata| {
            registry
                .find_method(&metadata, method)
                .is_some_and(|(_, m)| m.is_public() && m.is_static)
        });
    }
    registry.function(&name).is_some()
}

/// A declared type (`\ReflectionType`).
#[derive(Debug, Clone, PartialEq)]
enum Type {
    /// `\ReflectionNamedType`: `?Foo` is `Foo`, nullable.
    Named {
        name: String,
        nullable: bool,
    },
    Union(Vec<Type>),
    Intersection(Vec<Type>),
}

/// Splits `text` on `separator` outside parentheses.
fn split_top_level(text: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            c if c == separator && depth == 0 => {
                parts.push(&text[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

fn parse_member(text: &str) -> Type {
    let text = text.trim();
    let inner = text
        .strip_prefix('(')
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or(text);
    if inner.contains('&') {
        return Type::Intersection(
            inner
                .split('&')
                .map(|n| Type::Named {
                    name: n.trim().to_string(),
                    nullable: false,
                })
                .collect(),
        );
    }
    Type::Named {
        name: inner.to_string(),
        nullable: false,
    }
}

/// The type of a parameter, from its metadata.
fn parse_type(text: &str) -> Type {
    let text = text.trim();
    let parts = split_top_level(text, '|');
    if parts.len() > 1 {
        let members: Vec<Type> = parts.into_iter().map(parse_member).collect();
        // `Foo|null` is a nullable `\ReflectionNamedType`.
        if members.len() == 2 {
            let is_null = |t: &Type| matches!(t, Type::Named { name, .. } if name.eq_ignore_ascii_case("null"));
            if let Some(Type::Named { name, .. }) = members
                .iter()
                .find(|t| !is_null(t))
                .filter(|_| members.iter().any(is_null))
            {
                return Type::Named {
                    name: name.clone(),
                    nullable: true,
                };
            }
        }
        return Type::Union(members);
    }
    if let Some(name) = text.strip_prefix('?') {
        return Type::Named {
            name: name.trim().to_string(),
            nullable: true,
        };
    }
    parse_member(text)
}

/// `$parameter->allowsNull()`.
fn allows_null(type_: &Type) -> bool {
    let named_null = |t: &Type| matches!(t, Type::Named { name, nullable } if *nullable || name.eq_ignore_ascii_case("null") || name.eq_ignore_ascii_case("mixed"));
    match type_ {
        Type::Named { .. } => named_null(type_),
        Type::Union(members) => members.iter().any(named_null),
        Type::Intersection(_) => false,
    }
}

/// `$reflectionType->isBuiltin()`.
fn is_builtin(name: &str) -> bool {
    [
        "int", "float", "string", "bool", "array", "callable", "iterable", "object", "mixed",
        "void", "null", "never", "false", "true",
    ]
    .contains(&name.to_ascii_lowercase().as_str())
}

/// A parameter being checked: its metadata, position and function.
struct Param<'a> {
    metadata: &'a ParameterMetadata,
    position: usize,
    function: &'a FunctionReflector,
    type_: Type,
}

impl Param<'_> {
    /// `InvalidParameterTypeException($serviceId, $type, $parameter)`.
    fn error(&self, service_id: &str, passed: &str) -> Error {
        let accepted = match &self.type_ {
            Type::Named { name, .. } => name.clone(),
            _ => self.metadata.type_.clone().unwrap_or_default(),
        };
        let function = match self.function.class() {
            Some(class) if self.function.is_method() => {
                format!("{class}::{}", self.function.name())
            }
            _ => self.function.name().to_string(),
        };
        Error::invalid_parameter_type(service_id, passed, &function, self.position, &accepted)
    }
}

/// How `checkType()` fails: an `InvalidParameterTypeException` (with
/// its code, the passed type) or another error.
enum Fail {
    Mismatch(String),
    Error(Error),
}

impl From<Error> for Fail {
    fn from(e: Error) -> Self {
        Self::Error(e)
    }
}

/// `preg_replace('/'.$prefix.'_\w+_[a-f0-9]{32}/U', '', $value, -1, $c)`:
/// what is left, and the number of matches.
fn strip_env_placeholders(value: &[u8], prefix: &[u8]) -> (Vec<u8>, usize) {
    let is_word = |b: u8| b == b'_' || b.is_ascii_alphanumeric();
    let is_hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    // The (lazy) match starting at `start`, as its end.
    let match_at = |start: usize| -> Option<usize> {
        let rest = value.get(start..)?;
        if !rest.starts_with(prefix) || rest.get(prefix.len()) != Some(&b'_') {
            return None;
        }
        let word_start = start + prefix.len() + 1;
        let mut end = word_start;
        while end < value.len() && is_word(value[end]) {
            end += 1;
            // `\w+` matched value[word_start..end]; try `_[a-f0-9]{32}`.
            if value.get(end) == Some(&b'_')
                && value.len() >= end + 33
                && value[end + 1..end + 33].iter().all(|b| is_hex(*b))
            {
                return Some(end + 33);
            }
        }
        None
    };
    let mut out = Vec::new();
    let mut count = 0;
    let mut i = 0;
    while i < value.len() {
        if let Some(end) = match_at(i) {
            count += 1;
            i = end;
        } else {
            out.push(value[i]);
            i += 1;
        }
    }
    (out, count)
}

/// `preg_match('/^%([^%\s]+)%$/', $value)`, for a value starting with `%`.
fn is_whole_parameter(value: &[u8]) -> bool {
    let body = value.strip_suffix(b"\n").unwrap_or(value);
    let Some(inner) = body.strip_prefix(b"%").and_then(|b| b.strip_suffix(b"%")) else {
        return false;
    };
    !inner.is_empty()
        && inner
            .iter()
            .all(|b| *b != b'%' && !matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r'))
}

/// `CheckTypeDeclarationsPass`.
#[derive(Debug)]
pub struct CheckTypeDeclarationsPass {
    state: RecursivePassState,
    autoload: bool,
    skipped_ids: HashSet<String>,
}

impl Default for CheckTypeDeclarationsPass {
    fn default() -> Self {
        Self::new(false, Vec::new())
    }
}

impl CheckTypeDeclarationsPass {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Compiler\\CheckTypeDeclarationsPass";

    /// `new CheckTypeDeclarationsPass($autoload, $skippedIds)`; the
    /// skipped ids are the keys of upstream's array.
    pub fn new(autoload: bool, skipped_ids: Vec<String>) -> Self {
        let mut state = RecursivePassState::default();
        state.skip_scalars = true;
        Self {
            state,
            autoload,
            skipped_ids: skipped_ids.into_iter().collect(),
        }
    }

    fn current_id(&self) -> String {
        self.state.current_id.clone().unwrap_or_default()
    }

    /// `checkTypeDeclarations($checkedDefinition, $reflectionFunction, $values)`.
    fn check_type_declarations(
        &self,
        container: &mut ContainerBuilder,
        checked_definition: &Definition,
        reflection_function: &FunctionReflector,
        values: &PhpArray,
    ) -> Result<()> {
        let parameters = reflection_function.parameters();
        let number_of_required_parameters = parameters
            .iter()
            .rposition(|p| !p.is_optional)
            .map_or(0, |i| i + 1);

        if values.len() < number_of_required_parameters {
            return Err(Error::invalid_argument(format!(
                "Invalid definition for service \"{}\": \"{}::{}()\" requires {} arguments, {} passed.",
                self.current_id(),
                reflection_function.class().unwrap_or(""),
                reflection_function.name(),
                number_of_required_parameters,
                values.len()
            )));
        }

        let checks_count = parameters.len().min(values.len());

        let bag = container.get_parameter_bag();
        let env_placeholder_unique_prefix = bag
            .is_env_placeholder()
            .then(|| bag.get_env_placeholder_unique_prefix());

        let param = |position: usize| -> Option<Param<'_>> {
            let metadata = parameters.get(position)?;
            let type_ = parse_type(metadata.type_.as_deref()?);
            Some(Param {
                metadata,
                position,
                function: reflection_function,
                type_,
            })
        };

        for i in 0..checks_count {
            let Some(p) = param(i) else {
                continue;
            };
            if p.metadata.is_variadic {
                continue;
            }
            let by_name = Key::from(p.metadata.name.as_str());
            let value = if let Some(value) = values.get(&by_name) {
                value
            } else if let Some(value) = values.get(&Key::Int(i64::try_from(i).unwrap_or(i64::MAX)))
            {
                value
            } else {
                continue;
            };

            self.check(
                container,
                checked_definition,
                value,
                &p,
                env_placeholder_unique_prefix.as_deref(),
            )?;
        }

        if let Some(last) = parameters.len().checked_sub(1)
            && parameters[last].is_variadic
            && let Some(p) = param(last)
        {
            for variadic_parameter in values.values().skip(last) {
                self.check(
                    container,
                    checked_definition,
                    variadic_parameter,
                    &p,
                    env_placeholder_unique_prefix.as_deref(),
                )?;
            }
        }
        Ok(())
    }

    fn check(
        &self,
        container: &mut ContainerBuilder,
        checked_definition: &Definition,
        value: &Value,
        parameter: &Param<'_>,
        prefix: Option<&str>,
    ) -> Result<()> {
        match self.check_type(
            container,
            checked_definition,
            value,
            parameter,
            prefix,
            &parameter.type_,
        ) {
            Ok(()) => Ok(()),
            Err(Fail::Mismatch(passed)) => Err(parameter.error(&self.current_id(), &passed)),
            Err(Fail::Error(e)) => Err(e),
        }
    }

    /// `checkType($checkedDefinition, $value, $parameter, $envPlaceholderUniquePrefix, $reflectionType)`.
    #[expect(clippy::too_many_lines, reason = "one upstream method")]
    fn check_type(
        &self,
        container: &mut ContainerBuilder,
        checked_definition: &Definition,
        value: &Value,
        parameter: &Param<'_>,
        prefix: Option<&str>,
        reflection_type: &Type,
    ) -> std::result::Result<(), Fail> {
        let type_name = match reflection_type {
            Type::Union(members) => {
                let mut code = String::new();
                for t in members {
                    match self.check_type(
                        container,
                        checked_definition,
                        value,
                        parameter,
                        prefix,
                        t,
                    ) {
                        Ok(()) => return Ok(()),
                        Err(Fail::Mismatch(passed)) => code = passed,
                        Err(e) => return Err(e),
                    }
                }
                return Err(Fail::Mismatch(code));
            }
            Type::Intersection(members) => {
                for t in members {
                    self.check_type(container, checked_definition, value, parameter, prefix, t)?;
                }
                return Ok(());
            }
            Type::Named { name, .. } => name.clone(),
        };
        let registry = container.class_registry();
        let mut type_ = type_name.clone();
        let mut value = value.clone();

        if let Some(reference) = Reference::from_value(&value) {
            let id = reference.id().to_string();
            if !container.has(&id) {
                return Ok(());
            }
            if id == "service_container" && is_a(&registry, &type_, CONTAINER) {
                return Ok(());
            }
            value = container.find_definition(&id)?.to_value();
        }

        if type_ == "self" {
            type_ = parameter.function.class().unwrap_or("").to_string();
        }

        if type_ == "static" {
            type_ = checked_definition.class().unwrap_or_default();
        }

        let mut class: Option<String> = None;

        if let Some(definition) = Definition::from_value(&value) {
            if definition.has_errors() || definition.factory().is_truthy() {
                return Ok(());
            }

            let defined = definition.class().filter(|c| !c.is_empty() && c != "0");
            match defined {
                Some(c) if BUILTIN_TYPES.contains(&c.to_ascii_lowercase().as_str()) => {
                    class = Some(c.to_ascii_lowercase());
                }
                Some(c) if self.autoload || is_loaded(&registry, &c) => class = Some(c),
                _ => return Ok(()),
            }
        } else if let Some(p) = Parameter::from_value(&value) {
            value = container.get_parameter(p.id())?;
        } else if let Value::String(s) = &value {
            let bytes = s.as_bytes();
            if bytes.first() == Some(&b'%') && is_whole_parameter(bytes) {
                let name = String::from_utf8_lossy(&bytes[1..bytes.len() - 1]).into_owned();
                value = container.get_parameter(&name)?;
            }

            if let (Some(prefix), Value::String(s)) = (prefix.filter(|p| !p.is_empty()), &value)
                && s.as_bytes().windows(4).any(|w| w == b"env_")
            {
                // If the value is an env placeholder that is either mixed with a string or with another env placeholder, then its resolved value will always be a string, so we don't need to resolve it.
                // We don't need to change the value because it is already a string.
                let (rest, count) = strip_env_placeholders(s.as_bytes(), prefix.as_bytes());
                if rest.is_empty() && count == 1 {
                    match container.resolve_env_placeholders(
                        value.clone(),
                        &EnvFormat::Resolve,
                        None,
                    ) {
                        Ok(resolved) => value = resolved,
                        // If an env placeholder cannot be resolved, we skip the validation.
                        Err(_) => return Ok(()),
                    }
                }
            }
        }

        if value.is_null() && allows_null(&parameter.type_) {
            return Ok(());
        }

        let class = match class {
            Some(class) => class,
            None => {
                if IteratorArgument::from_value(&value).is_some() {
                    REWINDABLE_GENERATOR.to_string()
                } else if ServiceClosureArgument::from_value(&value).is_some() {
                    "Closure".to_string()
                } else if let Some(argument) = EnvClosureArgument::from_value(&value) {
                    if argument.is_stringable() {
                        ENV_CLOSURE.to_string()
                    } else {
                        "Closure".to_string()
                    }
                } else if ServiceLocatorArgument::from_value(&value).is_some() {
                    SERVICE_LOCATOR.to_string()
                } else if let Value::Object(object) = &value {
                    object.class().to_string()
                } else {
                    match value {
                        Value::Null => "NULL",
                        Value::Bool(_) => "bool",
                        Value::Int(_) => "int",
                        Value::Float(_) => "float",
                        Value::String(_) => "string",
                        Value::Array(_) | Value::Object(_) => "array",
                    }
                    .to_string()
                }
            }
        };

        if SCALAR_TYPES.contains(&type_.as_str()) && SCALAR_TYPES.contains(&class.as_str()) {
            return Ok(());
        }

        if type_ == "string" && is_a(&registry, &class, "Stringable") {
            return Ok(());
        }

        if type_ == "callable"
            && (class == "Closure" || method_exists(&registry, &class, "__invoke"))
        {
            return Ok(());
        }

        if type_ == "callable"
            && let Value::Array(array) = &value
            && let Some(first) = array.get(&Key::Int(0)).filter(|v| !v.is_null())
            && (Reference::from_value(first).is_some()
                || Definition::from_value(first).is_some()
                || matches!(first, Value::String(_)))
        {
            return Ok(());
        }

        if type_ == "iterable"
            && (matches!(value, Value::Array(_))
                || class == "array"
                || is_subclass_of(&registry, &class, "Traversable"))
        {
            return Ok(());
        }

        if type_ == class {
            return Ok(());
        }

        if type_ == "object" && !BUILTIN_TYPES.contains(&class.as_str()) {
            return Ok(());
        }

        if type_ == "mixed" {
            return Ok(());
        }

        if is_a(&registry, &class, &type_) {
            return Ok(());
        }

        if type_ == "false" {
            if matches!(value, Value::Bool(false)) {
                return Ok(());
            }
        } else if type_ == "true" {
            if matches!(value, Value::Bool(true)) {
                return Ok(());
            }
        } else if is_builtin(&type_name) {
            // `is_<type>($value)`.
            let passes = match type_.to_ascii_lowercase().as_str() {
                "int" => matches!(value, Value::Int(_)),
                "float" => matches!(value, Value::Float(_)),
                "string" => matches!(value, Value::String(_)),
                "bool" => matches!(value, Value::Bool(_)),
                "array" | "iterable" => matches!(value, Value::Array(_)),
                "object" => matches!(value, Value::Object(_)),
                "null" => value.is_null(),
                "callable" => is_callable(&registry, &value),
                other => {
                    return Err(Fail::Error(Error::new(
                        ErrorKind::Other("Error".to_string()),
                        format!("Call to undefined function is_{other}()"),
                    )));
                }
            };
            if passes {
                return Ok(());
            }
        }

        let passed = if matches!(value, Value::Object(_)) {
            class
        } else {
            value.debug_type()
        };
        Err(Fail::Mismatch(passed))
    }
}

impl CompilerPassInterface for CheckTypeDeclarationsPass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn process(&mut self, container: &mut ContainerBuilder) -> Result<()> {
        recursive_process(self, container)
    }
}

impl AbstractRecursivePass for CheckTypeDeclarationsPass {
    fn state(&mut self) -> &mut RecursivePassState {
        &mut self.state
    }

    fn process_value(
        &mut self,
        container: &mut ContainerBuilder,
        value: Value,
        is_root: bool,
    ) -> Result<Value> {
        if self.skipped_ids.contains(&self.current_id()) {
            return Ok(value);
        }

        let Some(definition) = Definition::from_value(&value) else {
            return self.parent_process_value(container, value, is_root);
        };
        if definition.has_errors() || definition.is_deprecated() {
            return self.parent_process_value(container, value, is_root);
        }

        if !self.autoload {
            let Some(class) = definition.class().filter(|c| !c.is_empty() && c != "0") else {
                return self.parent_process_value(container, value, is_root);
            };
            if !is_loaded(&container.class_registry(), &class) {
                return self.parent_process_value(container, value, is_root);
            }
        }

        if definition.class().as_deref() == Some(SERVICE_LOCATOR) {
            return self.parent_process_value(container, value, is_root);
        }

        if let Some(constructor) = self.get_constructor(container, &definition, false)? {
            self.check_type_declarations(
                container,
                &definition,
                &constructor,
                &definition.arguments(),
            )?;
        }

        for method_call in definition.method_calls().values() {
            let Some(method_call) = method_call.as_array() else {
                continue;
            };
            let method = method_call
                .get(&Key::Int(0))
                .and_then(Value::to_php_string)
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let reflection_method =
                match self.get_reflection_method(container, &definition, &method) {
                    Ok(r) => r,
                    Err(e) if e.instance_of(RUNTIME_EXCEPTION) => {
                        if definition.factory().is_truthy() {
                            continue;
                        }
                        return Err(e);
                    }
                    Err(e) => return Err(e),
                };

            let arguments = method_call
                .get(&Key::Int(1))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            self.check_type_declarations(container, &definition, &reflection_method, &arguments)?;
        }

        self.parent_process_value(container, value, is_root)
    }
}
