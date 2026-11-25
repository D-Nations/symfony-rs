//! DatePointTest.php at symfony/clock
//! 945ea3b64d6da9c1c2c354fe5c07b27455df6442. Names and assertions are
//! PHPUnit's; `#[TestWith]` rows and data provider cases run in a loop
//! inside the test. Each test takes `ClockSensitiveTrait::before_test()`,
//! the port of the trait's `#[Before]`/`#[After]` hooks.
//! `assertInstanceOf(DatePoint::class, ...)` holds by type.
//!
//! Upstream: symfony/clock Tests/DatePointTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use symfony_clock::{
    Clock, DatePoint, NativeClock,
    php::{DateError, DateTimeImmutable, DateTimeZone, Timestamp},
    test::ClockSensitiveTrait,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn testDatePoint() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    ClockSensitiveTrait::mock_time("2010-01-28 15:00:00 UTC")?;

    let date = DatePoint::new("now", None, None)?;
    assert_eq!("2010-01-28 15:00:00 UTC", date.format("Y-m-d H:i:s e"));

    let date = DatePoint::new("+1 day Europe/Paris", None, None)?;
    assert_eq!(
        "2010-01-29 16:00:00 Europe/Paris",
        date.format("Y-m-d H:i:s e")
    );

    let date = DatePoint::new("2022-01-28 15:00:00 Europe/Paris", None, None)?;
    assert_eq!(
        "2022-01-28 15:00:00 Europe/Paris",
        date.format("Y-m-d H:i:s e")
    );
    Ok(())
}

#[test]
fn testNowWithNativeClock() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    Clock::set(Arc::new(NativeClock::new(Some("Asia/Tokyo".into()))?));

    let before = DateTimeImmutable::now();
    let date = DatePoint::new("now", None, None)?;
    let after = DateTimeImmutable::now();

    assert_eq!("Asia/Tokyo", date.get_timezone().get_name());
    assert!(*date >= before);
    assert!(*date <= after);

    assert_eq!(
        "UTC",
        DatePoint::new("now", Some(&DateTimeZone::new("UTC")?), None)?
            .get_timezone()
            .get_name()
    );
    Ok(())
}

#[test]
fn testFallBackHourWithReference() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    for (expected, datetime, reference) in [
        (
            "2016-10-30 02:30:00+02:00",
            "now",
            "2016-10-30 00:30:00 UTC",
        ),
        (
            "2016-10-30 02:30:01+02:00",
            "+1 second",
            "2016-10-30 00:30:00 UTC",
        ),
        (
            "2016-10-30 02:30:00+01:00",
            "now",
            "2016-10-30 01:30:00 UTC",
        ),
        (
            "2016-10-30 02:30:01+01:00",
            "+1 second",
            "2016-10-30 01:30:00 UTC",
        ),
        (
            "2016-10-30 02:30:00+01:00",
            "2016-10-30 02:30",
            "2016-07-14 12:00:00 UTC",
        ),
        (
            "2016-10-30 02:30:00+02:00",
            "2016-10-30 02:30 Europe/Paris",
            "2016-01-14 12:00:00 UTC",
        ),
    ] {
        let timezone = DateTimeZone::new("Europe/Paris")?;

        let reference = DateTimeImmutable::new(reference, None)?;
        let date = DatePoint::new(datetime, Some(&timezone), Some(&reference))?;
        assert_eq!(expected, date.format("Y-m-d H:i:sP"));

        ClockSensitiveTrait::mock_time(reference)?;

        let date = DatePoint::new(datetime, Some(&timezone), None)?;
        assert_eq!(expected, date.format("Y-m-d H:i:sP"));
    }
    Ok(())
}

#[test]
fn testCreateFromFormat() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    let date = DatePoint::create_from_format("Y-m-d H:i:s", "2010-01-28 15:00:00", None)?;

    assert_eq!("2010-01-28 15:00:00", date.format("Y-m-d H:i:s"));

    let error = DatePoint::create_from_format("Y-m-d H:i:s", "Bad Date", None).unwrap_err();
    assert!(matches!(error, DateError::MalformedString(_)));
    assert!(
        error
            .message()
            .contains("A four digit year could not be found")
    );
    Ok(())
}

fn provideValidTimestamps() -> Vec<(&'static str, Timestamp, &'static str)> {
    vec![
        (
            "positive integer",
            Timestamp::Int(1_359_188_516),
            "2013-01-26T08:21:56.000000+00:00",
        ),
        (
            "positive float",
            Timestamp::Float(1_359_188_516.123_456),
            "2013-01-26T08:21:56.123456+00:00",
        ),
        (
            "positive integer-ish float",
            Timestamp::Float(1_359_188_516.0),
            "2013-01-26T08:21:56.000000+00:00",
        ),
        (
            "zero as integer",
            Timestamp::Int(0),
            "1970-01-01T00:00:00.000000+00:00",
        ),
        (
            "zero as float",
            Timestamp::Float(0.0),
            "1970-01-01T00:00:00.000000+00:00",
        ),
        (
            "negative integer",
            Timestamp::Int(-100),
            "1969-12-31T23:58:20.000000+00:00",
        ),
        (
            "negative float",
            Timestamp::Float(-100.123_456),
            "1969-12-31T23:58:19.876544+00:00",
        ),
        (
            "negative integer-ish float",
            Timestamp::Float(-100.0),
            "1969-12-31T23:58:20.000000+00:00",
        ),
    ]
}

#[test]
fn testCreateFromTimestamp() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    for (name, timestamp, expected) in provideValidTimestamps() {
        let date = DatePoint::create_from_timestamp(timestamp)?;

        assert_eq!(expected, date.format("Y-m-d\\TH:i:s.uP"), "{name}");
    }
    Ok(())
}

