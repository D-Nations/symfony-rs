//! Port of Symfony\Component\Clock\Resources\now.php: the `now()` helper.
//!
//! Upstream: symfony/clock Resources/now.php

use crate::{clock::Clock, date_point::DatePoint, php::DateError};

/// `now($modifier)`; `now()` is `now("now")`.
///
/// Throws `\DateMalformedStringException` when the modifier is invalid.
pub fn now(modifier: &str) -> Result<DatePoint, DateError> {
    if modifier != "now" {
        return DatePoint::new(modifier, None, None);
    }

    Ok(DatePoint::create_from_interface(&Clock::get().now()))
}
