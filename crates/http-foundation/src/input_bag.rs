//! Port of Symfony\Component\HttpFoundation\InputBag.php: a ParameterBag
//! for user input (`$_GET`, `$_POST`, `$_COOKIE`), whose getters accept
//! scalars only and throw `BadRequestException` otherwise.
//!
//! `get()`'s return type is `string|int|float|bool|null`, so PHP converts
//! a `\Stringable` value (or default) to its string on return; this port
//! does the same. The constructor stores what it is given unchecked, as
//! ParameterBag's does; `set()`, `add()` and `replace()` check.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/http-foundation InputBag.php

use php_value::{
    Key, Object, PhpArray, PhpString, Value,
    constants::Registry,
    filter::{FILTER_FORCE_ARRAY, FILTER_REQUIRE_ARRAY, FILTER_VALIDATE_BOOL, FILTER_VALIDATE_INT},
};

use crate::{
    exception::{Class, Error, Result},
    parameter_bag::{
        self, Flavor, ParameterBag, flags_of, keep, normalize_options, scalar_or_null_flags,
    },
};

pub const CLASS: &str = "Symfony\\Component\\HttpFoundation\\InputBag";

/// `new InputBag($parameters)`.
#[derive(Debug, Clone, Default)]
pub struct InputBag {
    bag: ParameterBag,
}

/// `string|int|float|bool|null` as a return type: a `\Stringable` becomes
/// its string.
fn returned(value: Value) -> Value {
    match &value {
        Value::Object(object) => match object.to_php_string() {
            Some(text) => Value::String(text),
            None => value,
        },
        _ => value,
    }
}

impl InputBag {
    pub fn new(parameters: PhpArray) -> Self {
        Self {
            bag: ParameterBag::new(parameters),
        }
    }

    /// `get($key, $default)`.
    pub fn get(&self, key: &str, default: Value) -> Result<Value> {
        if !default.is_null() && !default.is_scalar_or_stringable() {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                format!(
                    "Expected a scalar value as a 2nd argument to \"{CLASS}::get()\", \"{}\" given.",
                    default.debug_type()
                ),
            ));
        }
        match self.bag.all().get(&Key::from(key)) {
            None => Ok(returned(default)),
            Some(value) if !value.is_null() && !value.is_scalar_or_stringable() => Err(Error::new(
                Class::BadRequest,
                format!("Input value \"{key}\" contains a non-scalar value."),
            )),
            Some(value) => Ok(returned(value.clone())),
        }
    }

    /// `replace($inputs)`.
    pub fn replace(&mut self, inputs: PhpArray) -> Result<()> {
        self.bag.replace(PhpArray::new());
        self.add(inputs)
    }

    /// `add($inputs)`: `set()` for each.
    pub fn add(&mut self, inputs: PhpArray) -> Result<()> {
        for (key, value) in inputs.iter() {
            self.set_key(key.clone(), value.clone())?;
        }
        Ok(())
    }

    /// `set($key, $value)`.
    pub fn set(&mut self, key: &str, value: Value) -> Result<()> {
        self.set_key(Key::from(key), value)
    }

    fn set_key(&mut self, key: Key, value: Value) -> Result<()> {
        if !value.is_null() && !value.is_scalar_or_stringable() && value.as_array().is_none() {
            return Err(Error::new(
                Class::PhpInvalidArgument,
                format!(
                    "Expected a scalar, or an array as a 2nd argument to \"{CLASS}::set()\", \"{}\" given.",
                    value.debug_type()
                ),
            ));
        }
        let mut one = PhpArray::new();
        one.insert(key, value);
        self.bag.add(one);
        Ok(())
    }

    /// `getInt($key, $default)`.
    pub fn get_int(&self, key: &str, default: i64) -> Result<i64> {
        match self.filter(
            key,
            Value::Int(default),
            FILTER_VALIDATE_INT,
            &scalar_or_null_flags(),
        )? {
            Value::Int(i) => Ok(i),
            _ => Err(Error::new(
                Class::BadRequest,
                format!("Input value \"{key}\" cannot be converted to \"int\"."),
            )),
        }
    }

    /// `getBoolean($key, $default)`.
    pub fn get_boolean(&self, key: &str, default: bool) -> Result<bool> {
        match self.filter(
            key,
            Value::Bool(default),
            FILTER_VALIDATE_BOOL,
            &scalar_or_null_flags(),
        )? {
            Value::Bool(b) => Ok(b),
            _ => Err(Error::new(
                Class::BadRequest,
                format!("Input value \"{key}\" cannot be converted to \"bool\"."),
            )),
        }
    }

    /// `getEnum($key, $class, $default)`: ParameterBag's, with an
    /// UnexpectedValueException rethrown as BadRequestException.
    pub fn get_enum(
        &self,
        key: &str,
        enums: &Registry,
        class: &str,
        default: Option<Object>,
    ) -> Result<Option<Object>> {
        let rethrow = |error: Error| {
            if error.is_a(Class::UnexpectedValue.name()) {
                error.rethrown_as(Class::BadRequest)
            } else {
                error
            }
        };
        let value = self.get(key, Value::Null).map_err(rethrow)?;
        if value.is_null() {
            return Ok(default);
        }
        parameter_bag::enum_from(key, enums, class, &value)
            .map(Some)
            .map_err(rethrow)
    }

    /// `getString($key, $default)`.
    pub fn get_string(&self, key: &str, default: impl Into<PhpString>) -> Result<PhpString> {
        Ok(self
            .get(key, Value::String(default.into()))?
            .to_php_string()
            .unwrap_or_default())
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

    /// `filter($key, $default, $filter, $options)`.
    pub fn filter(&self, key: &str, default: Value, filter: i64, options: &Value) -> Result<Value> {
        let value = if self.has(key) {
            self.bag.get(key, Value::Null)
        } else {
            default
        };
        let options = normalize_options(options)?;
        let flags = flags_of(&options)?.unwrap_or(0);
        if matches!(value, Value::Array(_))
            && flags & (FILTER_REQUIRE_ARRAY | FILTER_FORCE_ARRAY) == 0
        {
            return Err(Error::new(
                Class::BadRequest,
                format!(
                    "Input value \"{key}\" contains an array, but \"FILTER_REQUIRE_ARRAY\" or \"FILTER_FORCE_ARRAY\" flags were not set."
                ),
            ));
        }
        parameter_bag::filter_value(Flavor::Input, key, value, filter, options)
    }

    // ParameterBag's methods InputBag inherits.

    /// `all()`.
    pub fn all(&self) -> &PhpArray {
        self.bag.all()
    }

    /// `all($key)`.
    pub fn all_of(&self, key: &str) -> Result<PhpArray> {
        self.bag.all_of(key)
    }

    /// `keys()`.
    pub fn keys(&self) -> Vec<Key> {
        self.bag.keys()
    }

    /// `has($key)`.
    pub fn has(&self, key: &str) -> bool {
        self.bag.has(key)
    }

    /// `remove($key)`.
    pub fn remove(&mut self, key: &str) {
        self.bag.remove(key);
    }

    /// `getIterator()`.
    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Value)> {
        self.bag.iter()
    }

    /// `count()`.
    pub fn count(&self) -> usize {
        self.bag.count()
    }
}
