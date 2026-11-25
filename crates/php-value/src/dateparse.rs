//! `strtotime()` and `DateTimeImmutable::createFromFormat(DATE_RFC2822)`
//! for the texts Symfony's Cookie and `HeaderBag` read: HTTP and cookie
//! dates, ISO dates, `@timestamp`, relative offsets and a few words.
//!
//! PHP's date parser accepts far more than this. Each function answers
//! `Ok(Some(..))` for a text it reads as PHP does, `Ok(None)` where PHP
//! returns `false` for a reason this module can see (a malformed number
//! or separator, a word PHP does not know), and [`Unsupported`] for any
//! other text: such a text is refused, never approximated. Written from
//! the PHP manual ("Supported Date and Time Formats", `createFromFormat()`)
//! and checked on the answers PHP 8.4.27 gave in the oracle
//! (tests/data/dates.json, `scripts/record.sh http-foundation`): wrong week
//! days move the date forward to that day, two-digit years 0-69 are 20xx
//! and 70-99 19xx in `strtotime()` (but stay as written in
//! `createFromFormat()`), out-of-range days and times roll over where PHP
//! rolls them, and a text without a zone is read in UTC, the oracle's
//! (and PHP's built-in) default time zone.
//!
//! Upstream: none

use std::fmt;

use crate::datetime::{DAY_NAMES, DateTimeImmutable, Fields, MONTH_NAMES, TimeZone, weekday};

/// A date text outside what this module reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported(pub String);

impl fmt::Display for Unsupported {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "date text not supported by this port: {}", self.0)
    }
}

impl std::error::Error for Unsupported {}

type Parsed<T> = Result<Option<T>, Unsupported>;

fn unsupported<T>(text: &[u8]) -> Parsed<T> {
    Err(Unsupported(String::from_utf8_lossy(text).into_owned()))
}

/// The time zone abbreviations PHP knows, as
/// `array_keys(DateTimeZone::listAbbreviations())` lists them in PHP
/// 8.4.27 (recorded in the oracle). A word among them is a zone, so it is
/// not refused as unknown.
const ZONE_ABBREVIATIONS: &[&str] = &[
    "acdt", "acst", "addt", "adt", "aedt", "aest", "ahdt", "ahst", "akdt", "akst", "amt", "apt",
    "ast", "awdt", "awst", "awt", "bdst", "bdt", "bmt", "bst", "cast", "cat", "cddt", "cdt",
    "cemt", "cest", "cet", "cmt", "cpt", "cst", "cwt", "chst", "dmt", "eat", "eddt", "edt", "eest",
    "eet", "emt", "ept", "est", "ewt", "ffmt", "fmt", "gdt", "gmt", "gst", "hdt", "hkst", "hkt",
    "hmt", "hpt", "hst", "hwt", "iddt", "idt", "imt", "ist", "jdt", "jmt", "jst", "kdt", "kmt",
    "kst", "lst", "mddt", "mdst", "mdt", "mest", "met", "mmt", "mpt", "msd", "msk", "mst", "mwt",
    "nddt", "ndt", "npt", "nst", "nwt", "nzdt", "nzmt", "nzst", "pddt", "pdt", "pkst", "pkt",
    "plmt", "pmt", "ppmt", "ppt", "pst", "pwt", "qmt", "rmt", "sast", "sdmt", "sjmt", "smt", "sst",
    "tbmt", "tmt", "uct", "utc", "wast", "wat", "wemt", "west", "wet", "wib", "wita", "wit", "wmt",
    "yddt", "ydt", "ypt", "yst", "ywt", "a", "b", "c", "d", "e", "f", "g", "h", "i", "k", "l", "m",
    "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y", "z",
];

