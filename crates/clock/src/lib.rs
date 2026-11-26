//! Port of the symfony/clock component. See README.md for the contract.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: none

#![forbid(unsafe_code)]

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeExample;

mod clock;
mod clock_aware_trait;
mod clock_interface;
mod date_point;
mod mock_clock;
mod monotonic_clock;
mod native_clock;
mod now;
pub use php_date as php;
pub mod test;

pub use clock::{AnyClock, Clock};
pub use clock_aware_trait::{ClockAlreadySet, ClockAware, ClockAwareTrait};
pub use clock_interface::{ClockInterface, PsrClockInterface, Seconds, SharedClock, TimeZoneInput};
pub use date_point::DatePoint;
pub use mock_clock::{MockClock, MockNow};
pub use monotonic_clock::MonotonicClock;
pub use native_clock::NativeClock;
pub use now::now;
