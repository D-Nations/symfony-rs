//! PHP's date extension, as far as Clock and its tests need it:
//! `DateTimeImmutable`, `DateTimeZone`, the date text parser behind the
//! constructor and `modify()`, `format()`, `createFromFormat()` and the
//! default time zone. Written from the PHP manual and from the answers
//! PHP 8.4.27 gave in the oracle; no PHP source code is used.
//!
//! Upstream: none

mod civil;
mod datetime;
mod error;
mod format;
mod from_format;
mod parse;
mod zone;

pub use datetime::{DateTimeImmutable, Timestamp};
pub use error::DateError;
pub use from_format::{FormatError, LastErrors};
pub use zone::{DateTimeZone, date_default_timezone_get, date_default_timezone_set};

pub(crate) use datetime::system_now;

/// The date and time fields of `date_parse($datetime)`; `None` is PHP's
/// `false`. Microseconds stand for `fraction`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DateParse {
    pub year: Option<i64>,
    pub month: Option<i64>,
    pub day: Option<i64>,
    pub hour: Option<i64>,
    pub minute: Option<i64>,
    pub second: Option<i64>,
    pub fraction: Option<i64>,
}

/// `date_parse($datetime)` for a text without errors. A text with errors
/// gives `\DateMalformedStringException` with the constructor's message
/// (PHP returns the fields it read and the errors instead; those partial
/// fields are not ported).
pub fn date_parse(datetime: &str) -> Result<DateParse, DateError> {
    let p = parse::parse(datetime).map_err(|e| match e.message(datetime) {
        Some(message) => DateError::MalformedString(message),
        None => DateError::unsupported("date text", datetime),
    })?;
    Ok(DateParse {
        year: p.year,
        month: p.month,
        day: p.day,
        hour: p.hour,
        minute: p.minute,
        second: p.second,
        fraction: p.fraction,
    })
}
