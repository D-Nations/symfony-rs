//! Port of Symfony\Component\DependencyInjection\ParameterBag\EnvPlaceholderParameterBag.php.
//!
//! The subclass is the crate's one [`ParameterBag`] type with an
//! [`EnvPlaceholders`] state, as `FrozenParameterBag` is that type with a
//! frozen flag: [`ParameterBag::env_placeholder`] builds it,
//! [`ParameterBag::is_env_placeholder`] is `instanceof`, and `get()` and
//! `resolve()` run the subclass's overrides. One type keeps
//! `resolveValue()` and `resolveString()` calling the overridden `get()`,
//! as PHP's late binding does, without a second copy of the resolver.
//!
//! `get()` records placeholders on a shared reference (`$this->...[] =`
//! in a getter), so the state sits behind a mutex; `clone` copies it, as
//! PHP's `clone` copies the arrays. The placeholder counter is
//! process-wide, as upstream's `static $counter`.
//!
//! Upstream: symfony/dependency-injection ParameterBag/EnvPlaceholderParameterBag.php

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use php_value::{PhpArray, Value, array::Key, serialize::serialize};
use symfony_config::resource::xxh128;

use super::ParameterBag;
use crate::exception::{Error, Result};
use crate::id_map::IdMap;

/// Env var names to their placeholders (`string[][]`; each inner list
/// is the keys of `array<placeholder, placeholder>`).
pub type EnvPlaceholderMap = IdMap<Vec<String>>;

/// `self::$counter`.
static COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default)]
struct State {
    unique_prefix: Option<String>,
    placeholders: EnvPlaceholderMap,
    unused: EnvPlaceholderMap,
    provided_types: IdMap<Vec<String>>,
}

/// The state `EnvPlaceholderParameterBag` adds to `ParameterBag`.
#[derive(Debug, Default)]
pub struct EnvPlaceholders(Mutex<State>);

impl Clone for EnvPlaceholders {
    fn clone(&self) -> Self {
        Self(Mutex::new(self.lock().clone()))
    }
}

impl EnvPlaceholders {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// `$a[$env] += $placeholders` for each env of `from`, after `$a += $from`.
fn merge_into(into: &mut EnvPlaceholderMap, from: &EnvPlaceholderMap) {
    for (env, placeholders) in from.iter() {
        let list = into.get_or_insert_with(env, Vec::new);
        for placeholder in placeholders {
            if !list.contains(placeholder) {
                list.push(placeholder.clone());
            }
        }
    }
}

/// `preg_match('/^(?:[-.\w\\\\]*+:)*+[\w.]*+$/', $env)`: `:`-separated
/// prefixes of `[-.\w\\]`, then a name of `[\w.]` (`\w` is ASCII without
/// the `u` flag; `$` also matches before a final newline).
fn is_valid_env_name(env: &str) -> bool {
    let env = env.strip_suffix('\n').unwrap_or(env);
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut parts: Vec<&str> = env.split(':').collect();
    let last = parts.pop().unwrap_or_default();
    parts.iter().all(|p| {
        p.bytes()
            .all(|b| word(b) || matches!(b, b'-' | b'.' | b'\\'))
    }) && last.bytes().all(|b| word(b) || b == b'.')
}

/// `array_walk_recursive($value, fn (&$v) => $v = null)`.
fn nulled_leaves(value: &Value) -> Value {
    match value {
        Value::Array(array) => {
            let mut out = PhpArray::new();
            for (key, v) in array.iter() {
                out.insert(key.clone(), nulled_leaves(v));
            }
            Value::Array(out)
        }
        _ => Value::Null,
    }
}

impl ParameterBag {
    /// `new EnvPlaceholderParameterBag($parameters)`.
    pub fn env_placeholder(parameters: &PhpArray) -> Result<Self> {
        let mut bag = Self::default();
        bag.env = Some(EnvPlaceholders::default());
        bag.add(parameters)?;
        Ok(bag)
    }

    /// `$bag instanceof EnvPlaceholderParameterBag`.
    pub fn is_env_placeholder(&self) -> bool {
        self.env.is_some()
    }

