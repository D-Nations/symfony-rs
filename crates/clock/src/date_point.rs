//! Port of Symfony\Component\Clock\DatePoint.php: an immutable `DateTime`
//! with stricter error handling and return types than the native one.
//!
//! `DatePoint` extends `\DateTimeImmutable` in PHP; here it wraps a
//! [`DateTimeImmutable`] and derefs to it for the methods it does not
//! override (`format()`, `getTimestamp()`, `getOffset()`, ...). `==` and
//! `<` compare instants. `createFromMutable()` (PHP's mutable
//! `\DateTime`) and `add()`/`sub()` (`\DateInterval`) are not ported.
//!
//! Upstream: symfony/clock DatePoint.php

use std::ops::Deref;

use crate::{
    clock::Clock,
    native_clock::NativeClock,
    php::{DateError, DateTimeImmutable, DateTimeZone, FormatError, Timestamp},
};

/// `DatePoint`.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct DatePoint(DateTimeImmutable);

impl Deref for DatePoint {
    type Target = DateTimeImmutable;

    fn deref(&self) -> &DateTimeImmutable {
        &self.0
    }
}

impl From<DatePoint> for DateTimeImmutable {
    fn from(value: DatePoint) -> Self {
        value.0
    }
}

impl DatePoint {
    /// `new DatePoint($datetime, $timezone, $reference)`; `new
    /// DatePoint()` is `DatePoint::new("now", None, None)`.
    ///
    /// Throws `\DateMalformedStringException` when `$datetime` is invalid.
    pub fn new(
        datetime: &str,
        timezone: Option<&DateTimeZone>,
        reference: Option<&DateTimeImmutable>,
    ) -> Result<Self, DateError> {
        let reference = if let Some(reference) = reference {
            reference.clone()
        } else {
            let clock = Clock::get();

            // copying the clock's date into $this goes through its local time, which is ambiguous during the DST fall-back hour
            if datetime == "now" && clock.is::<NativeClock>() {
                let zone = match timezone {
                    Some(zone) => zone.clone(),
                    None => clock.now().get_timezone(),
                };
                return Ok(Self(DateTimeImmutable::new("now", Some(&zone))?));
            }

            clock.now()
        };

        let mut now = reference;

        if datetime != "now" {
            let zone = timezone.cloned().unwrap_or_else(|| now.get_timezone());
            let built_in_date = DateTimeImmutable::new(datetime, Some(&zone))?;
            let timezone = built_in_date.get_timezone();

            let parsed = crate::php::date_parse(datetime).unwrap_or_default();

            if parsed.year.is_some() && parsed.month.is_some() && parsed.day.is_some() {
                // the native result does not depend on the current time when $datetime has a full date
                now = built_in_date;
            } else {
                now = now.set_timezone(&timezone);
                let mut datetime = datetime.to_owned();

                // the native constructor starts from midnight when $datetime has a date but no time, modify() does not
                if parsed.hour.is_none() && (parsed.month.is_some() || parsed.day.is_some()) {
                    datetime = format!("midnight {datetime}");
                } else if parsed.fraction.is_none()
                    && (parsed.year.is_some() || parsed.hour.is_some())
                {
                    // it also resets the microseconds when $datetime sets a field without a fraction
                    now = now.set_timestamp(now.get_timestamp());
                }

                now = now.modify(&datetime)?;
            }
        } else if let Some(zone) = timezone {
            now = now.set_timezone(zone);
        }

        let mut this = now.copy_through_local_time();

        // copying $now goes through its local time, which is ambiguous during the DST fall-back hour
        if this != now {
            // PHP picks the DST occurrence when the timezone is part of the string
            let date = now.format("Y-m-d H:i:s.u e");
            let candidate = DateTimeImmutable::new(&date, None)?;

            if now == candidate {
                this = candidate;
            }
        }

        Ok(Self(this))
    }

    /// `createFromFormat($format, $datetime, $timezone)`.
    ///
    /// Throws `\DateMalformedStringException` with PHP's first error, the
    /// one at position 0 (`getLastErrors()['errors'][0]`), or `Invalid
    /// date string or format.`
    pub fn create_from_format(
        format: &str,
        datetime: &str,
        timezone: Option<&DateTimeZone>,
    ) -> Result<Self, DateError> {
        match DateTimeImmutable::create_from_format(format, datetime, timezone) {
            Ok(date) => Ok(Self(date)),
            Err(FormatError::Failed(errors)) => Err(DateError::MalformedString(
                errors
                    .errors
                    .get(&0)
                    .cloned()
                    .unwrap_or_else(|| "Invalid date string or format.".to_owned()),
            )),
            Err(FormatError::Date(error)) => Err(error),
        }
    }

    /// `createFromInterface($object)`.
    pub fn create_from_interface(object: &DateTimeImmutable) -> Self {
        Self(DateTimeImmutable::create_from_interface(object))
    }

    /// `createFromTimestamp($timestamp)`.
    pub fn create_from_timestamp(timestamp: impl Into<Timestamp>) -> Result<Self, DateError> {
        DateTimeImmutable::create_from_timestamp(timestamp).map(Self)
    }

    /// `modify($modifier)`.
    ///
    /// Throws `\DateMalformedStringException` when `$modifier` is invalid.
    pub fn modify(&self, modifier: &str) -> Result<Self, DateError> {
        self.0.modify(modifier).map(Self)
    }

    /// `setTimestamp($value)`.
    #[must_use]
    pub fn set_timestamp(&self, value: i64) -> Self {
        Self(self.0.set_timestamp(value))
    }

    /// `setDate($year, $month, $day)`.
    pub fn set_date(&self, year: i64, month: i64, day: i64) -> Result<Self, DateError> {
        self.0.set_date(year, month, day).map(Self)
    }

    /// `setISODate($year, $week, $day)`.
    pub fn set_iso_date(&self, year: i64, week: i64, day: i64) -> Result<Self, DateError> {
        self.0.set_iso_date(year, week, day).map(Self)
    }

    /// `setTime($hour, $minute, $second, $microsecond)`.
    pub fn set_time(
        &self,
        hour: i64,
        minute: i64,
        second: i64,
        microsecond: i64,
    ) -> Result<Self, DateError> {
        self.0.set_time(hour, minute, second, microsecond).map(Self)
    }

    /// `setTimezone($timezone)`.
    #[must_use]
    pub fn set_timezone(&self, timezone: &DateTimeZone) -> Self {
        Self(self.0.set_timezone(timezone))
    }

    /// `getTimezone()`. A `DatePoint` always has one, so upstream's
    /// `\DateInvalidTimeZoneException` does not occur.
    pub fn get_timezone(&self) -> DateTimeZone {
        self.0.get_timezone()
    }

    /// `setMicrosecond($microsecond)`.
    ///
    /// Throws `\DateRangeError` outside 0 to 999999.
    pub fn set_microsecond(&self, microsecond: i64) -> Result<Self, DateError> {
        if !(0..=999_999).contains(&microsecond) {
            return Err(DateError::Range(format!(
                "DatePoint::setMicrosecond(): Argument #1 ($microsecond) must be between 0 and 999999, {microsecond} given"
            )));
        }

        self.0.set_microsecond(microsecond).map(Self)
    }

    /// The `\DateTimeImmutable` this point is.
    pub fn into_inner(self) -> DateTimeImmutable {
        self.0
    }

    /// The `\DateTimeImmutable` this point is.
    pub fn as_date_time(&self) -> &DateTimeImmutable {
        &self.0
    }
}
