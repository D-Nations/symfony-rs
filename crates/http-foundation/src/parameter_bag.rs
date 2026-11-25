//! Port of Symfony\Component\HttpFoundation\ParameterBag.php: a container
//! of key/value pairs holding PHP values.
//!
//! Keys follow PHP's array rules (`'12'` is the int key 12). `filter()`
//! runs on `php_value::filter::filter_var()`, which reproduces the
//! filters `ParameterBag` is used with and refuses others
//! ([`Class::NotPorted`]). `getEnum()` resolves the enum class in a
//! `php_value` [`Registry`], as PHP's class table would. Not reproduced:
//! the `E_DEPRECATED` PHP raises when `getEnum()` truncates a float, and
//! `filterArray()`'s guard against PHP references (`&`), which php-value
//! arrays cannot hold.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation ParameterBag.php

use php_value::{
    Key, Object, PhpArray, PhpString, Value,
    constants::Registry,
    filter::{
        FILTER_CALLBACK, FILTER_FORCE_ARRAY, FILTER_NULL_ON_FAILURE, FILTER_REQUIRE_ARRAY,
        FILTER_REQUIRE_SCALAR, FILTER_VALIDATE_BOOL, FILTER_VALIDATE_INT, filter_var,
    },
    object::{Closure, backed_enum_from},
};

use crate::exception::{Class, Error, Result};

pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\ParameterBag";

/// `new ParameterBag($parameters)`.
#[derive(Debug, Clone, Default)]
pub struct ParameterBag {
    parameters: PhpArray,
}

/// Which class's method is running, where `InputBag` overrides
/// `ParameterBag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Flavor {
    Parameter,
    Input,
}

impl Flavor {
    /// `__METHOD__` of `filter()`.
    fn filter_method(self) -> &'static str {
        match self {
            Self::Parameter => "Symfony\\Component\\HttpFoundation\\ParameterBag::filter",
            Self::Input => "Symfony\\Component\\HttpFoundation\\InputBag::filter",
        }
    }
}

/// `get_debug_type($value)`.
fn debug_type(value: &Value) -> String {
    value.debug_type()
}

/// `is_array($options) || !$options` becomes `['flags' => $options]` when
/// `$options` is truthy; `null` reads as an empty array. Other falsy
/// non-arrays make PHP fail on `$options['flags'] ??= 0`; they are refused.
pub(crate) fn normalize_options(options: &Value) -> Result<PhpArray> {
    match options {
        Value::Array(array) => Ok(array.clone()),
        Value::Null => Ok(PhpArray::new()),
        other if truthy(other) => {
            let mut array = PhpArray::new();
            array.insert("flags", other.clone());
            Ok(array)
        }
        other => Err(Error::new(
            Class::NotPorted,
            format!("filter() options of {}", other.debug_type()),
        )),
    }
}

/// PHP's boolean conversion of a non-array.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::String(s) => !(s.is_empty() || s.as_bytes() == b"0"),
        Value::Array(a) => !a.is_empty(),
        Value::Object(_) => true,
    }
}

/// `$options['flags']` as an int (`isset()` is false for null).
pub(crate) fn flags_of(options: &PhpArray) -> Result<Option<i64>> {
    match options.get(&Key::from("flags")) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Int(flags)) => Ok(Some(*flags)),
        Some(other) => Err(Error::new(
            Class::NotPorted,
            format!("filter() flags of {}", other.debug_type()),
        )),
    }
}

