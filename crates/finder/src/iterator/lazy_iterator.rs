//! Port of Symfony\Component\Finder\Iterator\LazyIterator.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//! Author: Jérémy Derussé.
//!
//! Upstream: symfony/finder Iterator/LazyIterator.php

/// Calls its factory each time it is iterated (`getIterator()`), not
/// before.
pub struct LazyIterator<F> {
    iterator_factory: F,
}

impl<F, I> LazyIterator<F>
where
    F: Fn() -> I,
    I: Iterator,
{
    pub fn new(iterator_factory: F) -> Self {
        Self { iterator_factory }
    }

    /// `getIterator()`.
    pub fn get_iterator(&self) -> I {
        (self.iterator_factory)()
    }
}
