//! StopwatchPeriodTest.php at symfony/stopwatch
//! 21c07b026905d596e8379caeb115d87aa479499d. Names and assertions are
//! PHPUnit's; each data provider is a loop over its rows.
//!
//! Upstream: symfony/stopwatch Tests/StopwatchPeriodTest.php
#![allow(non_snake_case)]
// 3.14 is upstream's test data, not an approximation of pi.
#![allow(clippy::approx_constant)]

use symfony_stopwatch::{Number, StopwatchPeriod};

use Number::{Float, Int};

fn provide_time_values() -> [(Number, bool, Number); 6] {
    [
        (Int(0), false, Int(0)),
        (Int(0), true, Float(0.0)),
        (Float(0.0), false, Int(0)),
        (Float(0.0), true, Float(0.0)),
        (Float(2.71), false, Int(2)),
        (Float(2.71), true, Float(2.71)),
    ]
}

fn provide_duration_values() -> [(Number, Number, bool, Number); 9] {
    [
        (Int(0), Int(0), false, Int(0)),
        (Int(0), Int(0), true, Float(0.0)),
        (Float(0.0), Float(0.0), false, Int(0)),
        (Float(0.0), Float(0.0), true, Float(0.0)),
        (Int(2), Float(3.14), false, Int(1)),
        (Int(2), Float(3.14), true, Float(1.1400000000000001)),
        (Int(2), Float(3.13), true, Float(1.13)),
        (Float(2.71), Float(3.14), false, Int(1)),
        (Float(2.71), Float(3.14), true, Float(0.43)),
    ]
}

#[test]
fn testGetStartTime() {
    for (start, use_more_precision, expected) in provide_time_values() {
        let period = StopwatchPeriod::new(start, start, use_more_precision, 0);
        assert_eq!(expected, period.get_start_time());
    }
}

#[test]
fn testGetEndTime() {
    for (end, use_more_precision, expected) in provide_time_values() {
        let period = StopwatchPeriod::new(end, end, use_more_precision, 0);
        assert_eq!(expected, period.get_end_time());
    }
}

#[test]
fn testGetDuration() {
    for (start, end, use_more_precision, duration) in provide_duration_values() {
        let period = StopwatchPeriod::new(start, end, use_more_precision, 0);
        let actual = period.get_duration().as_f64();
        assert!(
            (duration.as_f64() - actual).abs() <= f64::EPSILON,
            "{start:?}..{end:?}: {actual} is not {duration:?}"
        );
    }
}
