//! Port of Symfony\Component\OptionsResolver\OptionsResolver.php and
//! Options.php.
//!
//! PHP's resolver is an object: closures receive it (as `Options`) while
//! it resolves, and may call any method on it, including ones that throw
//! because it is locked. Here [`OptionsResolver`] is a handle to shared
//! state, methods take `&self`, and no lock is held while a user closure
//! runs, so closures re-enter it as PHP's do. [`OptionsResolver::handle`]
//! copies the handle (the same object, as a PHP variable holding it);
//! `resolve()` works on a copy of the state, as upstream's `clone $this`.
//!
//! What upstream learns by reflection (whether a default closure's first
//! parameter is typed `Options`, and whether it has a second parameter) is
//! stated by the caller: [`DefaultValue::Lazy`] versus a plain value.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/options-resolver OptionsResolver.php Options.php

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::{Key, PhpArray, PhpString, Value};
use symfony_deprecation_contracts::trigger_deprecation;

use crate::{
    exception::{Class, Error},
    option_configurator::OptionConfigurator,
    php,
};

/// `OptionsResolver::VALIDATION_FUNCTIONS`: the type names checked with
/// an `is_*()` function rather than `instanceof`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValidationFunction {
    Bool,
    Int,
    Float,
    Numeric,
    String,
    Scalar,
    Array,
    Iterable,
    Countable,
    Callable,
    Object,
    Resource,
}

impl ValidationFunction {
    fn from_type(name: &str) -> Option<Self> {
        Some(match name {
            "bool" | "boolean" => Self::Bool,
            "int" | "integer" | "long" => Self::Int,
            "float" | "double" | "real" => Self::Float,
            "numeric" => Self::Numeric,
            "string" => Self::String,
            "scalar" => Self::Scalar,
            "array" => Self::Array,
            "iterable" => Self::Iterable,
            "countable" => Self::Countable,
            "callable" => Self::Callable,
            "object" => Self::Object,
            "resource" => Self::Resource,
            _ => return None,
        })
    }

    fn check(self, value: &Value) -> bool {
        match self {
            Self::Bool => matches!(value, Value::Bool(_)),
            Self::Int => matches!(value, Value::Int(_)),
            Self::Float => matches!(value, Value::Float(_)),
            Self::Numeric => php::is_numeric_value(value),
            Self::String => matches!(value, Value::String(_)),
            Self::Scalar => php::is_scalar(value),
            Self::Array => matches!(value, Value::Array(_)),
            Self::Iterable => php::is_iterable(value),
            Self::Countable => php::is_countable(value),
            Self::Callable => php::is_callable(value),
            Self::Object => php::is_object(value),
            Self::Resource => php::is_resource(value),
        }
    }
}

type OptionFn = dyn Fn(&OptionsResolver, Value) -> Result<Value, Error> + Send + Sync;
type NestedFn = dyn Fn(&OptionsResolver, &OptionsResolver) -> Result<(), Error> + Send + Sync;
type AllowedFn = dyn Fn(&Value) -> Result<bool, Error> + Send + Sync;

/// A closure `function (Options $options, $value)`: a lazy default, a
/// normalizer or a deprecation message closure. It receives the resolver
/// being resolved and the value (for a lazy default, the previous
/// default). Clones are the same closure ([`OptionClosure::same`] is
/// `===`).
#[derive(Clone)]
pub struct OptionClosure(Arc<OptionFn>);

impl OptionClosure {
    pub fn new(
        f: impl Fn(&OptionsResolver, Value) -> Result<Value, Error> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(f))
    }

    /// `===`.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn call(&self, options: &OptionsResolver, value: Value) -> Result<Value, Error> {
        (self.0)(options, value)
    }
}

impl std::fmt::Debug for OptionClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Closure")
    }
}

/// A nested-options closure `function (OptionsResolver $resolver,
/// Options $parent)`.
#[derive(Clone)]
pub struct NestedClosure(Arc<NestedFn>);

impl NestedClosure {
    pub fn new(
        f: impl Fn(&OptionsResolver, &OptionsResolver) -> Result<(), Error> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(f))
    }

    /// `===`.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for NestedClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Closure")
    }
}

/// An allowed-value closure `function ($value)`; the value is accepted
/// when it returns true (the closure gives PHP's truthiness of its
/// result).
#[derive(Clone)]
pub struct AllowedClosure(Arc<AllowedFn>);

impl AllowedClosure {
    pub fn new(f: impl Fn(&Value) -> Result<bool, Error> + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// `===`.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for AllowedClosure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Closure")
    }
}

/// What `setDefault()` receives.
#[derive(Debug, Clone)]
pub enum DefaultValue {
    /// A value (including a closure that is not `function (Options $o)`,
    /// which is stored as it is, e.g. [`php::ClosureValue`]).
    Value(Value),
    /// A closure whose first parameter is typed `Options`: evaluated
    /// lazily. `previous` is whether it declares a second parameter (the
    /// previous default), which decides whether earlier lazy closures for
    /// the option are kept.
    Lazy {
        closure: OptionClosure,
        previous: bool,
    },
}

impl DefaultValue {
    /// `static function (Options $options) { ... }`.
    pub fn lazy(
        f: impl Fn(&OptionsResolver) -> Result<Value, Error> + Send + Sync + 'static,
    ) -> Self {
        Self::Lazy {
            closure: OptionClosure::new(move |options, _| f(options)),
            previous: false,
        }
    }

    /// `static function (Options $options, $previousValue) { ... }`.
    pub fn lazy_with_previous(
        f: impl Fn(&OptionsResolver, Value) -> Result<Value, Error> + Send + Sync + 'static,
    ) -> Self {
        Self::Lazy {
            closure: OptionClosure::new(f),
            previous: true,
        }
    }
}

impl<T: Into<Value>> From<T> for DefaultValue {
    fn from(value: T) -> Self {
        Self::Value(value.into())
    }
}

/// One allowed value: a value compared with `===`, or a closure.
#[derive(Debug, Clone)]
pub enum AllowedValue {
    Value(Value),
    Closure(AllowedClosure),
}

