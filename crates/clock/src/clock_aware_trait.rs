//! Port of Symfony\Component\Clock\ClockAwareTrait.php: a trait to help
//! write time-sensitive classes.
//!
//! PHP's trait adds a `private readonly ClockInterface $clock` property;
//! a Rust trait cannot add fields, so a type holds a [`ClockAware`] and
//! returns it from [`ClockAwareTrait::clock_aware`]. Like the readonly
//! property, the clock is set once: by `set_clock()` (`#[Required]`, for
//! the container) or, at the first `now()` without one, to a
//! [`Clock`] over the global clock.
//!
//! Upstream: symfony/clock ClockAwareTrait.php

use std::sync::{Arc, OnceLock};

use crate::{clock::Clock, clock_interface::PsrClockInterface, date_point::DatePoint};

/// The `$clock` property the trait adds.
#[derive(Debug, Default)]
pub struct ClockAware {
    clock: OnceLock<Arc<dyn PsrClockInterface>>,
}

/// `Error: Cannot modify readonly property ...::$clock`: the clock was
/// already set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockAlreadySet;

/// `ClockAwareTrait`.
pub trait ClockAwareTrait {
    /// The property the trait's methods use.
    fn clock_aware(&self) -> &ClockAware;

    /// `setClock($clock)`.
    fn set_clock(&self, clock: Arc<dyn PsrClockInterface>) -> Result<(), ClockAlreadySet> {
        self.clock_aware()
            .clock
            .set(clock)
            .map_err(|_| ClockAlreadySet)
    }

    /// `now()` (protected in PHP).
    fn now(&self) -> DatePoint {
        let clock = self
            .clock_aware()
            .clock
            .get_or_init(|| Arc::new(Clock::default()));

        DatePoint::create_from_interface(&clock.now())
    }
}
