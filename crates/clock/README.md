# Symfony Clock

Private workspace crate: a port of `symfony/clock` v8.1.9 (`945ea3b6`), as
installed by symfony/demo. MIT; Symfony source and tests copyright Fabien
Potencier. No PHP source code is used: the PHP date layer in `src/php/` is
written from the PHP manual and from recorded PHP answers. Unofficial, not
affiliated with or endorsed by Symfony or PHP.

```rust
use symfony_clock::{DatePoint, MockClock, php::DateTimeZone};

let clock = MockClock::new("2024-01-01 00:00:00", Some("Europe/Paris".into())).unwrap();
clock.sleep(1.5).unwrap();
assert_eq!(
    "2024-01-01 00:00:01.500000 Europe/Paris",
    clock.now().format("Y-m-d H:i:s.u e")
);

let utc = DateTimeZone::new("UTC").unwrap();
let date = DatePoint::new("2024-02-29 12:00", Some(&utc), None).unwrap();
assert_eq!("2025-03-01", date.modify("+1 year").unwrap().format("Y-m-d"));
assert!(DatePoint::new("Bad Date", None, None).is_err());
```

| Contract | Source | Evidence |
|---|---|---|
| `ClockInterface` (PSR-20 `now()`, `sleep()`, `withTimeZone()`) | `ClockInterface.php` | All tests below |
| `NativeClock`: system time in a zone (the default zone when none) | `NativeClock.php` | NativeClockTest (4) |
| `MonotonicClock`: a monotonic time (from `Instant`) shown in a zone | `MonotonicClock.php` | MonotonicClockTest (4) |
| `MockClock`: a frozen time; `sleep()` with PHP's `U.u` carry, `modify()`, the zone kept | `MockClock.php` | MockClockTest (8); oracle `sleep`, `mock_modify`, `mock_construct` |
| `Clock`: the global clock, a PSR-20 clock wrapped in `Clock`, a zone per instance | `Clock.php` | ClockTest (7), ClockBeforeClassTest (1) |
| `ClockAwareTrait`: a clock set once, the global clock when none | `ClockAwareTrait.php` | ClockAwareTraitTest (1) |
| `DatePoint`: the constructor (the clock's now as reference, `date_parse()`, midnight default, the DST fall-back hour), `createFromFormat()`, `createFromTimestamp()`, `modify()`, the setters, `setMicrosecond()` | `DatePoint.php` | DatePointTest (11); oracle `datepoint` (2,464), `setters` (264) |
| `now()` | `Resources/now.php` | ClockTest `testNowModifier`, `testInvalidNowModifier` |
| `ClockSensitiveTrait::mock_time()`, save and restore of the global clock | `Test/ClockSensitiveTrait.php` | ClockTest, DatePointTest |
| PHP's `DateTimeImmutable` and `DateTimeZone` subset: identifiers, offsets and abbreviations; `format()` (all characters); `createFromFormat()` with its warnings and errors; `createFromTimestamp()`; `modify()`; gap and fold rules | `src/php/` (Upstream: none) | Oracle `timezones`, `parse`, `modify` (5,840), `format`, `from_format`, `timestamps` |
| Exception messages, e.g. `Failed to parse time string (Bad Date) at position 0 (B): The timezone could not be found in the database` | All | Upstream tests; oracle answers compare class and message |

Vectors: `scripts/record.sh clock` runs `oracle/recorders/clock.php` on
PHP 8.4.27 and writes `tests/data/php.json` (about 9,800 answers);
`tests/php.rs` replays them. The committed file was recorded in the
`php:8.4.27-cli-bookworm` image, the base image of the oracle.

## Differences

- Date texts: the port reads the part of PHP's date and time grammar that
  the tests, `DatePoint` and the recorded cases use. It refuses other texts
  with `DateError::Unsupported`, and does not give a different answer.
  `tests/php.rs` counts the refused cases in each section: 4 zone names,
  275 parse cases, 1,100 modify cases and 10 setter cases. All other
  recorded cases agree with PHP.
- A setter (`setDate()`, `setTime()`, ...) that gives a local time in a
  DST gap is refused with `DateError::Unsupported`. PHP keeps the written
  local time in that case.
- `MonotonicClock` tests use the real monotonic clock. Upstream replaces
  `hrtime()` with phpunit-bridge's `ClockMock`; the assertions are the same.
- `date_parse()` returns the date and time fields only, not PHP's
  `warnings`, `errors` or relative parts.
- `DatePoint::createFromMutable()` (PHP's mutable `\DateTime`), `add()` and
  `sub()` (`\DateInterval`) are not ported.
- Zone data comes from jiff's bundled tzdb 2026f, so answers do not change
  with the host. PHP uses its own bundled copy; zones whose rules changed
  between the two copies can differ.

## Rust shape

- `DatePoint` wraps a `php::DateTimeImmutable` and derefs to it. `==` and
  `<` compare instants.
- `ClockInterface` methods that throw in PHP return
  `Result<_, DateError>`. `DateError::class()` gives the PHP exception
  class.
- `SharedClock` is `Arc<dyn ClockInterface>`. `instanceof` is
  `clock.is::<MockClock>()`.
- The global clock is a process-wide `RwLock` (no `unsafe`). Tests that
  change it take `ClockSensitiveTrait::before_test()`, which runs them one
  at a time and restores the clock at the end.
- `int|float` arguments are `Seconds` and `Timestamp`;
  `\DateTimeZone|string` is `TimeZoneInput`.
