//! Port of Symfony\Component\Clock\MonotonicClock.php: a monotonic clock
//! suitable for performance profiling.
//!
//! PHP's `hrtime()` counts from an arbitrary point; here it is
//! [`std::time::Instant`] counted from the first clock created in the
//! process. `hrtime()` cannot fail here, so upstream's
//! `\RuntimeException` for a missing monotonic timer does not occur.
//!
//! Upstream: symfony/clock MonotonicClock.php

use std::{
    sync::{Arc, OnceLock},
    time::Instant,
};

use crate::{
    clock_interface::{
        ClockInterface, PsrClockInterface, Seconds, SharedClock, TimeZoneInput, native_sleep,
    },
    date_point::DatePoint,
    php::{DateError, DateTimeImmutable, DateTimeZone, date_default_timezone_get},
};

static ORIGIN: OnceLock<Instant> = OnceLock::new();

/// `hrtime()`: seconds and nanoseconds.
fn hrtime() -> (i64, i64) {
    let elapsed = ORIGIN.get_or_init(Instant::now).elapsed();
    (
        i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX),
        i64::from(elapsed.subsec_nanos()),
    )
}

/// `MonotonicClock`.
#[derive(Debug, Clone)]
pub struct MonotonicClock {
    s_offset: i64,
    us_offset: i64,
    timezone: DateTimeZone,
}

impl MonotonicClock {
    /// `new MonotonicClock($timezone)`: the default time zone when `None`.
    pub fn new(timezone: Option<TimeZoneInput<'_>>) -> Result<Self, DateError> {
        let offset = hrtime();
        let (sec, usec) = crate::php::system_now();
        let timezone = match timezone {
            Some(timezone) => timezone.resolve()?,
            None => DateTimeZone::new(&date_default_timezone_get().get_name())?,
        };
        Ok(Self {
            s_offset: sec - offset.0,
            us_offset: i64::from(usec) - offset.1 / 1000,
            timezone,
        })
    }

    /// `now()`.
    pub fn now(&self) -> Result<DatePoint, DateError> {
        let (mut s, us) = hrtime();
        let mut us = us / 1000 + self.us_offset;
        if us >= 1_000_000 {
            s += 1;
            us -= 1_000_000;
        } else if us < 0 {
            s -= 1;
            us += 1_000_000;
        }
        let now = format!("@{}.{us:06}", s + self.s_offset);
        let date = DateTimeImmutable::new(&now, Some(&self.timezone))?;
        Ok(DatePoint::create_from_interface(&date).set_timezone(&self.timezone))
    }

    /// `sleep($seconds)`.
    pub fn sleep(&self, seconds: impl Into<Seconds>) {
        native_sleep(seconds.into());
    }

    /// `withTimeZone($timezone)`.
    pub fn with_time_zone<'a>(
        &self,
        timezone: impl Into<TimeZoneInput<'a>>,
    ) -> Result<Self, DateError> {
        let mut clone = self.clone();
        clone.timezone = timezone.into().resolve()?;
        Ok(clone)
    }
}

impl PsrClockInterface for MonotonicClock {
    fn now(&self) -> DateTimeImmutable {
        // The text is always `@seconds.micro`, which the parser reads.
        match Self::now(self) {
            Ok(now) => now.into_inner(),
            Err(_) => DateTimeImmutable::now().set_timezone(&self.timezone),
        }
    }
}

impl ClockInterface for MonotonicClock {
    fn sleep(&self, seconds: Seconds) -> Result<(), DateError> {
        native_sleep(seconds);
        Ok(())
    }

    fn with_time_zone(&self, timezone: TimeZoneInput<'_>) -> Result<SharedClock, DateError> {
        Ok(Arc::new(Self::with_time_zone(self, timezone)?))
    }
}
