//! The PHP built-ins `OptionsResolver` relies on, on `php_value::Value`:
//! `get_debug_type()`, the `is_*()` type checks behind its allowed types,
//! `instanceof` with a class name string, `(string)` conversion for its
//! messages, and `ksort()` of option names. Written from PHP's documented
//! and observed behaviour (no PHP source code, PLAN.md decision 8).
//!
//! `php_value` has no resources and no callable closures, so this module
//! adds two object kinds standing for them: [`Resource`] (a stream, as
//! `fopen()` returns) and [`ClosureValue`] (a `\Closure` passed around as
//! a plain value). The checks here treat a `Resource` as a resource, not
//! an object, as PHP does.
//!
//! Upstream: none

use std::{cmp::Ordering, sync::Arc};

use php_value::{
    Key, PhpArray, PhpObject, PhpString, Value,
    numeric::{float_to_string, is_numeric, string_to_float},
    object::EnumCase,
};

/// A PHP resource (`fopen()`'s stream). An `Object` only because
/// `php_value::Value` has no resource variant; [`is_object`] is false for
/// it and [`debug_type`] gives `resource (<type>)`.
#[derive(Debug)]
pub struct Resource {
    kind: String,
    closed: bool,
}

impl Resource {
    /// An open resource of `kind` (e.g. `stream`).
    pub fn new(kind: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            closed: false,
        }
    }

    /// A closed resource (`get_debug_type()` gives `resource (closed)`).
    pub fn closed() -> Self {
        Self {
            kind: String::new(),
            closed: true,
        }
    }

    /// An open stream resource as a value.
    pub fn stream() -> Value {
        Value::Object(Arc::new(Self::new("stream")))
    }
}

impl PhpObject for Resource {
    fn class(&self) -> &'static str {
        "resource"
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// A `\Closure` held as a plain value (e.g. a default that is not lazy).
/// It cannot be called; it keeps its identity (`===`) and its class.
#[derive(Debug, Default)]
pub struct ClosureValue {
    // A closure is never zero-sized in PHP; this keeps each `Arc`
    // allocation distinct, so identity is by handle.
    _id: u8,
}

impl ClosureValue {
    pub const CLASS: &'static str = "Closure";

    /// A new closure value, distinct (`!==`) from every other.
    pub fn value() -> Value {
        Value::Object(Arc::new(Self::default()))
    }
}

impl PhpObject for ClosureValue {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

fn resource(value: &Value) -> Option<&Resource> {
    value.as_object().and_then(|o| o.downcast_ref::<Resource>())
}

/// `is_resource()`: true for an open resource.
pub fn is_resource(value: &Value) -> bool {
    resource(value).is_some_and(|r| !r.closed)
}

/// `is_object()`.
pub fn is_object(value: &Value) -> bool {
    matches!(value, Value::Object(_)) && resource(value).is_none()
}

/// `get_debug_type()`.
pub fn debug_type(value: &Value) -> String {
    match resource(value) {
        Some(r) if r.closed => "resource (closed)".to_owned(),
        Some(r) => format!("resource ({})", r.kind),
        None => value.debug_type(),
    }
}

/// The classes and interfaces each known built-in class is an instance
/// of, lower-cased, itself excluded. Other classes are instances of their
/// own name only: their parents are not known here.
fn ancestors(class: &str) -> &'static [&'static str] {
    match class.to_ascii_lowercase().as_str() {
        "datetime" | "datetimeimmutable" => &["datetimeinterface"],
        "arrayobject" => &[
            "iteratoraggregate",
            "traversable",
            "arrayaccess",
            "serializable",
            "countable",
        ],
        "symfony\\component\\optionsresolver\\optionsresolver" => &[
            "symfony\\component\\optionsresolver\\options",
            "arrayaccess",
            "countable",
        ],
        _ => &[],
    }
}

/// `$value instanceof $class` with a class name string. A leading `\`
/// is ignored, and names compare case-insensitively, as PHP's do.
pub fn instance_of(value: &Value, class: &str) -> bool {
    if !is_object(value) {
        return false;
    }
    let Value::Object(object) = value else {
        return false;
    };
    let wanted = class
        .strip_prefix('\\')
        .unwrap_or(class)
        .to_ascii_lowercase();
    let own = object.class();
    if own.eq_ignore_ascii_case(&wanted) || ancestors(own).contains(&wanted.as_str()) {
        return true;
    }
    if let Some(case) = object.downcast_ref::<EnumCase>() {
        return wanted == "unitenum" || (wanted == "backedenum" && case.value().is_some());
    }
    false
}

/// `is_numeric()`.
pub fn is_numeric_value(value: &Value) -> bool {
    match value {
        Value::Int(_) | Value::Float(_) => true,
        Value::String(s) => is_numeric(s),
        _ => false,
    }
}

/// `is_scalar()`.
pub fn is_scalar(value: &Value) -> bool {
    matches!(
        value,
        Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_)
    )
}