/// The body both `filter()` methods share, from `FILTER_CALLBACK`'s check
/// on; `value` has passed the caller's own checks.
pub(crate) fn filter_value(
    flavor: Flavor,
    key: &str,
    value: Value,
    filter: i64,
    mut options: PhpArray,
) -> Result<Value> {
    if filter & FILTER_CALLBACK != 0 {
        let callback = options
            .get(&Key::from("options"))
            .cloned()
            .unwrap_or_default();
        let is_closure = callback
            .as_object()
            .is_some_and(|o| o.downcast_ref::<Closure>().is_some());
        if !is_closure {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                format!(
                    "A Closure must be passed to \"{}()\" when FILTER_CALLBACK is used, \"{}\" given.",
                    flavor.filter_method(),
                    debug_type(&callback)
                ),
            ));
        }
    }

    let mut flags = flags_of(&options)?.unwrap_or(0);
    let null_on_failure = flags & FILTER_NULL_ON_FAILURE != 0;
    flags |= FILTER_NULL_ON_FAILURE;

    let mut invalid = false;
    let is_array = matches!(value, Value::Array(_));
    let force_array =
        !is_array && flags & FILTER_FORCE_ARRAY != 0 && flags & FILTER_REQUIRE_ARRAY == 0;

    let value = match value {
        Value::Array(array)
            if !null_on_failure
                && !array.is_empty()
                && flags & FILTER_REQUIRE_SCALAR == 0
                && flags & (FILTER_REQUIRE_ARRAY | FILTER_FORCE_ARRAY) != 0 =>
        {
            flags &= !(FILTER_REQUIRE_ARRAY | FILTER_FORCE_ARRAY);
            options.insert("flags", Value::Int(flags));
            Value::Array(filter_array(&array, filter, &options, &mut invalid)?)
        }
        value => {
            if force_array {
                flags &= !FILTER_FORCE_ARRAY;
            }
            options.insert("flags", Value::Int(flags));
            let filtered = filter_var(&value, filter, &Value::Array(options))?;
            invalid = filtered.is_null();
            if force_array {
                Value::Array(PhpArray::list([filtered]))
            } else {
                filtered
            }
        }
    };

    if !invalid || null_on_failure {
        return Ok(value);
    }
    let message =
        format!("value \"{key}\" is invalid and flag \"FILTER_NULL_ON_FAILURE\" was not set.");
    Err(match flavor {
        Flavor::Parameter => Error::new(Class::PhpUnexpectedValue, format!("Parameter {message}")),
        Flavor::Input => Error::new(Class::BadRequest, format!("Input {message}")),
    })
}

/// `self::filterArray()`: every element filtered, at any depth.
fn filter_array(
    array: &PhpArray,
    filter: i64,
    options: &PhpArray,
    invalid: &mut bool,
) -> Result<PhpArray> {
    let mut out = PhpArray::new();
    for (key, item) in array.iter() {
        let filtered = match item {
            Value::Array(inner) => Value::Array(filter_array(inner, filter, options, invalid)?),
            scalar => {
                let filtered = filter_var(scalar, filter, &Value::Array(options.clone()))?;
                *invalid = *invalid || filtered.is_null();
                filtered
            }
        };
        out.insert(key.clone(), filtered);
    }
    Ok(out)
}

/// `$class::from($value)` inside `getEnum()`, with its catch of
/// `\ValueError|\TypeError`.
pub(crate) fn enum_from(key: &str, enums: &Registry, class: &str, value: &Value) -> Result<Object> {
    let Some(cases) = enums.cases(class) else {
        return Err(Error::new(
            Class::PhpError,
            format!("Class \"{class}\" not found"),
        ));
    };
    match backed_enum_from(class, cases, value) {
        None => Err(Error::new(
            Class::PhpError,
            format!("Call to undefined method {class}::from()"),
        )),
        Some(Ok(case)) => Ok(case),
        Some(Err(error)) => {
            let mut message =
                format!("Parameter \"{key}\" cannot be converted to enum: ").into_bytes();
            message.extend_from_slice(error.message().as_bytes());
            message.push(b'.');
            Err(Error::new(Class::UnexpectedValue, PhpString::from(message)))
        }
    }
}

/// `preg_replace('/[^[:class:]]/', '', $s)` without the `u` flag: PCRE's
/// classes cover ASCII only.
pub(crate) fn keep(s: &PhpString, class: fn(&u8) -> bool) -> PhpString {
    PhpString::from(
        s.as_bytes()
            .iter()
            .copied()
            .filter(class)
            .collect::<Vec<u8>>(),
    )
}

impl ParameterBag {
    pub fn new(parameters: PhpArray) -> Self {
        Self { parameters }
    }

    /// `all()`: the parameters.
    pub fn all(&self) -> &PhpArray {
        &self.parameters
    }

    /// `all($key)`: the array stored under `key`, or `[]` when there is
    /// none or it is null (`?? []`).
    pub fn all_of(&self, key: &str) -> Result<PhpArray> {
        match self.parameters.get(&Key::from(key)) {
            None | Some(Value::Null) => Ok(PhpArray::new()),
            Some(Value::Array(array)) => Ok(array.clone()),
            Some(other) => Err(Error::new(
                Class::BadRequest,
                format!(
                    "Unexpected value for parameter \"{key}\": expecting \"array\", got \"{}\".",
                    debug_type(other)
                ),
            )),
        }
    }

    /// `keys()`.
    pub fn keys(&self) -> Vec<Key> {
        self.parameters.keys().cloned().collect()
    }

    /// `replace($parameters)`.
    pub fn replace(&mut self, parameters: PhpArray) {
        self.parameters = parameters;
    }

    /// `add($parameters)`: `array_replace()`.
    pub fn add(&mut self, parameters: PhpArray) {
        for (key, value) in parameters.iter() {
            self.parameters.insert(key.clone(), value.clone());
        }
    }

