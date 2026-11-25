//! Built-in PHP object kinds: `stdClass`, enum cases, `\Closure` and
//! `\Stringable` objects, and the `(array)` / `(object)` casts between
//! arrays and `stdClass`.
//!
//! Upstream: none

use std::sync::Arc;

use crate::{
    array::{Key, PhpArray},
    string::PhpString,
    value::{Object, PhpObject, Thrown, Value},
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

/// `ArrayObject`: an object wrapping an array (its storage).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArrayObject {
    storage: PhpArray,
}

impl ArrayObject {
    pub const CLASS: &'static str = "ArrayObject";

    pub fn new(storage: PhpArray) -> Self {
        Self { storage }
    }

    /// `getArrayCopy()`, which is also what iterating it gives.
    pub fn storage(&self) -> &PhpArray {
        &self.storage
    }
}

impl PhpObject for ArrayObject {
    fn class(&self) -> &str {
        Self::CLASS
    }

    /// `(array)` of an `ArrayObject` gives its storage.
    fn properties(&self) -> PhpArray {
        self.storage.clone()
    }

    fn instance_of(&self, class: &str) -> bool {
        array_like_instance_of(class, "IteratorAggregate")
    }

    fn methods(&self) -> Vec<String> {
        [
            "__construct",
            "offsetExists",
            "offsetGet",
            "offsetSet",
            "offsetUnset",
            "append",
            "getArrayCopy",
            "count",
            "getFlags",
            "setFlags",
            "asort",
            "ksort",
            "uasort",
            "uksort",
            "natsort",
            "natcasesort",
            "unserialize",
            "serialize",
            "__serialize",
            "__unserialize",
            "__debugInfo",
            "getIterator",
            "exchangeArray",
            "setIteratorClass",
            "getIteratorClass",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    fn call_method(&self, name: &str, arguments: &[Value]) -> Option<Result<Value, Thrown>> {
        array_like_call(&self.storage, name, arguments)
    }

    /// Its storage entries are not properties.
    fn property(&self, _name: &[u8]) -> Option<Value> {
        None
    }

    fn offset_get(&self, offset: &Value) -> Option<Result<Value, Thrown>> {
        Some(Ok(array_offset(&self.storage, offset)))
    }

    fn offset_exists(&self, offset: &Value) -> Option<Result<bool, Thrown>> {
        Some(Ok(array_offset_exists(&self.storage, offset)))
    }

    fn php_count(&self) -> Option<Result<i64, Thrown>> {
        Some(Ok(i64::try_from(self.storage.len()).unwrap_or(i64::MAX)))
    }

    fn traverse(&self) -> Option<Result<Vec<(Value, Value)>, Thrown>> {
        Some(Ok(array_pairs(&self.storage)))
    }
}

/// `ArrayIterator`: an iterator over an array (its storage).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArrayIterator {
    storage: PhpArray,
}

impl ArrayIterator {
    pub const CLASS: &'static str = "ArrayIterator";

    pub fn new(storage: PhpArray) -> Self {
        Self { storage }
    }

    /// `getArrayCopy()`.
    pub fn storage(&self) -> &PhpArray {
        &self.storage
    }
}

impl PhpObject for ArrayIterator {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        self.storage.clone()
    }

    fn instance_of(&self, class: &str) -> bool {
        array_like_instance_of(class, "SeekableIterator") || matches!(class, "Iterator")
    }

