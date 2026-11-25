//! Port of Symfony\Component\Clock\MockClock.php: a clock that always
//! returns the same date, suitable for testing time-sensitive logic.
//!
//! The date sits behind a lock, so a `MockClock` shared as
//! [`SharedClock`](crate::SharedClock) moves for every holder when one of
//! them calls `sleep()` or `modify()`, as the PHP object does.
//!
//! Upstream: symfony/clock MockClock.php

use std::sync::{Arc, Mutex, PoisonError};

use crate::{
    clock_interface::{ClockInterface, PsrClockInterface, Seconds, SharedClock, TimeZoneInput},
    date_point::DatePoint,
    php::{DateError, DateTimeImmutable, DateTimeZone},
};

/// The `\DateTimeImmutable|string $now` argument.
#[derive(Debug, Clone)]
pub enum MockNow {
    Date(DateTimeImmutable),
    Point(DatePoint),
    Text(String),
}

impl From<&str> for MockNow {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<DateTimeImmutable> for MockNow {
    fn from(value: DateTimeImmutable) -> Self {
        Self::Date(value)
    }
}

impl From<&DateTimeImmutable> for MockNow {
    fn from(value: &DateTimeImmutable) -> Self {
        Self::Date(value.clone())
    }
}

impl From<DatePoint> for MockNow {
    fn from(value: DatePoint) -> Self {
        Self::Point(value)
    }
}

/// `MockClock`.
#[derive(Debug)]
pub struct MockClock {
    now: Mutex<DatePoint>,
}

impl Clone for MockClock {
    fn clone(&self) -> Self {
        Self {
            now: Mutex::new(self.current()),
        }
    }
}

impl MockClock {
    /// `new MockClock($now, $timezone)`; `new MockClock()` is
    /// `MockClock::new("now", None)`.
    ///
    /// Throws `\DateMalformedStringException` when `$now` is invalid and
    /// `\DateInvalidTimeZoneException` when `$timezone` is.
    pub fn new(
        now: impl Into<MockNow>,
        timezone: Option<TimeZoneInput<'_>>,
    ) -> Result<Self, DateError> {
        let timezone = timezone.map(TimeZoneInput::resolve).transpose()?;
        let now = match now.into() {
            MockNow::Text(text) => {
                let zone = timezone.clone().unwrap_or_else(DateTimeZone::utc);
                DatePoint::new(&text, Some(&zone), None)?
            }
            MockNow::Date(date) => DatePoint::create_from_interface(&date),
            MockNow::Point(point) => point,
        };
        let now = match &timezone {
            Some(zone) => now.set_timezone(zone),
            None => now,
        };
        Ok(Self {
            now: Mutex::new(now),
        })
    }

    fn current(&self) -> DatePoint {
        self.now
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn replace(&self, now: DatePoint) {
        *self.now.lock().unwrap_or_else(PoisonError::into_inner) = now;
    }

    /// `now()`: a copy of the date.
    pub fn now(&self) -> DatePoint {
        self.current()
    }

    /// `sleep($seconds)`: moves the date forward; nothing for 0 or less.
    pub fn sleep(&self, seconds: impl Into<Seconds>) -> Result<(), DateError> {
        let seconds = seconds.into();
        let value = seconds.as_f64();
        if 0.0 >= value || value.is_nan() {
            return Ok(());
        }
        let now = self.current();
        let timestamp: f64 = now.format("U").parse().unwrap_or(0.0);
        let mut s = timestamp + value.floor();
        let fraction: i64 = now.format("u").parse().unwrap_or(0);
        #[allow(clippy::cast_possible_truncation)]
        let mut us = fraction + ((value % 1.0) * 1e6).round() as i64;
        if us >= 1_000_000 {
            s += 1.0;
            us -= 1_000_000;
        }
        let text = format!("{s:.0}.{us:06}");
        let next = DatePoint::create_from_format("U.u", &text, None)?;
        self.replace(next.set_timezone(&now.get_timezone()));
        Ok(())
    }

    /// `modify($modifier)`.
    ///
    /// Throws `\DateMalformedStringException` when `$modifier` is invalid.
    pub fn modify(&self, modifier: &str) -> Result<(), DateError> {
        let next = self.current().modify(modifier)?;
        self.replace(next);
        Ok(())
    }

    /// `withTimeZone($timezone)`: a new clock, the same instant.
    pub fn with_time_zone<'a>(
        &self,
        timezone: impl Into<TimeZoneInput<'a>>,
    ) -> Result<Self, DateError> {
        let timezone = timezone.into().resolve()?;
        Ok(Self {
            now: Mutex::new(self.current().set_timezone(&timezone)),
        })
    }
}

impl PsrClockInterface for MockClock {
    fn now(&self) -> DateTimeImmutable {
        self.current().into_inner()
    }
}

impl ClockInterface for MockClock {
    fn sleep(&self, seconds: Seconds) -> Result<(), DateError> {
        Self::sleep(self, seconds)
    }

    fn with_time_zone(&self, timezone: TimeZoneInput<'_>) -> Result<SharedClock, DateError> {
        Ok(Arc::new(Self::with_time_zone(self, timezone)?))
    }
}
