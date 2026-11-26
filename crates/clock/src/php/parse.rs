//! The date texts `new DateTimeImmutable($text)`, `modify($text)` and
//! `date_parse($text)` read, as far as Clock needs them. Written from the
//! PHP manual ("Supported Date and Time Formats") and from the answers
//! PHP 8.4.27 gave in the oracle (tests/data/php.json,
//! `scripts/record.sh clock`).
//!
//! Read: `now`, `today`, `midnight`, `noon`, `tomorrow`, `yesterday`;
//! ISO dates `YYYY-MM-DD` and `YYYY-MM` (day 1); times `H:MM`,
//! `HH:MM:SS` and `HH:MM:SS.frac`, also after `T`/`t` or as `tHH`,
//! `tHHMM` and `tHHMMSS`; relative offsets `[+-]N unit` with `ago`
//! (units from microseconds to years, weeks and fortnights); `@seconds`
//! with up to six decimals; the zones `UTC`, `GMT`, `Z`, IANA identifiers
//! and `+HH:MM`, `+HHMM` or `+HH` after a time.
//!
//! PHP reads much more (day and month names, ordinals, `first day of`,
//! week numbers, two-digit years, US dates, most abbreviations). Such a
//! text is refused with [`ParseError::Unsupported`], never read another
//! way. Malformed texts give PHP's message where the vectors pin it: an
//! unknown word ("The timezone could not be found in the database"), a
//! second date or time ("Double date specification", "Double time
//! specification") and a blank text ("Empty string").
//!
//! Upstream: none

use super::zone::{ABBREVIATIONS, DateTimeZone};

/// Relative offsets (`+1 day`), added field by field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Relative {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
    pub microsecond: i64,
}

impl Relative {
    fn negate(&mut self) {
        for field in [
            &mut self.year,
            &mut self.month,
            &mut self.day,
            &mut self.hour,
            &mut self.minute,
            &mut self.second,
            &mut self.microsecond,
        ] {
            *field = -*field;
        }
    }
}

/// What a date text sets; `None` is `date_parse()`'s `false`.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
pub(crate) struct Parsed {
    pub year: Option<i64>,
    pub month: Option<i64>,
    pub day: Option<i64>,
    pub hour: Option<i64>,
    pub minute: Option<i64>,
    pub second: Option<i64>,
    /// `fraction`, in microseconds.
    pub fraction: Option<i64>,
    pub relative: Relative,
    pub zone: Option<DateTimeZone>,
    /// An `@seconds` text: `modify()` takes its `+00:00` zone too.
    pub timestamp: bool,
    have_date: bool,
    have_time: bool,
    have_relative: bool,
}

/// Why a text was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParseError {
    /// PHP's first error: position, the byte there, message.
    Malformed {
        position: usize,
        character: u8,
        message: &'static str,
    },
    /// A text outside what this port reads.
    Unsupported,
}

impl ParseError {
    /// `Failed to parse time string (...) at position N (c): message`.
    pub fn message(&self, text: &str) -> Option<String> {
        match self {
            Self::Malformed {
                position,
                character,
                message,
            } => Some(format!(
                "Failed to parse time string ({text}) at position {position} ({}): {message}",
                char::from(*character)
            )),
            Self::Unsupported => None,
        }
    }
}

const NOT_FOUND: &str = "The timezone could not be found in the database";

/// Words of PHP's date formats this port does not read (from the manual,
/// the same list as php-value's `dateparse`), plus the day and month
/// names. A text with one of them is refused.
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
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
    "mon",
    "tue",
    "wed",
    "thu",
    "fri",
    "sat",
    "sun",
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
    "jan",
    "feb",
    "mar",
    "apr",
    "jun",
    "jul",
    "aug",
    "sep",
    "oct",
    "nov",
    "dec",
];

/// A relative unit: (field, multiplier).
#[derive(Clone, Copy)]
enum Unit {
    Year,
    Month,
    Day(i64),
    Hour,
    Minute,
    Second,
    Microsecond(i64),
}

