//! Proleptic Gregorian calendar arithmetic on `i64` days and seconds,
//! with PHP's rollover of out-of-range fields (month 0 is December of the
//! year before, day 0 the last day of the month before, hour 24 the next
//! day).
//!
//! Upstream: none

pub(crate) const SECONDS_PER_DAY: i64 = 86_400;

/// Days since 1970-01-01 of a valid date (H. Hinnant's algorithm).
pub(crate) fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date of a day number from [`days_from_civil`].
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

pub(crate) fn is_leap(year: i64) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}

pub(crate) fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// 0 for Sunday to 6 for Saturday.
pub(crate) fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

/// Local calendar fields, any of which may be out of range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Fields {
    pub year: i64,
    pub month: i64,
    pub day: i64,
    pub hour: i64,
    pub minute: i64,
    pub second: i64,
    pub microsecond: i64,
}

impl Fields {
    /// The fields of a local time given as seconds (and microseconds) since
    /// 1970-01-01 00:00:00 local.
    pub fn from_local(seconds: i64, microsecond: u32) -> Self {
        let days = seconds.div_euclid(SECONDS_PER_DAY);
        let rest = seconds.rem_euclid(SECONDS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: rest / 3600,
            minute: rest / 60 % 60,
            second: rest % 60,
            microsecond: i64::from(microsecond),
        }
    }

    /// Local seconds since 1970-01-01 and microseconds, rolling every field
    /// over as PHP does. `None` when the result does not fit.
    pub fn to_local(self) -> Option<(i64, u32)> {
        let months = i128::from(self.year) * 12 + i128::from(self.month) - 1;
        let year = i64::try_from(months.div_euclid(12)).ok()?;
        let month = i64::try_from(months.rem_euclid(12)).ok()? + 1;
        // Keep the year inside what days_from_civil computes without
        // overflow (far beyond any i64 timestamp).
        if year.unsigned_abs() > 1 << 40 {
            return None;
        }
        let days = i128::from(days_from_civil(year, month, 1)) + i128::from(self.day) - 1;
        let us = i128::from(self.microsecond);
        let seconds = days * i128::from(SECONDS_PER_DAY)
            + i128::from(self.hour) * 3600
            + i128::from(self.minute) * 60
            + i128::from(self.second)
            + us.div_euclid(1_000_000);
        let us = u32::try_from(us.rem_euclid(1_000_000)).ok()?;
        Some((i64::try_from(seconds).ok()?, us))
    }
}

/// ISO 8601 week-numbering year and week of a day number.
pub(crate) fn iso_week(days: i64) -> (i64, i64) {
    // Monday = 1 .. Sunday = 7.
    let iso_weekday = (weekday(days) + 6) % 7 + 1;
    let thursday = days - iso_weekday + 4;
    let (iso_year, _, _) = civil_from_days(thursday);
    let first = days_from_civil(iso_year, 1, 1);
    (iso_year, (thursday - first) / 7 + 1)
}

/// The day number of ISO year `year`, week `week`, day `day` (1 = Monday),
/// with PHP's rollover of out-of-range weeks and days.
pub(crate) fn from_iso_week(year: i64, week: i64, day: i64) -> i64 {
    let jan4 = days_from_civil(year, 1, 4);
    let jan4_iso = (weekday(jan4) + 6) % 7 + 1;
    let week1_monday = jan4 - (jan4_iso - 1);
    week1_monday + (week - 1) * 7 + (day - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for days in [-800_000, -1, 0, 1, 59, 60, 365, 11_016, 2_932_896] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days, days_from_civil(y, m, d));
        }
        assert_eq!((1970, 1, 1), civil_from_days(0));
        assert_eq!((2016, 3, 1), civil_from_days(days_from_civil(2016, 2, 30)));
    }

    #[test]
    fn weeks() {
        // 2021-01-03 is a Sunday in ISO week 53 of 2020.
        assert_eq!((2020, 53), iso_week(days_from_civil(2021, 1, 3)));
        assert_eq!((2021, 1), iso_week(days_from_civil(2021, 1, 4)));
        assert_eq!(days_from_civil(2021, 1, 4), from_iso_week(2021, 1, 1));
    }
}
