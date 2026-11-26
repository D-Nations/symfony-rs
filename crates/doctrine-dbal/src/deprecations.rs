//! The part of doctrine/deprecations' `Deprecation` class that DBAL's
//! tests observe (`VerifyDeprecations`): `Deprecation::trigger()` counts
//! each triggered deprecation by its link while tracking is enabled, and
//! otherwise does nothing (the library's default: no logging, no
//! exception).
//!
//! doctrine/deprecations is not a pinned package of this repository, so
//! this is written from its documented behaviour. Differences: the
//! counts are per thread (PHP has one process-wide state; Rust tests run
//! in parallel threads), messages are not kept, and
//! `Deprecation::triggerIfCalledFromOutside()` is never raised (every
//! call here comes from inside the package).
//!
//! Upstream: none

use std::cell::RefCell;
use std::collections::BTreeMap;

thread_local! {
    static TRACKED: RefCell<Option<BTreeMap<String, u64>>> = const { RefCell::new(None) };
}

/// `Deprecation::enableTrackingDeprecations()`.
pub fn enable_tracking_deprecations() {
    TRACKED.with(|tracked| {
        tracked.borrow_mut().get_or_insert_with(BTreeMap::new);
    });
}

/// `Deprecation::disable()`: stops tracking and forgets the counts.
pub fn disable() {
    TRACKED.with(|tracked| *tracked.borrow_mut() = None);
}

/// `Deprecation::getTriggeredDeprecations()`: the count of each link.
pub fn get_triggered_deprecations() -> BTreeMap<String, u64> {
    TRACKED.with(|tracked| tracked.borrow().clone().unwrap_or_default())
}

/// How many times the deprecation `link` was triggered on this thread
/// while tracking.
pub fn count(link: &str) -> u64 {
    TRACKED.with(|tracked| {
        tracked
            .borrow()
            .as_ref()
            .and_then(|counts| counts.get(link).copied())
            .unwrap_or(0)
    })
}

/// `Deprecation::trigger('doctrine/dbal', $link, $message, ...)`. The
/// message is built only while tracking (and then not kept).
pub(crate) fn trigger(link: &str, message: impl FnOnce() -> String) {
    TRACKED.with(|tracked| {
        if let Some(counts) = tracked.borrow_mut().as_mut() {
            let _ = message;
            *counts.entry(link.to_owned()).or_insert(0) += 1;
        }
    });
}

/// `Deprecation::triggerIfCalledFromOutside('doctrine/dbal', ...)`: called
/// by a public method that the package's own code reaches through an
/// internal path, so that only an application's (or a test's) call
/// triggers it, as in PHP.
pub(crate) fn trigger_if_called_from_outside(link: &str, message: impl FnOnce() -> String) {
    trigger(link, message);
}
