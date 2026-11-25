//! `DateInterval`: `new DateInterval($duration)` for ISO 8601 durations
//! and `format()`.
//!
//! The constructor reads `P[nY][nM][nW][nD][T[nH][nM][nS]]` (in that order,
//! at least one part, at least one after `T`, weeks added to days as PHP 8
//! does) with spaces and tabs around it; any other text is PHP's
//! `DateMalformedIntervalStringException` "Unknown or bad format (...)",
//! except texts PHP reads in other ways (`R` recurrences, `/` periods, the
//! `P0001-02-03T04:05:06` form) and numbers of more than nine digits, which
//! are refused ([`Unsupported`]). Written from the PHP manual and checked on
//! the answers PHP 8.4.27 gave in the oracle (doctrine-dbal's
//! tests/data/php.json, key `date_interval`).
//!
//! Upstream: none

use std::fmt::Write;

use crate::{array::PhpArray, dateparse::Unsupported, string::PhpString, value::PhpObject};

/// A `DateInterval` built from a duration: no `days` (`false`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DateInterval {
    pub y: i64,
    pub m: i64,
    pub d: i64,
    pub h: i64,
    pub i: i64,
    pub s: i64,
    /// Microseconds (`$f` is this over 1,000,000).
    pub microsecond: i64,
    /// `$invert`: 1 for a negative interval.
    pub invert: bool,
}

impl DateInterval {
    pub const CLASS: &'static str = "DateInterval";

    /// `new DateInterval($duration)`: the interval, or the message of
    /// the `DateMalformedIntervalStringException` PHP throws.
    pub fn new(duration: &[u8]) -> Result<Result<Self, PhpString>, Unsupported> {
        let refused = || Unsupported(String::from_utf8_lossy(duration).into_owned());
        let bad = || {
            let mut message = b"Unknown or bad format (".to_vec();
            message.extend_from_slice(duration);
            message.push(b')');
            Ok(Err(PhpString::from(message)))
        };
        if duration.iter().any(|&b| {
            b == b'/' || b == 0 || !b.is_ascii() || matches!(b, b'\n' | b'\r' | 0x0b | 0x0c)
        }) {
            return Err(refused());
        }
        let blank = |b: &u8| matches!(b, b' ' | b'\t');
        let start = duration.iter().take_while(|b| blank(b)).count();
        let end = duration.len()
            - duration[start..]
                .iter()
                .rev()
                .take_while(|b| blank(b))
                .count();
        let text = &duration[start..end];
        if text.first().is_some_and(|b| b.eq_ignore_ascii_case(&b'R')) {
            return Err(refused());
        }
        let Some(rest) = text.strip_prefix(b"P") else {
            return bad();
        };
        if rest.contains(&b'-') || rest.contains(&b':') {
            return Err(refused());
        }
        let mut interval = Self::default();
        let mut weeks = 0;
        let mut at = 0;
        let mut in_time = false;
        // The designators still allowed, in order.
        let date_order = b"YMWD";
        let time_order = b"HMS";
        let mut next = 0;
        let mut parts = 0;
        let mut time_parts = 0;
        while at < rest.len() {
            if rest[at] == b'T' && !in_time {
                in_time = true;
                next = 0;
                at += 1;
                continue;
            }
            let digits_start = at;
            while rest.get(at).is_some_and(u8::is_ascii_digit) {
                at += 1;
            }
            let digits = &rest[digits_start..at];
            if digits.len() > 9 {
                return Err(refused());
            }
            let Some(&designator) = rest.get(at) else {
                return bad();
            };
            if digits.is_empty() {
                return bad();
            }
            at += 1;
            let order: &[u8] = if in_time { time_order } else { date_order };
            let Some(position) = order[next..].iter().position(|&d| d == designator) else {
                return bad();
            };
            next += position + 1;
            let value = digits
                .iter()
                .fold(0i64, |n, &d| n * 10 + i64::from(d - b'0'));
            match (in_time, designator) {
                (false, b'Y') => interval.y = value,
                (false, b'M') => interval.m = value,
                (false, b'W') => weeks = value,
                (false, b'D') => interval.d = value,
                (true, b'H') => interval.h = value,
                (true, b'M') => interval.i = value,
                _ => interval.s = value,
            }
            parts += 1;
            if in_time {
                time_parts += 1;
            }
        }
        if parts == 0 || (in_time && time_parts == 0) {
            return bad();
        }
        interval.d += 7 * weeks;
        if interval.d > i64::from(i32::MAX) {
            return Err(refused());
        }
        Ok(Ok(interval))
    }

    /// `format($format)` for `%Y %y %M %m %D %d %H %h %I %i %S %s %F %f %R
    /// %r %a %%`; any other `%x` is copied, as PHP copies it. `%a` is
    /// "(unknown)": an interval built from a duration has no day count.
    pub fn format(&self, format: &str) -> String {
        let mut out = String::new();
        let mut chars = format.chars();
        while let Some(c) = chars.next() {
            if c != '%' {
                out.push(c);
                continue;
            }
            let _ = match chars.next() {
                Some('Y') => write!(out, "{:02}", self.y),
                Some('y') => write!(out, "{}", self.y),
                Some('M') => write!(out, "{:02}", self.m),
                Some('m') => write!(out, "{}", self.m),
                Some('D') => write!(out, "{:02}", self.d),
                Some('d') => write!(out, "{}", self.d),
                Some('H') => write!(out, "{:02}", self.h),
                Some('h') => write!(out, "{}", self.h),
                Some('I') => write!(out, "{:02}", self.i),
                Some('i') => write!(out, "{}", self.i),
                Some('S') => write!(out, "{:02}", self.s),
                Some('s') => write!(out, "{}", self.s),
                Some('F') => write!(out, "{:06}", self.microsecond),
                Some('f') => write!(out, "{}", self.microsecond),
                Some('R') => write!(out, "{}", if self.invert { '-' } else { '+' }),
                Some('r') => write!(out, "{}", if self.invert { "-" } else { "" }),
                Some('a') => write!(out, "(unknown)"),
                Some(other) if other != '%' => write!(out, "%{other}"),
                Some(_) | None => write!(out, "%"),
            };
        }
        out
    }
}

impl PhpObject for DateInterval {
    fn class(&self) -> &str {
        Self::CLASS
    }

    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("y", self.y.into());
        properties.insert("m", self.m.into());
        properties.insert("d", self.d.into());
        properties.insert("h", self.h.into());
        properties.insert("i", self.i.into());
        properties.insert("s", self.s.into());
        #[expect(clippy::cast_precision_loss, reason = "microseconds below 10^6")]
        properties.insert("f", (self.microsecond as f64 / 1_000_000.0).into());
        properties.insert("invert", i64::from(self.invert).into());
        properties.insert("days", false.into());
        properties.insert("from_string", false.into());
        properties
    }
}
