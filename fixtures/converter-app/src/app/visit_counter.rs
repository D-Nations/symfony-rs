//! `Fixture\Service\VisitCounter` (oracle/apps/converter-fixture/src/Service/VisitCounter.php).
//!
//! A class with a property written after construction: a shared mutable
//! handle, `Arc<Mutex<_>>` in a newtype (converter contract 0.1, section 3).
//!
//! Upstream: none

use std::sync::{Arc, Mutex, PoisonError};

#[derive(Debug, Default)]
struct VisitCounterData {
    count: i64,
}

#[derive(Debug, Clone, Default)]
pub struct VisitCounter(Arc<Mutex<VisitCounterData>>);

impl VisitCounter {
    pub fn new() -> Self {
        Self::default()
    }

    /// `return ++$this->count;`
    pub fn increment(&self) -> i64 {
        // A poisoned lock only means another holder panicked; the counter
        // itself is still a valid integer.
        let mut this = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        // PHP turns an overflowing int into a float; 2^63 visits in one
        // request cannot happen, so saturating is not observable.
        this.count = this.count.saturating_add(1);
        this.count
    }
}