impl AllowedValue {
    pub fn closure(f: impl Fn(&Value) -> Result<bool, Error> + Send + Sync + 'static) -> Self {
        Self::Closure(AllowedClosure::new(f))
    }

    /// `===` (values by `===`, closures by handle).
    pub fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Value(a), Self::Value(b)) => a.identical(b),
            (Self::Closure(a), Self::Closure(b)) => a.same(b),
            _ => false,
        }
    }
}

impl<T: Into<Value>> From<T> for AllowedValue {
    fn from(value: T) -> Self {
        Self::Value(value.into())
    }
}

/// What `setAllowedValues()` / `addAllowedValues()` receive: an array of
/// allowed values (its keys kept, as upstream stores the array), or one
/// value or closure, which becomes `[$value]`.
#[derive(Debug, Clone, Default)]
pub struct AllowedValues(pub Vec<(Key, AllowedValue)>);

impl AllowedValues {
    /// `[$a, $b, ...]`.
    pub fn list(values: impl IntoIterator<Item = AllowedValue>) -> Self {
        Self(
            values
                .into_iter()
                .enumerate()
                .map(|(i, v)| (Key::Int(i as i64), v))
                .collect(),
        )
    }

    /// `array_merge($this, $other)`: int keys renumbered, string keys
    /// overwritten in place.
    fn merge(&mut self, other: Self) {
        let entries = std::mem::take(&mut self.0);
        for (key, value) in entries.into_iter().chain(other.0) {
            match key {
                Key::Int(_) => {
                    let next = self
                        .0
                        .iter()
                        .filter(|(k, _)| matches!(k, Key::Int(_)))
                        .count();
                    self.0.push((Key::Int(next as i64), value));
                }
                key => match self.0.iter_mut().find(|(k, _)| *k == key) {
                    Some(entry) => entry.1 = value,
                    None => self.0.push((key, value)),
                },
            }
        }
    }
}

impl From<AllowedValue> for AllowedValues {
    fn from(value: AllowedValue) -> Self {
        Self::list([value])
    }
}

impl From<Value> for AllowedValues {
    /// `\is_array($allowedValues) ? $allowedValues : [$allowedValues]`.
    fn from(value: Value) -> Self {
        match value {
            Value::Array(array) => Self(
                array
                    .iter()
                    .map(|(k, v)| (k.clone(), AllowedValue::Value(v.clone())))
                    .collect(),
            ),
            other => Self::list([AllowedValue::Value(other)]),
        }
    }
}

impl From<&str> for AllowedValues {
    fn from(value: &str) -> Self {
        Value::from(value).into()
    }
}

impl From<i64> for AllowedValues {
    fn from(value: i64) -> Self {
        Value::from(value).into()
    }
}

impl From<bool> for AllowedValues {
    fn from(value: bool) -> Self {
        Value::from(value).into()
    }
}

impl From<PhpArray> for AllowedValues {
    fn from(value: PhpArray) -> Self {
        Value::from(value).into()
    }
}

impl<const N: usize> From<[AllowedValue; N]> for AllowedValues {
    fn from(values: [AllowedValue; N]) -> Self {
        Self::list(values)
    }
}

impl From<Vec<AllowedValue>> for AllowedValues {
    fn from(values: Vec<AllowedValue>) -> Self {
        Self::list(values)
    }
}

/// A deprecation message: a string (with `%name%` for the option name)
/// or a closure returning one (`''` to skip).
#[derive(Debug, Clone)]
pub enum DeprecationMessage {
    Text(PhpString),
    Closure(OptionClosure),
}

impl DeprecationMessage {
    pub fn closure(
        f: impl Fn(&OptionsResolver, Value) -> Result<Value, Error> + Send + Sync + 'static,
    ) -> Self {
        Self::Closure(OptionClosure::new(f))
    }
}

impl From<&str> for DeprecationMessage {
    fn from(text: &str) -> Self {
        Self::Text(text.into())
    }
}

impl From<String> for DeprecationMessage {
    fn from(text: String) -> Self {
        Self::Text(text.into())
    }
}

impl From<OptionClosure> for DeprecationMessage {
    fn from(closure: OptionClosure) -> Self {
        Self::Closure(closure)
    }
}

/// `setDeprecated()`'s default message.
pub const DEFAULT_DEPRECATION_MESSAGE: &str = "The option \"%name%\" is deprecated.";

/// A deprecation as upstream stores it (`['package' => ..., 'version' =>
/// ..., 'message' => ...]`).
#[derive(Debug, Clone)]
pub struct Deprecation {
    pub package: String,
    pub version: String,
    pub message: DeprecationMessage,
}

/// `string|string[]` arguments: one name or several.
pub trait Names {
    fn names(self) -> Vec<String>;
}

impl Names for &str {
    fn names(self) -> Vec<String> {
        vec![self.to_owned()]
    }
}

impl Names for String {
    fn names(self) -> Vec<String> {
        vec![self]
    }
}

impl<const N: usize> Names for [&str; N] {
    fn names(self) -> Vec<String> {
        self.iter().map(|s| (*s).to_owned()).collect()
    }
}

impl Names for &[&str] {
    fn names(self) -> Vec<String> {
        self.iter().map(|s| (*s).to_owned()).collect()
    }
}

impl Names for Vec<&str> {
    fn names(self) -> Vec<String> {
        self.into_iter().map(str::to_owned).collect()
    }
}

impl Names for Vec<String> {
    fn names(self) -> Vec<String> {
        self
    }
}

/// An ordered map with PHP array keys, for the per-option lists.
#[derive(Debug, Clone)]
pub(crate) struct Map<V>(Vec<(Key, V)>);

