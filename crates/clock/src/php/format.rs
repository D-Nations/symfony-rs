//! `DateTimeInterface::format()`: every format character of the PHP
//! manual's table, a backslash escaping the next character, and any other
//! character copied as it is.
//!
//! Upstream: none

use std::fmt::Write;

use super::{
    civil::{Fields, days_from_civil, days_in_month, is_leap, iso_week, weekday},
    datetime::DateTimeImmutable,
    zone::offset_text,
};

pub(crate) const DAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

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

fn month_name(month: i64) -> &'static str {
    usize::try_from(month - 1)
        .ok()
        .and_then(|i| MONTH_NAMES.get(i))
        .copied()
        .unwrap_or("")
}

fn day_name(days: i64) -> &'static str {
    usize::try_from(weekday(days))
        .ok()
        .and_then(|i| DAY_NAMES.get(i))
        .copied()
        .unwrap_or("")
}

/// `Y`: at least four digits, `-` before years BCE.
fn year_text(year: i64) -> String {
    let sign = if year < 0 { "-" } else { "" };
    format!("{sign}{:04}", year.unsigned_abs())
}

fn suffix(day: i64) -> &'static str {
    match day {
        1 | 21 | 31 => "st",
        2 | 22 => "nd",
        3 | 23 => "rd",
        _ => "th",
    }
}

pub(crate) fn format(date: &DateTimeImmutable, format: &str) -> String {
    let info = date.zone_info();
    let local = date.local_seconds(&info);
    let f = Fields::from_local(local, date.get_microsecond());
    let days = days_from_civil(f.year, f.month, f.day);
    let hour12 = match f.hour % 12 {
        0 => 12,
        h => h,
    };
    // `p` writes `Z` for offset 0 in UTC, `Z` and offset zones, and
    // `+00:00` for GMT and UCT.
    let zulu =
        date.zone().timezone_type() == 1 || matches!(info.abbreviation.as_str(), "UTC" | "Z");
    let mut out = String::new();
    let mut chars = format.chars();
    while let Some(c) = chars.next() {
        // Writing to a String cannot fail.
        let _ = match c {
            'd' => write!(out, "{:02}", f.day),
            'D' => write!(out, "{}", &day_name(days)[..3]),
            'j' => write!(out, "{}", f.day),
            'l' => write!(out, "{}", day_name(days)),
            'N' => write!(out, "{}", (weekday(days) + 6) % 7 + 1),
            'S' => write!(out, "{}", suffix(f.day)),
            'w' => write!(out, "{}", weekday(days)),
            'z' => write!(out, "{}", days - days_from_civil(f.year, 1, 1)),
            'W' => write!(out, "{:02}", iso_week(days).1),
            'F' => write!(out, "{}", month_name(f.month)),
            'm' => write!(out, "{:02}", f.month),
            'M' => write!(out, "{}", &month_name(f.month)[..3]),
            'n' => write!(out, "{}", f.month),
            't' => write!(out, "{}", days_in_month(f.year, f.month)),
            'L' => write!(out, "{}", u8::from(is_leap(f.year))),
            'o' => write!(out, "{}", iso_week(days).0),
            'X' => {
                let sign = if f.year < 0 { '-' } else { '+' };
                write!(out, "{sign}{:04}", f.year.unsigned_abs())
            }
            'x' => {
                if f.year >= 10_000 {
                    write!(out, "+{}", f.year)
                } else {
                    write!(out, "{}", year_text(f.year))
                }
            }
            'Y' => write!(out, "{}", year_text(f.year)),
            'y' => write!(out, "{:02}", f.year % 100),
            'a' => write!(out, "{}", if f.hour >= 12 { "pm" } else { "am" }),
            'A' => write!(out, "{}", if f.hour >= 12 { "PM" } else { "AM" }),
            'B' => {
                let seconds = (i128::from(date.get_timestamp()) + 3600).rem_euclid(86_400);
                write!(out, "{:03}", seconds * 10 / 864)
            }
            'g' => write!(out, "{hour12}"),
            'G' => write!(out, "{}", f.hour),
            'h' => write!(out, "{hour12:02}"),
            'H' => write!(out, "{:02}", f.hour),
            'i' => write!(out, "{:02}", f.minute),
            's' => write!(out, "{:02}", f.second),
            'u' => write!(out, "{:06}", f.microsecond),
            'v' => write!(out, "{:03}", f.microsecond / 1000),
            'e' => write!(out, "{}", date.zone().get_name()),
            'I' => write!(out, "{}", u8::from(info.dst)),
            'O' => write!(out, "{}", offset_text(info.offset, false)),
            'P' | 'p' if 'P' == c || info.offset != 0 || !zulu => {
                write!(out, "{}", offset_text(info.offset, true))
            }
            'p' => write!(out, "Z"),
            'T' => write!(out, "{}", info.abbreviation),
            'Z' => write!(out, "{}", info.offset),
            // `c` and `r` print the year as `%04d`: `-001`.
            'c' => write!(out, "{:04}{}", f.year, self::format(date, "-m-d\\TH:i:sP")),
            'r' => write!(
                out,
                "{}{:04}{}",
                self::format(date, "D, d M "),
                f.year,
                self::format(date, " H:i:s O")
            ),
            'U' => write!(out, "{}", date.get_timestamp()),
            '\\' => match chars.next() {
                Some(next) => write!(out, "{next}"),
                None => write!(out, "\\"),
            },
            other => write!(out, "{other}"),
        };
    }
    out
}