/// The words of the manual's date and relative formats: a text made of
/// one of them may mean something to PHP, so it is refused rather than
/// read as `false`.
const KEYWORDS: &[&str] = &[
    "now",
    "today",
    "midnight",
    "noon",
    "tomorrow",
    "yesterday",
    "ago",
    "back",
    "front",
    "of",
    "at",
    "next",
    "last",
    "previous",
    "this",
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "weekday",
    "weekdays",
    "ms",
    "msec",
    "msecs",
    "millisecond",
    "milliseconds",
    "usec",
    "usecs",
    "microsecond",
    "microseconds",
    "sec",
    "secs",
    "seconds",
    "min",
    "mins",
    "minute",
    "minutes",
    "hour",
    "hours",
    "day",
    "days",
    "week",
    "weeks",
    "fortnight",
    "fortnights",
    "forthnight",
    "forthnights",
    "month",
    "months",
    "year",
    "years",
    "am",
    "pm",
    "ut",
    "utc",
    "gmt",
    "z",
    "t",
    "sept",
    "tues",
    "wednes",
    "thur",
    "thurs",
    "i",
    "ii",
    "iii",
    "iv",
    "v",
    "vi",
    "vii",
    "viii",
    "ix",
    "x",
    "xi",
    "xii",
];

/// PHP's whitespace for dates: space, tab and the line breaks.
fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn trim(text: &[u8]) -> &[u8] {
    let start = text
        .iter()
        .position(|&b| !is_space(b))
        .unwrap_or(text.len());
    let end = text
        .iter()
        .rposition(|&b| !is_space(b))
        .map_or(start, |i| i + 1);
    &text[start..end]
}

/// A day name, full or three letters: its `w` number.
fn day_name(word: &[u8]) -> Option<usize> {
    DAY_NAMES.iter().position(|name| {
        word.eq_ignore_ascii_case(name.as_bytes())
            || word.eq_ignore_ascii_case(&name.as_bytes()[..3])
    })
}

/// A month name, full, three letters or `sept`: its number.
fn month_name(word: &[u8]) -> Option<i64> {
    if word.eq_ignore_ascii_case(b"sept") {
        return Some(9);
    }
    MONTH_NAMES
        .iter()
        .position(|name| {
            word.eq_ignore_ascii_case(name.as_bytes())
                || word.eq_ignore_ascii_case(&name.as_bytes()[..3])
        })
        .and_then(|i| i64::try_from(i + 1).ok())
}

/// A word PHP may read: a day or month name, a keyword or a zone.
fn known_word(word: &[u8]) -> bool {
    let lower = word.to_ascii_lowercase();
    day_name(word).is_some()
        || month_name(word).is_some()
        || KEYWORDS.iter().any(|k| k.as_bytes() == lower.as_slice())
        || ZONE_ABBREVIATIONS
            .iter()
            .any(|z| z.as_bytes() == lower.as_slice())
}

/// A cursor over the text.
struct Scanner<'a> {
    text: &'a [u8],
    at: usize,
}

impl<'a> Scanner<'a> {
    fn new(text: &'a [u8]) -> Self {
        Self { text, at: 0 }
    }

    fn done(&self) -> bool {
        self.at >= self.text.len()
    }

    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }

    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn spaces(&mut self) -> usize {
        let start = self.at;
        while self.peek().is_some_and(is_space) {
            self.at += 1;
        }
        self.at - start
    }

    /// Spaces only (`createFromFormat()`'s format space).
    fn blanks(&mut self) {
        while self.peek() == Some(b' ') {
            self.at += 1;
        }
    }

    fn word(&mut self) -> &'a [u8] {
        let start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
            self.at += 1;
        }
        &self.text[start..self.at]
    }

    /// Up to `max` digits: their text.
    fn digits(&mut self, max: usize) -> &'a [u8] {
        let start = self.at;
        while self.at - start < max && self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        &self.text[start..self.at]
    }
}

fn number(digits: &[u8]) -> i64 {
    digits.iter().fold(0i64, |n, &d| {
        n.saturating_mul(10).saturating_add(i64::from(d - b'0'))
    })
}

