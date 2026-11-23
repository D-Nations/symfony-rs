//! `DateTimeImmutable`, as far as PHP components need it: an instant
//! with microseconds and a time zone, built from calendar fields that may
//! be out of range (PHP rolls them over: day 0 is the previous month's
//! last day, February 30 is in March, hour 24 is the next day), and
//! formatted with the `format()` characters listed on `format`.
//!
//! The behaviour is checked against PHP 8.4.27 through Yaml's timestamp
//! vectors (crates/yaml/tests/data/timestamps.json).
//!
//! Upstream: none

use std::fmt::Write;

use crate::{array::PhpArray, value::PhpObject};

/// A `DateTimeZone`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeZone {
    /// `new DateTimeZone('UTC')`.
    Utc,
    /// The `Z` abbreviation: offset 0, named "Z".
    Zulu,
    /// The `GMT` abbreviation: offset 0, named "GMT" (what
    /// `createFromFormat()` gives for a `GMT` zone).
    Gmt,
    /// A fixed UTC offset in seconds, named like "+05:30".
    Offset(i32),
}

impl TimeZone {
    pub fn offset_seconds(self) -> i32 {
        match self {
            Self::Utc | Self::Zulu | Self::Gmt => 0,
            Self::Offset(seconds) => seconds,
        }
    }

    /// `getName()`.
    pub fn name(self) -> String {
        match self {
            Self::Utc => "UTC".to_owned(),
            Self::Zulu => "Z".to_owned(),
            Self::Gmt => "GMT".to_owned(),
            Self::Offset(seconds) => offset_text(seconds, true),
        }
    }

    /// `format('T')`.
    fn abbreviation(self) -> String {
        match self {
            Self::Utc => "UTC".to_owned(),
            Self::Zulu => "Z".to_owned(),
            Self::Gmt => "GMT".to_owned(),
            Self::Offset(seconds) => format!("GMT{}", offset_text(seconds, false)),
        }
    }
}

/// `+HH:MM` (`P`) or `+HHMM` (`O`).
fn offset_text(seconds: i32, colon: bool) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let minutes = seconds.unsigned_abs() / 60;
    let (h, m) = (minutes / 60, minutes % 60);
    if colon {
        format!("{sign}{h:02}:{m:02}")
    } else {
        format!("{sign}{h:02}{m:02}")
    }
}

/// Calendar fields, any of which may be out of range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Fields {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
    pub microsecond: u32,
}

/// An immutable instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTimeImmutable {
    /// Seconds since the Unix epoch (floor), UTC.
    timestamp: i64,
    microsecond: u32,
    zone: TimeZone,
}

/// Days since 1970-01-01 of a proleptic Gregorian date (month 1..=12).
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day number: (year, month, day).
pub(crate) fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

impl DateTimeImmutable {
    pub const CLASS: &'static str = "DateTimeImmutable";

    /// The instant whose wall-clock time in `zone` is `fields`, rolling
    /// out-of-range fields over.
    pub fn from_fields(fields: Fields, zone: TimeZone) -> Self {
        let months = fields.month - 1;
        let year = fields.year + months.div_euclid(12);
        let month = months.rem_euclid(12) + 1;
        let days = days_from_civil(year, month, 1) + fields.day - 1;
        let local = days * 86_400 + fields.hour * 3_600 + fields.minute * 60 + fields.second;
        Self {
            timestamp: local - i64::from(zone.offset_seconds()),
            microsecond: fields.microsecond,
            zone,
        }
    }

