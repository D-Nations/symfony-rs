//! PHP values.
//!
//! Upstream: none

use std::{any::Any, fmt, sync::Arc};

use crate::{
    array::{Key, PhpArray},
    numeric::float_to_string,
    object::EnumCase,
    string::PhpString,
};

/// A PHP object. Objects are handles: cloning an `Object` gives the same
/// object, and `===` compares handles.
pub trait PhpObject: Any + Send + Sync + fmt::Debug {
    /// The class name, as `get_class()` gives it.
    fn class(&self) -> &str;

    /// The properties in declaration order, as `(array) $object` and
    /// `get_object_vars()` list them for public properties.
    fn properties(&self) -> PhpArray;

    /// `(string) $object` for a class that implements `\Stringable`
    /// (`__toString()`); `None` for any other class.
    fn to_php_string(&self) -> Option<PhpString> {
        None
    }
}

impl dyn PhpObject {
    pub fn downcast_ref<T: PhpObject>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }
}

pub type Object = Arc<dyn PhpObject>;

/// A PHP value.
#[derive(Clone, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(PhpString),
    Array(PhpArray),
    Object(Object),
}

impl Value {
    pub fn string(s: impl Into<PhpString>) -> Self {
        Self::String(s.into())
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn as_array(&self) -> Option<&PhpArray> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&PhpString> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Self::Object(o) => Some(o),
            _ => None,
        }
    }

    /// `get_debug_type()`.
    pub fn debug_type(&self) -> String {
        match self {
            Self::Null => "null".to_owned(),
            Self::Bool(_) => "bool".to_owned(),
            Self::Int(_) => "int".to_owned(),
            Self::Float(_) => "float".to_owned(),
            Self::String(_) => "string".to_owned(),
            Self::Array(_) => "array".to_owned(),
            Self::Object(o) => o.class().to_owned(),
        }
    }

    /// `===`. Floats compare as numbers (`NAN !== NAN`, `0.0 === -0.0`).
    pub fn identical(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => {
                std::ptr::addr_eq(Arc::as_ptr(a), Arc::as_ptr(b))
                    // Enum cases are singletons in PHP.
                    || matches!(
                        (a.downcast_ref::<EnumCase>(), b.downcast_ref::<EnumCase>()),
                        (Some(x), Some(y)) if x.class() == y.class() && x.name() == y.name()
                    )
            }
            _ => false,
        }
    }

    /// `(string) $value` for scalars and null; `None` for arrays and
    /// objects (PHP warns or fails there).
    pub fn to_php_string(&self) -> Option<PhpString> {
        Some(match self {
            Self::Null | Self::Bool(false) => PhpString::default(),
            Self::Bool(true) => PhpString::from("1"),
            Self::Int(i) => PhpString::from(i.to_string()),
            Self::Float(f) => PhpString::from(float_to_string(*f)),
            Self::String(s) => s.clone(),
            Self::Array(_) | Self::Object(_) => return None,
        })
    }

    /// `is_scalar($value) || $value instanceof \Stringable`.
    pub fn is_scalar_or_stringable(&self) -> bool {
        match self {
            Self::Bool(_) | Self::Int(_) | Self::Float(_) | Self::String(_) => true,
            Self::Object(o) => o.to_php_string().is_some(),
            Self::Null | Self::Array(_) => false,
        }
    }

    /// `(string) $value` for scalars, null and `\Stringable` objects;
    /// `None` for arrays and other objects.
    pub fn to_php_string_or_stringable(&self) -> Option<PhpString> {
        match self {
            Self::Object(o) => o.to_php_string(),
            other => other.to_php_string(),
        }
    }
}

/// `===`, so `assert_eq!` reads as PHPUnit's `assertSame`.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.identical(other)
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => f.write_str("null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(x) => write!(f, "{x:?}"),
            Self::String(s) => write!(f, "{s:?}"),
            Self::Array(a) => write!(f, "{a:?}"),
            Self::Object(o) => write!(f, "{o:?}"),
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<i64> for Value {
    fn from(i: i64) -> Self {
        Self::Int(i)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::String(s.into())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::String(s.into())
    }
}

impl From<PhpString> for Value {
    fn from(s: PhpString) -> Self {
        Self::String(s)
    }
}

impl From<PhpArray> for Value {
    fn from(a: PhpArray) -> Self {
        Self::Array(a)
    }
}

impl From<Key> for Value {
    fn from(k: Key) -> Self {
        k.to_value()
    }
}

/// `[...]` literal: `php_array![1 => "a", "k" => true]` or a list
/// `php_array!["a", "b"]`.
#[macro_export]
macro_rules! php_array {
    () => { $crate::PhpArray::new() };
    ($($k:expr => $v:expr),+ $(,)?) => {{
        let mut array = $crate::PhpArray::new();
        $( array.insert($crate::Key::from($k), $crate::Value::from($v)); )+
        array
    }};
    ($($v:expr),+ $(,)?) => {
        $crate::PhpArray::list([$($crate::Value::from($v)),+])
    };
}
