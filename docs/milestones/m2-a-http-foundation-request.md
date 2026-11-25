# M2-A HttpFoundation stage 5: Request

November 25. Branch `m2/http-foundation-request`. Crates touched:
`symfony-http-foundation`, `php-value`.

Built on HttpFoundation stages 1–4: the bags, HeaderUtils, IpUtils,
AcceptHeader, Cookie, the files. This is the request half of the HTTP path.
Next: routing's `matchRequest()` and `RequestContext::fromRequest()` on
this Request.

## Scope

Upstream symfony/http-foundation at the pin (v8.1.11,
`30fa3efe6e736b0d06f5331cc6abe813582c0a26`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Request.php` | `crates/http-foundation/src/request.rs` | RequestTest 113 of 120 |
| `Exception/SuspiciousOperationException.php`, `Exception/ConflictingHeadersException.php`, `Exception/JsonException.php` | `crates/http-foundation/src/exception.rs` | through RequestTest |

The other 7 RequestTest rows:
- **`n/a` (3):**
  - `testFactory` and `testFactoryCallable` need upstream's `NewRequest`
    subclass and its `getFoo()`.
  - `testDirectPropertyWriteFromSubclassReportsSubclass` needs that
    subclass's `static::class`.
  - A Rust struct has no subclasses. `setFactory()` is ported and tested
    by `factory_creates_the_request`. The deprecation for Request itself is
    `testDirectPropertyWriteIsDeprecated`.
- **Pending (4):**
  - `testHasSession`, `testGetSession` and `testHasPreviousSession` need
    the Session classes.
  - `testIntlLocale` needs intl's `\Locale` default.

Prior art (searched November 25; PLAN.md stack row):
- No Rust port of Symfony's Request was found.
- The trusted-proxy crates (trusted-proxies, ipware, client-ip) answer
  part of `getClientIps()` with their own rules.
- The HTML entity crates follow WHATWG's entity list.
- serde_json has no big-integer-as-string mode.
- Request is ported on the ported bags. The two PHP built-ins it calls
  are written in php-value.

## Port notes

- **Statics.** PHP's static properties (trusted proxies, header set and
  hosts, formats, method override settings, factory) are per thread, as
  PHP keeps them per request. `Request::reset_statics()` stands for the
  end of a PHP request. The tests' `tearDown()` and their reflection
  resets call it.
- **The PHP process.** `php::Globals` holds what Request reads from PHP:
  - the superglobals, which `createFromGlobals()`, `overrideGlobals()` and
    `setTrustedProxies('REMOTE_ADDR')` use;
  - `php://input`;
  - `request_parse_body()`'s answer;
  - `request_order` and `variables_order`.

  It is per thread too. The HTTP adapter fills it for each request.
  `php::Stream` is a PHP stream on bytes: `getContent(true)` returns one,
  and a resource passed in is shared, as PHP shares it.
- **Bags.**
  - `query()` and `query_mut()` read and change a bag, as
    `$request->query->set()` does.
  - `set_query()` replaces it and raises upstream's 8.1 deprecation, as
    the property hook does.
  - The same holds for the other six bags.
- **Values PHP coerces.** Server values that PHP converts to strings
  (an int `SERVER_PORT`, `null`) are converted the same way. An array or
  object where PHP wants a string is refused (`Class::NotPorted`).
  `getPort()` returns PHP's `int|string|null` as a `Value`.
  `getClientIps()` keeps PHP's `null` entries.
- **Trusted hosts.**
  - A constant pattern is a set lookup, as upstream does.
  - Each other pattern is matched on its own, as `{(?:pattern)}i`. That
    is what upstream's combined `(?|...)` pattern does for each pattern:
    the branch reset keeps group numbers, back references and inline
    options local to each one.
  - A pattern that does not compile on its own could join with its
    neighbours in the combined pattern, so it is refused at
    `setTrustedHosts()` (php-pcre translates no `(?|`).
- **Regular expressions.**
  - `getLanguageComponents()` runs upstream's pattern on php-pcre.
  - The rest are written out, each pinned by the tests and vectors:
    `isHostValid()`'s two patterns, the port suffix, `Via`'s version,
    `getETags()`'s split and `getUrlencodedPrefix()`'s tokens.
- **`filter_var(FILTER_VALIDATE_IP)`** is php-value's `net::validate_ip`,
  already recorded for IpUtils.
- **Linux PHP.** The URL methods follow PHP on Linux, where the oracle
  runs: `DIRECTORY_SEPARATOR` is `/`.
- **Not ported:** the session methods, and `setPhpDefaultLocale()`'s call
  to `\Locale::setDefault()` (no intl port). The demo's request subset
  (`form_fields`, `scheme`, `accept_language`) is unchanged.

## php-value

- `html::html_entity_decode()`, with PHP's default flags:
  - The 252 HTML 4.01 entities, taken from PHP's own
    `get_html_translation_table()`. They match case-sensitively and only
    with their `;`.
  - Numeric references of any length are decoded only for code points
    that HTML 4.01 allows.
  - It is linear on runs of `&`.
- `json::decode_assoc()`, `json_decode($json, true, $depth)` with
  `JSON_THROW_ON_ERROR`:
  - Big integers become strings or floats.
  - Opening the `$depth`-th level fails at once.
  - Errors come in PHP's order. The scanner's errors come before the
    parser's: control byte, malformed UTF-8, unpaired surrogate, then a
    state mismatch for `{]`/`[}`, then a syntax error.
  - It is iterative, so 511 levels do not use the stack.
- `clock::microtime()`, for `create()`'s `REQUEST_TIME_FLOAT`.

## Behaviour found by recording

- `json_decode()` reports `]` closing an object or `}` closing an array,
  where a closer may come, as `JSON_ERROR_STATE_MISMATCH` (2), not as a
  syntax error. After a high surrogate escape, anything but a low one
  (even a malformed `\u`) is `JSON_ERROR_UTF16`.
- `prepareRequestUri()` appends the query twice for a `REQUEST_URI` that
  `parse_url()` reads as a query without a path (`?a=b` becomes
  `?a=b?a=b`). The path info is still `/`. This is kept as PHP does it.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | RequestTest 113 of 120 (every provider row) | all pass |
| `Request` (`crates/http-foundation/tests/data/request.json`, `oracle/recorders/http_foundation_request.php`) | 2,500 generated `$_SERVER` arrays through `new Request()` and 1,500 generated URIs through `create()` (452 rejected), each under a generated configuration of trusted proxies, header set, trusted hosts and method overrides, answering 42 calls in order (149,016 answers, with every exception class Request throws); 24 `getFormat()` MIME types, 21 `normalizeQueryString()` strings, 13 JSON bodies with the exception's code and previous exception | all match |
| `html_entity_decode()` (`crates/php-value/tests/data/html_entity_decode.json`) | every entity in four spellings; numeric references at every boundary and 2,000 random code points, decimal and hex; 3,000 generated strings; two runs of 100,000 `&` | all match |
| `json_decode()` (`crates/php-value/tests/data/json_decode.json`) | 6,248 documents (edge numbers, escapes, surrogates, UTF-8, control bytes, keys, depth 505 to 515, generated documents and mutations), each with and without `JSON_BIGINT_AS_STRING` | all match |

Mutation checks on the recorded requests:

| Mutation | Answers that differ |
|---|---|
| `HTTPS` compared case-sensitively with `off` | 698 |
| An invalid host throwing every time, instead of once and then `''` | 3,374 |
| Stripping an IPv4 port with more than one colon | 60 |
| Dropping the `/\` check from `prepareBaseUrl()` | 41 (and two upstream tests) |

The `/\` check was first caught only by the upstream tests. Script names
beginning with a host were then added to the recorder.

## Advisories

- **GHSA-3rg7-wf37-54rm** (PATH_INFO without a leading slash) is now
  `covered`:
  - The fix's `getBaseUrlData()` row runs in `testGetBaseUrl`.
  - In the oracle, v7.3.6 (the release before the fix) gives the path
    info `foo` for that row's input and three others, which fails them.
- The other two http-foundation advisories that the watch lists
  (GHSA-r925-mgvr-h9cc and GHSA-38cx-cq6f-5755) are already covered.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner, November 23).
