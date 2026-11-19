//! PHP constants and enums, as `defined()`, `constant()` and
//! `enum_exists()` see them. PHP keeps these in global state; here a
//! `Registry` is passed to whatever resolves them (Yaml's `!php/const` and
//! `!php/enum`), and the application registers its own.
//!
//! Upstream: none

use std::collections::HashMap;

use crate::{
    object::EnumCase,
    value::{Object, Value},
};

/// Constants and enums visible to a resolver.
#[derive(Debug, Clone, Default)]
pub struct Registry {
    constants: HashMap<String, Value>,
    enums: HashMap<String, Vec<Object>>,
}

impl Registry {
    /// No constants at all.
    pub fn empty() -> Self {
        Self::default()
    }

    /// PHP's core constants that do not depend on the build: `PHP_INT_MAX`,
    /// `PHP_INT_MIN`, `PHP_INT_SIZE`, `PHP_FLOAT_*`, `PHP_EOL`, `M_PI`,
    /// `NAN`, `INF`, and `NULL` / `TRUE` / `FALSE`.
    pub fn with_core() -> Self {
        let mut registry = Self::default();
        for (name, value) in [
            ("PHP_INT_MAX", Value::Int(i64::MAX)),
            ("PHP_INT_MIN", Value::Int(i64::MIN)),
            ("PHP_INT_SIZE", Value::Int(8)),
            ("PHP_FLOAT_EPSILON", Value::Float(f64::EPSILON)),
            ("PHP_FLOAT_MAX", Value::Float(f64::MAX)),
            ("PHP_FLOAT_MIN", Value::Float(f64::MIN_POSITIVE)),
            ("PHP_FLOAT_DIG", Value::Int(15)),
            ("PHP_EOL", Value::from("\n")),
            ("M_PI", Value::Float(std::f64::consts::PI)),
            ("NAN", Value::Float(f64::NAN)),
            ("INF", Value::Float(f64::INFINITY)),
            ("NULL", Value::Null),
            ("TRUE", Value::Bool(true)),
            ("FALSE", Value::Bool(false)),
        ] {
            registry.define(name, value);
        }
        registry
    }

    /// `define()`, or a class constant as `Class::NAME`.
    pub fn define(&mut self, name: impl Into<String>, value: Value) {
        self.constants.insert(name.into(), value);
    }

    /// Registers an enum and its cases, in declaration order; each case is
    /// also defined as the constant `Class::CASE`.
    pub fn define_enum(&mut self, class: &str, cases: Vec<EnumCase>) {
        let objects: Vec<Object> = cases.into_iter().map(EnumCase::object).collect();
        for case in &objects {
            let name = case
                .downcast_ref::<EnumCase>()
                .map(|c| c.name().to_owned())
                .unwrap_or_default();
            self.define(format!("{class}::{name}"), Value::Object(case.clone()));
        }
        self.enums.insert(class.to_owned(), objects);
    }

    /// `constant($name)` when `defined($name)`. `null`, `true` and `false`
    /// are case-insensitive, as in PHP.
    pub fn constant(&self, name: &str) -> Option<&Value> {
        self.constants.get(name).or_else(|| {
            let upper = name.to_ascii_uppercase();
            matches!(upper.as_str(), "NULL" | "TRUE" | "FALSE")
                .then(|| self.constants.get(&upper))
                .flatten()
        })
    }

    /// `enum_exists($class)`.
    pub fn enum_exists(&self, class: &str) -> bool {
        self.enums.contains_key(class)
    }

    /// `$class::cases()`.
    pub fn cases(&self, class: &str) -> Option<&[Object]> {
        self.enums.get(class).map(Vec::as_slice)
    }
}
