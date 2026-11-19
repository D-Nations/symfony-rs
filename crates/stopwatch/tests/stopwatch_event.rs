//! StopwatchEventTest.php at symfony/stopwatch
//! 21c07b026905d596e8379caeb115d87aa479499d. Names and assertions are
//! PHPUnit's.
//!
//! Upstream runs this class in the `time-sensitive` group, where
//! symfony/phpunit-bridge's ClockMock replaces `microtime()` and makes
//! `usleep()` advance it. `ClockMock` below does the same, starting from
//! the real time as the bridge does.
//!
//! Upstream: symfony/stopwatch Tests/StopwatchEventTest.php
#![allow(non_snake_case)]

use std::sync::{Arc, Mutex};

use symfony_stopwatch::{Host, Number, StopwatchEvent, SystemHost};

const DELTA: f64 = 37.0;

struct ClockMock {
    now: Mutex<f64>,
}

impl ClockMock {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            now: Mutex::new(SystemHost.microtime()),
        })
    }

    fn usleep(&self, microseconds: u32) {
        *self.now.lock().unwrap() += f64::from(microseconds) / 1_000_000.0;
    }
}

impl Host for ClockMock {
    fn microtime(&self) -> f64 {
        *self.now.lock().unwrap()
    }
}

/// `new StopwatchEvent(microtime(true) * 1000, $category, $morePrecision, $name)`.
fn event(clock: &Arc<ClockMock>, category: Option<&str>, name: Option<&str>) -> StopwatchEvent {
    StopwatchEvent::new(
        clock.clone(),
        clock.microtime() * 1000.0,
        category,
        false,
        name,
    )
}

#[track_caller]
fn assert_equals_with_delta(expected: f64, actual: Number, delta: f64) {
    let actual = actual.as_f64();
    assert!(
        (expected - actual).abs() <= delta,
        "{actual} is not {expected} ± {delta}"
    );
}

#[test]
fn testGetOrigin() {
    let event = StopwatchEvent::new(ClockMock::new(), 12.0, None, false, None);
    assert_eq!(12.0, event.get_origin());
}

#[test]
fn testGetCategory() {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    assert_eq!("default", event.get_category());

    let event = self::event(&clock, Some("cat"), None);
    assert_eq!("cat", event.get_category());
}

#[test]
fn testGetPeriods() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    assert_eq!(
        Vec::<symfony_stopwatch::StopwatchPeriod>::new(),
        event.get_periods()
    );

    let event = self::event(&clock, None, None);
    event.start();
    event.stop()?;
    assert_eq!(1, event.get_periods().len());

    let event = self::event(&clock, None, None);
    event.start();
    event.stop()?;
    event.start();
    event.stop()?;
    assert_eq!(2, event.get_periods().len());
    Ok(())
}

#[test]
fn testLap() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    event.start();
    event.lap()?;
    event.stop()?;
    assert_eq!(2, event.get_periods().len());
    Ok(())
}

#[test]
fn testDuration() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    event.start();
    clock.usleep(200000);
    event.stop()?;
    assert_equals_with_delta(200.0, event.get_duration(), DELTA);

    let event = self::event(&clock, None, None);
    event.start();
    clock.usleep(100000);
    event.stop()?;
    clock.usleep(50000);
    event.start();
    clock.usleep(100000);
    event.stop()?;
    assert_equals_with_delta(200.0, event.get_duration(), DELTA);
    Ok(())
}

#[test]
fn testDurationBeforeStop() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    event.start();
    clock.usleep(200000);
    assert_equals_with_delta(200.0, event.get_duration(), DELTA);

    let event = self::event(&clock, None, None);
    event.start();
    clock.usleep(100000);
    event.stop()?;
    clock.usleep(50000);
    event.start();
    assert_equals_with_delta(100.0, event.get_duration(), DELTA);
    clock.usleep(100000);
    assert_equals_with_delta(200.0, event.get_duration(), DELTA);
    Ok(())
}

#[test]
fn testDurationWithMultipleStarts() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    event.start();
    clock.usleep(100000);
    event.start();
    clock.usleep(100000);
    assert_equals_with_delta(300.0, event.get_duration(), DELTA);
    event.stop()?;
    assert_equals_with_delta(300.0, event.get_duration(), DELTA);
    clock.usleep(100000);
    assert_equals_with_delta(400.0, event.get_duration(), DELTA);
    event.stop()?;
    assert_equals_with_delta(400.0, event.get_duration(), DELTA);
    Ok(())
}

#[test]
fn testStopWithoutStart() {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);

    assert!(event.stop().is_err());
}

#[test]
fn testIsStarted() {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    event.start();
    assert!(event.is_started());
}

#[test]
fn testIsNotStarted() {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    assert!(!event.is_started());
}

#[test]
fn testEnsureStopped() {
    // this also test overlap between two periods
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    event.start();
    clock.usleep(100000);
    event.start();
    clock.usleep(100000);
    event.ensure_stopped();
    assert_equals_with_delta(300.0, event.get_duration(), DELTA);
}

#[test]
fn testStartTime() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    assert!(event.get_start_time().as_f64() <= 0.5);

    let event = self::event(&clock, None, None);
    event.start();
    event.stop()?;
    assert!(event.get_start_time().as_f64() <= 1.0);

    let event = self::event(&clock, None, None);
    event.start();
    clock.usleep(100000);
    event.stop()?;
    assert_equals_with_delta(0.0, event.get_start_time(), DELTA);
    Ok(())
}

#[test]
fn testStartTimeWhenStartedLater() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    clock.usleep(100000);
    assert!(event.get_start_time().as_f64() <= 0.5);

    let event = self::event(&clock, None, None);
    clock.usleep(100000);
    event.start();
    event.stop()?;
    assert!(event.get_start_time().as_f64() <= 101.0);

    let event = self::event(&clock, None, None);
    clock.usleep(100000);
    event.start();
    clock.usleep(100000);
    assert_equals_with_delta(100.0, event.get_start_time(), DELTA);
    event.stop()?;
    assert_equals_with_delta(100.0, event.get_start_time(), DELTA);
    Ok(())
}

#[test]
fn testHumanRepresentation() -> Result<(), Box<dyn std::error::Error>> {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    assert_eq!("default/default: 0.00 MiB - 0 ms", event.to_string());
    event.start();
    event.stop()?;
    // preg_match('/default: [0-9\.]+ MiB - [0-9]+ ms/', ...)
    let text = event.to_string();
    let rest = text
        .split_once("default: ")
        .map(|(_, rest)| rest)
        .unwrap_or("");
    let (memory, rest) = rest.split_once(" MiB - ").unwrap_or(("", ""));
    let duration = rest.strip_suffix(" ms").unwrap_or("");
    assert!(
        !memory.is_empty() && memory.chars().all(|c| c.is_ascii_digit() || c == '.'),
        "{text}"
    );
    assert!(
        !duration.is_empty() && duration.chars().all(|c| c.is_ascii_digit()),
        "{text}"
    );

    let event = self::event(&clock, Some("foo"), None);
    assert_eq!("foo/default: 0.00 MiB - 0 ms", event.to_string());

    let event = self::event(&clock, Some("foo"), Some("name"));
    assert_eq!("foo/name: 0.00 MiB - 0 ms", event.to_string());
    Ok(())
}

#[test]
fn testGetName() {
    let clock = ClockMock::new();
    let event = self::event(&clock, None, None);
    assert_eq!("default", event.get_name());

    let event = self::event(&clock, Some("cat"), Some("name"));
    assert_eq!("name", event.get_name());
}