    /// `getTimestamp()` / `format('U')`.
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }

    pub fn microsecond(&self) -> u32 {
        self.microsecond
    }

    pub fn timezone(&self) -> TimeZone {
        self.zone
    }

    /// The wall-clock fields in this instant's zone.
    pub fn local_fields(&self) -> Fields {
        let local = self.timestamp + i64::from(self.zone.offset_seconds());
        let (year, month, day) = civil_from_days(local.div_euclid(86_400));
        let seconds = local.rem_euclid(86_400);
        Fields {
            year,
            month,
            day,
            hour: seconds / 3_600,
            minute: seconds % 3_600 / 60,
            second: seconds % 60,
            microsecond: self.microsecond,
        }
    }

    /// `format($format)` for the characters `Y m d j H i s u v P O Z U e
    /// T c D l M F w N` and `\` escapes; any other character is copied, as
    /// PHP copies characters that are not format characters.
    pub fn format(&self, format: &str) -> String {
        self.format_as(format, false)
    }

    /// `format()`, with `gmt` making `T` "GMT" as `gmdate()` writes it.
    fn format_as(&self, format: &str, gmt: bool) -> String {
        let f = self.local_fields();
        let weekday = weekday(f.year, f.month, f.day);
        let mut out = String::new();
        let mut chars = format.chars();
        while let Some(c) = chars.next() {
            let _ = match c {
                'Y' => {
                    if f.year < 0 {
                        write!(out, "-{:04}", -f.year)
                    } else {
                        write!(out, "{:04}", f.year)
                    }
                }
                'm' => write!(out, "{:02}", f.month),
                'd' => write!(out, "{:02}", f.day),
                'j' => write!(out, "{}", f.day),
                'D' => write!(out, "{}", &DAY_NAMES[weekday][..3]),
                'l' => write!(out, "{}", DAY_NAMES[weekday]),
                'w' => write!(out, "{weekday}"),
                'N' => write!(out, "{}", if weekday == 0 { 7 } else { weekday }),
                'M' => write!(out, "{}", &MONTH_NAMES[month_index(f.month)][..3]),
                'F' => write!(out, "{}", MONTH_NAMES[month_index(f.month)]),
                'H' => write!(out, "{:02}", f.hour),
                'i' => write!(out, "{:02}", f.minute),
                's' => write!(out, "{:02}", f.second),
                'u' => write!(out, "{:06}", f.microsecond),
                'v' => write!(out, "{:03}", f.microsecond / 1000),
                'P' => write!(out, "{}", offset_text(self.zone.offset_seconds(), true)),
                'O' => write!(out, "{}", offset_text(self.zone.offset_seconds(), false)),
                'Z' => write!(out, "{}", self.zone.offset_seconds()),
                'U' => write!(out, "{}", self.timestamp),
                'e' => write!(out, "{}", self.zone.name()),
                'T' if gmt => write!(out, "GMT"),
                'T' => write!(out, "{}", self.zone.abbreviation()),
                // ISO 8601: the year's sign counts toward its four digits
                // (-1 is "-001"), unlike `Y` ("-0001").
                'c' => write!(out, "{:04}{}", f.year, self.format_as("-m-d\\TH:i:sP", gmt)),
                '\\' => match chars.next() {
                    Some(escaped) => write!(out, "{escaped}"),
                    None => write!(out, "\\"),
                },
                other => write!(out, "{other}"),
            };
        }
        out
    }
}

/// English day names, Sunday first (`w` order).
pub(crate) const DAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

/// English month names, January first.
pub(crate) const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// A month number (1..=12) as an index into `MONTH_NAMES`.
fn month_index(month: i64) -> usize {
    usize::try_from(month - 1).unwrap_or(0).min(11)
}

/// The day of the week of a date, 0 for Sunday (`w`).
pub(crate) fn weekday(year: i64, month: i64, day: i64) -> usize {
    // 1970-01-01 was a Thursday.
    usize::try_from((days_from_civil(year, month, day) + 4).rem_euclid(7)).unwrap_or(0)
}

/// `gmdate($format, $timestamp)`: `format()` in UTC, with `T` written as
/// "GMT".
pub fn gmdate(format: &str, timestamp: i64) -> String {
    DateTimeImmutable::from_timestamp(timestamp, 0, TimeZone::Utc).format_as(format, true)
}

impl DateTimeImmutable {
    /// The instant `timestamp` (seconds since the epoch, UTC) shown in
    /// `zone`.
    pub fn from_timestamp(timestamp: i64, microsecond: u32, zone: TimeZone) -> Self {
        Self {
            timestamp,
            microsecond,
            zone,
        }
    }
}

impl PhpObject for DateTimeImmutable {
    fn class(&self) -> &str {
        Self::CLASS
    }

    /// What `var_dump()` and `(array)` show: `date`, `timezone_type`,
    /// `timezone`.
    fn properties(&self) -> PhpArray {
        let mut properties = PhpArray::new();
        properties.insert("date", self.format("Y-m-d H:i:s.u").into());
        let kind = match self.zone {
            TimeZone::Offset(_) => 1,
            TimeZone::Zulu | TimeZone::Gmt => 2,
            TimeZone::Utc => 3,
        };
        properties.insert("timezone_type", kind.into());
        properties.insert("timezone", self.zone.name().into());
        properties
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(year: i64, month: i64, day: i64, zone: TimeZone) -> DateTimeImmutable {
        DateTimeImmutable::from_fields(
            Fields {
                year,
                month,
                day,
                ..Fields::default()
            },
            zone,
        )
    }

    #[test]
    fn epoch_and_rollover() {
        assert_eq!(at(1970, 1, 1, TimeZone::Utc).timestamp(), 0);
        assert_eq!(at(2024, 2, 30, TimeZone::Utc).format("Y-m-d"), "2024-03-01");
        assert_eq!(at(1970, 0, 0, TimeZone::Utc).format("Y-m-d"), "1969-11-30");
        assert_eq!(at(0, 0, 10, TimeZone::Utc).format("Y-m-d"), "-0001-12-10");
    }

    #[test]
    fn offsets_and_formats() {
        let t = DateTimeImmutable::from_fields(
            Fields {
                year: 2001,
                month: 12,
                day: 14,
                hour: 21,
                minute: 59,
                second: 43,
                microsecond: 100_000,
            },
            TimeZone::Offset(-5 * 3600),
        );
        assert_eq!(t.timestamp(), 1_008_385_183);
        assert_eq!(t.format("c"), "2001-12-14T21:59:43-05:00");
        assert_eq!(
            t.format("Y-m-d\\TH:i:s.vP T Z e"),
            "2001-12-14T21:59:43.100-05:00 GMT-0500 -18000 -05:00"
        );
    }
}
