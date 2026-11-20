//! Port of Symfony\Component\Finder\Comparator\DateComparator.php.
//!
//! `new \DateTimeImmutable($s)` is the subset in src/date.rs (UTC); a date
//! outside it is refused with its own message (README, "Differences").
//!
//! Symfony source copyright (c) Fabien Potencier; distributed under MIT.
//!
//! Upstream: symfony/finder Comparator/DateComparator.php

use std::{
    ops::Deref,
    sync::LazyLock,
    time::{SystemTime, UNIX_EPOCH},
};

use super::Comparator;
use crate::{date, exception::Error, pcre::Pcre};

static TEST: LazyLock<Option<Pcre>> = LazyLock::new(|| {
    Pcre::new(br"#^\s*(==|!=|[<>]=?|after|since|before|until)?\s*(.+?)\s*$#i").ok()
});

/// A date test such as `since yesterday` or `< 2005-10-15`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateComparator(Comparator);

impl DateComparator {
    /// `new DateComparator($test)`, relative to the current time.
    pub fn new(test: &[u8]) -> Result<Self, Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
        Self::new_at(test, now)
    }

    /// `new DateComparator($test)` at the Unix time `now`.
    pub fn new_at(test: &[u8], now: i64) -> Result<Self, Error> {
        let pattern = TEST
            .as_ref()
            .ok_or_else(|| Error::Logic("DateComparator pattern".into()))?;
        let groups = pattern
            .captures_from(test, 0)
            .ok()
            .flatten()
            .ok_or_else(|| {
                Error::InvalidArgument(format!(
                    "Don't understand \"{}\" as a date test.",
                    String::from_utf8_lossy(test)
                ))
            })?;
        let group = |i: usize| -> &[u8] {
            match groups.get(i).copied().flatten() {
                Some((s, e)) => &test[s..e],
                None => b"",
            }
        };
        let date = group(2);
        let Some(target) = date::timestamp(date, now) else {
            return Err(Error::InvalidArgument(format!(
                "\"{}\" is not a valid date, or not a date format this port supports.",
                String::from_utf8_lossy(date)
            )));
        };
        let operator = String::from_utf8_lossy(group(1)).into_owned();
        let operator = match operator.as_str() {
            "" => "==",
            "since" | "after" => ">",
            "until" | "before" => "<",
            o => o,
        };
        Ok(Self(Comparator::new(target.to_string(), operator)?))
    }
}

impl Deref for DateComparator {
    type Target = Comparator;

    fn deref(&self) -> &Comparator {
        &self.0
    }
}