    fn methods(&self) -> Vec<String> {
        [
            "__construct",
            "offsetExists",
            "offsetGet",
            "offsetSet",
            "offsetUnset",
            "append",
            "getArrayCopy",
            "count",
            "getFlags",
            "setFlags",
            "asort",
            "ksort",
            "uasort",
            "uksort",
            "natsort",
            "natcasesort",
            "unserialize",
            "serialize",
            "__serialize",
            "__unserialize",
            "__debugInfo",
            "rewind",
            "current",
            "key",
            "next",
            "valid",
            "seek",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    }

    fn call_method(&self, name: &str, arguments: &[Value]) -> Option<Result<Value, Thrown>> {
        array_like_call(&self.storage, name, arguments)
    }

    fn property(&self, _name: &[u8]) -> Option<Value> {
        None
    }

    fn offset_get(&self, offset: &Value) -> Option<Result<Value, Thrown>> {
        Some(Ok(array_offset(&self.storage, offset)))
    }

    fn offset_exists(&self, offset: &Value) -> Option<Result<bool, Thrown>> {
        Some(Ok(array_offset_exists(&self.storage, offset)))
    }

    fn php_count(&self) -> Option<Result<i64, Thrown>> {
        Some(Ok(i64::try_from(self.storage.len()).unwrap_or(i64::MAX)))
    }

    fn traverse(&self) -> Option<Result<Vec<(Value, Value)>, Thrown>> {
        Some(Ok(array_pairs(&self.storage)))
    }
}

fn array_like_instance_of(class: &str, iterator: &str) -> bool {
    matches!(
        class.trim_start_matches('\\'),
        "ArrayAccess" | "Countable" | "Traversable" | "Serializable"
    ) || class.trim_start_matches('\\') == iterator
}

fn array_like_call(
    storage: &PhpArray,
    name: &str,
    arguments: &[Value],
) -> Option<Result<Value, Thrown>> {
    let first = arguments.first().cloned().unwrap_or_default();
    Some(Ok(match name.to_ascii_lowercase().as_str() {
        "getarraycopy" => Value::Array(storage.clone()),
        "count" => Value::Int(i64::try_from(storage.len()).unwrap_or(i64::MAX)),
        "offsetget" => array_offset(storage, &first),
        "offsetexists" => Value::Bool(array_offset_exists(storage, &first)),
        _ => return None,
    }))
}

fn array_offset(storage: &PhpArray, offset: &Value) -> Value {
    Key::from_value(offset)
        .and_then(|k| storage.get(&k).cloned())
        .unwrap_or_default()
}

fn array_offset_exists(storage: &PhpArray, offset: &Value) -> bool {
    // isset(): a null entry counts as missing
    Key::from_value(offset)
        .and_then(|k| storage.get(&k))
        .is_some_and(|v| !v.is_null())
}

fn array_pairs(storage: &PhpArray) -> Vec<(Value, Value)> {
    storage
        .iter()
        .map(|(k, v)| (k.to_value(), v.clone()))
        .collect()
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

/// An object of a class implementing `\Stringable`: its class name and
/// what `__toString()` returns. It has no public properties.
#[derive(Debug, Clone, PartialEq)]
pub struct StringableObject {
    class: String,
    text: PhpString,
}

impl StringableObject {
    pub fn new(class: impl Into<String>, text: impl Into<PhpString>) -> Self {
        Self {
            class: class.into(),
            text: text.into(),
        }
    }

    /// The object as a value.
    pub fn object(class: impl Into<String>, text: impl Into<PhpString>) -> Value {
        Value::Object(Arc::new(Self::new(class, text)))
    }
}

impl PhpObject for StringableObject {
    fn class(&self) -> &str {
        &self.class
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }

    fn to_php_string(&self) -> Option<PhpString> {
        Some(self.text.clone())
    }
}

/// The function a [`Closure`] runs: its arguments, its return value.
pub type ClosureFn = dyn Fn(&[Value]) -> Value + Send + Sync;

/// A `\Closure`: a Rust function taking PHP values. Cloning the value
/// gives the same closure, as PHP's objects are handles.
#[derive(Clone)]
pub struct Closure(Arc<ClosureFn>);

impl Closure {
    pub const CLASS: &'static str = "Closure";

    pub fn new(f: impl Fn(&[Value]) -> Value + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// The closure as a value.
    pub fn object(f: impl Fn(&[Value]) -> Value + Send + Sync + 'static) -> Value {
        Value::Object(Arc::new(Self::new(f)))
    }

    /// `$closure(...$arguments)`.
    pub fn call(&self, arguments: &[Value]) -> Value {
        (self.0)(arguments)
    }
}

impl std::fmt::Debug for Closure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Closure")
    }
}

impl PhpObject for Closure {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

/// Why `BackedEnum::from()` threw.
#[derive(Debug, Clone, PartialEq)]
pub enum EnumFromError {
    /// A `\TypeError`: the argument does not coerce to the backing type.
    TypeError(String),
    /// A `\ValueError`: no case has that backing value. The message holds
    /// the value's bytes.
    ValueError(PhpString),
}

impl EnumFromError {
    pub fn class(&self) -> &'static str {
        match self {
            Self::TypeError(_) => "TypeError",
            Self::ValueError(_) => "ValueError",
        }
    }

