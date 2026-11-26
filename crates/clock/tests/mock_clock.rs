//! MockClockTest.php at symfony/clock
//! 945ea3b64d6da9c1c2c354fe5c07b27455df6442. Names and assertions are
//! PHPUnit's; a data provider's cases run in a loop inside the test.
//!
//! `assertNotSame($clock->now(), $clock->now())` (two objects) becomes a
//! check that the two values live at different addresses: `now()`
//! returns a copy, as PHP's `clone`.
//!
//! Upstream: symfony/clock Tests/MockClockTest.php
#![allow(non_snake_case)]

use std::{thread::sleep, time::Duration};

use symfony_clock::{
    MockClock, Seconds,
    php::{DateError, DateTimeImmutable, DateTimeZone},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn utc() -> DateTimeZone {
    DateTimeZone::utc()
}

#[test]
fn testConstruct() -> TestResult {
    let clock = MockClock::new("now", None)?;
    assert_eq!("UTC", clock.now().get_timezone().get_name());

    let tz = "Europe/Paris";
    let clock = MockClock::new(tz, None)?;
    assert_eq!(tz, clock.now().get_timezone().get_name());

    let clock = MockClock::new("now", Some(tz.into()))?;
    assert_eq!(tz, clock.now().get_timezone().get_name());

    let clock = MockClock::new("now", Some(DateTimeZone::new(tz)?.into()))?;
    assert_eq!(tz, clock.now().get_timezone().get_name());

    let now = DateTimeImmutable::now();
    let clock = MockClock::new(&now, None)?;
    assert_eq!(now, *clock.now());

    let clock = MockClock::new("2022-06-20 00:00:00", None)?;
    assert_eq!("2022-06-20 00:00:00", clock.now().format("Y-m-d H:i:s"));
    Ok(())
}

#[test]
fn testNow() -> TestResult {
    let before = DateTimeImmutable::now();
    sleep(Duration::from_micros(1));
    let clock = MockClock::new("now", None)?;
    sleep(Duration::from_micros(1));
    let after = DateTimeImmutable::now();

    assert!(*clock.now() > before);
    assert!(*clock.now() < after);

    assert_eq!(clock.now(), clock.now());
    let (a, b) = (clock.now(), clock.now());
    assert!(!std::ptr::eq(&raw const a, &raw const b));
    Ok(())
}

#[test]
fn testSleep() -> TestResult {
    let clock = MockClock::new(
        DateTimeImmutable::new("2112-09-17 23:53:00.999Z", None)?.set_timezone(&utc()),
        None,
    )?;
    let tz = clock.now().get_timezone().get_name();

    clock.sleep(2.002_001)?;
    assert_eq!(
        "2112-09-17 23:53:03.001001",
        clock.now().format("Y-m-d H:i:s.u")
    );
    assert_eq!(tz, clock.now().get_timezone().get_name());
    Ok(())
}

fn provideSleepDurations() -> Vec<(&'static str, &'static str, Seconds, &'static str)> {
    vec![
        // integer
        (
            "2024-01-01T00:00:00.5Z",
            "UTC",
            Seconds::Int(3),
            "2024-01-01T00:00:03.500000+00:00",
        ),
        // microsecond carry
        (
            "2024-01-01T00:00:00.999999Z",
            "UTC",
            Seconds::Float(0.000_002),
            "2024-01-01T00:00:01.000001+00:00",
        ),
        // below one microsecond
        (
            "2024-01-01T00:00:00.5Z",
            "UTC",
            Seconds::Float(0.000_000_4),
            "2024-01-01T00:00:00.500000+00:00",
        ),
        // more than 2^31 microseconds
        (
            "2024-01-01T00:00:00.5Z",
            "UTC",
            Seconds::Int(3600),
            "2024-01-01T01:00:00.500000+00:00",
        ),
        // before 1970
        (
            "1969-12-31T23:59:59.5Z",
            "UTC",
            Seconds::Float(0.75),
            "1970-01-01T00:00:00.250000+00:00",
        ),
        // long before 1970
        (
            "1961-07-09T12:00:31.961111Z",
            "UTC",
            Seconds::Float(0.5),
            "1961-07-09T12:00:32.461111+00:00",
        ),
        // DST fall-back
        (
            "2026-10-25T00:59:59.9Z",
            "Europe/Madrid",
            Seconds::Float(0.2),
            "2026-10-25T02:00:00.100000+01:00",
        ),
    ]
}

#[test]
fn testSleepCarriesSecondsAndKeepsTimezone() -> TestResult {
    for (now, timezone, seconds, expected) in provideSleepDurations() {
        let clock = MockClock::new(DateTimeImmutable::new(now, None)?, Some(timezone.into()))?;

        clock.sleep(seconds)?;

        assert_eq!(expected, clock.now().format("Y-m-d\\TH:i:s.uP"));
        assert_eq!(timezone, clock.now().get_timezone().get_name());
    }
    Ok(())
}

fn provideValidModifyStrings() -> [(&'static str, &'static str); 2] {
    [
        // absolute datetime value
        ("2112-09-17 23:53:03.001", "2112-09-17 23:53:03.001000"),
        // relative modified date
        ("+2 days", "2112-09-19 23:53:00.999000"),
    ]
}

#[test]
fn testModifyWithSpecificDateTime() -> TestResult {
    for (modified_now, expected_now) in provideValidModifyStrings() {
        let clock = MockClock::new(
            DateTimeImmutable::new("2112-09-17 23:53:00.999Z", None)?.set_timezone(&utc()),
            None,
        )?;
        let tz = clock.now().get_timezone().get_name();

        clock.modify(modified_now)?;

        assert_eq!(expected_now, clock.now().format("Y-m-d H:i:s.u"));
        assert_eq!(tz, clock.now().get_timezone().get_name());
    }
    Ok(())
}

fn provideInvalidModifyStrings() -> [&'static str; 2] {
    [
        // Named holiday is not recognized
        "Halloween",
        // empty string
        "",
    ]
}

#[test]
fn testModifyThrowsOnInvalidString() -> TestResult {
    for modified_now in provideInvalidModifyStrings() {
        let clock = MockClock::new(
            DateTimeImmutable::new("2112-09-17 23:53:00.999Z", None)?.set_timezone(&utc()),
            None,
        )?;

        let error = clock.modify(modified_now).unwrap_err();
        assert!(matches!(error, DateError::MalformedString(_)));
        assert!(
            error
                .message()
                .contains(&format!("Failed to parse time string ({modified_now})"))
        );
    }
    Ok(())
}

#[test]
fn testWithTimeZone() -> TestResult {
    let clock = MockClock::new("now", None)?;
    let utc_clock = clock.with_time_zone("UTC")?;

    assert!(!std::ptr::eq(&raw const clock, &raw const utc_clock));
    assert_eq!("UTC", utc_clock.now().get_timezone().get_name());
    Ok(())
}

#[test]
fn testSleepWithNegativeValueDoesNothing() -> TestResult {
    let initial_time = DateTimeImmutable::new("2000-01-01 12:00:00 UTC", None)?;

    let clock = MockClock::new(&initial_time, None)?;
    clock.sleep(-10.5)?;

    assert_eq!(initial_time, *clock.now());
    Ok(())
}