impl<V> Default for Map<V> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<V> Map<V> {
    pub(crate) fn get(&self, key: &Key) -> Option<&V> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn get_mut(&mut self, key: &Key) -> Option<&mut V> {
        self.0.iter_mut().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub(crate) fn contains(&self, key: &Key) -> bool {
        self.get(key).is_some()
    }

    fn insert(&mut self, key: Key, value: V) {
        match self.get_mut(&key) {
            Some(slot) => *slot = value,
            None => self.0.push((key, value)),
        }
    }

    fn remove(&mut self, key: &Key) {
        self.0.retain(|(k, _)| k != key);
    }

    fn keys(&self) -> Vec<Key> {
        self.0.iter().map(|(k, _)| k.clone()).collect()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn ksort(&mut self) {
        self.0.sort_by(|(a, _), (b, _)| php::compare_keys(a, b));
    }
}

/// The resolver's properties.
#[derive(Clone, Default)]
pub(crate) struct State {
    pub(crate) defined: Map<()>,
    pub(crate) defaults: PhpArray,
    pub(crate) nested: Map<Vec<NestedClosure>>,
    required: Map<()>,
    resolved: PhpArray,
    pub(crate) normalizers: Map<Vec<OptionClosure>>,
    pub(crate) allowed_values: Map<AllowedValues>,
    pub(crate) allowed_types: Map<Vec<String>>,
    info: Map<String>,
    pub(crate) lazy: Map<Vec<OptionClosure>>,
    calling: Map<()>,
    pub(crate) deprecated: Map<Deprecation>,
    given: Map<()>,
    locked: bool,
    parents_options: Vec<Vec<u8>>,
    prototype: Option<bool>,
    prototype_index: Option<Key>,
    ignore_undefined: bool,
}

/// `formatOptions()`. Upstream shifts the first parent off the
/// resolver's own `$parentsOptions` (`array_shift($this->parentsOptions)`),
/// so every later message from the same resolver loses one level of
/// prefix; that is kept.
fn format_options(state: &mut State, options: &[Key]) -> Vec<u8> {
    let mut names: Vec<Vec<u8>> = options.iter().map(php::key_bytes).collect();
    if !state.parents_options.is_empty() {
        let mut prefix = state.parents_options.remove(0);
        if !state.parents_options.is_empty() {
            prefix.push(b'[');
            prefix.extend(php::implode(b"][", &state.parents_options));
            prefix.push(b']');
        }
        if state.prototype == Some(true)
            && let Some(index) = &state.prototype_index
        {
            prefix.push(b'[');
            prefix.extend(php::key_bytes(index));
            prefix.push(b']');
        }
        names = names
            .into_iter()
            .map(|option| {
                let mut name = prefix.clone();
                name.push(b'[');
                name.extend(option);
                name.push(b']');
                name
            })
            .collect();
    }
    php::implode(b"\", \"", &names)
}

/// `implode('", "', array_keys($this->defined))`.
fn defined_list(state: &State) -> Vec<u8> {
    let keys: Vec<Vec<u8>> = state.defined.keys().iter().map(php::key_bytes).collect();
    php::implode(b"\", \"", &keys)
}

/// `formatValue()`.
fn format_value(value: &Value) -> Vec<u8> {
    if php::is_resource(value) || php::debug_type(value).starts_with("resource") {
        return b"resource".to_vec();
    }
    match value {
        Value::Object(o) => o.class().as_bytes().to_vec(),
        Value::Array(_) => b"array".to_vec(),
        Value::String(s) => {
            let mut out = vec![b'"'];
            out.extend_from_slice(s);
            out.push(b'"');
            out
        }
        Value::Null => b"null".to_vec(),
        Value::Bool(false) => b"false".to_vec(),
        Value::Bool(true) => b"true".to_vec(),
        other => php::scalar_bytes(other),
    }
}

/// `formatValues()`.
fn format_values(values: &[Value]) -> Vec<u8> {
    let pieces: Vec<Vec<u8>> = values.iter().map(format_value).collect();
    php::implode(b", ", &pieces)
}

fn concat(parts: &[&[u8]]) -> Vec<u8> {
    parts.concat()
}

fn error(class: Class, message: Vec<u8>) -> Error {
    Error::new(class, message)
}

fn cyclic(state: &mut State) -> Error {
    let calling = state.calling.keys();
    let names = format_options(state, &calling);
    error(
        Class::OptionDefinition,
        concat(&[b"The options \"", &names, b"\" have a cyclic dependency."]),
    )
}

/// `setDeprecated()`'s `UndefinedOptionsException` ("does not exist,
/// defined options are: ...").
fn undefined_for_deprecation(state: &mut State, option: &Key) -> Error {
    let name = format_options(state, std::slice::from_ref(option));
    let defined = defined_list(state);
    error(
        Class::UndefinedOptions,
        concat(&[
            b"The option \"",
            &name,
            b"\" does not exist, defined options are: \"",
            &defined,
            b"\".",
        ]),
    )
}

/// The `UndefinedOptionsException` of the setters ("does not exist.
/// Defined options are: ...").
fn undefined_option(state: &mut State, option: &Key) -> Error {
    does_not_exist(state, option, Class::UndefinedOptions)
}

/// "The option ... does not exist. Defined options are: ...".
fn does_not_exist(state: &mut State, option: &Key, class: Class) -> Error {
    let name = format_options(state, std::slice::from_ref(option));
    let defined = defined_list(state);
    error(
        class,
        concat(&[
            b"The option \"",
            &name,
            b"\" does not exist. Defined options are: \"",
            &defined,
            b"\".",
        ]),
    )
}

/// Validates options and merges them with default values.
pub struct OptionsResolver(Arc<Mutex<State>>);

impl Default for OptionsResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for OptionsResolver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OptionsResolver")
    }
}

impl OptionsResolver {
    /// The class name, as `$resolver::class` gives it.
    pub const CLASS: &'static str = "Symfony\\Component\\OptionsResolver\\OptionsResolver";

    pub fn new() -> Self {
        Self::from_state(State::default())
    }

    fn from_state(state: State) -> Self {
        Self(Arc::new(Mutex::new(state)))
    }

    /// Another handle to the same resolver (a PHP variable holding the
    /// same object, not `clone`).
    pub fn handle(&self) -> Self {
        Self(Arc::clone(&self.0))
    }

