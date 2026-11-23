# Symfony request compatibility subset

Private workspace crate of symfony-rs, extracted from
[symfony-demo-rs](https://github.com/D-Nations/symfony-demo-rs). It implements the
request semantics already exercised by the demo, with the Symfony source
named in each module. PHP's own parsing (query strings, form bodies,
cookies, multipart) is written from the PHP manual and from PHP 8.4.27's
recorded answers, with no PHP source code (PLAN.md decision 8). It is not a
complete port of Symfony HttpFoundation.

| Module/API | Source behavior | Evidence | Limits |
|---|---|---|---|
| `query` | PHP's `$_GET` / `$_POST` (urlencoded) registration, recorded; Symfony `InputBag::get()` | 160 PHP-recorded query strings, each as a query string and as a form body (`tests/data/query_vectors.json`); query tests; demo form tests; request-lab HTTP tests | Fixed PHP defaults: 1,000 query variables (1,001 pairs in a form body, as PHP reads), nesting 64; values are stored as UTF-8 with invalid bytes replaced, and `is_utf8` reports a name whose value was not |
| `cookies` | PHP's `$_COOKIE`, recorded | 54 PHP-recorded Cookie headers (`tests/data/cookie_vectors.json`); cookie test; request-lab HTTP test | Arrays (`a[b]=1`) are kept and reachable through `all()`; `get()` returns strings only. Not a Symfony Cookie/ResponseHeaderBag API |
| `multipart` | PHP's `multipart/form-data` reading for `$_POST` and `request_parse_body()`, recorded | 274 PHP-recorded bodies (137 as POST and as PUT, `tests/data/multipart_vectors.json`) plus parser tests | Text fields only; file parts are read and discarded; fixed PHP upload limits (`max_input_vars` 1000, `max_file_uploads` 20) |
| `form_fields` | Symfony `Request::createFromGlobals()` method/format selection | 12 PHP-recorded content types (`tests/data/content_type_vectors.json`); demo write tests; request-lab HTTP tests; multipart comparison | URL-encoded and multipart bodies; no JSON payload API |
| `accept_language` | Symfony `Request`, `AcceptHeader`, `AcceptHeaderItem`, on `header_utils`'s tokenizer | Original 77-header test retained in the demo integration suite; float parsing test here | Caller supplies supported locale strings; no demo locale enum dependency |
| `parameter_bag` | Symfony `ParameterBag` on `php-value`: every method; `filter()` on php-value's `filter_var()`; `getEnum()` resolves the enum in a php-value `Registry` | ParameterBagTest 39 of 41 (2 `n/a`: PHP references; `testFilter` pending: `FILTER_VALIDATE_EMAIL`/`URL`); 2,450 recorded calls shared with `input_bag` (`tests/data/bags.json`) | Filters php-value does not reproduce are refused (`Class::NotPorted`); PHP's `E_DEPRECATED` for a truncated enum float is not raised |
| `input_bag` | Symfony `InputBag`: scalar-only getters throwing `BadRequestException`, checked `set`/`add`/`replace`, `\Stringable` returned as its string | InputBagTest 23 of 25 (2 `n/a`: PHP references); the recorded calls | As `parameter_bag` |
| `exception` | `BadRequestException`, `UnexpectedValueException`, and the PHP exceptions the bags throw, with `instanceof` (`Error::is_a`) | The bag tests' `expectException` checks | One error type; messages are upstream's, as bytes |
| `header_utils` | Symfony `HeaderUtils` (v8.1.11): `split()` (one linear pass), `combine()`, `toString()`, `quote()`, `unquote()`, `makeDisposition()` with v8.1.11's `/D` anchor (a fallback ending in a newline is refused) | HeaderUtilsTest 9 of 13, including GHSA-r925-mgvr-h9cc's test; 3,000 recorded `split()` calls, 633 `quote()`/`unquote()` strings and 2,880 `makeDisposition()` calls (`tests/data/header_utils.json`) | No `parseQuery()`; `toString()` refuses null, array and object values (PHP's TypeError) |
| `cookie` | Symfony `Cookie`: constructor, `create()`, `fromString()`, the `with*()` copies, the `Set-Cookie` text | CookieTest 53 of 53; 2,112 recorded `fromString()` calls and 165 constructions (`tests/data/cookie.json`) | `$expire` texts go through php-value's `strtotime()` subset; one it does not read is refused (`Class::NotPorted`) |
| `header_bag` | Symfony `HeaderBag` and `ResponseHeaderBag`: headers, Cache-Control directives, computed Cache-Control, Date, cookies, `allPreserveCase()` | HeaderBagTest 20 of 20, ResponseHeaderBagTest 19 of 19; 400 recorded call scripts (`tests/data/header_bags.json`) | `ResponseHeaderBag::makeDisposition()` is `header_utils::make_disposition`; values are strings or null; `getDate()` reads php-value's `DATE_RFC2822` subset and refuses other zones |
| `ip_utils` | Symfony `IpUtils` (v8.1.11): `checkIp()` (with the IPv4-mapped check), `checkIp4()`, `checkIp6()`, `anonymize()`, `isPrivateIp()`, `PRIVATE_SUBNETS`, `PRIVATE_RANGES`, the results cache | IpUtilsTest 18 of 19 (1 `n/a`: IPv6 always available), covering GHSA-38cx-cq6f-5755; 37,802 recorded calls with odd netmasks, plus php-value's IP built-ins (`tests/data/ip_utils.json`) | Arguments are `&str`; the cache is per thread (PHP's lives for a request). Not yet used by `scheme`, which trusts exact proxy IPs |
| `server_bag` | Symfony `ServerBag::getHeaders()`, including Basic, Digest and Bearer `Authorization` | ServerBagTest 14 of 14; 360 recorded `$_SERVER` arrays (`tests/data/server_bag.json`) | PHP's TypeError messages for array and object `Authorization` values |
| `scheme` | Symfony `Request` trusted HTTPS proxy subset | Demo proxy/security regression tests, request-lab untrusted-header test | Axum adapter; exact trusted peer IPs and one `X-Forwarded-Proto` value only |

