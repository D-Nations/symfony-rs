//! PHP's `time()`: seconds since the Unix epoch.
//!
//! Tests replace the clock for the current thread, as
//! symfony/phpunit-bridge's ClockMock does for a `time-sensitive` test
//! class: [`mock`] fixes `time()` (and through it `gmdate()` without a
//! timestamp, `strtotime()` without a base) until the guard is dropped,
//! and [`sleep`] moves the mocked clock forward.
//!
//! Upstream: none

use std::{
    cell::Cell,
    time::{SystemTime, UNIX_EPOCH},
};

thread_local! {
    static MOCKED: Cell<Option<i64>> = const { Cell::new(None) };
}

/// `time()`. A clock before 1970 reads as the epoch.
pub fn time() -> i64 {
    MOCKED.with(Cell::get).unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| {
                i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
            })
    })
}

/// Restores the clock that was in place when [`mock`] was called.
#[must_use = "the clock is restored when the guard is dropped"]
pub struct MockClock {
    previous: Option<i64>,
}

impl Drop for MockClock {
    fn drop(&mut self) {
        MOCKED.with(|mocked| mocked.set(self.previous));
    }
}

/// Fixes `time()` at `now` on this thread until the guard is dropped.
pub fn mock(now: i64) -> MockClock {
    let previous = MOCKED.with(|mocked| mocked.replace(Some(now)));
    MockClock { previous }
}

/// `sleep($seconds)` under a mocked clock: the clock moves forward. Has no
/// effect on the real clock.
pub fn sleep(seconds: i64) {
    MOCKED.with(|mocked| {
        if let Some(now) = mocked.get() {
            mocked.set(Some(now.saturating_add(seconds)));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mocking_is_scoped() {
        let real = time();
        {
            let _clock = mock(42);
            assert_eq!(time(), 42);
            sleep(3);
            assert_eq!(time(), 45);
            {
                let _inner = mock(7);
                assert_eq!(time(), 7);
            }
            assert_eq!(time(), 45);
        }
        assert!(time() >= real);
    }
}