/// A zone after a time: `GMT`, `UTC`, `UT`, `Z` (any case) or a numeric
/// offset `+H`, `+HH`, `+HMM`, `+HHMM`, `+HH:MM`. `Err` for any other
/// word, `Ok(None)` for a malformed offset.
fn zone(scanner: &mut Scanner<'_>, text: &[u8]) -> Result<Option<TimeZone>, Unsupported> {
    match scanner.peek() {
        Some(sign @ (b'+' | b'-')) => {
            scanner.at += 1;
            let hours_and_minutes = scanner.digits(4);
            let (hours, minutes) = match hours_and_minutes.len() {
                1 | 2 if scanner.eat(b':') => {
                    let minutes = scanner.digits(2);
                    if minutes.len() != 2 {
                        return unsupported(text);
                    }
                    (number(hours_and_minutes), number(minutes))
                }
                1 | 2 => (number(hours_and_minutes), 0),
                3 => (
                    number(&hours_and_minutes[..1]),
                    number(&hours_and_minutes[1..]),
                ),
                4 => (
                    number(&hours_and_minutes[..2]),
                    number(&hours_and_minutes[2..]),
                ),
                _ => return unsupported(text),
            };
            let seconds =
                i32::try_from(hours * 3600 + minutes * 60).map_err(|_| unsupported_err(text))?;
            Ok(Some(TimeZone::Offset(if sign == b'-' {
                -seconds
            } else {
                seconds
            })))
        }
        Some(b) if b.is_ascii_alphabetic() => {
            let word = scanner.word();
            if word.eq_ignore_ascii_case(b"gmt") {
                Ok(Some(TimeZone::Gmt))
            } else if word.eq_ignore_ascii_case(b"utc") {
                Ok(Some(TimeZone::Utc))
            } else if word.eq_ignore_ascii_case(b"z") {
                Ok(Some(TimeZone::Zulu))
            } else {
                unsupported(text)
            }
        }
        _ => unsupported(text),
    }
}

fn unsupported_err(text: &[u8]) -> Unsupported {
    Unsupported(String::from_utf8_lossy(text).into_owned())
}

/// The timestamp of `fields` in `zone`, moved forward to the week day
/// `day` when one was named and the date, after out-of-range fields have
/// rolled over (24:00 is the next day), falls on another.
fn resolve(fields: Fields, zone: TimeZone, day: Option<usize>) -> i64 {
    let mut fields = DateTimeImmutable::from_fields(fields, TimeZone::Utc).local_fields();
    if let Some(wanted) = day {
        let current = weekday(fields.year, fields.month, fields.day);
        fields.day += i64::try_from((wanted + 7 - current) % 7).unwrap_or(0);
    }
    DateTimeImmutable::from_fields(fields, zone).timestamp()
}

/// `strtotime($text, $now)`, the default time zone being UTC.
pub fn strtotime(text: &[u8], now: i64) -> Parsed<i64> {
    let trimmed = trim(text);
    if trimmed.is_empty() {
        return Ok(None);
    }
    if let Some(rest) = trimmed.strip_prefix(b"@") {
        let (negative, digits) = match rest.strip_prefix(b"-") {
            Some(digits) => (true, digits),
            None => (false, rest),
        };
        if digits.is_empty() || digits.len() > 18 || !digits.iter().all(u8::is_ascii_digit) {
            return unsupported(text);
        }
        let value = number(digits);
        return Ok(Some(if negative { -value } else { value }));
    }
    if trimmed.iter().all(u8::is_ascii_alphabetic) {
        return word_alone(trimmed, now);
    }
    if let Some(parsed) = relative(trimmed, now) {
        return Ok(Some(parsed));
    }
    if trimmed.first().is_some_and(u8::is_ascii_digit) && trimmed.get(4) == Some(&b'-') {
        return iso(trimmed);
    }
    http_date(trimmed)
}

/// The wall-clock fields of `now` (UTC).
fn now_fields(now: i64) -> Fields {
    DateTimeImmutable::from_timestamp(now, 0, TimeZone::Utc).local_fields()
}

fn midnight(fields: Fields) -> Fields {
    Fields {
        hour: 0,
        minute: 0,
        second: 0,
        microsecond: 0,
        ..fields
    }
}

fn word_alone(word: &[u8], now: i64) -> Parsed<i64> {
    let lower = word.to_ascii_lowercase();
    let today = midnight(now_fields(now));
    let at = |fields: Fields| {
        Ok(Some(
            DateTimeImmutable::from_fields(fields, TimeZone::Utc).timestamp(),
        ))
    };
    match lower.as_slice() {
        b"now" | b"gmt" | b"utc" | b"z" => return Ok(Some(now)),
        b"today" | b"midnight" => return at(today),
        b"noon" => return at(Fields { hour: 12, ..today }),
        b"tomorrow" => {
            return at(Fields {
                day: today.day + 1,
                ..today
            });
        }
        b"yesterday" => {
            return at(Fields {
                day: today.day - 1,
                ..today
            });
        }
        _ => {}
    }
    if let Some(day) = day_name(word) {
        return Ok(Some(resolve(today, TimeZone::Utc, Some(day))));
    }
    if known_word(word) {
        return unsupported(word);
    }
    Ok(None)
}

