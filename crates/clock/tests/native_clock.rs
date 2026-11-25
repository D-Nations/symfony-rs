//! NativeClockTest.php at symfony/clock
//! 945ea3b64d6da9c1c2c354fe5c07b27455df6442. Names and assertions are
//! PHPUnit's. Like upstream, these run on the system clock.
//!
//! Upstream: symfony/clock Tests/NativeClockTest.php
#![allow(non_snake_case)]

use std::{
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use symfony_clock::{
    NativeClock,
    php::{DateTimeImmutable, DateTimeZone, date_default_timezone_get},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// `microtime(true)`.
fn microtime() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

#[test]
fn testConstruct() -> TestResult {
    let clock = NativeClock::new(Some("UTC".into()))?;
    assert_eq!("UTC", clock.now().get_timezone().get_name());

    let tz = date_default_timezone_get().get_name();
    let clock = NativeClock::new(None)?;
    assert_eq!(tz, clock.now().get_timezone().get_name());

    let clock = NativeClock::new(Some(DateTimeZone::new(&tz)?.into()))?;
    assert_eq!(tz, clock.now().get_timezone().get_name());
    Ok(())
}

#[test]
fn testNow() -> TestResult {
    let clock = NativeClock::new(None)?;
    let before = DateTimeImmutable::now();
    sleep(Duration::from_micros(10));
    let now = clock.now();
    sleep(Duration::from_micros(10));
    let after = DateTimeImmutable::now();

    assert!(*now > before);
    assert!(*now < after);
    Ok(())
}

#[test]
fn testSleep() -> TestResult {
    let clock = NativeClock::new(None)?;
    let tz = clock.now().get_timezone().get_name();

    let before = microtime();
    clock.sleep(1.5);
    let now: f64 = clock.now().format("U.u").parse()?;
    sleep(Duration::from_micros(10));
    let after = microtime();

    assert!(now >= before + 1.5);
    assert!(now < after);
    assert_eq!(tz, clock.now().get_timezone().get_name());
    Ok(())
}

#[test]
fn testWithTimeZone() -> TestResult {
    let clock = NativeClock::new(None)?;
    let utc_clock = clock.with_time_zone("UTC")?;

    assert!(!std::ptr::eq(&raw const clock, &raw const utc_clock));
    assert_eq!("UTC", utc_clock.now().get_timezone().get_name());
    Ok(())
}