    /// `===`.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn check_unlocked(&self, message: &str) -> Result<MutexGuard<'_, State>, Error> {
        let state = self.state();
        if state.locked {
            return Err(Error::new(Class::Access, message));
        }
        Ok(state)
    }

    /// The guard of an unlocked resolver whose `option` is defined, or the
    /// setters' `UndefinedOptionsException`.
    fn check_defined(
        &self,
        locked_message: &str,
        option: &Key,
    ) -> Result<MutexGuard<'_, State>, Error> {
        let mut state = self.check_unlocked(locked_message)?;
        if !state.defined.contains(option) {
            return Err(undefined_option(&mut state, option));
        }
        Ok(state)
    }

    /// `setDefault()`.
    pub fn set_default(
        &self,
        option: impl Into<Key>,
        value: impl Into<DefaultValue>,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state =
            self.check_unlocked("Default values cannot be set from a lazy option or normalizer.")?;
        let value = match value.into() {
            DefaultValue::Lazy { closure, previous } => {
                // Initialize the option if no previous value exists
                if state.defaults.get(&option).is_none_or(Value::is_null) {
                    state.defaults.insert(option.clone(), Value::Null);
                }
                // Ignore previous lazy options if the closure has no second
                // parameter
                if !previous || !state.lazy.contains(&option) {
                    state.lazy.insert(option.clone(), Vec::new());
                }
                if let Some(list) = state.lazy.get_mut(&option) {
                    list.push(closure);
                }
                state.defined.insert(option.clone(), ());
                state.resolved.remove(&option);
                return Ok(self);
            }
            DefaultValue::Value(value) => value,
        };
        state.lazy.remove(&option);
        if !state.defined.contains(&option) || state.resolved.contains_key(&option) {
            state.resolved.insert(option.clone(), value.clone());
        }
        state.defaults.insert(option.clone(), value);
        state.defined.insert(option, ());
        Ok(self)
    }

    /// `setDefaults()`.
    pub fn set_defaults<K: Into<Key>, V: Into<DefaultValue>>(
        &self,
        defaults: impl IntoIterator<Item = (K, V)>,
    ) -> Result<&Self, Error> {
        for (option, value) in defaults {
            self.set_default(option, value)?;
        }
        Ok(self)
    }

    /// `setDefaults()` with a PHP array of plain values.
    pub fn set_defaults_array(&self, defaults: &PhpArray) -> Result<&Self, Error> {
        for (option, value) in defaults.iter() {
            self.set_default(option.clone(), value.clone())?;
        }
        Ok(self)
    }

    /// `hasDefault()`.
    pub fn has_default(&self, option: impl Into<Key>) -> bool {
        self.state().defaults.contains_key(&option.into())
    }

    /// `setRequired()`.
    pub fn set_required(&self, option_names: impl Names) -> Result<&Self, Error> {
        self.set_required_keys(option_names.names().into_iter().map(Key::from))
    }

    pub(crate) fn set_required_key(&self, option: Key) -> Result<&Self, Error> {
        self.set_required_keys([option])
    }

    fn set_required_keys(&self, options: impl IntoIterator<Item = Key>) -> Result<&Self, Error> {
        let mut state = self
            .check_unlocked("Options cannot be made required from a lazy option or normalizer.")?;
        for option in options {
            state.defined.insert(option.clone(), ());
            state.required.insert(option, ());
        }
        Ok(self)
    }

    /// `isRequired()`.
    pub fn is_required(&self, option: impl Into<Key>) -> bool {
        self.state().required.contains(&option.into())
    }

    /// `getRequiredOptions()`.
    pub fn get_required_options(&self) -> Vec<Key> {
        self.state().required.keys()
    }

    /// `isMissing()`.
    pub fn is_missing(&self, option: impl Into<Key>) -> bool {
        let option = option.into();
        let state = self.state();
        state.required.contains(&option) && !state.defaults.contains_key(&option)
    }

    /// `getMissingOptions()`.
    pub fn get_missing_options(&self) -> Vec<Key> {
        let state = self.state();
        state
            .required
            .keys()
            .into_iter()
            .filter(|k| !state.defaults.contains_key(k))
            .collect()
    }

    /// `setDefined()`.
    pub fn set_defined(&self, option_names: impl Names) -> Result<&Self, Error> {
        self.set_defined_keys(option_names.names().into_iter().map(Key::from))
    }

    pub(crate) fn set_defined_key(&self, option: Key) -> Result<&Self, Error> {
        self.set_defined_keys([option])
    }

    fn set_defined_keys(&self, options: impl IntoIterator<Item = Key>) -> Result<&Self, Error> {
        let mut state =
            self.check_unlocked("Options cannot be defined from a lazy option or normalizer.")?;
        for option in options {
            state.defined.insert(option, ());
        }
        Ok(self)
    }

    /// `isDefined()`.
    pub fn is_defined(&self, option: impl Into<Key>) -> bool {
        self.state().defined.contains(&option.into())
    }

    /// `getDefinedOptions()`.
    pub fn get_defined_options(&self) -> Vec<Key> {
        self.state().defined.keys()
    }

    /// `setOptions()`.
    pub fn set_options(
        &self,
        option: impl Into<Key>,
        nested: NestedClosure,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state = self
            .check_unlocked("Nested options cannot be defined from a lazy option or normalizer.")?;
        match state.nested.get_mut(&option) {
            Some(list) => list.push(nested),
            None => state.nested.insert(option.clone(), vec![nested]),
        }
        state
            .defaults
            .insert(option.clone(), Value::Array(PhpArray::new()));
        state.defined.insert(option.clone(), ());
        state.resolved.remove(&option);
        Ok(self)
    }

    /// `isNested()`.
    pub fn is_nested(&self, option: impl Into<Key>) -> bool {
        self.state().nested.contains(&option.into())
    }

    /// `setDeprecated()`. Upstream's check that the message is a string
    /// or a closure is the type of `message` here.
    pub fn set_deprecated(
        &self,
        option: impl Into<Key>,
        package: &str,
        version: &str,
        message: impl Into<DeprecationMessage>,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state =
            self.check_unlocked("Options cannot be deprecated from a lazy option or normalizer.")?;
        if !state.defined.contains(&option) {
            return Err(undefined_for_deprecation(&mut state, &option));
        }
        let message = message.into();
        // ignore if empty string
        if matches!(&message, DeprecationMessage::Text(text) if text.is_empty()) {
            return Ok(self);
        }
        state.deprecated.insert(
            option.clone(),
            Deprecation {
                package: package.to_owned(),
                version: version.to_owned(),
                message,
            },
        );
        state.resolved.remove(&option);
        Ok(self)
    }

    /// `isDeprecated()`.
    pub fn is_deprecated(&self, option: impl Into<Key>) -> bool {
        self.state().deprecated.contains(&option.into())
    }

    /// `setNormalizer()`.
    pub fn set_normalizer(
        &self,
        option: impl Into<Key>,
        normalizer: OptionClosure,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state = self.check_defined(
            "Normalizers cannot be set from a lazy option or normalizer.",
            &option,
        )?;
        state.normalizers.insert(option.clone(), vec![normalizer]);
        state.resolved.remove(&option);
        Ok(self)
    }

    /// `addNormalizer()`.
    pub fn add_normalizer(
        &self,
        option: impl Into<Key>,
        normalizer: OptionClosure,
        force_prepend: bool,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state = self.check_defined(
            "Normalizers cannot be set from a lazy option or normalizer.",
            &option,
        )?;
        match state.normalizers.get_mut(&option) {
            Some(list) if force_prepend => list.insert(0, normalizer),
            Some(list) => list.push(normalizer),
            None => state.normalizers.insert(option.clone(), vec![normalizer]),
        }
        state.resolved.remove(&option);
        Ok(self)
    }

    /// `setAllowedValues()`.
    pub fn set_allowed_values(
        &self,
        option: impl Into<Key>,
        allowed_values: impl Into<AllowedValues>,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state = self.check_defined(
            "Allowed values cannot be set from a lazy option or normalizer.",
            &option,
        )?;
        state
            .allowed_values
            .insert(option.clone(), allowed_values.into());
        state.resolved.remove(&option);
        Ok(self)
    }

    /// `addAllowedValues()`.
    pub fn add_allowed_values(
        &self,
        option: impl Into<Key>,
        allowed_values: impl Into<AllowedValues>,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state = self.check_defined(
            "Allowed values cannot be added from a lazy option or normalizer.",
            &option,
        )?;
        let allowed_values = allowed_values.into();
        match state.allowed_values.get_mut(&option) {
            Some(existing) => existing.merge(allowed_values),
            None => state.allowed_values.insert(option.clone(), allowed_values),
        }
        state.resolved.remove(&option);
        Ok(self)
    }

    /// `setAllowedTypes()`.
    pub fn set_allowed_types(
        &self,
        option: impl Into<Key>,
        allowed_types: impl Names,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state = self.check_defined(
            "Allowed types cannot be set from a lazy option or normalizer.",
            &option,
        )?;
        state
            .allowed_types
            .insert(option.clone(), allowed_types.names());
        state.resolved.remove(&option);
        Ok(self)
    }

    /// `addAllowedTypes()`.
    pub fn add_allowed_types(
        &self,
        option: impl Into<Key>,
        allowed_types: impl Names,
    ) -> Result<&Self, Error> {
        let option = option.into();
        let mut state = self.check_defined(
            "Allowed types cannot be added from a lazy option or normalizer.",
            &option,
        )?;
        let allowed_types = allowed_types.names();
        match state.allowed_types.get_mut(&option) {
            Some(existing) => existing.extend(allowed_types),
            None => state.allowed_types.insert(option.clone(), allowed_types),
        }
        state.resolved.remove(&option);
        Ok(self)
    }

    /// `define()`.
    pub fn define(&self, option: impl Into<Key>) -> Result<OptionConfigurator<'_>, Error> {
        let option = option.into();
        if self.state().defined.contains(&option) {
            return Err(error(
                Class::OptionDefinition,
                concat(&[
                    b"The option \"",
                    &php::key_bytes(&option),
                    b"\" is already defined.",
                ]),
            ));
        }
        OptionConfigurator::new(option, self)
    }

    /// `setInfo()`.
    pub fn set_info(&self, option: impl Into<Key>, info: &str) -> Result<&Self, Error> {
        let option = option.into();
        let mut state = self.check_defined(
            "The Info message cannot be set from a lazy option or normalizer.",
            &option,
        )?;
        state.info.insert(option, info.to_owned());
        Ok(self)
    }

    /// `getInfo()`.
    pub fn get_info(&self, option: impl Into<Key>) -> Result<Option<String>, Error> {
        let option = option.into();
        let mut state = self.state();
        if !state.defined.contains(&option) {
            return Err(undefined_option(&mut state, &option));
        }
        Ok(state.info.get(&option).cloned())
    }

    /// `setPrototype()`.
    pub fn set_prototype(&self, prototype: bool) -> Result<&Self, Error> {
        let mut state = self.check_unlocked(
            "The prototype property cannot be set from a lazy option or normalizer.",
        )?;
        if state.prototype.is_none() && prototype {
            return Err(Error::new(
                Class::Access,
                "The prototype property cannot be set from a root definition.",
            ));
        }
        state.prototype = Some(prototype);
        Ok(self)
    }

    /// `isPrototype()`.
    pub fn is_prototype(&self) -> bool {
        self.state().prototype.unwrap_or(false)
    }

    /// `remove()`.
    pub fn remove(&self, option_names: impl Names) -> Result<&Self, Error> {
        let mut state =
            self.check_unlocked("Options cannot be removed from a lazy option or normalizer.")?;
        for option in option_names.names() {
            let option = Key::from(option);
            state.defined.remove(&option);
            state.defaults.remove(&option);
            state.required.remove(&option);
            state.resolved.remove(&option);
            state.lazy.remove(&option);
            state.normalizers.remove(&option);
            state.allowed_types.remove(&option);
            state.allowed_values.remove(&option);
            state.info.remove(&option);
            state.deprecated.remove(&option);
        }
        Ok(self)
    }

    /// `clear()`.
    pub fn clear(&self) -> Result<&Self, Error> {
        let mut state =
            self.check_unlocked("Options cannot be cleared from a lazy option or normalizer.")?;
        state.defined = Map::default();
        state.defaults = PhpArray::new();
        state.nested = Map::default();
        state.required = Map::default();
        state.resolved = PhpArray::new();
        state.lazy = Map::default();
        state.normalizers = Map::default();
        state.allowed_types = Map::default();
        state.allowed_values = Map::default();
        state.deprecated = Map::default();
        state.info = Map::default();
        Ok(self)
    }

    /// `resolve()`.
    pub fn resolve(&self, options: &PhpArray) -> Result<PhpArray, Error> {
        let clone = {
            let state = self
                .check_unlocked("Options cannot be resolved from a lazy option or normalizer.")?;
            // Allow this method to be called multiple times
            OptionsResolver::from_state(state.clone())
        };
        let ignore_undefined = self.state().ignore_undefined;

        // Make sure that no unknown options are passed
        let mut diff: Map<()> = Map::default();
        if !ignore_undefined {
            let clone_state = clone.state();
            for key in options.keys() {
                if !clone_state.defined.contains(key) {
                    diff.insert(key.clone(), ());
                }
            }
        }
        if !diff.is_empty() {
            let defined = {
                let mut clone_state = clone.state();
                clone_state.defined.ksort();
                defined_list(&clone_state)
            };
            diff.ksort();
            let many = diff.0.len() > 1;
            let names = format_options(&mut self.state(), &diff.keys());
            return Err(error(
                Class::UndefinedOptions,
                concat(&[
                    if many {
                        b"The options \"".as_slice()
                    } else {
                        b"The option \"".as_slice()
                    },
                    &names,
                    if many {
                        b"\" do not exist.".as_slice()
                    } else {
                        b"\" does not exist.".as_slice()
                    },
                    b" Defined options are: \"",
                    &defined,
                    b"\".",
                ]),
            ));
        }

        // Override options set by the user
        let missing = {
            let mut clone_state = clone.state();
            for (option, value) in options.iter() {
                if ignore_undefined && !clone_state.defined.contains(option) {
                    continue;
                }
                clone_state.given.insert(option.clone(), ());
                clone_state.defaults.insert(option.clone(), value.clone());
                clone_state.resolved.remove(option);
                clone_state.lazy.remove(option);
            }
            // Check whether any required option is missing
            let mut missing: Map<()> = Map::default();
            for key in clone_state.required.keys() {
                if !clone_state.defaults.contains_key(&key) {
                    missing.insert(key, ());
                }
            }
            missing
        };
        if !missing.is_empty() {
            let mut missing = missing;
            missing.ksort();
            let many = missing.0.len() > 1;
            let names = format_options(&mut self.state(), &missing.keys());
            return Err(error(
                Class::MissingOptions,
                concat(&[
                    if many {
                        b"The required options \"".as_slice()
                    } else {
                        b"The required option \"".as_slice()
                    },
                    &names,
                    if many {
                        b"\" are missing.".as_slice()
                    } else {
                        b"\" is missing.".as_slice()
                    },
                ]),
            ));
        }

        // Lock the container
        let options: Vec<Key> = {
            let mut clone_state = clone.state();
            clone_state.locked = true;
            clone_state.defaults.keys().cloned().collect()
        };

        // Now process the individual options. Use offsetGet(), which
        // resolves the option itself and any options that the option
        // depends on
        for option in options {
            clone.offset_get_with(option, true)?;
        }

        let resolved = clone.state().resolved.clone();
        Ok(resolved)
    }

    /// `$options[$option]`: `offsetGet($option)`.
    pub fn offset_get(&self, option: impl Into<Key>) -> Result<Value, Error> {
        self.offset_get_with(option, true)
    }

    /// `offsetGet($option, $triggerDeprecation)`.
    pub fn offset_get_with(&self, option: impl Into<Key>, trigger: bool) -> Result<Value, Error> {
        let option = option.into();
        let mut state = self.state();
        if !state.locked {
            return Err(Error::new(
                Class::Access,
                "Array access is only supported within closures of lazy options and normalizers.",
            ));
        }

        // Shortcut for resolved options
        if let Some(value) = state.resolved.get(&option).cloned() {
            let deprecation = match state.deprecated.get(&option) {
                Some(Deprecation {
                    package,
                    version,
                    message: DeprecationMessage::Text(message),
                }) if trigger && (state.given.contains(&option) || !state.calling.is_empty()) => {
                    Some((package.clone(), version.clone(), message.clone()))
                }
                _ => None,
            };
            drop(state);
            if let Some((package, version, message)) = deprecation {
                trigger_deprecation(
                    package.as_bytes(),
                    version.as_bytes(),
                    &php::replace(&message, b"%name%", &php::key_bytes(&option)),
                );
            }
            return Ok(value);
        }

        // Check whether the option is set at all
        let Some(mut value) = state.defaults.get(&option).cloned() else {
            if !state.defined.contains(&option) {
                return Err(does_not_exist(&mut state, &option, Class::NoSuchOption));
            }
            let name = format_options(&mut state, std::slice::from_ref(&option));
            return Err(error(
                Class::NoSuchOption,
                concat(&[
                    b"The optional option \"",
                    &name,
                    b"\" has no value set. You should make sure it is set with \"isset\" before reading it.",
                ]),
            ));
        };

        // Resolve the option if the default value is lazily evaluated
        if let Some(lazy) = state.lazy.get(&option).cloned() {
            // If the closure is already being called, we have a cyclic
            // dependency
            if state.calling.contains(&option) {
                return Err(cyclic(&mut state));
            }
            state.calling.insert(option.clone(), ());
            drop(state);
            let result = lazy
                .iter()
                .try_fold(value, |value, closure| closure.call(self, value));
            state = self.state();
            state.calling.remove(&option);
            value = result?;
        }

        // Resolve the option if it is a nested definition
        if let Some(nested) = state.nested.get(&option).cloned() {
            // If the closure is already being called, we have a cyclic
            // dependency
            if state.calling.contains(&option) {
                return Err(cyclic(&mut state));
            }
            let Value::Array(array) = value else {
                let name = format_options(&mut state, std::slice::from_ref(&option));
                return Err(error(
                    Class::InvalidOptions,
                    concat(&[
                        b"The nested option \"",
                        &name,
                        b"\" with value ",
                        &format_value(&value),
                        b" is expected to be of type array, but is of type \"",
                        php::debug_type(&value).as_bytes(),
                        b"\".",
                    ]),
                ));
            };
            state.calling.insert(option.clone(), ());
            let resolver = OptionsResolver::new();
            {
                let mut child = resolver.state();
                child.prototype = Some(false);
                child.parents_options = state.parents_options.clone();
                if state.prototype == Some(true)
                    && let (Some(index), Some(last)) =
                        (&state.prototype_index, child.parents_options.last_mut())
                {
                    last.push(b'[');
                    last.extend(php::key_bytes(index));
                    last.push(b']');
                }
                child.parents_options.push(php::key_bytes(&option));
            }
            drop(state);
            let result = self.resolve_nested(&resolver, &nested, &option, array);
            resolver.state().prototype_index = None;
            state = self.state();
            state.calling.remove(&option);
            value = result?;
        }

        // Validate the type of the resolved option
        if let Some(allowed_types) = state.allowed_types.get(&option).cloned() {
            let mut valid = true;
            let mut invalid_types: Vec<String> = Vec::new();
            for allowed_type in &allowed_types {
                valid = verify_types(allowed_type, &value, Some(&mut invalid_types), 0);
                if valid {
                    break;
                }
            }
            if !valid {
                let fmt_actual_value = format_value(&value);
                let fmt_allowed_types = allowed_types.join("\" or \"");
                let fmt_provided_types = invalid_types.join("|");
                let contains_array_type = allowed_types.iter().any(|t| t.ends_with("[]"));
                let name = format_options(&mut state, std::slice::from_ref(&option));
                let tail: &[u8] = if matches!(value, Value::Array(_)) && contains_array_type {
                    b"\", but one of the elements is of type \""
                } else {
                    b"\", but is of type \""
                };
                return Err(error(
                    Class::InvalidOptions,
                    concat(&[
                        b"The option \"",
                        &name,
                        b"\" with value ",
                        &fmt_actual_value,
                        b" is expected to be of type \"",
                        fmt_allowed_types.as_bytes(),
                        tail,
                        fmt_provided_types.as_bytes(),
                        b"\".",
                    ]),
                ));
            }
        }

        // Validate the value of the resolved option
        if let Some(allowed_values) = state.allowed_values.get(&option).cloned() {
            drop(state);
            let mut success = false;
            let mut printable = Vec::new();
            for (_, allowed_value) in &allowed_values.0 {
                match allowed_value {
                    AllowedValue::Closure(closure) => {
                        if (closure.0)(&value)? {
                            success = true;
                            break;
                        }
                        // Don't include closures in the exception message
                    }
                    AllowedValue::Value(allowed) => {
                        if value.identical(allowed) {
                            success = true;
                            break;
                        }
                        printable.push(allowed.clone());
                    }
                }
            }
            state = self.state();
            if !success {
                let name = format_options(&mut state, std::slice::from_ref(&option));
                let mut message = concat(&[
                    b"The option \"",
                    &name,
                    b"\" with value ",
                    &format_value(&value),
                    b" is invalid.",
                ]);
                if !printable.is_empty() {
                    message.extend_from_slice(b" Accepted values are: ");
                    message.extend(format_values(&printable));
                    message.push(b'.');
                }
                if let Some(info) = state.info.get(&option) {
                    message.extend_from_slice(b" Info: ");
                    message.extend_from_slice(info.as_bytes());
                    message.push(b'.');
                }
                return Err(error(Class::InvalidOptions, message));
            }
        }

        // Check whether the option is deprecated and it is provided by the
        // user or is being called from a lazy evaluation
        if let Some(deprecation) = state.deprecated.get(&option).cloned() {
            let is_text = matches!(deprecation.message, DeprecationMessage::Text(_));
            if trigger && (state.given.contains(&option) || (!state.calling.is_empty() && is_text))
            {
                let message = match deprecation.message {
                    DeprecationMessage::Text(text) => text.to_vec(),
                    DeprecationMessage::Closure(closure) => {
                        // If the closure is already being called, we have a
                        // cyclic dependency
                        if state.calling.contains(&option) {
                            return Err(cyclic(&mut state));
                        }
                        state.calling.insert(option.clone(), ());
                        drop(state);
                        let result = closure.call(self, value.clone());
                        state = self.state();
                        state.calling.remove(&option);
                        match result? {
                            Value::String(text) => text.to_vec(),
                            other => {
                                return Err(error(
                                    Class::InvalidOptions,
                                    concat(&[
                                        b"Invalid type for deprecation message, expected string but got \"",
                                        php::debug_type(&other).as_bytes(),
                                        b"\", return an empty string to ignore.",
                                    ]),
                                ));
                            }
                        }
                    }
                };
                if !message.is_empty() {
                    drop(state);
                    trigger_deprecation(
                        deprecation.package.as_bytes(),
                        deprecation.version.as_bytes(),
                        &php::replace(&message, b"%name%", &php::key_bytes(&option)),
                    );
                    state = self.state();
                }
            }
        }

        // Normalize the validated option
        if let Some(normalizers) = state.normalizers.get(&option).cloned() {
            // If the closure is already being called, we have a cyclic
            // dependency
            if state.calling.contains(&option) {
                return Err(cyclic(&mut state));
            }
            state.calling.insert(option.clone(), ());
            drop(state);
            let result = normalizers
                .iter()
                .try_fold(value, |value, normalizer| normalizer.call(self, value));
            state = self.state();
            state.calling.remove(&option);
            value = result?;
        }

        // Mark as resolved
        state.resolved.insert(option, value.clone());
        Ok(value)
    }

    /// The nested part of `offsetGet()`, between upstream's `try` and
    /// `finally`.
    fn resolve_nested(
        &self,
        resolver: &OptionsResolver,
        nested: &[NestedClosure],
        option: &Key,
        value: PhpArray,
    ) -> Result<Value, Error> {
        for closure in nested {
            (closure.0)(resolver, self)?;
        }
        if resolver.state().prototype == Some(true) {
            let mut values = PhpArray::new();
            for (index, prototype_value) in value.iter() {
                let Value::Array(prototype_value) = prototype_value else {
                    let mut state = self.state();
                    let name = format_options(&mut state, std::slice::from_ref(option));
                    return Err(error(
                        Class::InvalidOptions,
                        concat(&[
                            b"The value of the option \"",
                            &name,
                            b"\" is expected to be of type array of array, but is of type array of \"",
                            php::debug_type(prototype_value).as_bytes(),
                            b"\".",
                        ]),
                    ));
                };
                resolver.state().prototype_index = Some(index.clone());
                values.insert(
                    index.clone(),
                    Value::Array(resolver.resolve(prototype_value)?),
                );
            }
            Ok(Value::Array(values))
        } else {
            Ok(Value::Array(resolver.resolve(&value)?))
        }
    }

    /// `isset($options[$option])`: `offsetExists()`.
    pub fn offset_exists(&self, option: impl Into<Key>) -> Result<bool, Error> {
        let state = self.state();
        if !state.locked {
            return Err(Error::new(
                Class::Access,
                "Array access is only supported within closures of lazy options and normalizers.",
            ));
        }
        Ok(state.defaults.contains_key(&option.into()))
    }

    /// `$options[$option] = $value`: not supported.
    pub fn offset_set(&self, _option: impl Into<Key>, _value: Value) -> Result<(), Error> {
        Err(Error::new(
            Class::Access,
            "Setting options via array access is not supported. Use setDefault() instead.",
        ))
    }

    /// `unset($options[$option])`: not supported.
    pub fn offset_unset(&self, _option: impl Into<Key>) -> Result<(), Error> {
        Err(Error::new(
            Class::Access,
            "Removing options via array access is not supported. Use remove() instead.",
        ))
    }

    /// `count($options)`.
    pub fn count(&self) -> Result<usize, Error> {
        let state = self.state();
        if !state.locked {
            return Err(Error::new(
                Class::Access,
                "Counting is only supported within closures of lazy options and normalizers.",
            ));
        }
        Ok(state.defaults.len())
    }

    /// `setIgnoreUndefined()`.
    pub fn set_ignore_undefined(&self, ignore: bool) -> &Self {
        self.state().ignore_undefined = ignore;
        self
    }
}

