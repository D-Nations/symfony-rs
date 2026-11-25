//! `DateTimeZone`: an IANA identifier (type 3), a fixed UTC offset
//! (type 1) or one of the zero-offset abbreviations `UTC`, `GMT`, `UCT`
//! and `Z` (type 2). Identifiers come from the IANA time zone database
//! that jiff bundles; PHP bundles its own copy of the same database.
//!
//! Names follow PHP: `new DateTimeZone('europe/paris')` finds
//! `Europe/Paris` but keeps the name as written; `UTC` written exactly so
//! is an identifier, any other case of it an abbreviation.
//!
//! Upstream: none

use std::sync::{PoisonError, RwLock};

use jiff::tz::{AmbiguousOffset, TimeZone as JiffZone};

use super::{
    civil::{Fields, SECONDS_PER_DAY},
    error::DateError,
};

/// The time zone abbreviations PHP knows, as
/// `array_keys(DateTimeZone::listAbbreviations())` lists them in PHP
/// 8.4.27 (the same list as php-value's `dateparse`). Only `utc`, `gmt`,
/// `uct` and `z` are read; the others are refused as unsupported, never
/// looked up as identifiers.
pub(crate) const ABBREVIATIONS: &[&str] = &[
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

/// The abbreviations this port reads: all have offset 0 and no DST.
const ZERO_ABBREVIATIONS: &[&str] = &["UTC", "GMT", "UCT", "Z"];

/// `DateTimeZone`.
#[derive(Debug, Clone)]
pub struct DateTimeZone {
    kind: Kind,
}

#[derive(Debug, Clone)]
enum Kind {
    /// `timezone_type` 3.
    Id { name: String, tz: JiffZone },
    /// `timezone_type` 1, in seconds east of UTC.
    Offset(i32),
    /// `timezone_type` 2 (offset 0, no DST).
    Abbr(&'static str),
}

/// What a zone says about one instant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OffsetInfo {
    pub offset: i32,
    pub dst: bool,
    pub abbreviation: String,
}

/// How a local time that occurs twice (a fall-back hour) or never (a
/// spring-forward gap) becomes an instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hint {
    /// Copying fields without DST information (`__unserialize()`, the
    /// constructor with a zone argument): the offset in effect at the
    /// local time read as UTC.
    None,
    /// Prefer the occurrence with this DST flag (`modify()` and the
    /// setters keep the flag of the date they change; an identifier
    /// written in the date text asks for DST).
    Dst(bool),
}

/// 400 Gregorian years, after which the calendar (and a POSIX rule)
/// repeats.
const PERIOD: i64 = 146_097 * SECONDS_PER_DAY;
/// jiff answers for years -9999 to 9999; instants beyond about year 8000
/// are moved into range by whole periods.
const LIMIT: i64 = 190_000_000_000;

fn shift(seconds: i64) -> (i64, i64) {
    if seconds > LIMIT {
        let k = (seconds - LIMIT) / PERIOD + 1;
        (seconds - k * PERIOD, k * PERIOD)
    } else if seconds < -LIMIT {
        let k = (-LIMIT - seconds) / PERIOD + 1;
        (seconds + k * PERIOD, -k * PERIOD)
    } else {
        (seconds, 0)
    }
}

impl DateTimeZone {
    /// `new DateTimeZone($timezone)`.
    pub fn new(timezone: &str) -> Result<Self, DateError> {
        let bad = || {
            DateError::InvalidTimeZone(format!(
                "DateTimeZone::__construct(): Unknown or bad timezone ({timezone})"
            ))
        };
        if timezone == "UTC" {
            return Ok(Self::utc());
        }
        let first = timezone.as_bytes().first().copied();
        match first {
            None => return Err(bad()),
            Some(b'+' | b'-') => {
                return match parse_offset(timezone) {
                    Some(Ok(seconds)) => Ok(Self::offset(seconds)),
                    Some(Err(())) => Err(DateError::InvalidTimeZone(format!(
                        "DateTimeZone::__construct(): Timezone offset is out of range ({timezone})"
                    ))),
                    None => Err(DateError::unsupported("time zone offset", timezone)),
                };
            }
            Some(c) if !c.is_ascii_alphabetic() => {
                return Err(DateError::unsupported("time zone", timezone));
            }
            Some(_) => {}
        }
        let lower = timezone.to_ascii_lowercase();
        if let Some(abbr) = ZERO_ABBREVIATIONS
            .iter()
            .find(|a| a.eq_ignore_ascii_case(timezone))
        {
            return Ok(Self {
                kind: Kind::Abbr(abbr),
            });
        }
        if ABBREVIATIONS.contains(&lower.as_str()) {
            return Err(DateError::unsupported("time zone abbreviation", timezone));
        }
        Self::identifier(timezone).ok_or_else(bad)
    }

