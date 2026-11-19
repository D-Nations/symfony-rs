# Symfony Stopwatch

Private workspace crate: a port of `symfony/stopwatch` v8.1.0 (`21c07b02`),
as installed by symfony/demo. MIT; Symfony source and tests copyright
Fabien Potencier. `php::round()` derives from php-src's BSD-3-Clause main
branch, copyright The PHP Group and Contributors
([licence](licenses/php-src-BSD-3-Clause.txt)). Unofficial, not affiliated
with or endorsed by Symfony or PHP.

```rust
use symfony_stopwatch::Stopwatch;

let stopwatch = Stopwatch::new(false);
stopwatch.open_section(None).unwrap();
stopwatch.start("render", Some("template"));
let event = stopwatch.stop("render").unwrap();
stopwatch.stop_section("request").unwrap();
assert_eq!(event.get_category(), "template");
assert_eq!(stopwatch.get_section_events("request").len(), 2); // + __section__
```

| Contract | Source | Evidence |
|---|---|---|
| Events by name and category (`default` when none), started, stopped and lapped; `is_started`, `get_event` | `Stopwatch.php`, `Section.php` | StopwatchTest (15) |
| Sections: open, re-open by id at the same level, stop by id; `__section__` and `__section__.child` events; events in first-started order, sections in first-stopped order | `Stopwatch.php`, `Section.php` | StopwatchTest; oracle vectors |
| Periods, overlapping starts, duration including running periods, start/end times, `ensureStopped` | `StopwatchEvent.php` | StopwatchEventTest (15), on a mocked clock as upstream |
| Times rounded with PHP's `round($t, 1)`; `int` milliseconds by default, `float` with more precision (`int|float` kept as `Number`) | `StopwatchEvent.php`, `StopwatchPeriod.php` | StopwatchPeriodTest (3 providers); 3,024 `round()` vectors |
| `category/name: 0.00 MiB - 0 ms` and `0.00 MiB - 0 ms` strings: PHP's `%.2F` (ties to even) and `%d` (truncating, wrapping) | `StopwatchEvent.php`, `StopwatchPeriod.php` | `testHumanRepresentation`; `%d`/`%.2F`/`(int)` vectors |
| `\LogicException` messages, e.g. `Event "x" is not started.` (also from `stopSection()` on a new stopwatch) | `Section.php`, `StopwatchEvent.php`, `Stopwatch.php` | Oracle vectors |

Vectors: `scripts/record.sh stopwatch` runs `oracle/recorders/stopwatch.php`
on PHP 8.4.27 and writes `tests/data/php.json` (a fixed seed, so it
re-records byte for byte); `tests/php.rs` checks them.

## Rust shape

- Events and sections are shared handles, as PHP objects are: `start()`
  returns the same event `get_event()` does (`same()` is `===`; `==`
  compares contents, like `assertEquals`). Everything is `Send + Sync`.
- `microtime(true)` and `memory_get_usage(true)` come from a `Host`.
  `SystemHost` uses the system clock and reports **memory as 0**: Rust has
  no figure like PHP's. A host can supply one (e.g. from a counting global
  allocator); tests use a mocked clock, like phpunit-bridge's ClockMock.
- Arrays keyed by name are `Vec<(String, _)>` in PHP order.
- `reset()` is ServiceContracts' `ResetInterface`, not ported yet.