fn provideOutOfRangeFloatTimestamps() -> [(&'static str, f64); 4] {
    [
        ("too large (positive)", 1e20),
        ("too large (negative)", -1e20),
        ("NaN", f64::NAN),
        ("infinity", f64::INFINITY),
    ]
}

#[test]
fn testCreateFromTimestampWithFloatOutOfRange() {
    let _clock = ClockSensitiveTrait::before_test();
    for (name, timestamp) in provideOutOfRangeFloatTimestamps() {
        let error = DatePoint::create_from_timestamp(timestamp).unwrap_err();
        assert!(matches!(error, DateError::Range(_)), "{name}");
        assert!(
            error.message().contains(
                "DateTimeImmutable::createFromTimestamp(): Argument #1 ($timestamp) must be a finite number between"
            ),
            "{name}"
        );
    }
}

#[test]
fn testModify() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    let date = DatePoint::new("2010-01-28 15:00:00", None, None)?;
    let date = date.modify("+1 day")?;

    assert_eq!("2010-01-29 15:00:00", date.format("Y-m-d H:i:s"));

    let error = date.modify("Bad Date").unwrap_err();
    assert!(matches!(error, DateError::MalformedString(_)));
    assert!(
        error
            .message()
            .contains("Failed to parse time string (Bad Date)")
    );
    Ok(())
}

#[test]
fn testMicrosecond() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    let date = DatePoint::new("2010-01-28 15:00:00.123456", None, None)?;

    assert_eq!("2010-01-28 15:00:00.123456", date.format("Y-m-d H:i:s.u"));

    let date = date.set_microsecond(789)?;

    assert_eq!("2010-01-28 15:00:00.000789", date.format("Y-m-d H:i:s.u"));
    assert_eq!(789, date.get_microsecond());

    let error = date.set_microsecond(1_000_000).unwrap_err();
    assert!(matches!(error, DateError::Range(_)));
    assert!(error.message().contains(
        "DatePoint::setMicrosecond(): Argument #1 ($microsecond) must be between 0 and 999999, 1000000 given"
    ));
    Ok(())
}

#[test]
fn testTimeDefaultsToMidnight() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    for (expected, datetime) in [
        ("2024-04-01 00:00:00.000000", "2024-04"),
        ("2024-04-09 00:00:00.000000", "2024-04-09"),
        ("2024-04-09 03:00:00.000000", "2024-04-09 03:00"),
        ("2024-04-09 00:00:00.123456", "2024-04-09 00:00:00.123456"),
    ] {
        let date = DateTimeImmutable::new(datetime, None)?;
        assert_eq!(expected, date.format("Y-m-d H:i:s.u"));

        let date = DatePoint::new(datetime, None, None)?;
        assert_eq!(expected, date.format("Y-m-d H:i:s.u"));
    }
    Ok(())
}

#[test]
fn testTimeDefaultsToMidnightWithReference() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    for (expected, datetime, timezone) in [
        ("2016-09-08 01:00:00.000000", "2016-09-08 +1 hour", "UTC"),
        ("2016-09-07 23:00:00.000000", "2016-09-08 -1 hour", "UTC"),
        ("2016-09-09 12:00:00.000000", "2016-09-08 +36 hours", "UTC"),
        ("2016-09-08 01:00:00.000000", "+1 hour 2016-09-08", "UTC"),
        ("2016-09-01 01:00:00.000000", "2016-09 +1 hour", "UTC"),
        (
            "2018-11-04 01:00:00.000000",
            "2018-11-04",
            "America/Sao_Paulo",
        ),
        ("2016-09-08 10:30:00.000000", "2016-09-08T1030", "UTC"),
        (
            "2016-09-09 10:30:00.000000",
            "2016-09-08 t1030 +1 day",
            "UTC",
        ),
    ] {
        let timezone = DateTimeZone::new(timezone)?;

        let date = DateTimeImmutable::new(datetime, Some(&timezone))?;
        assert_eq!(expected, date.format("Y-m-d H:i:s.u"));

        let reference = DateTimeImmutable::new("2024-01-10 22:20:02.654321", Some(&timezone))?;
        let date = DatePoint::new(datetime, Some(&timezone), Some(&reference))?;
        assert_eq!(expected, date.format("Y-m-d H:i:s.u"));
    }
    Ok(())
}

#[test]
fn testDatesOutsideThe32BitTimestampRange() -> TestResult {
    let _clock = ClockSensitiveTrait::before_test();
    for (expected, datetime, reference) in [
        ("3000-01-01 00:00:00.000000", "3000-01-01", None),
        ("1800-01-01 00:00:00.000000", "1800-01-01", None),
        (
            "3000-01-01 10:00:00.000000",
            "10:00",
            Some("3000-01-01 05:06:07.123456"),
        ),
        (
            "3000-01-01 10:30:00.000000",
            "t1030",
            Some("3000-01-01 05:06:07.123456"),
        ),
        (
            "1800-01-01 10:30:00.000000",
            "t1030",
            Some("1800-01-01 05:06:07.123456"),
        ),
        (
            "3000-01-02 05:06:07.123456",
            "+1 day",
            Some("3000-01-01 05:06:07.123456"),
        ),
    ] {
        let timezone = DateTimeZone::new("UTC")?;
        let reference = reference
            .map(|r| DateTimeImmutable::new(r, Some(&timezone)))
            .transpose()?;
        let date = DatePoint::new(datetime, Some(&timezone), reference.as_ref())?;

        assert_eq!(expected, date.format("Y-m-d H:i:s.u"));
    }
    Ok(())
}