/// A relative unit: what one of it adds to the date fields.
#[derive(Clone, Copy)]
enum Unit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Fortnight,
    Month,
    Year,
}

fn unit(word: &[u8]) -> Option<Unit> {
    let lower = word.to_ascii_lowercase();
    Some(match lower.as_slice() {
        b"sec" | b"secs" | b"second" | b"seconds" => Unit::Second,
        b"min" | b"mins" | b"minute" | b"minutes" => Unit::Minute,
        b"hour" | b"hours" => Unit::Hour,
        b"day" | b"days" => Unit::Day,
        b"week" | b"weeks" => Unit::Week,
        b"fortnight" | b"fortnights" | b"forthnight" | b"forthnights" => Unit::Fortnight,
        b"month" | b"months" => Unit::Month,
        b"year" | b"years" => Unit::Year,
        _ => return None,
    })
}

/// `[now] (±N unit)+ [ago]`: `None` when the text is not of that form.
fn relative(text: &[u8], now: i64) -> Option<i64> {
    let mut scanner = Scanner::new(text);
    let mut fields = now_fields(now);
    let start = scanner.at;
    let first = scanner.word();
    if !first.eq_ignore_ascii_case(b"now") {
        scanner.at = start;
    }
    let mut offsets: Vec<(Unit, i64)> = Vec::new();
    loop {
        scanner.spaces();
        if scanner.done() {
            break;
        }
        let negative = match scanner.peek() {
            Some(b'+') => {
                scanner.at += 1;
                false
            }
            Some(b'-') => {
                scanner.at += 1;
                true
            }
            _ => false,
        };
        scanner.spaces();
        let digits = scanner.digits(9);
        if digits.is_empty() {
            let word = scanner.word();
            if word.eq_ignore_ascii_case(b"ago") && !negative && !offsets.is_empty() {
                scanner.spaces();
                if !scanner.done() {
                    return None;
                }
                for offset in &mut offsets {
                    offset.1 = -offset.1;
                }
                break;
            }
            return None;
        }
        scanner.spaces();
        let unit = unit(scanner.word())?;
        let amount = number(digits);
        offsets.push((unit, if negative { -amount } else { amount }));
    }
    if offsets.is_empty() {
        return None;
    }
    for (unit, amount) in offsets {
        match unit {
            Unit::Second => fields.second += amount,
            Unit::Minute => fields.minute += amount,
            Unit::Hour => fields.hour += amount,
            Unit::Day => fields.day += amount,
            Unit::Week => fields.day += 7 * amount,
            Unit::Fortnight => fields.day += 14 * amount,
            Unit::Month => fields.month += amount,
            Unit::Year => fields.year += amount,
        }
    }
    Some(DateTimeImmutable::from_fields(fields, TimeZone::Utc).timestamp())
}

/// `HH:MM[:SS]` after a date: `Ok(None)` for values `strtotime()`
/// rejects (hour above 24, minute above 59, second above 60).
fn time(scanner: &mut Scanner<'_>, text: &[u8]) -> Result<Option<(i64, i64, i64)>, Unsupported> {
    let hour = scanner.digits(2);
    if hour.is_empty() || !scanner.eat(b':') {
        return Err(unsupported_err(text));
    }
    let minute = scanner.digits(2);
    if minute.len() != 2 {
        return Err(unsupported_err(text));
    }
    let mut second = 0;
    if scanner.eat(b':') {
        let digits = scanner.digits(2);
        if digits.len() != 2
            || scanner
                .peek()
                .is_some_and(|b| b.is_ascii_digit() || b == b'.')
        {
            return Err(unsupported_err(text));
        }
        second = number(digits);
    }
    let (hour, minute) = (number(hour), number(minute));
    if hour > 24 || minute > 59 || second > 60 {
        return Ok(None);
    }
    Ok(Some((hour, minute, second)))
}