HTTP adapters bound body reads using `POST_MAX_SIZE` (8 MiB), and supply an
empty body after a read failure. No socket I/O or body buffering is done by
the parsers. The runtime dependencies are the already-used Axum, for
`scheme`'s request-parts adapter, and the workspace's `php-value`. JSON, serde and base64 are test dependencies.

Run this crate independently with:

```sh
cargo test -p symfony-http-foundation
```

symfony-demo-rs's `src/request/mod.rs` re-exports this API. The second consumer is
[`request-lab`](../../examples/request-lab/README.md); it has no dependency on
the demo crate, its database, templates, translations or built assets.

The bag, HeaderUtils, IpUtils, Cookie, header-bag and ServerBag vectors
(and php-value's date vectors) are recorded by `scripts/record.sh
http-foundation` (`oracle/recorders/http_foundation_*.php`) at the pinned
v8.1.11, which the oracle mounts over the demo's v8.1.10 vendor copy.
`scripts/record.sh http-foundation-releases` records IpUtils and
`makeDisposition()` from earlier releases into `oracle/out/`, for the
advisory checks.

The request vectors are recorded in the oracle image: `scripts/record.sh multipart`
(`oracle/recorders/multipart*.py`) and `scripts/record.sh request-vars`
(`oracle/recorders/request_vars*`). The first 156 multipart bodies are the
ones moved from symfony-demo-rs; the rest were added for the rewrite.
The application-specific language test remains in symfony-demo-rs at
`tests/request_compatibility.rs`, so it continues to verify the demo's actual
enabled locale set rather than baking that set into this crate.

## Differences from the earlier implementation

The rewrite (M1-D request provenance gate) follows PHP's recorded answers
where the earlier, php-src-derived code did not:

- A `+` in a cookie value stays `+` (PHP `rawurldecode`s cookie values);
  the earlier code made it a space.
- A cookie named `a[b]` is an array, as in PHP; the earlier code kept the
  name `a[b]` as written.
- A URL-encoded form body reads 1,001 pairs, empty pairs counted, as PHP
  does; a query string reads 1,000 non-empty pairs. The earlier code used
  the query-string rule for both, and decoded the body as UTF-8 before
  parsing.
- `form_fields` ignores spaces and tabs around the content type, as the
  HTTP server in front of PHP drops them.

## Not implemented

Not implemented: general Request/Response objects, FileBag, AcceptHeader
as an API, `parse_str()`/`HeaderUtils::parseQuery()`, uploaded-file handling, JSON payload access, sessions, response cache
negotiation, streamed/binary responses, general proxy handling, routing,
HttpKernel events, a service container or bundle configuration. The next
milestone should choose a small additional compatibility contract and port
its upstream tests, rather than treating these omissions as supported.

MIT; derived from Symfony Demo (copyright Fabien Potencier) and the existing
Rust port. See the repository's LICENSE and source mappings. Unofficial;
not affiliated with or endorsed by Symfony.
