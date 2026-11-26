//! Port of Symfony\Component\Clock\Clock.php: a global clock.
//!
//! PHP keeps the global clock in a static property of the process (one
//! request at a time). Here it is a process-wide `RwLock`, safe to read
//! from any thread; tests that change it run one at a time through
//! [`ClockSensitiveTrait`](crate::test::ClockSensitiveTrait).
//!
//! `Clock::set()` takes a Symfony clock; a clock that only implements
//! PSR-20 goes through [`Clock::set_psr`], which wraps it in a `Clock` as
//! upstream does for a clock that is not a `ClockInterface`.
//!
//! Upstream: symfony/clock Clock.php

use std::sync::{Arc, PoisonError, RwLock};

use crate::{
    clock_interface::{ClockInterface, PsrClockInterface, Seconds, SharedClock, TimeZoneInput},
    date_point::DatePoint,
    native_clock::NativeClock,
    php::{DateError, DateTimeImmutable, DateTimeZone},
};

static GLOBAL_CLOCK: RwLock<Option<SharedClock>> = RwLock::new(None);

/// The clock a [`Clock`] wraps: PSR-20 only, or a Symfony clock.
#[derive(Debug, Clone)]
pub enum AnyClock {
    Psr(Arc<dyn PsrClockInterface>),
    Symfony(SharedClock),
}

impl AnyClock {
    fn now(&self) -> DateTimeImmutable {
        match self {
            Self::Psr(clock) => clock.now(),
            Self::Symfony(clock) => clock.now(),
        }
    }
}

/// `Clock`.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    clock: Option<AnyClock>,
    timezone: Option<DateTimeZone>,
}

impl Clock {
    /// `new Clock($clock, $timezone)`.
    pub fn new(clock: Option<AnyClock>, timezone: Option<DateTimeZone>) -> Self {
        Self { clock, timezone }
    }

    /// Returns the current global clock.
    ///
    /// Note that you should prefer injecting a `ClockInterface` or using
    /// `ClockAwareTrait` when possible instead of using this method.
    pub fn get() -> SharedClock {
        if let Some(clock) = GLOBAL_CLOCK
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
        {
            return Arc::clone(clock);
        }
        let mut global = GLOBAL_CLOCK.write().unwrap_or_else(PoisonError::into_inner);
        Arc::clone(global.get_or_insert_with(|| {
            let clock: SharedClock = match NativeClock::new(None) {
                Ok(clock) => Arc::new(clock),
                // The default zone is always a valid identifier.
                Err(_) => Arc::new(Self::default()),
            };
            clock
        }))
    }

    /// `Clock::set($clock)` for a Symfony clock.
    pub fn set(clock: SharedClock) {
        *GLOBAL_CLOCK.write().unwrap_or_else(PoisonError::into_inner) = Some(clock);
    }

    /// `Clock::set($clock)` for a PSR-20 clock that is not a Symfony
    /// clock: it is wrapped in a `Clock`.
    pub fn set_psr(clock: Arc<dyn PsrClockInterface>) {
        Self::set(Arc::new(Self::new(Some(AnyClock::Psr(clock)), None)));
    }

    /// `now()`.
    pub fn now(&self) -> DatePoint {
        let now = match &self.clock {
            Some(clock) => clock.now(),
            None => Self::get().now(),
        };
        let now = DatePoint::create_from_interface(&now);

        match &self.timezone {
            Some(zone) => now.set_timezone(zone),
            None => now,
        }
    }

    /// `sleep($seconds)`.
    pub fn sleep(&self, seconds: impl Into<Seconds>) -> Result<(), DateError> {
        let seconds = seconds.into();
        let clock = match &self.clock {
            Some(clock) => clock.clone(),
            None => AnyClock::Symfony(Self::get()),
        };

        match clock {
            AnyClock::Symfony(clock) => clock.sleep(seconds),
            AnyClock::Psr(_) => NativeClock::new(None)?.sleep_dyn(seconds),
        }
    }

    /// `withTimeZone($timezone)`.
    ///
    /// Throws `\DateInvalidTimeZoneException` when `$timezone` is invalid.
    pub fn with_time_zone<'a>(
        &self,
        timezone: impl Into<TimeZoneInput<'a>>,
    ) -> Result<Self, DateError> {
        let mut clone = self.clone();
        clone.timezone = Some(timezone.into().resolve()?);
        Ok(clone)
    }
}

impl NativeClock {
    fn sleep_dyn(&self, seconds: Seconds) -> Result<(), DateError> {
        ClockInterface::sleep(self, seconds)
    }
}

impl PsrClockInterface for Clock {
    fn now(&self) -> DateTimeImmutable {
        Self::now(self).into_inner()
    }
}

impl ClockInterface for Clock {
    fn sleep(&self, seconds: Seconds) -> Result<(), DateError> {
        Self::sleep(self, seconds)
    }

    fn with_time_zone(&self, timezone: TimeZoneInput<'_>) -> Result<SharedClock, DateError> {
        Ok(Arc::new(Self::with_time_zone(self, timezone)?))
    }
}
