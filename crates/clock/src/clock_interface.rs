//! Port of Symfony\Component\Clock\ClockInterface.php, with PSR-20's
//! `Psr\Clock\ClockInterface` that it extends.
//!
//! PHP's clocks are shared objects: `Clock::get()` and a test hold the
//! same `MockClock`, and `sleep()` moves it for both. Here a clock is
//! shared as [`SharedClock`] (`Arc<dyn ClockInterface>`), and
//! `MockClock` changes its time through a lock. `instanceof` is
//! [`ClockInterface::is`].
//!
//! Upstream: symfony/clock ClockInterface.php

use std::{any::Any, fmt, sync::Arc};

use crate::php::{DateError, DateTimeImmutable, DateTimeZone};

/// `Psr\Clock\ClockInterface`.
pub trait PsrClockInterface: Any + Send + Sync + fmt::Debug {
    /// `now(): \DateTimeImmutable`.
    fn now(&self) -> DateTimeImmutable;
}

/// `Symfony\Component\Clock\ClockInterface`.
pub trait ClockInterface: PsrClockInterface {
    /// `sleep(float|int $seconds)`.
    fn sleep(&self, seconds: Seconds) -> Result<(), DateError>;

    /// `withTimeZone(\DateTimeZone|string $timezone): static`, as a new
    /// shared clock.
    fn with_time_zone(&self, timezone: TimeZoneInput<'_>) -> Result<SharedClock, DateError>;
}

/// A clock shared as PHP shares an object.
pub type SharedClock = Arc<dyn ClockInterface>;

impl dyn ClockInterface {
    /// `$clock instanceof T`.
    pub fn is<T: ClockInterface>(&self) -> bool {
        (self as &dyn Any).is::<T>()
    }

    /// The clock as a `T`, when it is one.
    pub fn downcast_ref<T: ClockInterface>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref::<T>()
    }
}

/// The `float|int` seconds of `sleep()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seconds {
    Int(i64),
    Float(f64),
}

impl Seconds {
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn as_f64(self) -> f64 {
        match self {
            Self::Int(n) => n as f64,
            Self::Float(x) => x,
        }
    }
}

impl From<i64> for Seconds {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<i32> for Seconds {
    fn from(value: i32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<f64> for Seconds {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

/// The `\DateTimeZone|string` arguments.
#[derive(Debug, Clone)]
pub enum TimeZoneInput<'a> {
    Zone(DateTimeZone),
    Name(&'a str),
}

impl TimeZoneInput<'_> {
    /// `is_string($timezone) ? new \DateTimeZone($timezone) : $timezone`.
    pub fn resolve(self) -> Result<DateTimeZone, DateError> {
        match self {
            Self::Zone(zone) => Ok(zone),
            Self::Name(name) => DateTimeZone::new(name),
        }
    }
}

impl From<DateTimeZone> for TimeZoneInput<'_> {
    fn from(value: DateTimeZone) -> Self {
        Self::Zone(value)
    }
}

impl From<&DateTimeZone> for TimeZoneInput<'_> {
    fn from(value: &DateTimeZone) -> Self {
        Self::Zone(value.clone())
    }
}

impl<'a> From<&'a str> for TimeZoneInput<'a> {
    fn from(value: &'a str) -> Self {
        Self::Name(value)
    }
}

/// `sleep()` and `usleep()` as `NativeClock` and `MonotonicClock` call them:
/// `(int) $seconds` whole seconds, then the rest in microseconds.
pub(crate) fn native_sleep(seconds: Seconds) {
    let value = seconds.as_f64();
    #[allow(clippy::cast_possible_truncation)]
    let whole = match seconds {
        Seconds::Int(n) => n,
        Seconds::Float(x) => x as i64,
    };
    if let Ok(s) = u64::try_from(whole)
        && s > 0
    {
        std::thread::sleep(std::time::Duration::from_secs(s));
    }
    #[allow(clippy::cast_precision_loss)]
    let rest = value - whole as f64;
    if rest > 0.0 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let us = (rest * 1e6) as u64;
        std::thread::sleep(std::time::Duration::from_micros(us));
    }
}
