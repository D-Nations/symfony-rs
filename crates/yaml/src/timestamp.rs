//! YAML timestamps: `Inline::getTimestampRegex()` and what PHP's
//! `new \DateTimeImmutable($scalar, new \DateTimeZone('UTC'))` makes of
//! the strings it matches.
//!
//! PHP's date parser is not ported; its behaviour on exactly these shapes
//! was recorded (`oracle/recorders/yaml_timestamps.php`, 1,200+ inputs) and
//! reduced to these rules, which all recorded cases follow:
//! - rejected: month above 12, day above 31, hour above 24, minute above
//!   59, second above 60, offset minutes above 59;
//! - rejected: an upper-case `T` separator with a one-digit month, day or
//!   hour when a non-empty fraction follows (`t` and spaces are fine);
//! - otherwise accepted, rolling over out-of-range fields (day 0, month 0,
//!   February 30, hour 24, second 60); fractions keep 6 digits
//!   (truncated); `Z` is the zone "Z", `±H`, `±HH` and `±HH:MM` a fixed
//!   offset (any hour up to 99), no zone UTC.
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/yaml Inline.php

use std::sync::LazyLock;

use php_value::datetime::{DateTimeImmutable, Fields, TimeZone};
use regex::bytes::Regex;

/// `getTimestampRegex()`, without the `x` flag's layout. PHP's `$` also
/// matches before a final line feed.
static TIMESTAMP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?-u)^(?P<year>[0-9][0-9][0-9][0-9])",
        r"-(?P<month>[0-9][0-9]?)",
        r"-(?P<day>[0-9][0-9]?)",
        r"(?:(?P<sep>[Tt]|[ \t]+)",
        r"(?P<hour>[0-9][0-9]?)",
        r":(?P<minute>[0-9][0-9])",
        r":(?P<second>[0-9][0-9])",
        r"(?:\.(?P<fraction>[0-9]*))?",
        r"(?:[ \t]*(?P<tz>Z|(?P<tz_sign>[-+])(?P<tz_hour>[0-9][0-9]?)",
        r"(?::(?P<tz_minute>[0-9][0-9]))?))?)?",
        r"\n?\z",
    ))
    .unwrap_or_else(|e| unreachable!("constant pattern: {e}"))
});

/// Whether `scalar` matches the timestamp regex.
pub(crate) fn is_timestamp(scalar: &[u8]) -> bool {
    TIMESTAMP.is_match(scalar)
}

fn number(bytes: Option<regex::bytes::Match<'_>>) -> i64 {
    bytes.map_or(0, |m| {
        m.as_bytes()
            .iter()
            .fold(0, |n, &d| n * 10 + i64::from(d - b'0'))
    })
}

/// The instant PHP makes of a timestamp, or `None` where PHP's date
/// parser throws.
pub(crate) fn parse(scalar: &[u8]) -> Option<DateTimeImmutable> {
    let c = TIMESTAMP.captures(scalar)?;
    let len = |name: &str| c.name(name).map_or(0, |m| m.len());
    let (month, day) = (number(c.name("month")), number(c.name("day")));
    if month > 12 || day > 31 {
        return None;
    }
    let (hour, minute, second) = (
        number(c.name("hour")),
        number(c.name("minute")),
        number(c.name("second")),
    );
    if c.name("hour").is_some() && (hour > 24 || minute > 59 || second > 60) {
        return None;
    }
    let fraction = c.name("fraction").map(|m| m.as_bytes()).unwrap_or(b"");
    let separator = c.name("sep").map(|m| m.as_bytes());
    if separator == Some(b"T")
        && !(len("month") == 2 && len("day") == 2 && len("hour") == 2)
        && !fraction.is_empty()
    {
        return None;
    }
    let zone = match c.name("tz").map(|m| m.as_bytes()) {
        None => TimeZone::Utc,
        Some(b"Z") => TimeZone::Zulu,
        Some(_) => {
            let tz_minute = number(c.name("tz_minute"));
            if tz_minute > 59 {
                return None;
            }
            let seconds = (number(c.name("tz_hour")) * 3600 + tz_minute * 60) as i32;
            let negative = c.name("tz_sign").is_some_and(|m| m.as_bytes() == b"-");
            TimeZone::Offset(if negative { -seconds } else { seconds })
        }
    };
    let microsecond = fraction
        .iter()
        .chain(std::iter::repeat(&b'0'))
        .take(6)
        .fold(0u32, |n, &d| n * 10 + u32::from(d - b'0'));
    let fields = Fields {
        year: number(c.name("year")),
        month,
        day,
        hour,
        minute,
        second,
        microsecond,
    };
    Some(DateTimeImmutable::from_fields(fields, zone))
}
