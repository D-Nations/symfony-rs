//! `DateTimeImmutable::createFromFormat()` for formats made of `Y`, `m`,
//! `n`, `d`, `j`, `H`, `G`, `i`, `s`, `u`, `v`, `U`, the separators
//! `; : / . , - ( )`, spaces and the resets `!` and `|`. Other format
//! characters are refused as unsupported.
//!
//! The rules come from the PHP manual and the answers PHP 8.4.27 gave in
//! the oracle (tests/data/php.json): a number field that meets other
//! characters records "Unexpected data found." and skips to the next
//! digit (with the field's own message when there is none); `i` and `s`
//! need two digits; a separator that does not match records "The
//! separation symbol could not be found" and does not advance; a space
//! skips any spaces and tabs; text left over is "Trailing data", a format
//! left over "Not enough data available to satisfy format". Errors are
//! keyed by position, a later one replacing an earlier one at the same
//! position, as `getLastErrors()` shows them. Invalid dates and times
//! (February 30, hour 24) roll over with a warning, as in PHP.
//!
//! Upstream: none

use std::collections::BTreeMap;

use super::{
    civil::Fields,
    datetime::DateTimeImmutable,
    error::DateError,
    zone::{DateTimeZone, Hint, date_default_timezone_get},
};

/// `DateTimeImmutable::getLastErrors()` after a failed or warned parse.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LastErrors {
    pub warning_count: usize,
    pub warnings: BTreeMap<usize, String>,
    pub error_count: usize,
    pub errors: BTreeMap<usize, String>,
}

impl LastErrors {
    fn error(&mut self, position: usize, message: &str) {
        self.error_count += 1;
        self.errors.insert(position, message.to_owned());
    }

    fn warning(&mut self, position: usize, message: &str) {
        self.warning_count += 1;
        self.warnings.insert(position, message.to_owned());
    }
}

/// Why `createFromFormat()` gave no date.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// PHP's `false`, with what `getLastErrors()` would hold.
    Failed(LastErrors),
    /// A format or value outside what this port reads.
    Date(DateError),
}

#[derive(Default)]
struct Values {
    year: Option<i64>,
    month: Option<i64>,
    day: Option<i64>,
    hour: Option<i64>,
    minute: Option<i64>,
    second: Option<i64>,
    microsecond: Option<i64>,
    timestamp: Option<i64>,
    reset: bool,
}

struct Field {
    max: usize,
    exact: bool,
    message: &'static str,
}

fn field(c: char) -> Option<Field> {
    let (max, exact, message) = match c {
        'Y' => (4, false, "A four digit year could not be found"),
        'm' | 'n' => (2, false, "A two digit month could not be found"),
        'd' | 'j' => (2, false, "A two digit day could not be found"),
        'H' | 'G' => (2, false, "A two digit hour could not be found"),
        'i' => (2, true, "A two digit minute could not be found"),
        's' => (2, true, "A two digit second could not be found"),
        'u' => (6, false, "A six digit microsecond could not be found"),
        'v' => (3, false, "A three digit millisecond could not be found"),
        _ => return None,
    };
    Some(Field {
        max,
        exact,
        message,
    })
}