fn unit(word: &[u8]) -> Option<Unit> {
    let lower = word.to_ascii_lowercase();
    Some(match lower.as_slice() {
        b"sec" | b"secs" | b"second" | b"seconds" => Unit::Second,
        b"min" | b"mins" | b"minute" | b"minutes" => Unit::Minute,
        b"hour" | b"hours" => Unit::Hour,
        b"day" | b"days" => Unit::Day(1),
        b"week" | b"weeks" => Unit::Day(7),
        b"fortnight" | b"fortnights" => Unit::Day(14),
        b"month" | b"months" => Unit::Month,
        b"year" | b"years" => Unit::Year,
        b"ms" | b"msec" | b"msecs" | b"millisecond" | b"milliseconds" => Unit::Microsecond(1000),
        b"usec" | b"usecs" | b"microsecond" | b"microseconds" | b"\xC2\xB5s" | b"\xC2\xB5sec"
        | b"\xC2\xB5secs" => Unit::Microsecond(1),
        _ => return None,
    })
}

fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t')
}

type Step = Result<(), ParseError>;

struct Parser<'a> {
    s: &'a [u8],
    pos: usize,
    out: Parsed,
}

/// Reads a date text.
pub(crate) fn parse(text: &str) -> Result<Parsed, ParseError> {
    let s = text.as_bytes();
    if s.iter().all(|&b| is_space(b)) {
        return Err(ParseError::Malformed {
            position: 0,
            character: s.first().copied().unwrap_or(b' '),
            message: "Empty string",
        });
    }
    let mut p = Parser {
        s,
        pos: 0,
        out: Parsed::default(),
    };
    if let Some(at) = s.iter().position(|&b| b == b'@') {
        p.timestamp(at)?;
        return Ok(p.out);
    }
    loop {
        while p.pos < s.len() && is_space(s[p.pos]) {
            p.pos += 1;
        }
        let Some(&c) = s.get(p.pos) else { break };
        let start = p.pos;
        match c {
            b'0'..=b'9' => p.number(start)?,
            b'+' | b'-' => p.signed(start)?,
            b't' | b'T' if p.peek(1).is_some_and(|b| b.is_ascii_digit()) => p.t_time(start)?,
            b'a'..=b'z' | b'A'..=b'Z' => p.word(start)?,
            _ => return Err(ParseError::Unsupported),
        }
    }
    Ok(p.out)
}

