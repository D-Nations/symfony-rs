//! `DateTimeImmutable`: an instant (seconds since the epoch and
//! microseconds) in a [`DateTimeZone`], with the constructor, `modify()`,
//! the setters and the static constructors Clock calls.
//!
//! Local times are computed field by field as PHP does: out-of-range
//! fields roll over, relative offsets are added to the wall-clock fields
//! (also hours and seconds), and a local time is then placed in the zone.
//! A time in a spring-forward gap moves forward by the gap; a time in a
//! fall-back hour takes the occurrence `zone::Hint` names (`modify()` and
//! the setters keep the DST flag of the date they start from, an
//! identifier written in the text asks for DST, and the constructor with
//! a zone argument takes the offset in effect at the local time read as
//! UTC). Recorded from PHP 8.4.27 in tests/data/php.json.
//!
//! `add()`/`sub()` (`DateInterval`) are not ported.
//!
//! Upstream: none

use std::{
    cmp::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};

use super::{
    civil::{Fields, from_iso_week},
    error::DateError,
    format,
    from_format::{self, FormatError},
    parse::{ParseError, Parsed, parse},
    zone::{DateTimeZone, Hint, OffsetInfo, date_default_timezone_get},
};

/// `DateTimeImmutable`. `==` and `<` compare instants, as PHP's do.
#[derive(Debug, Clone)]
pub struct DateTimeImmutable {
    sse: i64,
    us: u32,
    zone: DateTimeZone,
}

/// The `int|float` argument of `createFromTimestamp()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Timestamp {
    Int(i64),
    Float(f64),
}

impl From<i64> for Timestamp {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for Timestamp {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

/// The system clock: seconds since the epoch and microseconds.
pub(crate) fn system_now() -> (i64, u32) {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d) => (
            i64::try_from(d.as_secs()).unwrap_or(i64::MAX),
            d.subsec_micros(),
        ),
        Err(e) => {
            let d = e.duration();
            let secs = i64::try_from(d.as_secs()).unwrap_or(i64::MAX);
            match d.subsec_micros() {
                0 => (-secs, 0),
                us => (-secs - 1, 1_000_000 - us),
            }
        }
    }
}

fn unsupported(text: &str) -> DateError {
    DateError::unsupported("date text", text)
}

fn parse_error(error: &ParseError, text: &str, prefix: &str) -> DateError {
    match error.message(text) {
        Some(message) => DateError::MalformedString(format!("{prefix}{message}")),
        None => unsupported(text),
    }
}

/// `%g`-style text of a float as PHP's argument errors print it:
/// `1.0e+20`, `9.22337e+18`, `NAN`, `-INF`.
pub(crate) fn float_text(value: f64) -> String {
    if value.is_nan() {
        return "NAN".to_owned();
    }
    if value.is_infinite() {
        return if value < 0.0 { "-INF" } else { "INF" }.to_owned();
    }
    let text = format!("{value:.5e}");
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if (-4..6).contains(&exponent) {
        let digits = usize::try_from(5 - exponent).unwrap_or(0);
        let fixed = format!("{value:.digits$}");
        let fixed = if fixed.contains('.') {
            fixed.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            fixed
        };
        return if fixed.contains('.') {
            fixed
        } else {
            format!("{fixed}.0")
        };
    }
    let mantissa = mantissa.trim_end_matches('0').trim_end_matches('.');
    let mantissa = if mantissa.contains('.') {
        mantissa.to_owned()
    } else {
        format!("{mantissa}.0")
    };
    let sign = if exponent < 0 { '-' } else { '+' };
    format!("{mantissa}e{sign}{:02}", exponent.unsigned_abs())
}

impl DateTimeImmutable {
    pub(crate) fn from_parts(sse: i64, us: u32, zone: DateTimeZone) -> Self {
        Self { sse, us, zone }
    }

    /// `new DateTimeImmutable($datetime, $timezone)`, on the system clock.
    pub fn new(datetime: &str, timezone: Option<&DateTimeZone>) -> Result<Self, DateError> {
        Self::new_at(datetime, timezone, system_now())
    }

    /// `new DateTimeImmutable()`: now, in the default zone.
    pub fn now() -> Self {
        let (sse, us) = system_now();
        Self::from_parts(sse, us, date_default_timezone_get())
    }

