# M2-A HttpFoundation stage 6: Response

November 25. Branch `m2/http-foundation-response`. Crates touched:
`symfony-http-foundation`, `php-value`.

Built on HttpFoundation stages 2 (ResponseHeaderBag, Cookie) and 5
(Request). This is the first half of what the HTTP path still needed:
a response that sets its headers, cache rules and status the way
Symfony does, prepared against the request it answers. The HttpKernel
lifecycle comes next.

## Scope

Upstream symfony/http-foundation at the pin (v8.1.11).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Response.php` | `crates/http-foundation/src/response.rs` | ResponseTest 80 of 81 (1 `n/a`), including ResponseTestCase's test |
| `ResponseHeaderBag.php` (`getCookies()` handing out its objects) | `header_bag.rs`, `cookies_mut()` | covered by `prepare()`'s secure cookies |
| (PHP's output functions) | `crates/http-foundation/src/php.rs`, `Output` | covered by the recorded calls |

Not ported here:
- `ResponseFunctionalTest::testCookie` stays `pending`. It compares what
  PHP's built-in server sends for the `header()` calls. That is SAPI
  behaviour, which the HTTP adapter will reproduce from `Output::calls`.
- `testInformationalResponse` is `n/a`: it needs FrankenPHP on port 80.
  The `headers_send()` call it relies on is recorded and checked.
- `testDirectHeadersWriteFromSubclassReportsSubclass` is `n/a`, as for
  RequestTest: a Rust struct has no subclass.

Prior art (searched November 25): no Rust port of Symfony's Response.
http-fresh, http-cache-semantics and http-cache implement other cache
rules (Express's, RFC 7234's) and none of `setCache()`, `prepare()` or
`sendHeaders()`. See the stack row.

## Port notes

- **Sending.** PHP's `header()`, `header_remove()`, `headers_send()`,
  `echo`, `ob_end_*()`, `flush()` and the finish functions change the
  process. Here they are `OutputCall`s, appended in order to this thread's
  `php::Output`. That structure also answers what Response asks:
  `PHP_SAPI`, `headers_sent()`, `function_exists('headers_send')`, which
  finish function exists, and `ob_get_status()`. `prepare()`'s
  `ini_set('default_mimetype', '')` empties `Output::default_mimetype`.
  Nothing applies the calls yet: what a SAPI makes of a status line,
  `Location` or a replaced header is the adapter's contract.
- **Objects as handles.** PHP's `getCookies()` returns the bag's own
  Cookie objects, so `prepare()`'s `setSecureDefault(true)` changes the
  bag. Here the bag lends them (`cookies_mut()`), and the test reads the
  cookie back from the bag.
- **Types.** Status codes are `i64` and texts are bytes. `\DateTimeInterface`
  arguments are php-value's `DateTimeImmutable`. `setCache()` takes PHP's
  options array, converting each value as a call without `strict_types`
  does (`\TypeError` without the "called in" suffix). `$statusTexts` is
  the constant `STATUS_TEXTS`.
- **Failures as in PHP.** `setStatusCode()` keeps an invalid code before
  it throws. `getAge()` and `getMaxAge()` on a response whose Date is null
  give PHP's `\Error` ("Call to a member function format() on null").
  `getExpires()` turns only `\RuntimeException` into "two days ago"; a
  date text php-value refuses still propagates.
- **PHP rules kept.** `'1.0' == $version` and `'HTTP/1.0' !=
  SERVER_PROTOCOL` are loose. In `isNotModified()`, `strtotime()`'s `false`
  is compared with `>=` as PHP compares a bool. The MSIE rule matches
  `/MSIE (.*?);/i` but strips with a case-sensitive pattern, so `msie
  7.0;` counts as version 0 and drops Cache-Control.

## Behaviour found by recording

- `DateTimeImmutable::createFromFormat(DATE_RFC2822, 'foo')` is `false`
  in PHP; php-value refused it. A day name that is no word PHP's date
  parser knows is now `false`. The 209 `createFromFormat()` inputs of
  `php-value/tests/data/dates.json` confirm this (one listed refusal,
  `Xyz, 4 Sep 2012 …`, is now read as PHP reads it). Words the parser
  does know (months, keywords, zones) are still refused.
- `strtotime('-1')` is a time in PHP (an offset read as a zone).
  php-value refuses bare numbers, so an `If-Modified-Since` of `-1` is
  refused (`Class::NotPorted`). The recorded test lists both scripts
  where this happens and stops them at the refused call. Teaching
  php-value PHP's number rules is open.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | ResponseTest 80 of 81 (ResponseTestCase's included); `testReasonPhraseDefaultsAgainstIana` on upstream's IANA fixture | all pass |
| Call scripts (`crates/http-foundation/tests/data/response.json`, `oracle/recorders/http_foundation_response.php`) | 1,200 seeded scripts: construction (32 fail as in PHP), 6,491 calls with the process calls each made, 26,864 reader answers; 33,387 answers in all. `===` for values, class and message for exceptions | all match, except the two scripts stopped at the listed refusal |
| `closeOutputBuffers()` | 300 scripted `ob_get_status()` answers (flags, `del`, levels, negative targets) | all match |
| `prepare()`'s protocol comparisons | 11 versions × 11 typed `SERVER_PROTOCOL` values | all match |
| php-value dates (`crates/php-value/tests/dates.rs`) | the existing 173 + 209 answers | all match |

Mutation checks on the recorded test:
- `strtotime()`'s `false` treated as "modified": fails 8 answers.
- Ignoring `headers_sent()`: fails 90 answers.
- A strict `'1.0' === $version`, or a strict `SERVER_PROTOCOL` check:
  each fails the protocol cases. The random scripts alone missed the
  first, which is why the protocol section was added.

## Advisories

The upstream watch lists no advisory for `Response.php`.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner, November 23); M2-A's acceptance by
comparison stays open.
