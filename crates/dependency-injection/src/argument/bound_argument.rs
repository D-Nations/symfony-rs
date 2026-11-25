//! Port of Symfony\Component\DependencyInjection\Argument\BoundArgument.php.
//!
//! The usage sequence is process-wide, as upstream's static counter is
//! per PHP process.
//!
//! Upstream: symfony/dependency-injection Argument/BoundArgument.php

use std::sync::atomic::{AtomicI64, Ordering};

use php_value::{PhpArray, Value, array::Key};

use crate::object::{ObjectData, Shared};

static SEQUENCE: AtomicI64 = AtomicI64::new(0);

/// `BoundArgument::*_BINDING`.
pub const SERVICE_BINDING: i64 = 0;
pub const DEFAULTS_BINDING: i64 = 1;
pub const INSTANCEOF_BINDING: i64 = 2;

#[derive(Debug, Clone, Default)]
pub struct BoundArgumentData {
    value: Value,
    identifier: Option<i64>,
    used: Option<bool>,
    type_: i64,
    file: Option<String>,
}

impl ObjectData for BoundArgumentData {
    fn class(&self) -> &'static str {
        BoundArgument::CLASS
    }

    fn properties(&self) -> PhpArray {
        self.values()
    }
}

impl BoundArgumentData {
    fn values(&self) -> PhpArray {
        PhpArray::list([
            self.value.clone(),
            self.identifier.map_or(Value::Null, Value::Int),
            self.used.map_or(Value::Null, Value::Bool),
            Value::Int(self.type_),
            self.file.as_deref().map_or(Value::Null, Value::from),
        ])
    }
}

/// `BoundArgument`: a value bound to an argument name or type.
#[derive(Debug, Clone)]
pub struct BoundArgument(Shared<BoundArgumentData>);

impl BoundArgument {
    pub const CLASS: &'static str =
        "Symfony\\Component\\DependencyInjection\\Argument\\BoundArgument";

    /// `new BoundArgument($value, $trackUsage, $type, $file)`.
    pub fn new(value: Value, track_usage: bool, type_: i64, file: Option<String>) -> Self {
        let (identifier, used) = if track_usage {
            (Some(SEQUENCE.fetch_add(1, Ordering::Relaxed) + 1), None)
        } else {
            (None, Some(true))
        };
        Self(Shared::new(BoundArgumentData {
            value,
            identifier,
            used,
            type_,
            file,
        }))
    }

    /// `getValues()`: `[$value, $identifier, $used, $type, $file]`.
    pub fn values(&self) -> PhpArray {
        self.0.read(BoundArgumentData::values)
    }

    /// `setValues($values)`: all five fields, or the first three when the
    /// array has another count. A missing entry reads as null.
    pub fn set_values(&self, values: &PhpArray) {
        // List destructuring reads keys 0 to 4.
        let at = |i: i64| values.get(&Key::Int(i)).cloned().unwrap_or(Value::Null);
        let all = values.len();
        self.0.write(|b| {
            b.value = at(0);
            b.identifier = match at(1) {
                Value::Int(i) => Some(i),
                _ => None,
            };
            b.used = match at(2) {
                Value::Null => None,
                v => Some(v.is_truthy()),
            };
            if all == 5 {
                b.type_ = match at(3) {
                    Value::Int(i) => i,
                    _ => 0,
                };
                b.file = at(4).as_string().map(|s| s.to_string_lossy().into_owned());
            }
        });
    }

    /// The bound value (`getValues()[0]`).
    pub fn value(&self) -> Value {
        self.0.read(|b| b.value.clone())
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }

    pub fn to_value(&self) -> Value {
        self.0.to_value()
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        Shared::from_value(value).map(Self)
    }
}

impl From<BoundArgument> for Value {
    fn from(argument: BoundArgument) -> Self {
        argument.to_value()
    }
}