    /// The constructor with "now" given.
    pub(crate) fn new_at(
        datetime: &str,
        timezone: Option<&DateTimeZone>,
        now: (i64, u32),
    ) -> Result<Self, DateError> {
        let text = if datetime.is_empty() { "now" } else { datetime };
        let parsed = parse(text).map_err(|e| parse_error(&e, text, ""))?;
        let zone = parsed
            .zone
            .clone()
            .or_else(|| timezone.cloned())
            .unwrap_or_else(date_default_timezone_get);
        let hint = match &parsed.zone {
            Some(z) if z.is_identifier() => Hint::Dst(true),
            _ => Hint::None,
        };
        let base = Self::from_parts(now.0, now.1, zone.clone());
        let mut f = base.fields();
        let has_date = parsed.year.is_some() || parsed.month.is_some() || parsed.day.is_some();
        if let (Some(y), Some(m), Some(d)) = (parsed.year, parsed.month, parsed.day) {
            (f.year, f.month, f.day) = (y, m, d);
        }
        if let Some(h) = parsed.hour {
            f.hour = h;
            f.minute = parsed.minute.unwrap_or(0);
            f.second = parsed.second.unwrap_or(0);
            f.microsecond = parsed.fraction.unwrap_or(0);
        } else if has_date {
            (f.hour, f.minute, f.second, f.microsecond) = (0, 0, 0, 0);
        }
        Self::build(f, &parsed, zone, hint).ok_or_else(|| unsupported(text))
    }

    /// Adds the relative part and places the local time in the zone.
    fn build(mut f: Fields, parsed: &Parsed, zone: DateTimeZone, hint: Hint) -> Option<Self> {
        let r = parsed.relative;
        f.year = f.year.checked_add(r.year)?;
        f.month = f.month.checked_add(r.month)?;
        f.day = f.day.checked_add(r.day)?;
        f.hour = f.hour.checked_add(r.hour)?;
        f.minute = f.minute.checked_add(r.minute)?;
        f.second = f.second.checked_add(r.second)?;
        f.microsecond = f.microsecond.checked_add(r.microsecond)?;
        Self::from_fields(f, zone, hint)
    }

    /// The instant of local fields in a zone.
    pub(crate) fn from_fields(f: Fields, zone: DateTimeZone, hint: Hint) -> Option<Self> {
        let (local, us) = f.to_local()?;
        let sse = zone.to_instant(local, hint);
        Some(Self::from_parts(sse, us, zone))
    }

    pub(crate) fn zone_info(&self) -> OffsetInfo {
        self.zone.info(self.sse)
    }

    pub(crate) fn local_seconds(&self, info: &OffsetInfo) -> i64 {
        self.sse.saturating_add(i64::from(info.offset))
    }

    /// The local calendar fields.
    pub(crate) fn fields(&self) -> Fields {
        let info = self.zone_info();
        Fields::from_local(self.local_seconds(&info), self.us)
    }

    pub(crate) fn zone(&self) -> &DateTimeZone {
        &self.zone
    }

    /// `modify($modifier)`.
    pub fn modify(&self, modifier: &str) -> Result<Self, DateError> {
        let parsed = parse(modifier)
            .map_err(|e| parse_error(&e, modifier, "DateTimeImmutable::modify(): "))?;
        let info = self.zone_info();
        let mut f = Fields::from_local(self.local_seconds(&info), self.us);
        let mut zone = self.zone.clone();
        if parsed.timestamp {
            zone = DateTimeZone::offset(0);
        }
        if let (Some(y), Some(m), Some(d)) = (parsed.year, parsed.month, parsed.day) {
            (f.year, f.month, f.day) = (y, m, d);
        }
        if let Some(h) = parsed.hour {
            f.hour = h;
            f.minute = parsed.minute.unwrap_or(0);
            f.second = parsed.second.unwrap_or(0);
        }
        if let Some(us) = parsed.fraction {
            f.microsecond = us;
        }
        Self::build(f, &parsed, zone, Hint::Dst(info.dst)).ok_or_else(|| unsupported(modifier))
    }

    /// `format($format)`.
    pub fn format(&self, format: &str) -> String {
        format::format(self, format)
    }

    /// `getTimestamp()`.
    pub fn get_timestamp(&self) -> i64 {
        self.sse
    }

    /// `getMicrosecond()`.
    pub fn get_microsecond(&self) -> u32 {
        self.us
    }

    /// `getOffset()`.
    pub fn get_offset(&self) -> i32 {
        self.zone_info().offset
    }

    /// `getTimezone()`.
    pub fn get_timezone(&self) -> DateTimeZone {
        self.zone.clone()
    }

    /// `setTimezone($timezone)`: the same instant in another zone.
    #[must_use]
    pub fn set_timezone(&self, timezone: &DateTimeZone) -> Self {
        Self::from_parts(self.sse, self.us, timezone.clone())
    }

    /// `setTimestamp($timestamp)`: microseconds become 0.
    #[must_use]
    pub fn set_timestamp(&self, timestamp: i64) -> Self {
        Self::from_parts(timestamp, 0, self.zone.clone())
    }