    /// The first part of `EnvPlaceholderParameterBag::get()`: the
    /// placeholder of an `env(NAME)` parameter, `None` for another name
    /// or another bag.
    pub(super) fn env_placeholder_get(&self, name: &str) -> Result<Option<Value>> {
        let Some(env_state) = &self.env else {
            return Ok(None);
        };
        let Some(env) = name
            .strip_prefix("env(")
            .and_then(|rest| rest.strip_suffix(')'))
            .filter(|_| name != "env()")
        else {
            return Ok(None);
        };
        {
            let state = env_state.lock();
            for map in [&state.placeholders, &state.unused] {
                if let Some(first) = map.get(env).and_then(|list| list.first()) {
                    return Ok(Some(Value::from(first.as_str())));
                }
            }
        }
        if !is_valid_env_name(env) {
            return Err(Error::invalid_argument(format!(
                "The given env var name \"{name}\" contains invalid characters (allowed characters: letters, digits, hyphens, backslashes, dots and colons)."
            )));
        }
        if self.has(name) {
            let default = self.parent_get(name)?;
            if !matches!(default, Value::Null | Value::String(_)) {
                return Err(Error::runtime(format!(
                    "The default value of an env() parameter must be a string or null, but \"{}\" given to \"{name}\".",
                    default.debug_type()
                )));
            }
        }
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let unique_name = xxh128(format!("{name}_{counter}").as_bytes());
        let placeholder = format!(
            "{}_{}_{}",
            self.get_env_placeholder_unique_prefix(),
            env.replace([':', '-', '.', '\\'], "_"),
            String::from_utf8_lossy(&unique_name)
        );
        let mut state = env_state.lock();
        let list = state.placeholders.get_or_insert_with(env, Vec::new);
        if !list.contains(&placeholder) {
            list.push(placeholder.clone());
        }
        Ok(Some(Value::from(placeholder)))
    }

    /// `getEnvPlaceholderUniquePrefix()`: `env_` and 16 hex digits from
    /// the shape of the parameters (their keys, every leaf nulled). Empty
    /// for another bag.
    pub fn get_env_placeholder_unique_prefix(&self) -> String {
        let Some(env_state) = &self.env else {
            return String::new();
        };
        if let Some(prefix) = &env_state.lock().unique_prefix {
            return prefix.clone();
        }
        let entropy = nulled_leaves(&Value::Array(self.parameters.clone()));
        let hash = xxh128(serialize(&entropy).as_bytes());
        let hash = String::from_utf8_lossy(&hash[hash.len().saturating_sub(16)..]).into_owned();
        let prefix = format!("env_{hash}");
        env_state.lock().unique_prefix = Some(prefix.clone());
        prefix
    }

    /// `getEnvPlaceholders()`: empty for another bag.
    pub fn get_env_placeholders(&self) -> EnvPlaceholderMap {
        self.env
            .as_ref()
            .map(|e| e.lock().placeholders.clone())
            .unwrap_or_default()
    }

    /// `getUnusedEnvPlaceholders()`: empty for another bag.
    pub fn get_unused_env_placeholders(&self) -> EnvPlaceholderMap {
        self.env
            .as_ref()
            .map(|e| e.lock().unused.clone())
            .unwrap_or_default()
    }

    /// `clearUnusedEnvPlaceholders()`.
    pub fn clear_unused_env_placeholders(&self) {
        if let Some(env) = &self.env {
            env.lock().unused.clear();
        }
    }

    /// The `$unusedEnvPlaceholders` a subclass sets
    /// (`MergeExtensionConfigurationParameterBag`).
    pub fn set_unused_env_placeholders(&self, unused: EnvPlaceholderMap) {
        if let Some(env) = &self.env {
            env.lock().unused = unused;
        }
    }

    /// `mergeEnvPlaceholders($bag)`.
    pub fn merge_env_placeholders(&self, bag: &ParameterBag) {
        let Some(env) = &self.env else {
            return;
        };
        let new = bag.get_env_placeholders();
        let new_unused = bag.get_unused_env_placeholders();
        let mut state = env.lock();
        merge_into(&mut state.placeholders, &new);
        merge_into(&mut state.unused, &new_unused);
    }

    /// `setProvidedTypes($providedTypes)`.
    pub fn set_provided_types(&self, provided_types: IdMap<Vec<String>>) {
        if let Some(env) = &self.env {
            env.lock().provided_types = provided_types;
        }
    }

    /// `getProvidedTypes()`: empty for another bag.
    pub fn get_provided_types(&self) -> IdMap<Vec<String>> {
        self.env
            .as_ref()
            .map(|e| e.lock().provided_types.clone())
            .unwrap_or_default()
    }

    /// The end of `EnvPlaceholderParameterBag::resolve()`: every
    /// `env(NAME)` default of a used placeholder is a string or null.
    pub(super) fn check_env_defaults(&self) -> Result<()> {
        let Some(env) = &self.env else {
            return Ok(());
        };
        let envs: Vec<String> = env.lock().placeholders.keys().map(String::from).collect();
        for env in envs {
            let name = format!("env({env})");
            if let Some(default) = self.parameters.get(&Key::from(name.as_str()))
                && !matches!(default, Value::Null | Value::String(_))
            {
                return Err(Error::runtime(format!(
                    "The default value of env parameter \"{env}\" must be a string or null, \"{}\" given.",
                    default.debug_type()
                )));
            }
        }
        Ok(())
    }
}
