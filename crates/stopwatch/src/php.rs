//! The PHP built-ins symfony/stopwatch calls: `microtime(true)`,
//! `memory_get_usage(true)`, `round($x, $places)`, `(int) $x`,
//! `sprintf('%d')` and `sprintf('%.2F')`, and PHP's `int|float` result.
//!
//! None of this comes from php-src. Each is written from PHP 8.4.27's
//! observed behaviour and checked against vectors recorded from it
//! (oracle/recorders/stopwatch.php -> tests/data/php.json):
//! - `round()` rounds half away from zero, with the half-way points
//!   computed as doubles (so `round(0.285, 2)` is 0.29). This matches PHP
//!   while `|x| * 10^places < 2^52`; above that PHP's answers depend on its
//!   internal arithmetic and are not reproduced (README, "Differences");
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
/// symfony/phpunit-bridge's `ClockMock` does for `time-sensitive` tests.
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
        #[expect(
            clippy::cast_precision_loss,
            reason = "PHP's microtime(true) is a float of the seconds"
        )]
        let seconds = now.as_secs() as f64;
        seconds + f64::from(now.subsec_micros()) / 1e6
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

    /// PHP's int-to-float conversion, which rounds beyond 2^53.
    #[expect(clippy::cast_precision_loss, reason = "PHP int-to-float conversion")]
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
                .map_or(Self::Float(self.as_f64() + other.as_f64()), Self::Int),
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
                .map_or(Self::Float(self.as_f64() - other.as_f64()), Self::Int),
            (a, b) => Self::Float(a.as_f64() - b.as_f64()),
        }
    }
}

/// `round($value, $places)` in PHP's default mode (half away from zero),
/// for `$places` in `0..=22`.
///
/// The result is `n / 10^places` for the integer `n` whose half-way
/// points, computed as doubles, bracket the value:
/// `(n - 0.5) / 10^places <= |value| < (n + 0.5) / 10^places`. Matches PHP
/// 8.4 while `|value| * 10^places < 2^52` (checked on 171,025 values);
/// beyond that, where `n + 0.5` is no longer a double, it may differ.
/// Values whose scaled magnitude reaches 1e16 are returned unchanged, as
/// PHP does.
pub fn round(value: f64, places: usize) -> f64 {
    if !value.is_finite() || value == 0.0 {
        return value;
    }
    let Some(&scale) = POWERS_OF_TEN.get(places) else {
        return value;
    };
    let magnitude = value.abs();
    let mut n = (magnitude * scale).round();
    if n >= 1e16 {
        return value;
    }
    // The estimate is at most one away from the bracketing integer.
    if n > 0.0 && magnitude < (n - 0.5) / scale {
        n -= 1.0;
    } else if magnitude >= (n + 0.5) / scale {
        n += 1.0;
    }
    (n / scale).copysign(value)
}

/// Exact powers of ten as doubles.
const POWERS_OF_TEN: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// `(int) $value` for a float.
#[expect(
    clippy::cast_possible_truncation,
    reason = "PHP's (int) truncates toward zero; the range is checked first"
)]
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