    fn with_fields(&self, change: impl FnOnce(&mut Fields)) -> Result<Self, DateError> {
        let info = self.zone_info();
        let mut f = Fields::from_local(self.local_seconds(&info), self.us);
        change(&mut f);
        let range = || DateError::Unsupported("date out of the supported range".to_owned());
        // PHP keeps a local time set into a spring-forward gap as written,
        // with the later offset: a state this port does not represent.
        let (local, _) = f.to_local().ok_or_else(range)?;
        if self.zone.in_gap(local) {
            return Err(DateError::Unsupported(
                "a local time in a DST gap set by setTime(), setDate() or setISODate()".to_owned(),
            ));
        }
        Self::from_fields(f, self.zone.clone(), Hint::Dst(info.dst)).ok_or_else(range)
    }

    /// `setTime($hour, $minute, $second, $microsecond)`.
    pub fn set_time(
        &self,
        hour: i64,
        minute: i64,
        second: i64,
        microsecond: i64,
    ) -> Result<Self, DateError> {
        self.with_fields(|f| {
            (f.hour, f.minute, f.second, f.microsecond) = (hour, minute, second, microsecond);
        })
    }

    /// `setDate($year, $month, $day)`.
    pub fn set_date(&self, year: i64, month: i64, day: i64) -> Result<Self, DateError> {
        self.with_fields(|f| (f.year, f.month, f.day) = (year, month, day))
    }

    /// `setISODate($year, $week, $dayOfWeek)`.
    pub fn set_iso_date(&self, year: i64, week: i64, day_of_week: i64) -> Result<Self, DateError> {
        if year.unsigned_abs() > 1 << 40
            || week.unsigned_abs() > 1 << 40
            || day_of_week.unsigned_abs() > 1 << 40
        {
            return Err(DateError::Unsupported(
                "date out of the supported range".to_owned(),
            ));
        }
        let days = from_iso_week(year, week, day_of_week);
        self.with_fields(|f| (f.year, f.month, f.day) = (1970, 1, 1 + days))
    }

    /// `setMicrosecond($microsecond)` of `DateTimeImmutable`.
    pub fn set_microsecond(&self, microsecond: i64) -> Result<Self, DateError> {
        match u32::try_from(microsecond) {
            Ok(us) if us <= 999_999 => Ok(Self::from_parts(self.sse, us, self.zone.clone())),
            _ => Err(DateError::Range(format!(
                "DateTimeImmutable::setMicrosecond(): Argument #1 ($microsecond) must be between 0 and 999999, {microsecond} given"
            ))),
        }
    }

    /// `createFromTimestamp($timestamp)`: zone `+00:00`.
    pub fn create_from_timestamp(timestamp: impl Into<Timestamp>) -> Result<Self, DateError> {
        let zone = DateTimeZone::offset(0);
        match timestamp.into() {
            Timestamp::Int(seconds) => Ok(Self::from_parts(seconds, 0, zone)),
            Timestamp::Float(value) => {
                // -2^63 <= value < 2^63, the range PHP states.
                #[allow(clippy::cast_precision_loss)]
                let limit = i64::MAX as f64;
                if !value.is_finite() || value < -limit || value >= limit {
                    return Err(DateError::Range(format!(
                        "DateTimeImmutable::createFromTimestamp(): Argument #1 ($timestamp) must be a finite number between -9223372036854775808 and 9223372036854775807.999999, {} given",
                        float_text(value)
                    )));
                }
                let floor = value.floor();
                #[allow(clippy::cast_possible_truncation)]
                let mut seconds = floor as i64;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let mut us = ((value - floor) * 1e6).round() as u32;
                if us >= 1_000_000 {
                    seconds = seconds.saturating_add(1);
                    us -= 1_000_000;
                }
                Ok(Self::from_parts(seconds, us, zone))
            }
        }
    }

    /// `createFromFormat($format, $datetime, $timezone)`. PHP's `false`
    /// is [`FormatError::Failed`], with what `getLastErrors()` holds.
    pub fn create_from_format(
        format: &str,
        datetime: &str,
        timezone: Option<&DateTimeZone>,
    ) -> Result<Self, FormatError> {
        from_format::create_from_format(format, datetime, timezone, system_now())
    }

    /// `createFromInterface($object)`.
    pub fn create_from_interface(object: &Self) -> Self {
        object.clone()
    }

    /// `__unserialize((array) $date)`: the local time and zone copied
    /// without DST information (see `Hint::None`).
    #[must_use]
    pub fn copy_through_local_time(&self) -> Self {
        Self::from_fields(self.fields(), self.zone.clone(), Hint::None)
            .unwrap_or_else(|| self.clone())
    }
}

impl PartialEq for DateTimeImmutable {
    fn eq(&self, other: &Self) -> bool {
        (self.sse, self.us) == (other.sse, other.us)
    }
}

impl PartialOrd for DateTimeImmutable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some((self.sse, self.us).cmp(&(other.sse, other.us)))
    }
}
