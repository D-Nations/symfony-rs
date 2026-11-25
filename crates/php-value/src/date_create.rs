//! `DateTime(Immutable)::createFromFormat($format, $text, $zone)` and
//! `new DateTime(Immutable)($text, $zone)`, for the formats and texts
//! that database drivers give and Doctrine DBAL reads.
//!
//! As in [`crate::dateparse`], each function answers what PHP answers for
//! the texts it knows, and [`Unsupported`] for any other: such a text is
//! refused, never approximated. Written from the PHP manual
//! (`DateTimeImmutable::createFromFormat()`, "Supported Date and Time
//! Formats") and checked on the answers PHP 8.4.27 gave in the oracle
//! (doctrine-dbal's tests/data/php.json, `oracle/recorders/doctrine_dbal.php`,
//! keys `create_from_format` and `date_create`). The default time zone is
//! UTC, as in the oracle.
//!
//! `createFromFormat()` reads the format characters `d j m n Y H G i s u
//! v T e O P`, a leading `!`, a trailing `|`, `\` escapes, the separators
//! `; : / . , - ( )` and spaces (a format space matches any run of spaces
//! and tabs, none included). Without `!` or `|`, fields the format does not
//! set come from the current time, except that once an hour, minute or
//! second is read the others are 0, and microseconds are 0 unless read.
//! Out-of-range values roll over (month 13 is January of the next year).
//!
//! The constructor reads `now` (or nothing), and a date `YYYY-M-D` or
//! `YYYY/M/D` with an optional time `H:M[:S[.fraction]]` (after spaces or
//! `T`) and zone (`Z`, `UTC`, `GMT`, `±HH`, `±HHMM`, `±HH:MM`). A text that
//! starts with a word PHP does not know (not a day, month, keyword or zone
//! abbreviation) is PHP's `DateMalformedStringException` "The timezone could
//! not be found in the database" at position 0.
//!
//! Upstream: none

use crate::{
    dateparse::{Unsupported, known_word},
    datetime::{DateTimeImmutable, Fields, TimeZone},
    string::PhpString,
};

/// What the constructor gives: the date, or the message of the
/// `DateMalformedStringException` it throws.
pub type Created = Result<Result<DateTimeImmutable, PhpString>, Unsupported>;

fn unsupported(text: &[u8]) -> Unsupported {
    Unsupported(String::from_utf8_lossy(text).into_owned())
}

/// A cursor over the text.
struct Cursor<'a> {
    text: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
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

    fn done(&self) -> bool {
        self.at >= self.text.len()
    }

    /// Up to `max` digits.
    fn digits(&mut self, max: usize) -> &'a [u8] {
        let start = self.at;
        while self.at - start < max && self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        &self.text[start..self.at]
    }

    fn blanks(&mut self) -> usize {
        let start = self.at;
        while matches!(self.peek(), Some(b' ' | b'\t')) {
            self.at += 1;
        }
        self.at - start
    }

    fn word(&mut self) -> &'a [u8] {
        let start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_alphabetic()) {
            self.at += 1;
        }
        &self.text[start..self.at]
    }
}

fn number(digits: &[u8]) -> i64 {
    digits
        .iter()
        .fold(0, |n: i64, &d| n * 10 + i64::from(d - b'0'))
}

/// A fraction of a second's digits as microseconds (the first six).
fn microseconds(digits: &[u8]) -> i64 {
    let mut value = 0;
    for i in 0..6 {
        value = value * 10 + digits.get(i).map_or(0, |&d| i64::from(d - b'0'));
    }
    value
}

/// A zone: `Z`, `UTC`, `GMT` (any case), `±HH`, `±HHMM` or `±HH:MM`.
/// `Err` for any other text, which may be a zone PHP knows.
fn zone(cursor: &mut Cursor<'_>) -> Result<TimeZone, ()> {
    match cursor.peek() {
        Some(sign @ (b'+' | b'-')) => {
            cursor.at += 1;
            let hours = cursor.digits(4);
            let (hours, minutes) = match hours.len() {
                2 if cursor.eat(b':') => {
                    let minutes = cursor.digits(2);
                    if minutes.len() != 2 {
                        return Err(());
                    }
                    (number(hours), number(minutes))
                }
                2 => (number(hours), 0),
                4 => (number(&hours[..2]), number(&hours[2..])),
                _ => return Err(()),
            };
            if cursor.peek().is_some_and(|b| b.is_ascii_digit()) {
                return Err(());
            }
            let seconds = i32::try_from(hours * 3600 + minutes * 60).map_err(|_| ())?;
            Ok(TimeZone::Offset(if sign == b'-' {
                -seconds
            } else {
                seconds
            }))
        }
        Some(b) if b.is_ascii_alphabetic() => {
            let word = cursor.word();
            if matches!(cursor.peek(), Some(b'/' | b'_')) {
                return Err(());
            }
            if word.eq_ignore_ascii_case(b"z") {
                Ok(TimeZone::Zulu)
            } else if word.eq_ignore_ascii_case(b"utc") {
                Ok(TimeZone::Utc)
            } else if word.eq_ignore_ascii_case(b"gmt") {
                Ok(TimeZone::Gmt)
            } else {
                Err(())
            }
        }
        _ => Err(()),
    }
}

