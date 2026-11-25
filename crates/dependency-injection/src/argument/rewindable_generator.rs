//! Port of Symfony\Component\DependencyInjection\Argument\RewindableGenerator.php.
//!
//! A generator that starts again on each iteration: the closure gives a
//! new iterator of `key => value` pairs each time (a creation error is an
//! `Err` item), and the count is a number or a callable called once, on
//! the first `count()`.
//!
//! Upstream: symfony/dependency-injection Argument/RewindableGenerator.php

use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use php_value::{PhpArray, Value};

use crate::exception::Result;

/// `Symfony\Component\DependencyInjection\Argument\RewindableGenerator`.
pub const REWINDABLE_GENERATOR_CLASS: &str =
    "Symfony\\Component\\DependencyInjection\\Argument\\RewindableGenerator";

/// One run of the generator: `key => value` pairs.
pub type Generator = Box<dyn Iterator<Item = Result<(Value, Value)>>>;

type GeneratorFn = Arc<dyn Fn() -> Generator + Send + Sync>;
type CountFn = Arc<dyn Fn() -> Result<i64> + Send + Sync>;

enum Count {
    Known(i64),
    Lazy(CountFn),
}

/// `RewindableGenerator` (`\IteratorAggregate`, `\Countable`).
pub struct RewindableGenerator {
    generator: GeneratorFn,
    count: Mutex<Count>,
}

impl fmt::Debug for RewindableGenerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RewindableGenerator#{:p}", std::ptr::from_ref(self))
    }
}

impl php_value::value::PhpObject for RewindableGenerator {
    fn class(&self) -> &str {
        REWINDABLE_GENERATOR_CLASS
    }

    fn properties(&self) -> PhpArray {
        PhpArray::new()
    }
}

impl RewindableGenerator {
    /// `new RewindableGenerator($generator, $count)` with a known count.
    pub fn new(generator: impl Fn() -> Generator + Send + Sync + 'static, count: i64) -> Self {
        Self {
            generator: Arc::new(generator),
            count: Mutex::new(Count::Known(count)),
        }
    }

    /// `new RewindableGenerator($generator, $count)` with a count
    /// callable.
    pub fn with_count_callback(
        generator: impl Fn() -> Generator + Send + Sync + 'static,
        count: impl Fn() -> Result<i64> + Send + Sync + 'static,
    ) -> Self {
        Self {
            generator: Arc::new(generator),
            count: Mutex::new(Count::Lazy(Arc::new(count))),
        }
    }

    /// A generator over fixed pairs, as an `\ArrayIterator` is.
    pub fn from_pairs(pairs: Vec<(Value, Value)>) -> Self {
        let count = i64::try_from(pairs.len()).unwrap_or(i64::MAX);
        Self::new(
            move || -> Generator { Box::new(pairs.clone().into_iter().map(Ok)) },
            count,
        )
    }

    /// The generator as an object value.
    pub fn into_value(self) -> Value {
        Value::Object(Arc::new(self))
    }

    /// `$generator instanceof $class`.
    pub fn instance_of(class: &str) -> bool {
        matches!(
            class.trim_start_matches('\\'),
            REWINDABLE_GENERATOR_CLASS | "IteratorAggregate" | "Traversable" | "Countable"
        )
    }

    /// `getIterator()`: a new run.
    pub fn get_iterator(&self) -> Generator {
        (self.generator)()
    }

    /// `count()`.
    pub fn count(&self) -> Result<i64> {
        let callback = match &*self.count.lock().unwrap_or_else(PoisonError::into_inner) {
            Count::Known(count) => return Ok(*count),
            Count::Lazy(callback) => callback.clone(),
        };
        let count = callback()?;
        *self.count.lock().unwrap_or_else(PoisonError::into_inner) = Count::Known(count);
        Ok(count)
    }
}