/// `strtotime()`'s two-digit years.
fn full_year(digits: &[u8]) -> Option<i64> {
    let year = number(digits);
    match digits.len() {
        2 if year < 70 => Some(2000 + year),
        2 => Some(1900 + year),
        4 => Some(year),
        _ => None,
    }
}

/// The time and zone that may follow a date, then the end of the text.
fn time_and_zone(
    scanner: &mut Scanner<'_>,
    text: &[u8],
    date: Fields,
    day: Option<usize>,
) -> Parsed<i64> {
    let mut fields = date;
    let mut zone_found = TimeZone::Utc;
    if scanner.spaces() > 0 && !scanner.done() {
        match time(scanner, text)? {
            Some((hour, minute, second)) => {
                fields.hour = hour;
                fields.minute = minute;
                fields.second = second;
            }
            None => return Ok(None),
        }
        let spaced = scanner.spaces() > 0 && !scanner.done();
        if spaced || matches!(scanner.peek(), Some(b'+' | b'-')) {
            zone_found = match zone(scanner, text)? {
                Some(zone) => zone,
                None => return Ok(None),
            };
        }
    }
    if !scanner.done() {
        return unsupported(text);
    }
    if fields.day > 31 {
        return Ok(None);
    }
    Ok(Some(resolve(fields, zone_found, day)))
}

/// `[Day[,]] DD(-| )Mon(-| )YY[YY] [HH:MM[:SS] [zone]]` (RFC 1123, RFC
/// 850, cookie dates) and `Day Mon D HH:MM:SS YYYY` (asctime).
fn http_date(text: &[u8]) -> Parsed<i64> {
    let mut scanner = Scanner::new(text);
    let mut day = None;
    if scanner.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
        let word = scanner.word();
        let Some(found) = day_name(word) else {
            return unsupported(text);
        };
        day = Some(found);
        scanner.eat(b',');
        scanner.spaces();
        if scanner.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
            return asctime(&mut scanner, text, day);
        }
    }
    let day_of_month = scanner.digits(2);
    if day_of_month.is_empty() {
        return unsupported(text);
    }
    let separator = scanner.peek();
    if !(scanner.eat(b'-') || scanner.spaces() > 0) {
        return unsupported(text);
    }
    let Some(month) = month_name(scanner.word()) else {
        return unsupported(text);
    };
    match separator {
        Some(b'-') if scanner.eat(b'-') => {}
        Some(b'-') => return unsupported(text),
        _ => {
            if scanner.spaces() == 0 {
                return unsupported(text);
            }
        }
    }
    let year_digits = scanner.digits(5);
    if scanner.peek().is_some_and(|b| b.is_ascii_digit()) {
        return unsupported(text);
    }
    let Some(year) = full_year(year_digits) else {
        return unsupported(text);
    };
    let date = Fields {
        year,
        month,
        day: number(day_of_month),
        ..Fields::default()
    };
    time_and_zone(&mut scanner, text, date, day)
}

/// `Mon D HH:MM:SS YYYY`, after the day name.
fn asctime(scanner: &mut Scanner<'_>, text: &[u8], day: Option<usize>) -> Parsed<i64> {
    let Some(month) = month_name(scanner.word()) else {
        return unsupported(text);
    };
    if scanner.spaces() == 0 {
        return unsupported(text);
    }
    let day_of_month = scanner.digits(2);
    if day_of_month.is_empty() || scanner.spaces() == 0 {
        return unsupported(text);
    }
    let Some((hour, minute, second)) = time(scanner, text)? else {
        return Ok(None);
    };
    if scanner.spaces() == 0 {
        return unsupported(text);
    }
    let year = scanner.digits(4);
    if year.len() != 4 || !scanner.done() {
        return unsupported(text);
    }
    let fields = Fields {
        year: number(year),
        month,
        day: number(day_of_month),
        hour,
        minute,
        second,
        microsecond: 0,
    };
    if fields.day > 31 {
        return Ok(None);
    }
    Ok(Some(resolve(fields, TimeZone::Utc, day)))
}

