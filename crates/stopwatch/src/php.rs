//! The PHP built-ins symfony/stopwatch calls: `microtime(true)`,
//! `memory_get_usage(true)`, `round($x, $places)`, `(int) $x`,
//! `sprintf('%d')` and `sprintf('%.2F')`, and PHP's `int|float` result.
//!
//! All are checked against vectors recorded from PHP 8.4.27
//! (oracle/recorders/stopwatch.php -> tests/data/php.json). Only `round()`
//! follows php-src code, taken from its BSD-3-Clause main branch (PLAN.md
//! decision 8); the rest is written from the recorded behaviour:
//! - `round()` rounds half away from zero in double arithmetic, so above
//!   about 1e14 it can differ from rounding the shortest decimal form;
//! - `(int)` and `%d` truncate, wrap modulo 2^64 out of range, and give 0
//!   for INF and NaN;
//! - `%.2F` rounds the exact binary value, ties to even (0.125 gives
//!   "0.12"), prints `-0.0` as "0.00", and INF/NaN as "INF", "-INF", "NaN".
//!
//! These belong to the PHP value layer that PLAN.md decision 3 assigns to
//! php2rust's php-rt; they move there once that is decided.
//!
//! Upstream: none

use std::{
    fmt,
    ops::{Add, Sub},
    time::{SystemTime, UNIX_EPOCH},
};

/// What PHP asks its host for. Tests substitute a mocked clock, as
/// symfony/phpunit-bridge's ClockMock does for `time-sensitive` tests.
pub trait Host: Send + Sync {
    /// `microtime(true)`: seconds since the Unix epoch, microsecond
    /// resolution.
    fn microtime(&self) -> f64;

    /// `memory_get_usage(true)`: bytes the process holds for PHP. Rust has
    /// no such figure, so the default reports 0; a host can supply one,
    /// e.g. from a counting global allocator.
    fn memory_get_usage(&self) -> i64 {
        0
    }
}

/// The real clock; memory is reported as 0.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemHost;

impl Host for SystemHost {
    fn microtime(&self) -> f64 {
        // A clock before 1970 reads as the epoch.
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        now.as_secs() as f64 + f64::from(now.subsec_micros()) / 1e6
    }
}

/// PHP's `int|float`. Equality is PHP's `===`: `Int(0) != Float(0.0)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl Number {
    pub fn is_float(self) -> bool {
        matches!(self, Self::Float(_))
    }

    pub fn as_f64(self) -> f64 {
        match self {
            Self::Int(i) => i as f64,
            Self::Float(f) => f,
        }
    }

    /// `(int) $x`.
    pub fn to_int(self) -> i64 {
        match self {
            Self::Int(i) => i,
            Self::Float(f) => to_int(f),
        }
    }
}

/// PHP `+`: two ints stay an int unless the sum overflows.
impl Add for Number {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a
                .checked_add(b)
                .map_or(Self::Float(a as f64 + b as f64), Self::Int),
            (a, b) => Self::Float(a.as_f64() + b.as_f64()),
        }
    }
}

/// PHP `-`: two ints stay an int unless the difference overflows.
impl Sub for Number {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => a
                .checked_sub(b)
                .map_or(Self::Float(a as f64 - b as f64), Self::Int),
            (a, b) => Self::Float(a.as_f64() - b.as_f64()),
        }
    }
}

/// `round($value, $places)` in PHP's default mode (half away from zero),
/// for `$places >= 0`.
///
/// Derived from `_php_math_round()` and `php_round_helper()` in
/// ext/standard/math.c of php-src's BSD-3-Clause main branch (commit
/// b90212aa0f564ee700eec42e728f5f4aa26eb1fe), whose code is identical to
/// PHP 8.4.27's. Copyright (c) The PHP Group and Contributors; see
/// licenses/php-src-BSD-3-Clause.txt.
pub fn round(value: f64, places: usize) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    if places == 0 && value == value.trunc() {
        return value;
    }
    let exponent = pow10(places);
    // Take the integral part of value * 10^places. When the product fell
    // just short of a whole number through binary error (0.285 * 100 is
    // 28.499999999999996), the next integral divided back equals value.
    let scaled = if places > 0 {
        value * exponent
    } else {
        value / exponent
    };
    let (mut integral, next) = if value >= 0.0 {
        let integral = scaled.floor();
        (integral, integral + 1.0)
    } else {
        let integral = scaled.ceil();
        (integral, integral - 1.0)
    };
    if (if places > 0 {
        next / exponent
    } else {
        next * exponent
    }) == value
    {
        integral = next;
    }
    // Beyond double precision, rounding is pointless.
    if integral.abs() >= 1e16 {
        return value;
    }
    let edge = if places > 0 {
        ((integral + 0.5f64.copysign(integral)) / exponent).abs()
    } else {
        ((integral + 0.5f64.copysign(integral)) * exponent).abs()
    };
    if value.abs() >= edge {
        integral += 1.0f64.copysign(integral);
    }
    if places < 23 {
        return if places > 0 {
            integral / exponent
        } else {
            integral * exponent
        };
    }
    // Division is inexact here; go through the decimal string, as
    // `snprintf("%15fe%d")` and `zend_strtod()` do.
    match format!("{integral:.6}e-{places}").parse::<f64>() {
        Ok(rounded) if rounded.is_finite() => rounded,
        _ => value,
    }
}

/// `php_intpow10()`: exact for 0..=22.
fn pow10(power: usize) -> f64 {
    const POWERS: [f64; 23] = [
        1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
        1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
    ];
    POWERS
        .get(power)
        .copied()
        .unwrap_or_else(|| 10f64.powf(power as f64))
}

/// `(int) $value` for a float.
pub fn to_int(value: f64) -> i64 {
    const TWO_63: f64 = 9_223_372_036_854_775_808.0;
    const TWO_64: f64 = 18_446_744_073_709_551_616.0;
    if !value.is_finite() {
        return 0;
    }
    if (-TWO_63..TWO_63).contains(&value) {
        return value as i64;
    }
    // Beyond 2^63 every double is an integer; reduce it modulo 2^64.
    let mut wrapped = value % TWO_64;
    if wrapped >= TWO_63 {
        wrapped -= TWO_64;
    } else if wrapped < -TWO_63 {
        wrapped += TWO_64;
    }
    wrapped as i64
}

/// `sprintf('%d', $value)`.
pub struct Decimal(pub Number);

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_int())
    }
}

/// `sprintf('%.2F', $value)`.
pub struct Fixed2(pub f64);

impl fmt::Display for Fixed2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.0;
        if value.is_nan() {
            f.write_str("NaN")
        } else if value.is_infinite() {
            f.write_str(if value > 0.0 { "INF" } else { "-INF" })
        } else if value == 0.0 {
            f.write_str("0.00")
        } else {
            write!(f, "{value:.2}")
        }
    }
}
