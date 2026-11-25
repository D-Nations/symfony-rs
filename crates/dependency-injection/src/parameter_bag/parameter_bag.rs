//! Port of Symfony\Component\DependencyInjection\ParameterBag\ParameterBag.php,
//! FrozenParameterBag.php and ParameterBagInterface.php.
//!
//! One [`ParameterBag`] type: [`ParameterBag::frozen`] builds the frozen
//! subclass, whose mutators fail with upstream's `LogicException`s.
//! Parameter names are `String`s (a numeric name is refused, as
//! upstream), values are PHP values. The names `add()` takes (array keys)
//! and those inside placeholders keep their bytes, so a name that is not
//! UTF-8 is found and suggested as PHP does (recorded:
//! `tests/data/parameter_bag.json`); messages show it lossily.
//!
//! `resolveString()`'s patterns are matched by hand: `^%([^%\s]+)%$`
//! (PCRE's `$` also matches before a final newline) and
//! `%%|%([^%\s]+)%`, with `\s` the six ASCII spaces PCRE uses without
//! Unicode mode.
//!
//! Upstream: symfony/dependency-injection ParameterBag/ParameterBag.php ParameterBag/FrozenParameterBag.php ParameterBag/ParameterBagInterface.php

use php_value::{PhpArray, PhpString, Value, array::Key, numeric::is_numeric, string::levenshtein};
use symfony_deprecation_contracts::trigger_deprecation;

use super::env_placeholder_parameter_bag::EnvPlaceholders;
use crate::exception::{Error, ErrorKind, ParameterNotFound, Result};

/// A parameter deprecation: the arguments of `trigger_deprecation()`.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterDeprecation {
    pub package: String,
    pub version: String,
    pub message: String,
    /// The `...$args` of the message (upstream passes the name).
    pub args: Vec<Value>,
}

/// `ParameterBag` (and `FrozenParameterBag`).
#[derive(Debug, Clone, Default)]
pub struct ParameterBag {
    pub(super) parameters: PhpArray,
    pub(super) resolved: bool,
    deprecated: Vec<(String, ParameterDeprecation)>,
    non_empty: Vec<(String, String)>,
    frozen: bool,
    /// The `EnvPlaceholderParameterBag` state, for that subclass.
    pub(super) env: Option<EnvPlaceholders>,
}

fn key_name(key: &Key) -> String {
    match key {
        Key::Int(i) => i.to_string(),
        Key::Str(s) => s.to_string_lossy().into_owned(),
    }
}

/// `str_contains($haystack, $needle)`.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    needle.is_empty() || haystack.windows(needle.len()).any(|w| w == needle)
}

/// `\s` in a PCRE pattern without Unicode mode.
fn is_space(b: u8) -> bool {
    matches!(b, b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b' ')
}

/// The name of a `%name%` placeholder at the start of `s` (after its
/// first `%`), with the length matched, for `%([^%\s]+)%`.
fn placeholder_at(s: &[u8]) -> Option<(&[u8], usize)> {
    if s.first() != Some(&b'%') {
        return None;
    }
    let end = s[1..].iter().position(|&b| b == b'%' || is_space(b))?;
    if end == 0 || s[1 + end] != b'%' {
        return None;
    }
    Some((&s[1..=end], end + 2))
}

impl ParameterBag {
    /// `new ParameterBag($parameters)`.
    pub fn new(parameters: &PhpArray) -> Result<Self> {
        let mut bag = Self::default();
        bag.add(parameters)?;
        Ok(bag)
    }

    /// `new FrozenParameterBag($parameters, $deprecatedParameters,
    /// $nonEmptyParameters)`: resolved, and closed to changes.
    pub fn frozen(
        parameters: PhpArray,
        deprecated: Vec<(String, ParameterDeprecation)>,
        non_empty: Vec<(String, String)>,
    ) -> Self {
        Self {
            parameters,
            resolved: true,
            deprecated,
            non_empty,
            frozen: true,
            env: None,
        }
    }

    /// `$bag instanceof FrozenParameterBag`.
    pub fn is_frozen(&self) -> bool {
        self.frozen
    }

    fn refuse(&self, method: &str) -> Result<()> {
        if self.frozen {
            Err(Error::logic(format!(
                "Impossible to call {method}() on a frozen ParameterBag."
            )))
        } else {
            Ok(())
        }
    }

    /// `clear()`.
    pub fn clear(&mut self) -> Result<()> {
        self.refuse("clear")?;
        self.parameters = PhpArray::new();
        self.deprecated.clear();
        self.resolved = false;
        Ok(())
    }

    /// `add($parameters)`.
    pub fn add(&mut self, parameters: &PhpArray) -> Result<()> {
        self.refuse("add")?;
        for (key, value) in parameters.iter() {
            self.set_bytes(&key.bytes(), value.clone())?;
        }
        Ok(())
    }