/// `verifyTypes()`. `invalid_types` collects the debug types of the
/// failing values, in first-seen order (upstream's array keys).
fn verify_types(
    allowed_type: &str,
    value: &Value,
    mut invalid_types: Option<&mut Vec<String>>,
    level: usize,
) -> bool {
    let allowed_type = php_trim(allowed_type);
    let allowed_types = split_outside_parenthesis(allowed_type);
    let record = |invalid_types: Option<&mut Vec<String>>| {
        if let Some(invalid) = invalid_types
            && (invalid.is_empty() || level > 0)
        {
            let debug = php::debug_type(value);
            if !invalid.contains(&debug) {
                invalid.push(debug);
            }
        }
    };
    if allowed_types.len() > 1 {
        if allowed_types
            .iter()
            .any(|t| verify_types(t, value, None, 0))
        {
            return true;
        }
        record(invalid_types);
        return false;
    }

    let allowed_type = allowed_types[0];
    if allowed_type.len() >= 2 && allowed_type.starts_with('(') && allowed_type.ends_with(')') {
        return verify_types(
            &allowed_type[1..allowed_type.len() - 1],
            value,
            invalid_types,
            level,
        );
    }

    if let (Value::Array(array), Some(inner)) = (value, allowed_type.strip_suffix("[]")) {
        let mut valid = true;
        for (_, item) in array.iter() {
            if !verify_types(inner, item, invalid_types.as_deref_mut(), level + 1) {
                valid = false;
            }
        }
        return valid;
    }

    let matches = match ValidationFunction::from_type(allowed_type) {
        _ if allowed_type == "null" && value.is_null() => true,
        Some(function) => function.check(value),
        None => php::instance_of(value, allowed_type),
    };
    if matches {
        return true;
    }
    record(invalid_types);
    false
}