/// `is_iterable()`: arrays and `Traversable` objects.
pub fn is_iterable(value: &Value) -> bool {
    matches!(value, Value::Array(_)) || instance_of(value, "Traversable")
}

/// `is_countable()`: arrays and `Countable` objects.
pub fn is_countable(value: &Value) -> bool {
    matches!(value, Value::Array(_)) || instance_of(value, "Countable")
}

/// `is_callable()`, for closures only: function names (strings) and
/// `[object, method]` arrays are not known here and count as not
/// callable (crate README, "Differences").
pub fn is_callable(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.downcast_ref::<ClosureValue>().is_some())
}

/// `(string) $key`.
pub fn key_bytes(key: &Key) -> Vec<u8> {
    key.bytes().into_owned()
}

/// `(string) $value` for an int or float (the only kinds
/// `formatValue()` converts this way).
pub fn scalar_bytes(value: &Value) -> Vec<u8> {
    match value {
        Value::Float(f) => float_to_string(*f).into_bytes(),
        other => other
            .to_php_string()
            .map(|s| s.to_vec())
            .unwrap_or_default(),
    }
}

/// `implode($glue, $pieces)` of byte strings.
pub fn implode(glue: &[u8], pieces: &[Vec<u8>]) -> Vec<u8> {
    pieces.join(glue)
}

/// `strtr($subject, [$from => $to])`: every non-overlapping occurrence,
/// left to right.
pub fn replace(subject: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    if from.is_empty() {
        return subject.to_vec();
    }
    let mut out = Vec::with_capacity(subject.len());
    let mut i = 0;
    while i < subject.len() {
        if subject[i..].starts_with(from) {
            out.extend_from_slice(to);
            i += from.len();
        } else {
            out.push(subject[i]);
            i += 1;
        }
    }
    out
}

/// PHP 8's comparison of two array keys as `ksort()` (`SORT_REGULAR`)
/// orders them: ints numerically; numeric strings with ints or each
/// other as numbers; otherwise as byte strings.
#[expect(
    clippy::cast_precision_loss,
    reason = "PHP compares an int with a numeric string as floats"
)]
pub fn compare_keys(left: &Key, right: &Key) -> Ordering {
    fn numeric(s: &PhpString) -> Option<f64> {
        is_numeric(s).then(|| string_to_float(s))
    }
    let by_number = |x: f64, y: f64| x.partial_cmp(&y).unwrap_or(Ordering::Equal);
    match (left, right) {
        (Key::Int(x), Key::Int(y)) => x.cmp(y),
        (Key::Int(x), Key::Str(text)) => match numeric(text) {
            Some(y) => by_number(*x as f64, y),
            None => x.to_string().as_bytes().cmp(text.as_ref()),
        },
        (Key::Str(text), Key::Int(y)) => match numeric(text) {
            Some(x) => by_number(x, *y as f64),
            None => text.as_ref().cmp(y.to_string().as_bytes()),
        },
        (Key::Str(x), Key::Str(y)) => match (numeric(x), numeric(y)) {
            (Some(first), Some(second)) => by_number(first, second),
            _ => x.as_ref().cmp(y.as_ref()),
        },
    }
}