    /// An IANA identifier, found without regard to case and named as
    /// written. `None` when the database has no such zone.
    pub(crate) fn identifier(name: &str) -> Option<Self> {
        if !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'_' | b'-' | b'+'))
            || name.eq_ignore_ascii_case("Etc/Unknown")
        {
            return None;
        }
        let tz = JiffZone::get(name).ok()?;
        Some(Self {
            kind: Kind::Id {
                name: name.to_owned(),
                tz,
            },
        })
    }

    /// `new DateTimeZone('UTC')`.
    pub fn utc() -> Self {
        Self {
            kind: Kind::Id {
                name: "UTC".to_owned(),
                tz: JiffZone::UTC,
            },
        }
    }

    /// A type 1 zone (`+02:00`), as `@timestamp` and `P` give.
    pub(crate) fn offset(seconds: i32) -> Self {
        Self {
            kind: Kind::Offset(seconds),
        }
    }

    /// A type 2 zone of offset 0 (`Z`, `GMT`, `UTC`, `UCT`).
    pub(crate) fn zero_abbreviation(name: &str) -> Option<Self> {
        ZERO_ABBREVIATIONS
            .iter()
            .find(|a| a.eq_ignore_ascii_case(name))
            .map(|abbr| Self {
                kind: Kind::Abbr(abbr),
            })
    }

    /// `getName()`.
    pub fn get_name(&self) -> String {
        match &self.kind {
            Kind::Id { name, .. } => name.clone(),
            Kind::Offset(seconds) => offset_name(*seconds),
            Kind::Abbr(abbr) => (*abbr).to_owned(),
        }
    }

    /// `timezone_type` of `(array) $date`: 1 offset, 2 abbreviation, 3
    /// identifier.
    pub fn timezone_type(&self) -> u8 {
        match self.kind {
            Kind::Offset(_) => 1,
            Kind::Abbr(_) => 2,
            Kind::Id { .. } => 3,
        }
    }

    pub(crate) fn is_identifier(&self) -> bool {
        matches!(self.kind, Kind::Id { .. })
    }

    /// The offset, DST flag and abbreviation in effect at an instant.
    pub(crate) fn info(&self, sse: i64) -> OffsetInfo {
        match &self.kind {
            Kind::Offset(seconds) => OffsetInfo {
                offset: *seconds,
                dst: false,
                abbreviation: format!("GMT{}", offset_text(*seconds, false)),
            },
            Kind::Abbr(abbr) => OffsetInfo {
                offset: 0,
                dst: false,
                abbreviation: (*abbr).to_owned(),
            },
            Kind::Id { tz, .. } => {
                let (shifted, _) = shift(sse);
                match jiff::Timestamp::new(shifted, 0) {
                    Ok(ts) => {
                        let info = tz.to_offset_info(ts);
                        OffsetInfo {
                            offset: info.offset().seconds(),
                            dst: info.dst().is_dst(),
                            abbreviation: info.abbreviation().to_owned(),
                        }
                    }
                    // Unreachable after the shift; UTC keeps the value total.
                    Err(_) => OffsetInfo {
                        offset: 0,
                        dst: false,
                        abbreviation: "UTC".to_owned(),
                    },
                }
            }
        }
    }

    /// The offsets jiff gives for a local time in an identifier zone.
    fn ambiguous(&self, local: i64) -> Option<AmbiguousOffset> {
        let Kind::Id { tz, .. } = &self.kind else {
            return None;
        };
        let (shifted, _) = shift(local);
        let f = Fields::from_local(shifted, 0);
        let civil = jiff::civil::DateTime::new(
            i16::try_from(f.year).ok()?,
            i8::try_from(f.month).ok()?,
            i8::try_from(f.day).ok()?,
            i8::try_from(f.hour).ok()?,
            i8::try_from(f.minute).ok()?,
            i8::try_from(f.second).ok()?,
            0,
        )
        .ok()?;
        Some(tz.to_ambiguous_timestamp(civil).offset())
    }

    /// Whether a local time falls in a spring-forward gap.
    pub(crate) fn in_gap(&self, local: i64) -> bool {
        matches!(self.ambiguous(local), Some(AmbiguousOffset::Gap { .. }))
    }

    /// The instant (seconds since the epoch) of a local time, given as
    /// local seconds since 1970-01-01 00:00:00.
    pub(crate) fn to_instant(&self, local: i64, hint: Hint) -> i64 {
        match &self.kind {
            Kind::Offset(seconds) => return local - i64::from(*seconds),
            Kind::Abbr(_) => return local,
            Kind::Id { .. } => {}
        }
        let instant = |offset: jiff::tz::Offset| local - i64::from(offset.seconds());
        match self.ambiguous(local) {
            None => local,
            Some(AmbiguousOffset::Unambiguous { offset }) => instant(offset),
            // PHP moves a time in a gap forward by the gap's length.
            Some(AmbiguousOffset::Gap { before, .. }) => instant(before),
            Some(AmbiguousOffset::Fold { before, after }) => {
                let (early, late) = (instant(before), instant(after));
                if let Hint::Dst(dst) = hint {
                    if self.info(early).dst == dst {
                        return early;
                    }
                    if self.info(late).dst == dst {
                        return late;
                    }
                }
                if self.info(local).offset == after.seconds() {
                    late
                } else {
                    early
                }
            }
        }
    }
}