    /// `all()`.
    pub fn all(&self) -> &PhpArray {
        &self.parameters
    }

    /// `allDeprecated()`.
    pub fn all_deprecated(&self) -> &[(String, ParameterDeprecation)] {
        &self.deprecated
    }

    /// `allNonEmpty()`: the message for each parameter that must not be
    /// empty.
    pub fn all_non_empty(&self) -> &[(String, String)] {
        &self.non_empty
    }

    fn non_empty_message(&self, name: &str) -> Option<&str> {
        self.non_empty
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, m)| m.as_str())
    }

    /// `get($name)` (`EnvPlaceholderParameterBag::get()` for that
    /// subclass).
    pub fn get(&self, name: &str) -> Result<Value> {
        self.get_bytes(name.as_bytes())
    }

    /// `get($name)` for a name of any bytes (names in placeholders).
    fn get_bytes(&self, name_bytes: &[u8]) -> Result<Value> {
        if let Ok(name) = std::str::from_utf8(name_bytes)
            && let Some(placeholder) = self.env_placeholder_get(name)?
        {
            return Ok(placeholder);
        }
        self.parent_get_bytes(name_bytes)
    }

    /// `ParameterBag::get($name)`.
    pub(super) fn parent_get(&self, name: &str) -> Result<Value> {
        self.parent_get_bytes(name.as_bytes())
    }

    /// `ParameterBag::get($name)` for a name of any bytes.
    fn parent_get_bytes(&self, name_bytes: &[u8]) -> Result<Value> {
        let Some(value) = self.parameters.get(&Key::from_bytes(name_bytes)) else {
            return Err(self.not_found(name_bytes));
        };
        let name = &*String::from_utf8_lossy(name_bytes);
        if let Some((_, deprecation)) = self.deprecated.iter().find(|(n, _)| n == name) {
            trigger_parameter_deprecation(deprecation)?;
        }
        if let Some(message) = self.non_empty_message(name) {
            let empty = match value {
                Value::Null => true,
                Value::String(s) => s.is_empty(),
                Value::Array(a) => a.is_empty(),
                _ => false,
            };
            if empty {
                return Err(Error::empty_parameter_value(message));
            }
        }
        Ok(value.clone())
    }

    /// The `ParameterNotFoundException` of `get($name)` for a missing name.
    fn not_found(&self, name_bytes: &[u8]) -> Error {
        let name = &*String::from_utf8_lossy(name_bytes);
        // `if (!$name)`: "" and "0".
        if name.is_empty() || name == "0" {
            return Error::parameter_not_found(ParameterNotFound::new(name));
        }
        if let Some(message) = self.non_empty_message(name) {
            return Error::parameter_not_found(ParameterNotFound {
                extra_message: Some(message.to_string()),
                ..ParameterNotFound::new(name)
            });
        }
        let mut alternatives = Vec::new();
        for key in self.parameters.keys() {
            let key = key.bytes();
            let lev = levenshtein(name_bytes, &key);
            #[expect(clippy::cast_precision_loss, reason = "PHP's `strlen($name) / 3`")]
            let limit = name_bytes.len() as f64 / 3.0;
            #[expect(clippy::cast_precision_loss, reason = "PHP's int and float comparison")]
            if lev as f64 <= limit || contains(&key, name_bytes) {
                alternatives.push(String::from_utf8_lossy(&key).into_owned());
            }
        }
        let mut non_nested = None;
        if alternatives.is_empty() && name_bytes.contains(&b'.') {
            let mut lengths: Vec<usize> =
                name_bytes.split(|&b| b == b'.').map(<[u8]>::len).collect();
            let cut = |s: &[u8], n: usize| -> Vec<u8> {
                if n >= s.len() {
                    Vec::new()
                } else {
                    s[..s.len() - n].to_vec()
                }
            };
            let last = lengths.pop().unwrap_or(0);
            let mut key = cut(name_bytes, 1 + last);
            while !lengths.is_empty() {
                if self.parameters.contains_key(&Key::from_bytes(&key)) {
                    if matches!(self.get_bytes(&key), Ok(Value::Array(_))) {
                        non_nested = Some(String::from_utf8_lossy(&key).into_owned());
                    }
                    break;
                }
                let last = lengths.pop().unwrap_or(0);
                key = cut(&key, 1 + last);
            }
        }
        Error::parameter_not_found(ParameterNotFound {
            alternatives,
            non_nested_alternative: non_nested,
            ..ParameterNotFound::new(name)
        })
    }

    /// `set($name, $value)`.
    pub fn set(&mut self, name: &str, value: Value) -> Result<()> {
        self.set_bytes(name.as_bytes(), value)
    }

    /// `set($name, $value)` for a name of any bytes (`add()`'s keys).
    fn set_bytes(&mut self, name: &[u8], value: Value) -> Result<()> {
        self.refuse("set")?;
        if is_numeric(name) {
            return Err(Error::invalid_argument(format!(
                "The parameter name \"{}\" cannot be numeric.",
                String::from_utf8_lossy(name)
            )));
        }
        self.parameters.insert(Key::from_bytes(name), value);
        Ok(())
    }

    /// `deprecate($name, $package, $version, $message)`; upstream's
    /// default message is `'The parameter "%s" is deprecated.'`.
    pub fn deprecate(
        &mut self,
        name: &str,
        package: &str,
        version: &str,
        message: Option<&str>,
    ) -> Result<()> {
        self.refuse("deprecate")?;
        if !self.has(name) {
            return Err(Error::parameter_not_found(ParameterNotFound::new(name)));
        }
        let deprecation = ParameterDeprecation {
            package: package.to_string(),
            version: version.to_string(),
            message: message
                .unwrap_or("The parameter \"%s\" is deprecated.")
                .to_string(),
            args: vec![Value::from(name)],
        };
        if let Some(entry) = self.deprecated.iter_mut().find(|(n, _)| n == name) {
            entry.1 = deprecation;
        } else {
            self.deprecated.push((name.to_string(), deprecation));
        }
        Ok(())
    }

    /// `cannotBeEmpty($name, $message)`.
    pub fn cannot_be_empty(&mut self, name: &str, message: &str) -> Result<()> {
        self.refuse("cannotBeEmpty")?;
        if let Some(entry) = self.non_empty.iter_mut().find(|(n, _)| n == name) {
            entry.1 = message.to_string();
        } else {
            self.non_empty.push((name.to_string(), message.to_string()));
        }
        Ok(())
    }

    /// `has($name)`.
    pub fn has(&self, name: &str) -> bool {
        self.parameters.contains_key(&Key::from(name))
    }

    /// `remove($name)`.
    pub fn remove(&mut self, name: &str) -> Result<()> {
        self.refuse("remove")?;
        self.parameters.remove(&Key::from(name));
        self.deprecated.retain(|(n, _)| n != name);
        self.non_empty.retain(|(n, _)| n != name);
        Ok(())
    }

    /// `resolve()`: every value resolved and unescaped once.
    pub fn resolve(&mut self) -> Result<()> {
        if self.resolved {
            return Ok(());
        }
        let mut parameters = PhpArray::new();
        for (key, value) in self.parameters.iter() {
            match self.resolve_value(value, &[]) {
                Ok(value) => parameters.insert(key.clone(), unescape_value(&value)),
                Err(mut e) => {
                    if e.is(&ErrorKind::ParameterNotFound) {
                        let source = key_name(key);
                        e.update_parameter_not_found(|f| f.source_key = Some(source));
                    }
                    return Err(e);
                }
            }
        }
        self.parameters = parameters;
        self.resolved = true;
        self.check_env_defaults()
    }

    /// `resolveValue($value, $resolving)`: placeholders in strings, and in
    /// array keys and values.
    pub fn resolve_value(&self, value: &Value, resolving: &[String]) -> Result<Value> {
        match value {
            Value::Array(array) => {
                let mut args = PhpArray::new();
                for (key, v) in array.iter() {
                    let resolved_key = match key {
                        Key::Str(s) => {
                            let resolved =
                                self.resolve_value(&Value::String(s.clone()), resolving)?;
                            let scalar_or_stringable = match &resolved {
                                Value::Bool(_)
                                | Value::Int(_)
                                | Value::Float(_)
                                | Value::String(_) => true,
                                Value::Object(o) => o.is_stringable(),
                                Value::Null | Value::Array(_) => false,
                            };
                            if !scalar_or_stringable {
                                return Err(Error::runtime(format!(
                                    "Array keys must be a scalar-value, but found key \"{}\" to resolve to type \"{}\".",
                                    s.to_string_lossy(),
                                    resolved.debug_type()
                                )));
                            }
                            match &resolved {
                                Value::Object(o) => {
                                    Key::from(o.to_php_string().unwrap_or_default())
                                }
                                other => Key::from_value(other).unwrap_or_else(|| key.clone()),
                            }
                        }
                        Key::Int(_) => key.clone(),
                    };
                    args.insert(resolved_key, self.resolve_value(v, resolving)?);
                }
                Ok(Value::Array(args))
            }
            Value::String(s) if !s.is_empty() && s.as_bytes().contains(&b'%') => {
                self.resolve_string(s, resolving)
            }
            other => Ok(other.clone()),
        }
    }

    /// `resolveString($value, $resolving)`.
    pub fn resolve_string(&self, value: &PhpString, resolving: &[String]) -> Result<Value> {
        let bytes = value.as_bytes();
        // `/^%([^%\s]+)%$/`: `$` also matches before a final "\n".
        let body = bytes.strip_suffix(b"\n").unwrap_or(bytes);
        if let Some((name, len)) = placeholder_at(body)
            && len == body.len()
        {
            let key = String::from_utf8_lossy(name).into_owned();
            if resolving.contains(&key) {
                return Err(Error::parameter_circular_reference(resolving.to_vec()));
            }
            let mut resolving = resolving.to_vec();
            resolving.push(key.clone());
            let found = self.get_bytes(name)?;
            return if self.resolved {
                Ok(escape_value(&found))
            } else {
                self.resolve_value(&found, &resolving)
            };
        }
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i..].starts_with(b"%%") {
                out.extend_from_slice(b"%%");
                i += 2;
                continue;
            }
            let Some((name, len)) = placeholder_at(&bytes[i..]) else {
                out.push(bytes[i]);
                i += 1;
                continue;
            };
            let key = String::from_utf8_lossy(name).into_owned();
            if resolving.contains(&key) {
                return Err(Error::parameter_circular_reference(resolving.to_vec()));
            }
            let resolved = self.get_bytes(name)?;
            let text = match &resolved {
                Value::String(s) => s.clone(),
                Value::Int(_) | Value::Float(_) => resolved.to_php_string().unwrap_or_default(),
                other => {
                    return Err(Error::runtime(format!(
                        "A string value must be composed of strings and/or numbers, but found parameter \"{key}\" of type \"{}\" inside string value \"{}\".",
                        other.debug_type(),
                        value.to_string_lossy()
                    )));
                }
            };
            let mut inner = resolving.to_vec();
            inner.push(key);
            let replacement = if self.resolved {
                escape_value(&Value::String(text))
            } else {
                self.resolve_string(&text, &inner)?
            };
            // preg_replace_callback() casts the callback's result to string.
            out.extend_from_slice(callback_string(&replacement).as_bytes());
            i += len;
        }
        Ok(Value::String(PhpString::from(out)))
    }

    /// `isResolved()`.
    pub fn is_resolved(&self) -> bool {
        self.resolved
    }

    /// `escapeValue($value)`.
    pub fn escape_value(&self, value: &Value) -> Value {
        escape_value(value)
    }

    /// `unescapeValue($value)`.
    pub fn unescape_value(&self, value: &Value) -> Value {
        unescape_value(value)
    }

    /// Sets the resolved flag, for subclasses that resolve on their own
    /// terms (`$this->resolved = …`).
    pub fn set_resolved(&mut self, resolved: bool) {
        self.resolved = resolved;
    }

    /// The parameters array, for subclasses (`$this->parameters`).
    pub fn parameters_mut(&mut self) -> &mut PhpArray {
        &mut self.parameters
    }
}