/// Bytes PHP's parsers treat as white space that this module does not
/// read.
fn has_unread_bytes(text: &[u8]) -> bool {
    text.iter()
        .any(|&b| matches!(b, b'\n' | b'\r' | 0x0b | 0x0c | 0) || !b.is_ascii())
}

/// The fields a format has read.
#[derive(Default)]
struct Read {
    year: Option<i64>,
    month: Option<i64>,
    day: Option<i64>,
    hour: Option<i64>,
    minute: Option<i64>,
    second: Option<i64>,
    microsecond: Option<i64>,
    zone: Option<TimeZone>,
}

/// Stores a field; a field read twice is refused.
fn set(field: &mut Option<i64>, value: i64, text: &[u8]) -> Result<(), Unsupported> {
    if field.replace(value).is_some() {
        return Err(unsupported(text));
    }
    Ok(())
}

/// `createFromFormat($format, $text, $zone)`: `Ok(None)` for PHP's
/// `false`. `zone` is the `$timezone` argument, used when the text names
/// no zone (UTC when `None`).
pub fn create_from_format(
    format: &[u8],
    text: &[u8],
    zone_argument: Option<TimeZone>,
) -> Result<Option<DateTimeImmutable>, Unsupported> {
    if has_unread_bytes(text) || has_unread_bytes(format) {
        return Err(unsupported(text));
    }
    let mut read = Read::default();
    let mut reset = false;
    let mut cursor = Cursor { text, at: 0 };
    let mut i = 0;
    while let Some(&c) = format.get(i) {
        i += 1;
        match c {
            b'!' if i == 1 => reset = true,
            b'|' if i == format.len() => reset = true,
            b'\\' => {
                let Some(&literal) = format.get(i) else {
                    return Err(unsupported(text));
                };
                i += 1;
                if !cursor.eat(literal) {
                    return Ok(None);
                }
            }
            b' ' => {
                cursor.blanks();
            }
            b';' | b':' | b'/' | b'.' | b',' | b'-' | b'(' | b')' => {
                if !cursor.eat(c) {
                    return Ok(None);
                }
            }
            b'd' | b'j' | b'm' | b'n' | b'Y' | b'H' | b'G' => {
                let digits = cursor.digits(if c == b'Y' { 4 } else { 2 });
                if digits.is_empty() {
                    return Ok(None);
                }
                let field = match c {
                    b'd' | b'j' => &mut read.day,
                    b'm' | b'n' => &mut read.month,
                    b'Y' => &mut read.year,
                    _ => &mut read.hour,
                };
                set(field, number(digits), text)?;
            }
            b'i' | b's' => {
                let digits = cursor.digits(2);
                if digits.len() != 2 {
                    return Ok(None);
                }
                let field = if c == b'i' {
                    &mut read.minute
                } else {
                    &mut read.second
                };
                set(field, number(digits), text)?;
            }
            b'u' | b'v' => {
                let digits = cursor.digits(if c == b'u' { 6 } else { 3 });
                if digits.is_empty() {
                    return Ok(None);
                }
                set(&mut read.microsecond, microseconds(digits), text)?;
            }
            b'T' | b'e' | b'O' | b'P' => {
                if cursor.done() {
                    return Ok(None);
                }
                let found = zone(&mut cursor).map_err(|()| unsupported(text))?;
                if read.zone.replace(found).is_some() {
                    return Err(unsupported(text));
                }
            }
            _ => return Err(unsupported(format)),
        }
    }
    if !cursor.done() {
        return Ok(None);
    }
    resolve(&read, reset, zone_argument, text)
}

/// The date a format's fields give.
fn resolve(
    read: &Read,
    reset: bool,
    zone_argument: Option<TimeZone>,
    text: &[u8],
) -> Result<Option<DateTimeImmutable>, Unsupported> {
    let zone = read.zone.or(zone_argument).unwrap_or(TimeZone::Utc);
    let nothing_read = read.year.is_none()
        && read.month.is_none()
        && read.day.is_none()
        && read.hour.is_none()
        && read.minute.is_none()
        && read.second.is_none()
        && read.microsecond.is_none();
    let fields = if reset {
        Fields {
            year: read.year.unwrap_or(1970),
            month: read.month.unwrap_or(1),
            day: read.day.unwrap_or(1),
            hour: read.hour.unwrap_or(0),
            minute: read.minute.unwrap_or(0),
            second: read.second.unwrap_or(0),
            microsecond: 0,
        }
    } else if nothing_read {
        if read.zone.is_some() {
            return Err(unsupported(text));
        }
        return Ok(Some(DateTimeImmutable::now(zone)));
    } else {
        let now = DateTimeImmutable::now(zone).local_fields();
        let time_read = read.hour.is_some() || read.minute.is_some() || read.second.is_some();
        let time =
            |field: Option<i64>, current: i64| field.unwrap_or(if time_read { 0 } else { current });
        Fields {
            year: read.year.unwrap_or(now.year),
            month: read.month.unwrap_or(now.month),
            day: read.day.unwrap_or(now.day),
            hour: time(read.hour, now.hour),
            minute: time(read.minute, now.minute),
            second: time(read.second, now.second),
            microsecond: 0,
        }
    };
    let microsecond = u32::try_from(read.microsecond.unwrap_or(0)).unwrap_or(0);
    Ok(Some(DateTimeImmutable::from_fields(
        Fields {
            microsecond,
            ..fields
        },
        zone,
    )))
}