impl PartialEq for DateTimeZone {
    fn eq(&self, other: &Self) -> bool {
        self.timezone_type() == other.timezone_type() && self.get_name() == other.get_name()
    }
}

/// `+HH:MM` (`P`, type 1 names, with `:SS` when there are seconds) or
/// `+HHMM` (`O`).
pub(crate) fn offset_text(seconds: i32, colon: bool) -> String {
    let sign = if seconds < 0 { '-' } else { '+' };
    let abs = seconds.unsigned_abs();
    let (h, m) = (abs / 3600, abs / 60 % 60);
    if colon {
        format!("{sign}{h:02}:{m:02}")
    } else {
        format!("{sign}{h:02}{m:02}")
    }
}

fn offset_name(seconds: i32) -> String {
    let s = seconds.unsigned_abs() % 60;
    if s == 0 {
        offset_text(seconds, true)
    } else {
        format!("{}:{s:02}", offset_text(seconds, true))
    }
}

/// `+H`, `+HH`, `+HHMM`, `+HH:MM` or `+HH:MM:SS`: `Some(Ok(seconds))`,
/// `Some(Err(()))` when PHP says the offset is out of range, `None` for a
/// form this port does not read.
pub(crate) fn parse_offset(text: &str) -> Option<Result<i32, ()>> {
    let bytes = text.as_bytes();
    let sign = match bytes.first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let rest = &bytes[1..];
    let digits = |b: &[u8]| -> Option<i32> {
        if b.is_empty() || !b.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(b).ok()?.parse().ok()
    };
    // (hour digits, minute digits, second digits) by the text's shape.
    let (hours, minutes, seconds): (&[u8], &[u8], &[u8]) = match rest {
        [_] | [_, _] => (rest, &[], &[]),
        [_, _, _] => (&rest[..1], &rest[1..], &[]),
        [_, b':', _, _] => (&rest[..1], &rest[2..], &[]),
        [_, _, _, _] => (&rest[..2], &rest[2..], &[]),
        [_, _, b':', _, _] => (&rest[..2], &rest[3..], &[]),
        [_, _, b':', _, _, b':', _, _] => (&rest[..2], &rest[3..5], &rest[6..]),
        _ => return None,
    };
    let number = |b: &[u8]| if b.is_empty() { Some(0) } else { digits(b) };
    let (h, m, s) = (number(hours)?, number(minutes)?, number(seconds)?);
    if s > 59 {
        return None;
    }
    // Minutes past 59 carry into the hour; PHP refuses more than 99:59.
    let seconds = h * 3600 + m * 60 + s;
    if seconds > 99 * 3600 + 59 * 60 + 59 {
        return Some(Err(()));
    }
    Some(Ok(sign * seconds))
}

static DEFAULT: RwLock<Option<DateTimeZone>> = RwLock::new(None);

/// `date_default_timezone_get()`: `UTC` unless set, PHP's own default
/// without `date.timezone` in php.ini.
pub fn date_default_timezone_get() -> DateTimeZone {
    DEFAULT
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone()
        .unwrap_or_else(DateTimeZone::utc)
}

/// `date_default_timezone_set()`: only identifiers are accepted, as in
/// PHP (which warns and returns false for anything else).
pub fn date_default_timezone_set(timezone_identifier: &str) -> bool {
    let zone = if timezone_identifier == "UTC" {
        Some(DateTimeZone::utc())
    } else {
        DateTimeZone::identifier(timezone_identifier)
    };
    match zone {
        Some(zone) => {
            *DEFAULT.write().unwrap_or_else(PoisonError::into_inner) = Some(zone);
            true
        }
        None => false,
    }
}
