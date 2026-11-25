//! Port of Symfony\Component\Clock\Test\ClockSensitiveTrait.php: helps
//! with mocking the time in your test cases.
//!
//! This trait provides one `mock_time()` method that freezes the time.
//! It restores the global clock after each test case. `mock_time()`
//! accepts either a string (eg '+1 days' or '2022-12-22'), a
//! `DateTimeImmutable`, or a boolean (to freeze/restore the global clock).
//!
//! PHPUnit calls `saveClockBeforeTest()` before the class and before each
//! test, and `restoreClockAfterTest()` after each test. Rust's test runner
//! has no such hooks and runs tests in parallel threads, so a test takes
//! [`ClockSensitiveTrait::before_test`]: it waits for the other tests of
//! the process that hold one, saves the clock, and restores it when
//! dropped.
//!
//! Upstream: symfony/clock Test/ClockSensitiveTrait.php

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::{
    clock::Clock,
    clock_interface::SharedClock,
    mock_clock::MockClock,
    now::now,
    php::{DateError, DateTimeImmutable},
};

/// The `string|\DateTimeImmutable|bool $when` argument.
#[derive(Debug, Clone)]
pub enum When {
    Freeze(bool),
    Date(DateTimeImmutable),
    Text(String),
}

impl From<bool> for When {
    fn from(value: bool) -> Self {
        Self::Freeze(value)
    }
}

impl From<DateTimeImmutable> for When {
    fn from(value: DateTimeImmutable) -> Self {
        Self::Date(value)
    }
}

impl From<&str> for When {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

static ORIGINAL_CLOCK: Mutex<Option<SharedClock>> = Mutex::new(None);
static TEST_LOCK: Mutex<()> = Mutex::new(());

/// `ClockSensitiveTrait`'s static methods.
pub struct ClockSensitiveTrait;

/// One test's hold on the global clock (see the module documentation).
#[must_use = "the clock is restored when the guard is dropped"]
pub struct ClockSensitiveGuard {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for ClockSensitiveGuard {
    fn drop(&mut self) {
        ClockSensitiveTrait::restore_clock_after_test();
    }
}

impl ClockSensitiveTrait {
    /// `mockTime($when)`; `mockTime()` is `mock_time(true)`.
    pub fn mock_time(when: impl Into<When>) -> Result<SharedClock, DateError> {
        let clock: SharedClock = match when.into() {
            When::Freeze(false) => match Self::save_clock_before_test(false) {
                Some(original) => original,
                None => Clock::get(),
            },
            When::Freeze(true) => Arc::new(MockClock::new("now", None)?),
            When::Date(date) => Arc::new(MockClock::new(date, None)?),
            When::Text(text) => Arc::new(MockClock::new(now(&text)?, None)?),
        };
        Clock::set(clock);

        Ok(Clock::get())
    }

    /// `saveClockBeforeTest($save)`: saves the global clock, after
    /// restoring the one saved before, and returns it; without `$save`
    /// returns the saved clock (`None` before the first save).
    pub fn save_clock_before_test(save: bool) -> Option<SharedClock> {
        let saved = ORIGINAL_CLOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();

        if save && saved.is_some() {
            Self::restore_clock_after_test();
        }

        if save {
            let clock = Clock::get();
            *ORIGINAL_CLOCK
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(clock.clone());
            Some(clock)
        } else {
            saved
        }
    }

    /// `restoreClockAfterTest()`.
    pub fn restore_clock_after_test() {
        if let Some(clock) = Self::save_clock_before_test(false) {
            Clock::set(clock);
        }
    }

    /// The `#[Before]` hook, for one test: see the module documentation.
    pub fn before_test() -> ClockSensitiveGuard {
        let lock = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        Self::save_clock_before_test(true);
        ClockSensitiveGuard { _lock: lock }
    }
}