/// `trim()` with its default characters (" \t\n\r\0\x0B").
fn php_trim(s: &str) -> &str {
    s.trim_matches([' ', '\t', '\n', '\r', '\0', '\x0B'])
}

/// `splitOutsideParenthesis()`: split at `|` outside balanced
/// parentheses. An opening parenthesis without its balanced closing one
/// does not protect the pipes after it, as with upstream's pattern.
fn split_outside_parenthesis(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => match balanced_end(bytes, i) {
                Some(end) => i = end,
                None => i += 1,
            },
            b'|' => {
                pieces.push(&s[start..i]);
                i += 1;
                start = i;
            }
            _ => i += 1,
        }
    }
    pieces.push(&s[start..]);
    pieces
}

/// The index after the parenthesis closing the one at `open`, if any.
fn balanced_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, &b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_matches_upstream_pattern() {
        assert_eq!(split_outside_parenthesis("a|b"), ["a", "b"]);
        assert_eq!(split_outside_parenthesis("(a|b)|c"), ["(a|b)", "c"]);
        assert_eq!(
            split_outside_parenthesis("(a|(b|c))[]|d"),
            ["(a|(b|c))[]", "d"]
        );
        assert_eq!(split_outside_parenthesis("(a|b"), ["(a", "b"]);
        assert_eq!(split_outside_parenthesis("a|"), ["a", ""]);
        assert_eq!(split_outside_parenthesis("((a)|b"), ["((a)", "b"]);
    }
}
