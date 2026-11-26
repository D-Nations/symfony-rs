//! ClockAwareTraitTest.php at symfony/clock
//! 945ea3b64d6da9c1c2c354fe5c07b27455df6442. Names and assertions are
//! PHPUnit's. `assertInstanceOf(DatePoint::class, ...)` holds by type.
//!
//! Upstream: symfony/clock Tests/ClockAwareTraitTest.php
#![allow(non_snake_case)]

use std::sync::Arc;

use symfony_clock::{ClockAware, ClockAwareTrait, MockClock};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Default)]
struct ClockAwareTestImplem {
    clock: ClockAware,
}

impl ClockAwareTrait for ClockAwareTestImplem {
    fn clock_aware(&self) -> &ClockAware {
        &self.clock
    }
}

/// PHP's `round($x, 1)` for the small values compared here.
fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

#[test]
fn testTrait() -> TestResult {
    let sut = ClockAwareTestImplem::default();

    let _ = sut.now();

    let clock = Arc::new(MockClock::new("now", None)?);
    let sut = ClockAwareTestImplem::default();
    sut.set_clock(clock.clone())
        .map_err(|_| "clock already set")?;

    let ts = sut.now().get_timestamp();
    assert_eq!(clock.now(), sut.now());
    clock.sleep(1)?;
    assert_eq!(clock.now(), sut.now());
    #[allow(clippy::cast_precision_loss)]
    let elapsed = (sut.now().get_timestamp() - ts) as f64;
    assert!((1.0 - round1(elapsed)).abs() < f64::EPSILON);
    Ok(())
}
