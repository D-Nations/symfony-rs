//! Port of Symfony\Component\Stopwatch\StopwatchPeriod.php.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/stopwatch StopwatchPeriod.php

use std::fmt;

use crate::php::{Decimal, Fixed2, Number, to_int};

/// A period of an event. Times are milliseconds relative to the event's
/// origin: `int` by default, `float` with more precision.
#[derive(Debug, Clone, PartialEq)]
pub struct StopwatchPeriod {
    start: Number,
    end: Number,
    memory: i64,
}

impl StopwatchPeriod {
    /// `new StopwatchPeriod($start, $end, $morePrecision)`; `memory` is
    /// `memory_get_usage(true)` at construction.
    pub fn new(start: Number, end: Number, more_precision: bool, memory: i64) -> Self {
        let cast = |time: Number| {
            if more_precision {
                Number::Float(time.as_f64())
            } else {
                Number::Int(match time {
                    Number::Int(i) => i,
                    Number::Float(f) => to_int(f),
                })
            }
        };
        Self {
            start: cast(start),
            end: cast(end),
            memory,
        }
    }

    pub fn get_start_time(&self) -> Number {
        self.start
    }

    pub fn get_end_time(&self) -> Number {
        self.end
    }

    pub fn get_duration(&self) -> Number {
        self.end - self.start
    }

    pub fn get_memory(&self) -> i64 {
        self.memory
    }
}

impl fmt::Display for StopwatchPeriod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} MiB - {} ms",
            Fixed2(self.memory as f64 / 1024.0 / 1024.0),
            Decimal(self.get_duration())
        )
    }
}