    /// `get($key, $default)`.
    pub fn get(&self, key: &str, default: Value) -> Value {
        self.parameters
            .get(&Key::from(key))
            .cloned()
            .unwrap_or(default)
    }

    /// `set($key, $value)`.
    pub fn set(&mut self, key: &str, value: Value) {
        self.parameters.insert(Key::from(key), value);
    }

    /// `has($key)`.
    pub fn has(&self, key: &str) -> bool {
        self.parameters.contains_key(&Key::from(key))
    }

    /// `remove($key)`.
    pub fn remove(&mut self, key: &str) {
        self.parameters.remove(&Key::from(key));
    }

    /// `getAlpha($key, $default)`.
    pub fn get_alpha(&self, key: &str, default: impl Into<PhpString>) -> Result<PhpString> {
        Ok(keep(
            &self.get_string(key, default)?,
            u8::is_ascii_alphabetic,
        ))
    }

    /// `getAlnum($key, $default)`.
    pub fn get_alnum(&self, key: &str, default: impl Into<PhpString>) -> Result<PhpString> {
        Ok(keep(
            &self.get_string(key, default)?,
            u8::is_ascii_alphanumeric,
        ))
    }

    /// `getDigits($key, $default)`.
    pub fn get_digits(&self, key: &str, default: impl Into<PhpString>) -> Result<PhpString> {
        Ok(keep(&self.get_string(key, default)?, u8::is_ascii_digit))
    }

    /// `getString($key, $default)`.
    pub fn get_string(&self, key: &str, default: impl Into<PhpString>) -> Result<PhpString> {
        let value = self.get(key, Value::String(default.into()));
        if !value.is_scalar_or_stringable() {
            return Err(Error::new(
                Class::UnexpectedValue,
                format!("Parameter value \"{key}\" cannot be converted to \"string\"."),
            ));
        }
        Ok(value.to_php_string_or_stringable().unwrap_or_default())
    }

    /// `getInt($key, $default)`.
    pub fn get_int(&self, key: &str, default: i64) -> Result<i64> {
        let value = self.filter(
            key,
            Value::Int(default),
            FILTER_VALIDATE_INT,
            &scalar_or_null_flags(),
        )?;
        match value {
            Value::Int(i) => Ok(i),
            _ => Err(Error::new(
                Class::UnexpectedValue,
                format!("Parameter value \"{key}\" cannot be converted to \"int\"."),
            )),
        }
    }

    /// `getBoolean($key, $default)`.
    pub fn get_boolean(&self, key: &str, default: bool) -> Result<bool> {
        let value = self.filter(
            key,
            Value::Bool(default),
            FILTER_VALIDATE_BOOL,
            &scalar_or_null_flags(),
        )?;
        match value {
            Value::Bool(b) => Ok(b),
            _ => Err(Error::new(
                Class::UnexpectedValue,
                format!("Parameter value \"{key}\" cannot be converted to \"bool\"."),
            )),
        }
    }

    /// `getEnum($key, $class, $default)`, with `class` looked up in
    /// `enums`.
    pub fn get_enum(
        &self,
        key: &str,
        enums: &Registry,
        class: &str,
        default: Option<Object>,
    ) -> Result<Option<Object>> {
        let value = self.get(key, Value::Null);
        if value.is_null() {
            return Ok(default);
        }
        enum_from(key, enums, class, &value).map(Some)
    }

    /// `filter($key, $default, $filter, $options)`; `options` is PHP's
    /// argument (an int of flags or an array). PHP's defaults are
    /// `FILTER_DEFAULT` and an empty array.
    pub fn filter(&self, key: &str, default: Value, filter: i64, options: &Value) -> Result<Value> {
        let value = self.get(key, default);
        let mut options = normalize_options(options)?;
        if matches!(value, Value::Array(_)) && flags_of(&options)?.is_none() {
            options.insert("flags", Value::Int(FILTER_REQUIRE_ARRAY));
        }
        if let Value::Object(object) = &value
            && object.to_php_string().is_none()
        {
            return Err(Error::new(
                Class::UnexpectedValue,
                format!("Parameter value \"{key}\" cannot be filtered."),
            ));
        }
        filter_value(Flavor::Parameter, key, value, filter, options)
    }

    /// `getIterator()`.
    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Value)> {
        self.parameters.iter()
    }

    /// `count()`.
    pub fn count(&self) -> usize {
        self.parameters.len()
    }
}

/// `['flags' => \FILTER_REQUIRE_SCALAR | \FILTER_NULL_ON_FAILURE]`.
pub(crate) fn scalar_or_null_flags() -> Value {
    let mut options = PhpArray::new();
    options.insert(
        "flags",
        Value::Int(FILTER_REQUIRE_SCALAR | FILTER_NULL_ON_FAILURE),
    );
    Value::Array(options)
}
