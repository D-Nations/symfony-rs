//! ClockBeforeClassTest.php at symfony/clock
//! 945ea3b64d6da9c1c2c354fe5c07b27455df6442. Names and assertions are
//! PHPUnit's.
//!
//! PHPUnit runs, in order: the trait's `#[BeforeClass]`
//! `saveClockBeforeTest()`, the class's `setUpBeforeClass()`, the
//! `#[Before]` hook, the test, the `#[After]` hook and
//! `tearDownAfterClass()`. The test calls them in that order.
//!
//! Upstream: symfony/clock Tests/ClockBeforeClassTest.php
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex, PoisonError};

use symfony_clock::{Clock, MockClock, NativeClock, SharedClock, test::ClockSensitiveTrait};

type TestResult = Result<(), Box<dyn std::error::Error>>;

static CLOCK: Mutex<Option<SharedClock>> = Mutex::new(None);

fn class_clock() -> Option<SharedClock> {
    CLOCK.lock().unwrap_or_else(PoisonError::into_inner).clone()
}

fn setUpBeforeClass() -> TestResult {
    *CLOCK.lock().unwrap_or_else(PoisonError::into_inner) =
        Some(ClockSensitiveTrait::mock_time(true)?);
    Ok(())
}

fn tearDownAfterClass() {
    *CLOCK.lock().unwrap_or_else(PoisonError::into_inner) = None;
}

#[test]
fn testMockClock() -> TestResult {
    ClockSensitiveTrait::save_clock_before_test(true);
    setUpBeforeClass()?;
    let guard = ClockSensitiveTrait::before_test();

    let saved = class_clock().ok_or("no class clock")?;
    assert!(saved.is::<MockClock>());
    assert!(Clock::get().is::<NativeClock>());

    let clock = ClockSensitiveTrait::mock_time(true)?;
    assert!(Clock::get().is::<MockClock>());
    assert!(Arc::ptr_eq(&Clock::get(), &clock));

    assert!(!Arc::ptr_eq(&clock, &saved));

    ClockSensitiveTrait::restore_clock_after_test();
    ClockSensitiveTrait::save_clock_before_test(true);

    assert!(saved.is::<MockClock>());
    assert!(Clock::get().is::<NativeClock>());

    let clock = ClockSensitiveTrait::mock_time(true)?;
    assert!(Clock::get().is::<MockClock>());
    assert!(Arc::ptr_eq(&Clock::get(), &clock));

    assert!(!Arc::ptr_eq(&clock, &saved));

    drop(guard);
    tearDownAfterClass();
    Ok(())
}