/// `YYYY-MM-DD[(T| )HH:MM[:SS][zone]]` with a month 1-12 and a day 1-31.
fn iso(text: &[u8]) -> Parsed<i64> {
    let mut scanner = Scanner::new(text);
    let year = scanner.digits(4);
    if year.len() != 4 || !scanner.eat(b'-') {
        return unsupported(text);
    }
    let month = scanner.digits(2);
    if month.len() != 2 || !scanner.eat(b'-') {
        return unsupported(text);
    }
    let day = scanner.digits(2);
    if day.len() != 2 || scanner.peek().is_some_and(|b| b.is_ascii_digit()) {
        return unsupported(text);
    }
    let (month, day_number) = (number(month), number(day));
    if !(1..=12).contains(&month) || !(1..=31).contains(&day_number) {
        return unsupported(text);
    }
    let mut fields = Fields {
        year: number(year),
        month,
        day: day_number,
        ..Fields::default()
    };
    let mut zone_found = TimeZone::Utc;
    if scanner.eat(b'T') || scanner.eat(b't') || scanner.spaces() > 0 {
        let Some((hour, minute, second)) = time(&mut scanner, text)? else {
            return Ok(None);
        };
        fields.hour = hour;
        fields.minute = minute;
        fields.second = second;
        if !scanner.done() {
            zone_found = match zone(&mut scanner, text)? {
                Some(zone) => zone,
                None => return Ok(None),
            };
        }
    }
    if !scanner.done() {
        return unsupported(text);
    }
    Ok(Some(resolve(fields, zone_found, None)))
}

/// `DateTimeImmutable::createFromFormat(DATE_RFC2822, $text)`, that is
/// the format `D, d M Y H:i:s O`: a day name (moving the date forward to
/// that day), a day of one or two digits, a month name, a year of one to
/// four digits taken as written, an hour of one or two digits, minutes
/// and seconds of two, then a zone. Days, hours, minutes and seconds out
/// of range roll over. A format space matches any run of spaces,
/// including none.
pub fn create_from_format_rfc2822(text: &[u8]) -> Parsed<DateTimeImmutable> {
    let mut scanner = Scanner::new(text);
    let word = scanner.word();
    if word.is_empty() {
        return Ok(None);
    }
    let Some(day) = day_name(word) else {
        // `D` finds no day in a word PHP does not know; a word its date
        // parser knows (a month, a keyword, a zone) is refused.
        if known_word(word) {
            return unsupported(text);
        }
        return Ok(None);
    };
    if !scanner.eat(b',') {
        return Ok(None);
    }
    scanner.blanks();
    let day_of_month = scanner.digits(2);
    if day_of_month.is_empty() {
        return Ok(None);
    }
    scanner.blanks();
    let word = scanner.word();
    if word.is_empty() {
        return Ok(None);
    }
    let Some(month) = month_name(word) else {
        return unsupported(text);
    };
    scanner.blanks();
    let year = scanner.digits(4);
    if year.is_empty() {
        return Ok(None);
    }
    scanner.blanks();
    let hour = scanner.digits(2);
    if hour.is_empty() || !scanner.eat(b':') {
        return Ok(None);
    }
    let minute = scanner.digits(2);
    if minute.len() != 2 || !scanner.eat(b':') {
        return Ok(None);
    }
    let second = scanner.digits(2);
    if second.len() != 2 {
        return Ok(None);
    }
    scanner.blanks();
    if scanner.done() {
        return Ok(None);
    }
    let Some(zone) = zone(&mut scanner, text)? else {
        return Ok(None);
    };
    if !scanner.done() {
        // Trailing whitespace is an error; anything else may continue a
        // zone PHP reads (`GMT+2`, `Europe/Paris`).
        if scanner.text[scanner.at..].iter().all(|&b| is_space(b)) {
            return Ok(None);
        }
        return unsupported(text);
    }
    let fields = Fields {
        year: number(year),
        month,
        day: number(day_of_month),
        hour: number(hour),
        minute: number(minute),
        second: number(second),
        microsecond: 0,
    };
    let timestamp = resolve(fields, zone, Some(day));
    Ok(Some(DateTimeImmutable::from_timestamp(timestamp, 0, zone)))
}
