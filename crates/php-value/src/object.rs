//! Built-in PHP object kinds: `stdClass` and enum cases, and the
//! `(array)` / `(object)` casts between arrays and `stdClass`.
//!
//! Upstream: none

use std::sync::Arc;

use crate::{
    array::PhpArray,
    value::{Object, PhpObject, Value},
};

/// `stdClass`: dynamic public properties. Property names are kept as
/// array keys, so `(object) ['0' => 1]` and back gives the int key `0`,
/// as PHP 7.2+ does.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StdClass {
    properties: PhpArray,
}

impl StdClass {
    pub const CLASS: &'static str = "stdClass";

    /// `(object) $array`.
    pub fn from_array(properties: PhpArray) -> Self {
        Self { properties }
    }

    /// `(object) $array` as a value.
    pub fn object(properties: PhpArray) -> Value {
        Value::Object(Arc::new(Self::from_array(properties)))
    }
}

impl PhpObject for StdClass {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        self.properties.clone()
    }
}

/// An object of any other class: its class name and properties, as
/// `unserialize()` and recorded vectors produce them.
#[derive(Debug, Clone, PartialEq)]
pub struct GenericObject {
    class: String,
    properties: PhpArray,
}

impl GenericObject {
    pub fn new(class: impl Into<String>, properties: PhpArray) -> Self {
        Self {
            class: class.into(),
            properties,
        }
    }
}

impl PhpObject for GenericObject {
    fn class(&self) -> &str {
        &self.class
    }

    fn properties(&self) -> PhpArray {
        self.properties.clone()
    }
}

/// A case of a PHP enum. Cases are singletons in PHP, so `===` compares
/// the class and the case name.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumCase {
    class: String,
    name: String,
    /// The backing value of a backed enum (`->value`).
    value: Option<Value>,
}

impl EnumCase {
    pub fn new(class: impl Into<String>, name: impl Into<String>, value: Option<Value>) -> Self {
        Self {
            class: class.into(),
            name: name.into(),
            value,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    pub fn object(self) -> Object {
        Arc::new(self)
    }
}

impl PhpObject for EnumCase {
    fn class(&self) -> &str {
        &self.class
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("name", Value::from(self.name.as_str()));
        if let Some(value) = &self.value {
            properties.insert("value", value.clone());
        }
        properties
    }
}

/// `(array) $value` for objects: their properties; other values as PHP
/// casts them (`null` → `[]`, scalars → `[0 => $value]`).
pub fn to_array(value: &Value) -> PhpArray {
    match value {
        Value::Array(array) => array.clone(),
        Value::Object(object) => object.properties(),
        Value::Null => PhpArray::new(),
        scalar => PhpArray::list([scalar.clone()]),
    }
}
