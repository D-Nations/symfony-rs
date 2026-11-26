//! Port of Symfony\Component\Clock\NativeClock.php: a clock that relies
//! on the system time.
//!
//! Upstream: symfony/clock NativeClock.php

use std::sync::Arc;

use crate::{
    clock_interface::{
        ClockInterface, PsrClockInterface, Seconds, SharedClock, TimeZoneInput, native_sleep,
    },
    date_point::DatePoint,
    php::{DateError, DateTimeImmutable, DateTimeZone, date_default_timezone_get},
};

/// `NativeClock`.
#[derive(Debug, Clone)]
pub struct NativeClock {
    timezone: DateTimeZone,
}

impl NativeClock {
    /// `new NativeClock($timezone)`: the default time zone when `None`.
    ///
    /// Throws `\DateInvalidTimeZoneException` when the zone is invalid.
    pub fn new(timezone: Option<TimeZoneInput<'_>>) -> Result<Self, DateError> {
        let timezone = match timezone {
            Some(timezone) => timezone.resolve()?,
            // date_default_timezone_get() returns a name, read again.
            None => DateTimeZone::new(&date_default_timezone_get().get_name())?,
        };
        Ok(Self { timezone })
    }

    /// `now()`.
    pub fn now(&self) -> DatePoint {
        let (sse, us) = crate::php::system_now();
        DatePoint::create_from_interface(&DateTimeImmutable::from_parts(
            sse,
            us,
            self.timezone.clone(),
        ))
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

impl PsrClockInterface for NativeClock {
    fn now(&self) -> DateTimeImmutable {
        Self::now(self).into_inner()
    }
}

impl ClockInterface for NativeClock {
    fn sleep(&self, seconds: Seconds) -> Result<(), DateError> {
        native_sleep(seconds);
        Ok(())
    }

    fn with_time_zone(&self, timezone: TimeZoneInput<'_>) -> Result<SharedClock, DateError> {
        Ok(Arc::new(Self::with_time_zone(self, timezone)?))
    }
}