    pub fn message(&self) -> PhpString {
        match self {
            Self::TypeError(message) => PhpString::from(message.as_str()),
            Self::ValueError(message) => message.clone(),
        }
    }
}

/// `$class::from($value)` for a backed enum whose `cases` are given, called
/// from code without `strict_types` (PHP's coercive mode), as recorded
/// (crates/http-foundation/tests/data/bags.json). An int-backed enum takes
/// an int, a bool, a float that fits in an int, or a numeric string with
/// the same rules; a fraction is dropped (PHP's `E_DEPRECATED` "Implicit
/// conversion ... loses precision" is not reproduced). A string-backed
/// enum takes `string|int`: an int, a bool or a float that fits in an int
/// (truncated) as the int's decimal digits, any other float as PHP prints
/// it (`"1.0E+20"`, `"NAN"`), a `\Stringable` as its string. `None` when
/// the cases are not a backed enum's.
#[expect(
    clippy::cast_possible_truncation,
    reason = "PHP's int and float conversions and comparisons"
)]
pub fn backed_enum_from(
    class: &str,
    cases: &[Object],
    value: &Value,
) -> Option<Result<Object, EnumFromError>> {
    use crate::numeric::{Number, float_to_string, numeric_value};

    let backing = |c: &Object| {
        c.downcast_ref::<EnumCase>()
            .and_then(EnumCase::value)
            .cloned()
    };
    let int_backed = match cases.iter().find_map(backing)? {
        Value::Int(_) => true,
        Value::String(_) => false,
        _ => return None,
    };
    let type_error = |given: &Value| {
        let wanted = if int_backed { "int" } else { "string|int" };
        Err(EnumFromError::TypeError(format!(
            "{class}::from(): Argument #1 ($value) must be of type {wanted}, {} given",
            given.debug_type()
        )))
    };
    // A float that fits in an int (in [-2^63, 2^63)), truncated.
    let float_int = |f: f64| {
        (-9.223_372_036_854_776e18..9.223_372_036_854_776e18)
            .contains(&f)
            .then_some(f as i64)
    };
    let wanted = if int_backed {
        let int = match value {
            Value::Int(i) => Some(*i),
            Value::Bool(b) => Some(i64::from(*b)),
            Value::Float(f) => float_int(*f),
            Value::String(s) => match numeric_value(s.as_bytes()) {
                Some(Number::Int(i)) => Some(i),
                Some(Number::Float(f)) => float_int(f),
                None => None,
            },
            _ => None,
        };
        match int {
            Some(i) => Value::Int(i),
            None => return Some(type_error(value)),
        }
    } else {
        let text = match value {
            Value::String(s) => s.clone(),
            Value::Int(i) => PhpString::from(i.to_string()),
            Value::Bool(b) => PhpString::from(if *b { "1" } else { "0" }),
            Value::Float(f) => match float_int(*f) {
                Some(i) => PhpString::from(i.to_string()),
                None => PhpString::from(float_to_string(*f)),
            },
            Value::Object(o) => match o.to_php_string() {
                Some(text) => text,
                None => return Some(type_error(value)),
            },
            _ => return Some(type_error(value)),
        };
        Value::String(text)
    };
    let found = cases
        .iter()
        .find(|c| backing(c).is_some_and(|v| v.identical(&wanted)));
    Some(if let Some(case) = found {
        Ok(case.clone())
    } else {
        let mut message = match &wanted {
            Value::Int(i) => i.to_string().into_bytes(),
            Value::String(s) => [b"\"".as_slice(), s.as_bytes(), b"\""].concat(),
            _ => Vec::new(),
        };
        message.extend_from_slice(
            format!(" is not a valid backing value for enum {class}").as_bytes(),
        );
        Err(EnumFromError::ValueError(PhpString::from(message)))
    })
}