impl Parser<'_> {
    fn peek(&self, ahead: usize) -> Option<u8> {
        self.s.get(self.pos + ahead).copied()
    }

    fn at_end_or_space(&self) -> bool {
        self.s.get(self.pos).is_none_or(|&b| is_space(b))
    }

    fn expect_separator(&self) -> Step {
        if self.at_end_or_space() {
            Ok(())
        } else {
            Err(ParseError::Unsupported)
        }
    }

    fn digit_count(&self) -> usize {
        self.s[self.pos..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    }

    /// Reads `n` digits as a number (at most 18, so it fits).
    fn digits(&mut self, n: usize) -> Result<i64, ParseError> {
        if n == 0 || n > 18 {
            return Err(ParseError::Unsupported);
        }
        let value = self.s[self.pos..self.pos + n]
            .iter()
            .fold(0i64, |acc, b| acc * 10 + i64::from(b - b'0'));
        self.pos += n;
        Ok(value)
    }

    fn malformed(&self, position: usize, message: &'static str) -> ParseError {
        ParseError::Malformed {
            position,
            character: self.s.get(position).copied().unwrap_or(b' '),
            message,
        }
    }

    /// `@seconds[.fraction]`, alone in the text.
    fn timestamp(&mut self, at: usize) -> Step {
        let before_ok = self.s[..at].iter().all(|&b| is_space(b));
        if !before_ok {
            return Err(ParseError::Unsupported);
        }
        self.pos = at + 1;
        let negative = self.peek(0) == Some(b'-');
        if negative {
            self.pos += 1;
        }
        let n = self.digit_count();
        let seconds = self.digits(n)?;
        let mut micro = 0;
        if self.peek(0) == Some(b'.') {
            self.pos += 1;
            let n = self.digit_count();
            if n == 0 || n > 6 {
                return Err(ParseError::Unsupported);
            }
            micro = self.digits(n)? * 10i64.pow(u32::try_from(6 - n).unwrap_or(0));
        }
        if !self.s[self.pos..].iter().all(|&b| is_space(b)) {
            return Err(ParseError::Unsupported);
        }
        let sign = if negative { -1 } else { 1 };
        let out = &mut self.out;
        out.year = Some(1970);
        out.month = Some(1);
        out.day = Some(1);
        out.hour = Some(0);
        out.minute = Some(0);
        out.second = Some(0);
        out.fraction = Some(0);
        out.relative.second = sign * seconds;
        out.relative.microsecond = sign * micro;
        out.zone = Some(DateTimeZone::offset(0));
        out.timestamp = true;
        Ok(())
    }

    fn number(&mut self, start: usize) -> Step {
        let n = self.digit_count();
        let next = self.peek(n);
        if n == 4 && next == Some(b'-') && self.peek(5).is_some_and(|b| b.is_ascii_digit()) {
            return self.date(start);
        }
        if (1..=2).contains(&n) && next == Some(b':') {
            return self.time(start);
        }
        if n <= 3 {
            let value = self.digits(n)?;
            return self.relative(value);
        }
        Err(ParseError::Unsupported)
    }

    /// `YYYY-MM[-DD]`, then `T` and a time, a separator or the end.
    fn date(&mut self, start: usize) -> Step {
        if self.out.have_date {
            return Err(self.malformed(start, "Double date specification"));
        }
        let year = self.digits(4)?;
        self.pos += 1;
        let n = self.digit_count();
        if n > 2 {
            return Err(ParseError::Unsupported);
        }
        let month = self.digits(n)?;
        let mut day = 1;
        if self.peek(0) == Some(b'-') {
            self.pos += 1;
            let n = self.digit_count();
            if !(1..=2).contains(&n) {
                return Err(ParseError::Unsupported);
            }
            day = self.digits(n)?;
        }
        if month > 12 || day > 31 {
            return Err(ParseError::Unsupported);
        }
        self.out.year = Some(year);
        self.out.month = Some(month);
        self.out.day = Some(day);
        self.out.have_date = true;
        if matches!(self.peek(0), Some(b'T' | b't'))
            && self.peek(1).is_some_and(|b| b.is_ascii_digit())
        {
            let at = self.pos;
            return self.t_time(at);
        }
        self.expect_separator()
    }

    fn set_time(&mut self, start: usize, h: i64, i: i64, s: i64, fraction: Option<i64>) -> Step {
        if self.out.have_time {
            return Err(self.malformed(start, "Double time specification"));
        }
        if h > 24 || i > 59 || s > 60 {
            return Err(ParseError::Unsupported);
        }
        self.out.hour = Some(h);
        self.out.minute = Some(i);
        self.out.second = Some(s);
        self.out.fraction = fraction;
        self.out.have_time = true;
        Ok(())
    }

    /// `H:MM`, `HH:MM:SS` or `HH:MM:SS.frac`; a zone may follow at once.
    #[allow(clippy::many_single_char_names)]
    fn time(&mut self, start: usize) -> Step {
        let n = self.digit_count();
        let h = self.digits(n)?;
        self.pos += 1; // ':'
        if self.digit_count() != 2 {
            return Err(ParseError::Unsupported);
        }
        let i = self.digits(2)?;
        let mut s = 0;
        let mut fraction = 0;
        if self.peek(0) == Some(b':') {
            self.pos += 1;
            if self.digit_count() != 2 {
                return Err(ParseError::Unsupported);
            }
            s = self.digits(2)?;
            if self.peek(0) == Some(b'.') {
                self.pos += 1;
                let n = self.digit_count();
                // PHP keeps six digits and drops the rest.
                for k in 0..n.min(6) {
                    fraction = fraction * 10 + i64::from(self.s[self.pos + k] - b'0');
                }
                for _ in n.min(6)..6 {
                    fraction *= 10;
                }
                self.pos += n;
            }
        }
        self.set_time(start, h, i, s, Some(fraction))?;
        match self.peek(0) {
            None | Some(b'+' | b'-' | b'a'..=b'z' | b'A'..=b'Z') => Ok(()),
            Some(b) if is_space(b) => Ok(()),
            Some(_) => Err(ParseError::Unsupported),
        }
    }

    /// `tHH`, `tHHMM`, `tHHMMSS` or `tHH:MM...`.
    fn t_time(&mut self, start: usize) -> Step {
        self.pos += 1;
        let n = self.digit_count();
        if n == 2 && self.peek(2) == Some(b':') {
            return self.time(start);
        }
        let (h, i, s, fraction) = match n {
            2 => (self.digits(2)?, 0, 0, Some(0)),
            4 => (self.digits(2)?, self.digits(2)?, 0, None),
            6 => (self.digits(2)?, self.digits(2)?, self.digits(2)?, Some(0)),
            _ => return Err(ParseError::Unsupported),
        };
        self.set_time(start, h, i, s, fraction)?;
        self.expect_separator()
    }

    /// `[+-] N unit`, or a `+HH:MM` zone after a time.
    fn signed(&mut self, _start: usize) -> Step {
        let sign = if self.s[self.pos] == b'-' { -1 } else { 1 };
        self.pos += 1;
        let adjacent = self.peek(0).is_some_and(|b| b.is_ascii_digit());
        while self.pos < self.s.len() && is_space(self.s[self.pos]) {
            self.pos += 1;
        }
        let n = self.digit_count();
        if n == 0 {
            return Err(ParseError::Unsupported);
        }
        let mut after = self.pos + n;
        while after < self.s.len() && is_space(self.s[after]) {
            after += 1;
        }
        if self.s.get(after).is_some_and(u8::is_ascii_alphabetic)
            || self.s.get(after) == Some(&0xC2)
        {
            let value = self.digits(n)?;
            return self.relative(sign * value);
        }
        if !adjacent || !self.out.have_time || self.out.zone.is_some() {
            return Err(ParseError::Unsupported);
        }
        let (h, m) = match (n, self.peek(n)) {
            (2, Some(b':')) => {
                let h = self.digits(2)?;
                self.pos += 1;
                if self.digit_count() != 2 {
                    return Err(ParseError::Unsupported);
                }
                (h, self.digits(2)?)
            }
            (2, _) => (self.digits(2)?, 0),
            (4, _) => (self.digits(2)?, self.digits(2)?),
            _ => return Err(ParseError::Unsupported),
        };
        if m > 59 {
            return Err(ParseError::Unsupported);
        }
        self.expect_separator()?;
        let seconds =
            i32::try_from(sign * (h * 3600 + m * 60)).map_err(|_| ParseError::Unsupported)?;
        self.out.zone = Some(DateTimeZone::offset(seconds));
        Ok(())
    }

    /// The unit after a number.
    fn relative(&mut self, value: i64) -> Step {
        while self.pos < self.s.len() && is_space(self.s[self.pos]) {
            self.pos += 1;
        }
        let start = self.pos;
        while self.pos < self.s.len()
            && (self.s[self.pos].is_ascii_alphabetic() || matches!(self.s[self.pos], 0xC2 | 0xB5))
        {
            self.pos += 1;
        }
        let Some(unit) = unit(&self.s[start..self.pos]) else {
            return Err(ParseError::Unsupported);
        };
        self.expect_separator()?;
        let r = &mut self.out.relative;
        let add = |field: &mut i64, by: i64| -> Step {
            *field = by.checked_add(*field).ok_or(ParseError::Unsupported)?;
            Ok(())
        };
        let scaled = |m: i64| value.checked_mul(m).ok_or(ParseError::Unsupported);
        match unit {
            Unit::Year => add(&mut r.year, value)?,
            Unit::Month => add(&mut r.month, value)?,
            Unit::Day(m) => add(&mut r.day, scaled(m)?)?,
            Unit::Hour => add(&mut r.hour, value)?,
            Unit::Minute => add(&mut r.minute, value)?,
            Unit::Second => add(&mut r.second, value)?,
            Unit::Microsecond(m) => add(&mut r.microsecond, scaled(m)?)?,
        }
        self.out.have_relative = true;
        Ok(())
    }

    fn reset_time(&mut self, hour: i64) {
        self.out.hour = Some(hour);
        self.out.minute = Some(0);
        self.out.second = Some(0);
        self.out.fraction = Some(0);
    }

    fn word(&mut self, start: usize) -> Step {
        while self.pos < self.s.len() && self.s[self.pos].is_ascii_alphabetic() {
            self.pos += 1;
        }
        let word = &self.s[start..self.pos];
        let capitalized = word[0].is_ascii_uppercase()
            && word.len() > 1
            && word[1..].iter().all(u8::is_ascii_lowercase);
        let continues = matches!(self.peek(0), Some(b'/' | b'_' | b'-'))
            && self.peek(1).is_some_and(|b| b.is_ascii_alphabetic());
        if capitalized && continues {
            while matches!(self.peek(0), Some(b'/' | b'_' | b'-'))
                && self.peek(1).is_some_and(|b| b.is_ascii_alphabetic())
            {
                self.pos += 1;
                while self.pos < self.s.len() && self.s[self.pos].is_ascii_alphabetic() {
                    self.pos += 1;
                }
            }
            self.expect_separator()?;
            if self.out.zone.is_some() {
                return Ok(());
            }
            let name = std::str::from_utf8(&self.s[start..self.pos])
                .map_err(|_| ParseError::Unsupported)?;
            return match DateTimeZone::identifier(name) {
                Some(zone) => {
                    self.out.zone = Some(zone);
                    Ok(())
                }
                None => Err(self.malformed(start, NOT_FOUND)),
            };
        }
        self.expect_separator()?;
        let lower = word.to_ascii_lowercase();
        match lower.as_slice() {
            b"now" => {}
            b"today" | b"midnight" => {
                self.reset_time(0);
                self.out.have_time = false;
            }
            b"noon" => {
                self.reset_time(12);
                self.out.have_time = true;
            }
            b"tomorrow" | b"yesterday" => {
                self.out.relative.day += if lower == b"tomorrow" { 1 } else { -1 };
                self.out.have_relative = true;
                self.reset_time(0);
                self.out.have_time = false;
            }
            b"ago" if self.out.have_relative => self.out.relative.negate(),
            b"utc" | b"gmt" | b"z" => {
                if self.out.zone.is_none() {
                    // `UTC` as written is the identifier, other cases the
                    // abbreviation, as for `new DateTimeZone()`.
                    self.out.zone = if word == b"UTC" {
                        Some(DateTimeZone::utc())
                    } else {
                        DateTimeZone::zero_abbreviation(&String::from_utf8_lossy(&lower))
                    };
                }
            }
            _ => return self.zone_word(start, &lower),
        }
        Ok(())
    }

    /// A word that is no keyword: a zone abbreviation or identifier
    /// without `/`, like `Japan`, or an unknown word.
    fn zone_word(&mut self, start: usize, lower: &[u8]) -> Step {
        let is_keyword = |w: &[u8]| KEYWORDS.iter().any(|k| k.as_bytes() == w);
        let is_abbreviation = |w: &[u8]| ABBREVIATIONS.iter().any(|a| a.as_bytes() == w);
        // A keyword may start a longer word PHP still reads (`mondays`).
        if is_abbreviation(lower)
            || KEYWORDS
                .iter()
                .any(|k| k.len() >= 3 && lower.starts_with(k.as_bytes()))
            || is_keyword(lower)
        {
            return Err(ParseError::Unsupported);
        }
        let word = &self.s[start..self.pos];
        if word.len() > 6 {
            // PHP reads a zone of at most six letters, unless a longer
            // keyword starts the word.
            let prefix = &lower[..6];
            if KEYWORDS
                .iter()
                .any(|k| k.len() > 6 && lower.starts_with(k.as_bytes()))
                || is_abbreviation(prefix)
                || std::str::from_utf8(&word[..6])
                    .ok()
                    .and_then(DateTimeZone::identifier)
                    .is_some()
            {
                return Err(ParseError::Unsupported);
            }
            if self.out.zone.is_some() {
                return Err(ParseError::Unsupported);
            }
            return Err(self.malformed(start, NOT_FOUND));
        }
        if self.out.zone.is_some() {
            // PHP ignores a second zone.
            return Ok(());
        }
        match std::str::from_utf8(word)
            .ok()
            .and_then(DateTimeZone::identifier)
        {
            Some(zone) => {
                self.out.zone = Some(zone);
                Ok(())
            }
            None => Err(self.malformed(start, NOT_FOUND)),
        }
    }
}