/// `new DateTimeImmutable($text, $zone)` (and `new DateTime(...)`, whose
/// object holds the same instant and zone).
pub fn date_create(text: &[u8], zone_argument: Option<TimeZone>) -> Created {
    if has_unread_bytes(text) {
        return Err(unsupported(text));
    }
    let default_zone = zone_argument.unwrap_or(TimeZone::Utc);
    let start = text
        .iter()
        .take_while(|&&b| matches!(b, b' ' | b'\t'))
        .count();
    let end = text.len()
        - text[start..]
            .iter()
            .rev()
            .take_while(|&&b| matches!(b, b' ' | b'\t'))
            .count();
    let trimmed = &text[start..end];
    // Blanks alone: "Empty string" (not ported).
    if trimmed.is_empty() && !text.is_empty() {
        return Err(unsupported(text));
    }
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(b"now") {
        return Ok(Ok(DateTimeImmutable::now(default_zone)));
    }
    let mut cursor = Cursor {
        text: trimmed,
        at: 0,
    };
    let Some(first) = cursor.peek() else {
        return Err(unsupported(text));
    };
    if first.is_ascii_alphabetic() {
        let word = cursor.word();
        if start != 0 || known_word(word) || matches!(cursor.peek(), Some(b'/' | b'_')) {
            return Err(unsupported(text));
        }
        let mut message = b"Failed to parse time string (".to_vec();
        message.extend_from_slice(text);
        message.extend_from_slice(b") at position 0 (");
        message.push(first);
        message.extend_from_slice(b"): The timezone could not be found in the database");
        return Ok(Err(PhpString::from(message)));
    }
    let year = cursor.digits(4);
    let separator = cursor.peek();
    if year.len() != 4 || !matches!(separator, Some(b'-' | b'/')) {
        return Err(unsupported(text));
    }
    cursor.at += 1;
    let month = cursor.digits(2);
    if month.is_empty() || !separator.is_some_and(|s| cursor.eat(s)) {
        return Err(unsupported(text));
    }
    let day = cursor.digits(2);
    if day.is_empty() || cursor.peek().is_some_and(|b| b.is_ascii_digit()) {
        return Err(unsupported(text));
    }
    let mut fields = Fields {
        year: number(year),
        month: number(month),
        day: number(day),
        ..Fields::default()
    };
    if !(1..=12).contains(&fields.month) || fields.day > 31 {
        return Err(unsupported(text));
    }
    let mut zone = None;
    if !cursor.done() {
        zone = time_and_zone(&mut cursor, &mut fields).map_err(|()| unsupported(text))?;
    }
    Ok(Ok(DateTimeImmutable::from_fields(
        fields,
        zone.unwrap_or(default_zone),
    )))
}

/// The time and zone after a date, to the end of the text: `Err` for a
/// text this module does not read.
fn time_and_zone(cursor: &mut Cursor<'_>, fields: &mut Fields) -> Result<Option<TimeZone>, ()> {
    let text = cursor.text;
    let mut found = None;
    {
        if !(cursor.eat(b'T') || cursor.eat(b't') || cursor.blanks() > 0) {
            return Err(());
        }
        let hour = cursor.digits(2);
        if hour.is_empty() || !cursor.eat(b':') {
            return Err(());
        }
        let minute = cursor.digits(2);
        if minute.is_empty() {
            return Err(());
        }
        fields.hour = number(hour);
        fields.minute = number(minute);
        if cursor.eat(b':') {
            let second = cursor.digits(2);
            if second.is_empty() {
                return Err(());
            }
            fields.second = number(second);
            if cursor.eat(b'.') {
                let start = cursor.at;
                while cursor.peek().is_some_and(|b| b.is_ascii_digit()) {
                    cursor.at += 1;
                }
                let digits = &text[start..cursor.at];
                if digits.is_empty() {
                    return Err(());
                }
                fields.microsecond = u32::try_from(microseconds(digits)).unwrap_or(0);
            }
        }
        if cursor.peek().is_some_and(|b| b.is_ascii_digit())
            || fields.hour > 24
            || fields.minute > 59
            || fields.second > 60
        {
            return Err(());
        }
        cursor.blanks();
        if !cursor.done() {
            found = Some(zone(cursor)?);
            cursor.blanks();
            if !cursor.done() {
                return Err(());
            }
        }
    }
    Ok(found)
}