/// `(string)` of a `preg_replace_callback()` callback's result.
fn callback_string(value: &Value) -> PhpString {
    match value {
        Value::Array(_) => PhpString::from("Array"),
        other => other.to_php_string_or_stringable().unwrap_or_default(),
    }
}

fn map_strings(value: &Value, f: &dyn Fn(&[u8]) -> Vec<u8>) -> Value {
    match value {
        Value::String(s) => Value::String(PhpString::from(f(s.as_bytes()))),
        Value::Array(array) => {
            let mut result = PhpArray::new();
            for (k, v) in array.iter() {
                result.insert(k.clone(), map_strings(v, f));
            }
            Value::Array(result)
        }
        other => other.clone(),
    }
}

fn replace(haystack: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut i = 0;
    while i < haystack.len() {
        if haystack[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(haystack[i]);
            i += 1;
        }
    }
    out
}

/// `escapeValue()`: `%` doubled in strings, recursively in arrays.
pub fn escape_value(value: &Value) -> Value {
    map_strings(value, &|s| replace(s, b"%", b"%%"))
}

/// `unescapeValue()`: `%%` back to `%` in strings, recursively in arrays.
pub fn unescape_value(value: &Value) -> Value {
    map_strings(value, &|s| replace(s, b"%%", b"%"))
}

/// `trigger_deprecation(...$deprecation)`.
pub(crate) fn trigger_parameter_deprecation(deprecation: &ParameterDeprecation) -> Result<()> {
    let message = if deprecation.args.is_empty() {
        deprecation.message.as_bytes().to_vec()
    } else {
        symfony_config::php::sprintf(deprecation.message.as_bytes(), &deprecation.args).map_err(
            |e| {
                Error::new(
                    ErrorKind::Other(e.class().name().to_string()),
                    String::from_utf8_lossy(e.get_message()).into_owned(),
                )
            },
        )?
    };
    trigger_deprecation(
        deprecation.package.as_bytes(),
        deprecation.version.as_bytes(),
        &message,
    );
    Ok(())
}
