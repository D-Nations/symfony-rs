//! ClockTest.php at symfony/clock
//! 945ea3b64d6da9c1c2c354fe5c07b27455df6442. Names and assertions are
//! PHPUnit's. Each test takes `ClockSensitiveTrait::before_test()`, the
//! port of the trait's `#[Before]`/`#[After]` hooks.
//!
//! `assertInstanceOf(DatePoint::class, now())` holds by type here: the
//! test checks that `now()` succeeds.
//!
//! Upstream: symfony/clock Tests/ClockTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use symfony_clock::{
    Clock, MockClock, NativeClock, PsrClockInterface, Seconds, now,
    php::{DateError, DateTimeImmutable},
    test::ClockSensitiveTrait,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn testMockClock() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    assert!(Clock::get().is::<NativeClock>());

    let clock = ClockSensitiveTrait::mock_time(true)?;
    assert!(Clock::get().is::<MockClock>());
    assert!(Arc::ptr_eq(&Clock::get(), &clock));
    Ok(())
}

#[test]
fn testNativeClock() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    now("now")?;
    assert!(Clock::get().is::<NativeClock>());
    Ok(())
}

#[test]
fn testNowModifier() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    assert_eq!("2023-08-14", now("2023-08-14")?.format("Y-m-d"));
    assert_eq!(
        "Europe/Paris",
        now("Europe/Paris")?.get_timezone().get_name()
    );
    assert_eq!("UTC", now("UTC")?.get_timezone().get_name());
    Ok(())
}

#[test]
fn testInvalidNowModifier() {
    let _clock = ClockSensitiveTrait::before_test();
    assert!(matches!(
        now("invalid date"),
        Err(DateError::MalformedString(_))
    ));
}

#[test]
fn testMockClockDisable() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    assert!(Clock::get().is::<NativeClock>());

    assert!(ClockSensitiveTrait::mock_time(true)?.is::<MockClock>());
    assert!(ClockSensitiveTrait::mock_time(false)?.is::<NativeClock>());
    Ok(())
}

#[test]
fn testMockClockFreeze() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    ClockSensitiveTrait::mock_time(DateTimeImmutable::new("2021-12-19", None)?)?;

    assert_eq!("2021-12-19", now("now")?.format("Y-m-d"));
    assert_eq!("2021-12-20", now("+1 days")?.format("Y-m-d"));

    ClockSensitiveTrait::mock_time("+1 days")?;
    assert_eq!("2021-12-20", now("now")?.format("Y-m-d"));
    Ok(())
}

/// The anonymous `implements ClockInterface` class of testPsrClock.
#[derive(Debug)]
struct PsrClock;

impl PsrClockInterface for PsrClock {
    fn now(&self) -> DateTimeImmutable {
        DateTimeImmutable::new("@1234567", None).unwrap_or_else(|_| DateTimeImmutable::now())
    }
}

#[test]
fn testPsrClock() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    let psr_clock = Arc::new(PsrClock);

    Clock::set_psr(psr_clock);

    assert!(Clock::get().is::<Clock>());

    assert_eq!(1_234_567, now("now")?.get_timestamp());

    assert_eq!(
        "UTC",
        Clock::get()
            .with_time_zone("UTC".into())?
            .now()
            .get_timezone()
            .get_name()
    );
    assert_eq!(
        "Europe/Paris",
        Clock::get()
            .with_time_zone("Europe/Paris".into())?
            .now()
            .get_timezone()
            .get_name()
    );

    Clock::get().sleep(Seconds::Float(0.1))?;

    assert_eq!(1_234_567, now("now")?.get_timestamp());
    Ok(())
}
