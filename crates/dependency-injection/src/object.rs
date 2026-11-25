//! PHP object identity for the component's mutable classes.
//!
//! A `Definition`, an `Alias` or an argument object is shared by
//! reference in PHP: a compiler pass that changes the definition it got
//! from `getDefinition()` changes the one in the container, and an inline
//! definition inside an argument array is the same object wherever it
//! appears. [`Shared`] gives that: a cloned handle is the same object
//! (`===`), and `clone` is [`Shared::clone_object`]. A handle is also a
//! php-value object ([`Shared::to_value`], [`Shared::from_value`]), so
//! argument arrays are ordinary [`PhpArray`]s.
//!
//! The data sits behind a mutex. No method holds the lock while it calls
//! out of this crate, so a callback may use the same object again.
//!
//! Upstream: none

use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use php_value::{Object, PhpArray, Value, value::PhpObject};

/// The state of a shared object class.
pub trait ObjectData: Send + 'static {
    /// The PHP class name.
    fn class(&self) -> &'static str;

    /// `(array) $object` with the private-name prefixes removed, for
    /// debugging and dumps.
    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

pub(crate) struct Cell<T> {
    data: Mutex<T>,
}

impl<T> Cell<T> {
    fn lock(&self) -> MutexGuard<'_, T> {
        self.data.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl<T: ObjectData> fmt::Debug for Cell<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No lock and no recursion: an object can contain itself.
        write!(f, "{}#{:p}", self.lock().class(), std::ptr::from_ref(self))
    }
}

impl<T: ObjectData> PhpObject for Cell<T> {
    fn class(&self) -> &str {
        self.lock().class()
    }

    fn properties(&self) -> PhpArray {
        self.lock().properties()
    }
}

/// A handle to a shared PHP object of class data `T`.
pub struct Shared<T>(Arc<Cell<T>>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T: ObjectData> fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: ObjectData> Shared<T> {
    pub fn new(data: T) -> Self {
        Self(Arc::new(Cell {
            data: Mutex::new(data),
        }))
    }

    /// Reads the data.
    pub fn read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.0.lock())
    }

    /// Changes the data.
    pub fn write<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.0.lock())
    }

    /// `$a === $b`.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The object as a PHP value.
    pub fn to_value(&self) -> Value {
        Value::Object(self.0.clone())
    }

    /// The handle, when `object` is of this class data.
    pub fn from_object(object: &Object) -> Option<Self> {
        let any: Arc<dyn Any + Send + Sync> = object.clone();
        any.downcast::<Cell<T>>().ok().map(Self)
    }

    /// The handle, when `value` is an object of this class data.
    pub fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Object(object) => Self::from_object(object),
            _ => None,
        }
    }
}

impl<T: ObjectData + Clone> Shared<T> {
    /// `clone $object`: a new object with a shallow copy of the data.
    #[must_use]
    pub fn clone_object(&self) -> Self {
        Self::new(self.read(Clone::clone))
    }
}

/// The value as `T`, when it is an object of an immutable class `T`.
pub fn downcast<T: PhpObject>(value: &Value) -> Option<&T> {
    match value {
        Value::Object(object) => object.downcast_ref::<T>(),
        _ => None,
    }
}

/// An immutable object as a PHP value.
pub fn object_value<T: PhpObject>(object: T) -> Value {
    Value::Object(Arc::new(object))
}
