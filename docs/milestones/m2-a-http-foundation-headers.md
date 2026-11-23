# M2-A stage 2: HttpFoundation cookies and header bags

November 23. Branch `m2/http-foundation-headers`.

## Scope

Upstream symfony/http-foundation at the pin (v8.1.10,
`79325f87c3a2acc7564ddce9f457046e980e7255`). The v8.1.10 to v8.1.11 diff
does not touch these files (decision 16's move is a separate stage).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Cookie.php` | `crates/http-foundation/src/cookie.rs` | CookieTest: 53 of 53 |
| `HeaderBag.php`, `ResponseHeaderBag.php` (no `makeDisposition()`) | `crates/http-foundation/src/header_bag.rs` | HeaderBagTest 20 of 20, ResponseHeaderBagTest 19 of 19 |
| `ServerBag.php` | `crates/http-foundation/src/server_bag.rs` | ServerBagTest: 14 of 14 |

PHP built-ins they call, added to `php-value` (written from the PHP manual
and recorded answers, no PHP source, decision 8):

- `clock::time()`, which a test fixes per thread as symfony/phpunit-bridge's
  ClockMock does for `time-sensitive` classes;
- `datetime::gmdate()` and the `D l M F j w N` format characters, and a
  `GMT` zone;
- `dateparse::strtotime()` and `dateparse::create_from_format_rfc2822()`:
  HTTP, cookie and ISO dates, `@timestamp`, relative offsets and single
  words. Each answers PHP's timestamp, PHP's `false` where the reason is
  visible (a malformed number or separator, an unknown word), or refuses
  the text (`Unsupported`, `Class::NotPorted` in the crate) rather than
  guess. The zone abbreviations come from `DateTimeZone::listAbbreviations()`
  recorded in the oracle; the default time zone is UTC, as in the oracle;
- `url::{urlencode, rawurlencode, urldecode, rawurldecode}`;
- `base64::decode()` (non-strict: skips bytes outside the alphabet);
- `PhpArray::ksort()` and `array::compare_keys()` (PHP 8's `<=>` on keys).

## Recorded vectors

`scripts/record.sh http-foundation`, PHP 8.4.27 in the oracle:

| Recorder | Cases | Result |
|---|---|---|
| `http_foundation_dates.php` | 173 `strtotime()`, 209 `createFromFormat(DATE_RFC2822)`, 13 `gmdate()` timestamps | all read texts match; 59 and 13 texts refused, each listed in `crates/php-value/tests/dates.rs` |
| `http_foundation_cookie.php` | 2,112 `fromString()` calls, 165 constructions, on a fixed clock | all match, none refused |
| `http_foundation_header_bags.php` | 400 seeded call scripts (HeaderBag and ResponseHeaderBag) | all match |
| `http_foundation_server_bag.php` | 360 `$_SERVER` arrays | all match |

The recorders fix the clock with functions in HttpFoundation's namespace
(`time()`, `gmdate()`, `strtotime()`), ClockMock's own technique; the
demo's vendor has no phpunit-bridge.

What recording found:

- `strtotime()` and `createFromFormat()` move a date to a named week day
  after out-of-range fields roll over: `Fri, 20 May 2011 24:00:00 GMT`
  is Saturday 00:00, then the next Friday (May 27).
- The header-bag harness must build the initial headers as PHP's array
  does: a repeated name keeps its first place and its last value.

## Differences from PHP

- A header's values are strings or null; PHP also keeps an int or other
  scalar inside a list of values as given.
- `ResponseHeaderBag::set('Set-Cookie', [null])`'s TypeError lacks PHP's
  ", called in FILE on line N".
- Date texts outside the subset are refused (`NotPorted`), never read.

## Advisories

`parity/advisories.tsv` lists none for Cookie, HeaderBag,
ResponseHeaderBag or ServerBag. GHSA-r925 (HeaderUtils::split) is
unchanged; IpUtils and PATH_INFO stay `not-ported`.

## Not in this stage

AcceptHeader and AcceptHeaderItem as an API, `parse_str()` and
`HeaderUtils::parseQuery()` (3 tests), FileBag (needs UploadedFile),
`makeDisposition()` (decision 16).
