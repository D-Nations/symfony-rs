# M2-A routing stage 6: matchRequest() and RequestContext::fromRequest()

November 25. Branch `m2/routing-match-request`. Crates touched:
`symfony-routing`, `symfony-http-foundation`.

Built on routing stages 2 and 5 (RequestContext, the two matchers) and
HttpFoundation stage 5 (Request). This connects the matchers to a request:
`fromRequest()` fills a context from a Request, and `matchRequest()`
matches a Request. Route conditions, which need ExpressionLanguage, are
the only matcher tests still pending.

## Scope

Upstream symfony/routing and symfony/http-foundation at the pin (v8.1.11).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `RequestContext.php` (`fromRequest()`) | `crates/routing/src/request_context.rs` | RequestContextTest 17 of 18 (1 `n/a`) |
| `Matcher/UrlMatcher.php` (`matchRequest()`), `Matcher/RequestMatcherInterface.php` | `crates/routing/src/url_matcher.rs` | UrlMatcherTest 85 of 91 |
| `Matcher/RedirectableUrlMatcher.php` (inherits `matchRequest()`) | `crates/routing/src/redirectable_url_matcher.rs` | RedirectableUrlMatcherTest 98 of 104 |
| `Request.php` (`getPort()`, a fix) | `crates/http-foundation/src/request.rs` | covered by the recorded vectors below |

Newly ported tests: `testFromRequest`, and
`testMatchRequestHonorsTheRequestsMethodOverTheStaticContext` and
`testMatchRequestRestoresTheContextAfterwards` on both matchers. The six
condition tests on each matcher stay pending.

Prior art (searched November 25): no Rust port of these methods was found.
Upstream's code is two short methods on classes this repository has
already ported, so porting them was the only reasonable option. See the
stack row.

## Port notes

- **Borrowing.** Request's getters cache what they compute, so
  `from_request()` and `match_request()` take `&mut Request`.
- **Context restore.** `matchRequest()` matches on a copy of the context
  filled by `fromRequest()`. The matcher's own context is never changed,
  so upstream's `finally` (which restores it) has nothing to do.
  `$this->request` is not kept, since only route conditions read it.
  `RedirectableUrlMatcher::match_request()` runs its own `match()` on the
  copy, as PHP's inherited method calls the subclass's `match()`.
- **Arguments.** `RequestContext.php` has no `strict_types`. The port
  from `getPort()` goes to `setHttpPort(int)` / `setHttpsPort(int)`, and
  `server->get('QUERY_STRING', '')` goes to `setQueryString(?string)`,
  converted as PHP converts arguments. A value that does not convert is
  PHP's `\TypeError`, with upstream's function name and without PHP's
  ", called in ..." suffix. php.rs gains `int_argument()` and
  `nullable_string_argument()`.
- **Order.** The setters run in upstream's order, with `isSecure()` and
  `getPort()` evaluated as PHP's `||` and `&&` short-circuit. A failure
  leaves the earlier setters applied, as in PHP.
- **Request exceptions.** An exception thrown by a Request getter
  (`SuspiciousOperationException` for an untrusted or invalid host,
  `ConflictingHeadersException`, ...) propagates. Routing's error type
  carries it as `Class::HttpFoundation(class)`, with the same class name
  and `instanceof` chain. HttpFoundation's `Class::ancestry()` becomes
  public for this.

## Behaviour found by recording: getPort()'s return type

`Request::getPort()` is declared `int|string|null` and returns
`SERVER_PORT` as the server bag holds it. In coercive mode PHP converts
that value to the return type:
- A bool becomes 0 or 1.
- A float that fits an int is truncated. `8.5` becomes `8` (PHP 8.1+ adds
  a deprecation notice) and `-2^63` becomes `PHP_INT_MIN`.
- Any other float becomes its string at `precision = 14`: `"1.0E+30"`,
  `"9.2233720368548E+18"` for 2^63, `"NAN"`, `"INF"`.
- An array is a `\TypeError` ("Return value must be of type
  string|int|null, array returned").

The Request port (stage 5) returned the value unchanged. The routing
vectors exposed this: `SERVER_PORT = 1e30` gave "float given" in Rust and
"string given" in PHP. It is fixed in `request.rs` (`port_return()`). The
Request recorder gains a `ports` section of 23 typed `SERVER_PORT` values,
appended after its seeded cases, so the existing 4,000 cases are recorded
byte for byte as before.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | RequestContextTest 17 of 18, UrlMatcherTest 85 of 91, RedirectableUrlMatcherTest 98 of 104 | all pass |
| `fromRequest()`, `matchRequest()` on both matchers, and the matchers' contexts afterwards (`crates/routing/tests/data/from_request.json`, `oracle/recorders/routing_from_request.php`) | The 3,538 requests of http-foundation's `request.json` that `create()` accepts, replayed under their trusted-proxy and trusted-host configuration, from three starting contexts. Plus 304 requests whose `SERVER_PORT` (19 values) and `QUERY_STRING` (8 values) are of every type, with and without HTTPS. 3,842 requests, 15,368 answers (contexts and attributes compared with `===`; exceptions by class and message) | all match |
| `getPort()` of typed `SERVER_PORT` values (`crates/http-foundation/tests/data/request.json`, `ports`) | 23 | all match |

Mutation checks:
- `getPort()` returning `SERVER_PORT` unchanged (the stage 5 code) fails
  17 `ports` answers and 96 routing answers.
- Using port 0 instead of the context's own port when `getPort()` is null
  fails 43 routing answers.
- Passing `QUERY_STRING` without the `?string` argument conversion fails
  78 routing answers.

## Advisories

No advisory in the upstream watch concerns these methods.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner, November 23).
