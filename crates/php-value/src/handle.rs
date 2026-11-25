//! A PHP object handle for a Rust value that PHP code passes around and
//! changes in place: a request on the request stack that a listener
//! modifies, a response an event holds while the kernel keeps it, an event
//! another object refers to.
//!
//! In PHP, `$a = $b` for an object copies the handle: both names reach the
//! same object, a change through one is seen through the other, and `===`
//! compares handles. [`Handle`] is that: cloning it gives the same object
//! ([`Handle::same`] is `===`), and [`Handle::with`] / [`Handle::with_mut`]
//! reach the value. `clone $object` is [`Handle::deep_clone`], a new
//! object with a copy of the value.
//!
//! The value sits behind a mutex, so handles can be held by services
//! shared between threads (a listener holding the request stack). PHP runs
//! one request on one thread; a handle is meant to be used the same way.
//! Do not call `with`/`with_mut` on a handle from inside a call on the same
//! handle: PHP would allow it, a mutex does not, and the call would block.
//!
//! Upstream: none

use std::{
    fmt,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
};

/// A PHP object handle to a `T`.
pub struct Handle<T>(Arc<Mutex<T>>);

impl<T> Handle<T> {
    /// A new object (`new ...`).
    pub fn new(value: T) -> Self {
        Self(Arc::new(Mutex::new(value)))
    }

    fn lock(&self) -> MutexGuard<'_, T> {
        // A panic while holding the value leaves it as the panic left it;
        // PHP has no poisoning, so the value is used as is.
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Reads the object.
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.lock())
    }

    /// Changes the object; every handle to it sees the change.
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(&mut self.lock())
    }

    /// `$a === $b`: the same object.
    pub fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Replaces the object's value, returning the old one.
    pub fn replace(&self, value: T) -> T {
        std::mem::replace(&mut self.lock(), value)
    }
}

impl<T: Clone> Handle<T> {
    /// `clone $object`: a new object with a copy of the value.
    pub fn deep_clone(&self) -> Self {
        Self::new(self.with(T::clone))
    }

    /// A copy of the object's current value.
    pub fn get(&self) -> T {
        self.with(T::clone)
    }
}

impl<T> Clone for Handle<T> {
    /// `$a = $b`: the same object.
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<T: fmt::Debug> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.try_lock() {
            Ok(value) => f.debug_tuple("Handle").field(&*value).finish(),
            Err(_) => f.write_str("Handle(<in use>)"),
        }
    }
}

impl<T: PartialEq> PartialEq for Handle<T> {
    /// PHP's `==` on objects of one class: the same object, or equal
    /// values (PHPUnit's `assertEquals()`).
    fn eq(&self, other: &Self) -> bool {
        self.same(other) || self.with(|a| other.with(|b| a == b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_is_the_same_object() {
        let a = Handle::new(vec![1]);
        let b = a.clone();
        b.with_mut(|v| v.push(2));
        assert_eq!(vec![1, 2], a.get());
        assert!(a.same(&b));
    }

    #[test]
    fn clone_is_a_new_object() {
        let a = Handle::new(vec![1]);
        let b = a.deep_clone();
        b.with_mut(|v| v.push(2));
        assert_eq!(vec![1], a.get());
        assert!(!a.same(&b));
        assert_ne!(a, b);
        assert_eq!(a, Handle::new(vec![1]));
    }
}