#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
pub(crate) fn create_from_format(
    format: &str,
    datetime: &str,
    timezone: Option<&DateTimeZone>,
    now: (i64, u32),
) -> Result<DateTimeImmutable, FormatError> {
    let unsupported = || FormatError::Date(DateError::unsupported("date format", format));
    let s = datetime.as_bytes();
    let mut ptr = 0;
    let mut v = Values::default();
    let mut log = LastErrors::default();
    let mut chars = format.chars().peekable();
    while let Some(&c) = chars.peek() {
        if ptr >= s.len() && !matches!(c, '!' | '|') {
            break;
        }
        chars.next();
        match c {
            '!' => {
                v = Values {
                    reset: true,
                    ..Values::default()
                }
            }
            '|' => v.reset = true,
            ' ' => {
                while ptr < s.len() && matches!(s[ptr], b' ' | b'\t') {
                    ptr += 1;
                }
            }
            ';' | ':' | '/' | '.' | ',' | '-' | '(' | ')' => {
                if char::from(s[ptr]) == c {
                    ptr += 1;
                } else {
                    log.error(ptr, "The separation symbol could not be found");
                }
            }
            'U' => {
                let start = ptr;
                if !(s[ptr].is_ascii_digit() || matches!(s[ptr], b'+' | b'-')) {
                    log.error(start, "Unexpected data found.");
                    while ptr < s.len() && !s[ptr].is_ascii_digit() {
                        ptr += 1;
                    }
                    if ptr >= s.len() {
                        log.error(start, "Found unexpected data");
                        continue;
                    }
                }
                let begin = ptr;
                if matches!(s[ptr], b'+' | b'-') {
                    ptr += 1;
                }
                let n = s[ptr..].iter().take_while(|b| b.is_ascii_digit()).count();
                if n == 0 {
                    return Err(unsupported());
                }
                ptr += n;
                let number = std::str::from_utf8(&s[begin..ptr]).map_err(|_| unsupported())?;
                match number.parse::<i64>() {
                    Ok(value) => v.timestamp = Some(value),
                    Err(_) => log.error(start, "Number out of range"),
                }
            }
            c => {
                let f = field(c).ok_or_else(unsupported)?;
                let start = ptr;
                if !s[ptr].is_ascii_digit() {
                    log.error(start, "Unexpected data found.");
                    while ptr < s.len() && !s[ptr].is_ascii_digit() {
                        ptr += 1;
                    }
                    if ptr >= s.len() {
                        log.error(start, f.message);
                        continue;
                    }
                }
                let n = s[ptr..]
                    .iter()
                    .take(f.max)
                    .take_while(|b| b.is_ascii_digit())
                    .count();
                let value = s[ptr..ptr + n]
                    .iter()
                    .fold(0i64, |acc, b| acc * 10 + i64::from(b - b'0'));
                ptr += n;
                if f.exact && n != f.max {
                    log.error(start, f.message);
                    continue;
                }
                match c {
                    'Y' => v.year = Some(value),
                    'm' | 'n' => v.month = Some(value),
                    'd' | 'j' => v.day = Some(value),
                    'H' | 'G' => v.hour = Some(value),
                    'i' => v.minute = Some(value),
                    's' => v.second = Some(value),
                    // `u` and `v`: the digits are the start of six.
                    _ => {
                        v.microsecond = Some(value * 10i64.pow(u32::try_from(6 - n).unwrap_or(0)));
                    }
                }
            }
        }
    }
    if ptr < s.len() {
        log.error(ptr, "Trailing data");
    } else if chars.any(|c| !matches!(c, '!' | '|')) {
        log.error(ptr, "Not enough data available to satisfy format");
    }
    if log.error_count > 0 {
        return Err(FormatError::Failed(log));
    }
    build(&v, timezone, now, s.len(), &mut log).ok_or_else(unsupported)
}

fn build(
    v: &Values,
    timezone: Option<&DateTimeZone>,
    now: (i64, u32),
    end: usize,
    log: &mut LastErrors,
) -> Option<DateTimeImmutable> {
    if let Some(seconds) = v.timestamp {
        if v.year.is_some()
            || v.month.is_some()
            || v.day.is_some()
            || v.hour.is_some()
            || v.minute.is_some()
            || v.second.is_some()
        {
            return None;
        }
        let us = u32::try_from(v.microsecond.unwrap_or(0)).ok()?;
        return Some(DateTimeImmutable::from_parts(
            seconds,
            us,
            DateTimeZone::offset(0),
        ));
    }
    let zone = timezone.cloned().unwrap_or_else(date_default_timezone_get);
    let base = if v.reset {
        Fields {
            year: 1970,
            month: 1,
            day: 1,
            ..Fields::default()
        }
    } else {
        DateTimeImmutable::from_parts(now.0, now.1, zone.clone()).fields()
    };
    let time_given = [v.hour, v.minute, v.second]
        .iter()
        .filter(|x| x.is_some())
        .count();
    if time_given != 0 && time_given != 3 && !v.reset {
        return None;
    }
    let any = v.year.is_some()
        || v.month.is_some()
        || v.day.is_some()
        || time_given > 0
        || v.microsecond.is_some();
    let f = Fields {
        year: v.year.unwrap_or(base.year),
        month: v.month.unwrap_or(base.month),
        day: v.day.unwrap_or(base.day),
        hour: v.hour.unwrap_or(base.hour),
        minute: v.minute.unwrap_or(base.minute),
        second: v.second.unwrap_or(base.second),
        microsecond: v
            .microsecond
            .unwrap_or(if any || v.reset { 0 } else { base.microsecond }),
    };
    let valid_date = (1..=12).contains(&f.month)
        && f.day >= 1
        && f.day <= super::civil::days_in_month(f.year, f.month);
    if !valid_date {
        log.warning(end, "The parsed date was invalid");
    } else if f.hour > 23 || f.minute > 59 || f.second > 59 {
        log.warning(end, "The parsed time was invalid");
    }
    DateTimeImmutable::from_fields(f, zone, Hint::None)
}
