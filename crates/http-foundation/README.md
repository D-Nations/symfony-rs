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
| `query` | PHP's `$_GET` / `$_POST` (urlencoded) registration, recorded, on php-value's `vars::register()` (shared with `parse_str()`); `vars()` gives PHP's binary-safe array; Symfony `InputBag::get()` | 160 PHP-recorded query strings, each as a query string and as a form body (`tests/data/query_vectors.json`); query tests; demo form tests; request-lab HTTP tests | Fixed PHP defaults: 1,000 query variables (1,001 pairs in a form body, as PHP reads), nesting 64; values are stored as UTF-8 with invalid bytes replaced, and `is_utf8` reports a name whose value was not |
| `cookies` | PHP's `$_COOKIE`, recorded | 54 PHP-recorded Cookie headers (`tests/data/cookie_vectors.json`); cookie test; request-lab HTTP test | Arrays (`a[b]=1`) are kept and reachable through `all()`; `get()` returns strings only. Not a Symfony Cookie/ResponseHeaderBag API |
| `multipart` | PHP's `multipart/form-data` reading for `$_POST`, `$_FILES` and `request_parse_body()`, recorded: text fields, and file parts with their `name`, `filename`, first `Content-Type`, data, completeness and the `MAX_FILE_SIZE` before them; PHP's warnings (`Warning::text()`) | 274 PHP-recorded bodies (137 as POST and as PUT, `tests/data/multipart_vectors.json`) plus parser tests; the file cases of `tests/data/sapi.json` | Fixed PHP upload limits (`max_input_vars` 1000, `max_file_uploads` 20); `$_FILES` itself is built by `sapi` |
| `form_fields` | Symfony `Request::createFromGlobals()` method/format selection | 12 PHP-recorded content types (`tests/data/content_type_vectors.json`); demo write tests; request-lab HTTP tests; multipart comparison | URL-encoded and multipart bodies; no JSON payload API |
| `accept_header` | Symfony `AcceptHeader` and `AcceptHeaderItem`: `fromString()`, canonical keys, sorting, `has()`, `get()` by RFC 9110 media ranges, `filter()`, the string forms | AcceptHeaderTest 6 of 6, AcceptHeaderItemTest 5 of 5 (every provider row); 1,500 recorded headers with 6,000 `has()`/`get()` queries, 2,000 `get()` calls among overlapping ranges, 21 `filter()` calls, 500 items (`tests/data/accept_header.json`) | Attribute values are strings (PHP's coercion of `true` to `"1"`); `filter()` patterns run on php-pcre, a pattern it does not translate is refused (`Class::NotPorted`); TypeError messages lack PHP's "called in" suffix |
| `accept_language` | Symfony `Request::getLanguages()` / `getPreferredLanguage()` on `accept_header` | Original 77-header test retained in the demo integration suite; the demo's locale flows | Caller supplies supported locale strings; no demo locale enum dependency |
| `parameter_bag` | Symfony `ParameterBag` on `php-value`: every method; `filter()` on php-value's `filter_var()`; `getEnum()` resolves the enum in a php-value `Registry` | ParameterBagTest 39 of 41 (2 `n/a`: PHP references; `testFilter` pending: `FILTER_VALIDATE_EMAIL`/`URL`); 2,450 recorded calls shared with `input_bag` (`tests/data/bags.json`) | Filters php-value does not reproduce are refused (`Class::NotPorted`); PHP's `E_DEPRECATED` for a truncated enum float is not raised |
| `input_bag` | Symfony `InputBag`: scalar-only getters throwing `BadRequestException`, checked `set`/`add`/`replace`, `\Stringable` returned as its string | InputBagTest 23 of 25 (2 `n/a`: PHP references); the recorded calls | As `parameter_bag` |
| `exception` | `BadRequestException`, `UnexpectedValueException`, and the PHP exceptions the bags throw, with `instanceof` (`Error::is_a`) | The bag tests' `expectException` checks | One error type; messages are upstream's, as bytes |
| `header_utils` | Symfony `HeaderUtils` (v8.1.11): `split()` (one linear pass), `combine()`, `toString()`, `quote()`, `unquote()`, `makeDisposition()` with v8.1.11's `/D` anchor (a fallback ending in a newline is refused), `parseQuery()` | HeaderUtilsTest 13 of 13, including GHSA-r925-mgvr-h9cc's test; 3,000 recorded `split()` calls, 633 `quote()`/`unquote()` strings, 2,880 `makeDisposition()` calls and 3,000 `parseQuery()` calls (`tests/data/header_utils.json`) | `parseQuery()` runs on php-value's `parse_str()`; `toString()` refuses null, array and object values (PHP's TypeError) |
| `cookie` | Symfony `Cookie`: constructor, `create()`, `fromString()`, the `with*()` copies, the `Set-Cookie` text | CookieTest 53 of 53; 2,112 recorded `fromString()` calls and 165 constructions (`tests/data/cookie.json`) | `$expire` texts go through php-value's `strtotime()` subset; one it does not read is refused (`Class::NotPorted`) |
| `header_bag` | Symfony `HeaderBag` and `ResponseHeaderBag`: headers, Cache-Control directives, computed Cache-Control, Date, cookies (`cookies_mut()` lends the bag's own, as PHP's `getCookies()` returns its objects), `allPreserveCase()` | HeaderBagTest 20 of 20, ResponseHeaderBagTest 19 of 19; 400 recorded call scripts (`tests/data/header_bags.json`) | `ResponseHeaderBag::makeDisposition()` is `header_utils::make_disposition`; values are strings or null; `getDate()` reads php-value's `DATE_RFC2822` subset and refuses other zones |
| `ip_utils` | Symfony `IpUtils` (v8.1.11): `checkIp()` (with the IPv4-mapped check), `checkIp4()`, `checkIp6()`, `anonymize()`, `isPrivateIp()`, `PRIVATE_SUBNETS`, `PRIVATE_RANGES`, the results cache | IpUtilsTest 18 of 19 (1 `n/a`: IPv6 always available), covering GHSA-38cx-cq6f-5755; 37,802 recorded calls with odd netmasks, plus php-value's IP built-ins (`tests/data/ip_utils.json`) | Arguments are `&str`; the cache is per thread (PHP's lives for a request). Not yet used by `scheme`, which trusts exact proxy IPs |
| `server_bag` | Symfony `ServerBag::getHeaders()`, including Basic, Digest and Bearer `Authorization` | ServerBagTest 14 of 14; 360 recorded `$_SERVER` arrays (`tests/data/server_bag.json`) | PHP's TypeError messages for array and object `Authorization` values |
| `file` | Symfony `File` (`File.php`) with the `\SplFileInfo` methods callers use: `getMimeType()`, `guessExtension()`, `move()` (`move_to`), `getContent()`; the File exceptions in `exception` | FileTest 9 of 9 | MIME types come from `symfony-mime`'s default `MimeTypes`, its finfo port first (a pure-Rust libmagic, so some answers differ from PHP's; PLAN.md decision 18), then the `file` command; a failed rename's warning is given as a web SAPI escapes it |
| `uploaded_file` | Symfony `UploadedFile`: client name, extension, path and MIME type, `guessClientExtension()`, errors and their messages, `isValid()`, `move()`, `getMaxFilesize()` | UploadedFileTest 23 of 23; 108 recorded `parseFilesize()` texts, 126 `getMaxFilesize()` ini pairs with their message, 55 names, 33 MIME types, 26 error codes (`tests/data/files.json`) | `getMaxFilesize()` reads PHP's defaults (8M, 2M); `max_filesize()` takes other ini texts. Outside test mode, `isValid()` and `move()` need the upload code to register its temporary files (`php::register_uploaded_file`, cleared with `forget_uploaded_files` at the end of the request) |
| `file_bag` | Symfony `FileBag`: `$_FILES` arrays (and the array-named fields' layout) into `UploadedFile` values, empty uploads removed from lists | FileBagTest 9 of 9; 71 recorded arrays with PHP's coercions and TypeErrors | A sibling field that is a string or object where `name` is an array (PHP string offsets), nesting over 512 and non-UTF-8 keys are refused (`Class::NotPorted`); TypeError messages lack PHP's "called in" suffix |
| `request` | Symfony `Request` (v8.1.11): `new`/`initialize()`, `create()` with its URI checks, `createFromGlobals()`, `duplicate()`, `__toString()`, `overrideGlobals()`; trusted proxies and headers (`Forwarded`, `X-Forwarded-*`, conflicts), trusted hosts; client IPs, scheme, host, port (`SERVER_PORT` coerced to `getPort()`'s `int|string|null` return type: bools and floats that fit to ints, other floats to their 14-digit string, arrays a `\TypeError`), base URL and path, path info, request URI (IIS included), URIs; method and overrides; formats and MIME types; languages, charsets, encodings, content types; content, `toArray()`, `getPayload()`; the bags behind accessors, `set_*()` deprecated as upstream | RequestTest 113 of 120 (3 `n/a`: a PHP subclass; 4 pending: Session, intl), covering GHSA-3rg7-wf37-54rm; 4,000 recorded requests answering 42 calls each (149,016 answers), 24 `getFormat()` MIME types, 21 `normalizeQueryString()` strings and 13 JSON bodies and 23 typed `SERVER_PORT` values for `getPort()` (`tests/data/request.json`) | Static properties are per thread, `reset_statics()` ending a PHP request; the session methods and `\Locale::setDefault()` are not ported; a trusted host pattern that does not compile alone is refused (`Class::NotPorted`); an array or object where PHP converts to a string is refused; URL methods follow PHP on Linux (`/` separator) |
| `request_stack` | Symfony `RequestStack` (v8.1.11): `push()`, `pop()`, the current, main and parent requests, `resetRequestFormats()`; requests are `RequestHandle`s (php-value `Handle<Request>`), the stack is one too | RequestStackTest 5 of 5; symfony-http-kernel's recorded lifecycles | Sessions are not ported: `getSession()` always throws `SessionNotFoundException`, as PHP does for a request without one |
| `response` | Symfony `Response` (v8.1.11; `setContent()` returns a `Result` and `getContent()` an `Option`, for `StreamedResponse`; `sendContent()` and `send()` return a throwable): status codes and texts (`STATUS_TEXTS`, the `HTTP_*` constants), content, protocol version, charset, `__toString()`; Cache-Control (`setPublic()`/`setPrivate()`/`setImmutable()`, max-age, s-maxage, stale directives, TTLs, `setCache()` with PHP's coercions and `\TypeError`s), Date/Expires/Last-Modified, ETag, Vary, `setContentSafe()`; `prepare()` against a `Request` (content type and charset, HEAD, 1xx/204/304, HTTP/1.0 `Pragma`, the IE-over-SSL rule, secure cookies); `isNotModified()`; `sendHeaders()` (103 early hints included), `sendContent()`, `send()`, `closeOutputBuffers()`; the `headers` property behind accessors, `set_headers()` deprecated as upstream | ResponseTest 80 of 81 and ResponseTestCase (1 `n/a`: a PHP subclass); 1,200 recorded call scripts (6,491 calls with their process calls, 26,864 reader answers, 32 failed constructions) 300 `closeOutputBuffers()` calls and 121 protocol version and typed `SERVER_PROTOCOL` pairs through `prepare()` (`tests/data/response.json`) | Sending records PHP's `header()`, `header_remove()`, `headers_send()`, `echo`, output-buffer and finish calls in `php::Output`; applying them as a SAPI does is the HTTP adapter's job, not claimed here. `strtotime()` texts php-value refuses (an `If-Modified-Since` of `-1`) are refused (`Class::NotPorted`); `$statusTexts` is a constant; `\DateTimeInterface` arguments are php-value's `DateTimeImmutable` |
| `json_response` | Symfony `JsonResponse` (v8.1.11), a `Response` by `Deref`: the constructor (data, or JSON text with its `\TypeError`; null as `{}`), `fromJsonString()`, `setData()` with `getEncodingOptions()` (`DEFAULT_ENCODING_OPTIONS` 15; `JSON_THROW_ON_ERROR`'s `\JsonException`, else `json_last_error_msg()` as `\InvalidArgumentException`; what `jsonSerialize()` throws passed on), `setJson()`, `setCallback()` (upstream's pattern through php-pcre, the reserved words, `/**/cb(json);` as `text/javascript`), `setEncodingOptions()` (decode to `stdClass`, encode again), the Content-Type rule of `update()` | JsonResponseTest 22 of 22; 1,500 recorded call scripts, 3,255 callback names (`tests/data/json_redirect.json`) | Headers are given as `(name, values)` pairs, PHP's array; `json_encode()`/`json_decode()` are php-value's |
| `redirect_response` | Symfony `RedirectResponse` (v8.1.11), a `Response` by `Deref`: the constructor (status validity, empty URL, not-a-redirect messages in PHP's order; a 301's default Cache-Control removed unless the headers array has a `cache-control` key in any case), `getTargetUrl()`, `setTargetUrl()` (upstream's HTML page with `htmlspecialchars($url, ENT_QUOTES, 'UTF-8')`, Location, `text/html; charset=utf-8`) | RedirectResponseTest 8 of 8; 1,000 recorded constructions (same file) | Headers are given as `(name, values)` pairs, PHP's array |
| `streamed_response` | Symfony `StreamedResponse` (v8.1.11): a `Response` carrying the subclass's state (`Response::stream()` is `instanceof`), since `Response`'s own methods call what it overrides: the constructor (a callback, chunks, or null; `[]` sets none), `setCallback()`, `getCallback()`, `setChunks()` (each chunk echoed, then `@ob_flush()` and `flush()`), `sendHeaders()` once after a non-informational call, `sendContent()` once (`\LogicException` without a callback; the callback's throwable passed on), `setContent()` (null marks the content sent, as `prepare()` for HEAD and `setNotModified()` do; other content a `\LogicException`), `getContent()` `false` | StreamedResponseTest 13 of 13; 1,500 recorded scripts through every method the subclass changes, with the process calls in order (`tests/data/streamed_response.json`) | The callback is a Rust closure writing through `streamed_response::echo`/`flush`; chunks are a list (a generator's lazy chunks are a callback); `is_callable()` on an array of strings is not reproduced |
| `event_stream_response` | Symfony `EventStreamResponse` (v8.1.11): a streamed response carrying an `EventStream` (`$retry`, `$lastRetry`), which the callback is given as PHP gives `$this`; the default headers added by exact key, as `+=` does; `setCallback()` wrapping before a callback is set; `sendEvent()` (an event without a retry gets the response's when it differs from the last sent; each part echoed, and outside the CLI `closeOutputBuffers(0, true)` and `flush()`); `getRetry()`, `setRetry()`; the loop ends once `connection_aborted()` | EventStreamResponseTest 14 of 14; 1,200 recorded scripts (`tests/data/event_stream.json`); the non-CLI flush path by a test of its own (the oracle's `PHP_SAPI` is the CLI's) | The callback returns its events as an iterator; an event callback set after one is called without `$this` and its result ignored (PHP's `ArgumentCountError` for a callback declaring the parameter is not reproduced) |
| `server_event` | Symfony `ServerEvent` (v8.1.11): the fields and their accessors; `getIterator()` as `parts()` (comment, id, retry above 0, type, written when PHP finds them truthy; data lines split on CR, LF and CRLF; line breaks removed from id and type) | EventStreamResponseTest; the recorded scripts | Data is a string or a list: a generator's lines are collected first (same bytes) |
| `streamed_json_response` | Symfony `StreamedJsonResponse` (v8.1.11): a streamed response writing a JSON document from PHP data (an array or a `\Traversable` object); `application/json` unless a truthy `Content-Type` is given; the structure around objects encoded with the `"__symfony_json__"` placeholder and split there (a placeholder already in it, as a value or the key of a leaf, written back as itself; as the key of an array not counted, so the parts misalign as in PHP); iterables a list when their first key is `0`, else an object, keys encoded without `JSON_NUMERIC_CHECK`; `JsonSerializable` objects encoded, other `\Traversable` ones streamed | StreamedJsonResponseTest 15 of 15; 1,500 recorded scripts (`tests/data/streamed_json.json`), each echo compared as a chunk of its own | `\Traversable` and `\JsonSerializable` are what `PhpObject::instance_of` answers; a key that is an array is `"Array"` without PHP's warning; the `TypeError` has no "called in" suffix |
| `generator` | A PHP `\Generator` (no Symfony file): a lazy body yielding key and value pairs, run as `foreach` asks for them; `foreach`'s rewind (a generator stopped at its first yield gives it again); "Cannot traverse an already closed generator", "Cannot rewind a generator that was already run", "Cannot resume an already running generator"; no properties; `\Traversable`, `\Iterator` | Unit tests; the StreamedJsonResponse scripts | Only `foreach` (and `traverse()`): no `send()`, `getReturn()` or `yield from` |
| `php` (`Globals`, `Stream`) | What `Request` reads from the PHP process: superglobals, `php://input`, `request_parse_body()`'s answer, `request_order`/`variables_order`; what `Response` sends (`Output`: the calls in order, with `PHP_SAPI`, `headers_sent()`, `function_exists()` answers, `ob_get_status()` and `default_mimetype`); PHP streams on bytes (`fread()`, `feof()`, `rewind()`, `stream_get_contents()`); `is_uploaded_file()` / `move_uploaded_file()` | RequestTest's content and globals tests | Per thread (the uploaded file list too); `sapi::start()` fills `Globals`, `parsed_body` included |
| `sapi` | PHP-FPM (PHP 8.4.27, no php.ini): `start()` builds `$_SERVER` (environment, FastCGI parameters last first, `FCGI_ROLE`, `PHP_SELF`, `PHP_AUTH_*`, request times, `argv`/`argc`), `$_GET`, `$_COOKIE`, `$_POST` and `$_FILES` (POST only; `post_max_size`; uploaded files written, with every `UPLOAD_ERR_*` outcome PHP gives, the malformed-name stop and nameless parts), `$_REQUEST`, `php://input`, `request_parse_body()`'s answer for PUT, DELETE, PATCH and QUERY; `finish()` removes unmoved uploads. `Sapi`: `header()` (status lines, replace, `Location`'s 302, `WWW-Authenticate`'s 401, the `Content-Type` charset rewrite, CR/LF/NUL refusals), `header_remove()`, `http_response_code()`, `headers_list()`, `headers_sent()`, `setcookie()`/`setrawcookie()`, `echo`, output buffers, `flush()`, `fastcgi_finish_request()`, `ini_set()` of `default_mimetype`/`default_charset`; `apply()` of `Response`'s recorded calls; the CGI response PHP-FPM writes (`Status:`, default content type, 304, HEAD) | 115 recorded FastCGI requests and 946 output scripts, 600 of them generated (`tests/data/sapi.json`, `scripts/record.sh runtime-fpm`), all matching, byte for byte for responses; 27 mutation checks of its rules, all caught; RequestFunctionalTest 2 of 2 and five of ResponseFunctionalTest's six cookie fixtures (`tests/functional.rs`) | Warnings are returned, never displayed, and lack PHP's "output started at"; `setcookie()` with an expiry time is refused; reason phrases for codes the recordings do not show follow Symfony's table (418 has none, as recorded); `default_charset` names outside a short list warn |
| `scheme` | Symfony `Request` trusted HTTPS proxy subset | Demo proxy/security regression tests, request-lab untrusted-header test | Axum adapter; exact trusted peer IPs and one `X-Forwarded-Proto` value only |

HTTP adapters bound body reads using `POST_MAX_SIZE` (8 MiB), and supply an
empty body after a read failure. No socket I/O or body buffering is done by
the parsers. The runtime dependencies are the already-used Axum, for
`scheme`'s request-parts adapter, and the workspace's `php-value`, `php-pcre`, `symfony-deprecation-contracts` (Request's deprecations), and for the file classes
`symfony-mime`, `symfony-filesystem` and `symfony-finder`. JSON, serde and base64 are test dependencies.

Run this crate independently with:

```sh
cargo test -p symfony-http-foundation
```

symfony-demo-rs's `src/request/mod.rs` re-exports this API. The second consumer is
[`request-lab`](../../examples/request-lab/README.md); it has no dependency on
the demo crate, its database, templates, translations or built assets.

The Request vectors are recorded by `scripts/record.sh
http-foundation-request` (`oracle/recorders/http_foundation_request.php`).
The Response vectors are recorded by `scripts/record.sh
http-foundation-response` (`oracle/recorders/http_foundation_response.php`),
with `header()`, `ob_*()`, `ini_set()` and the clock replaced in the
HttpFoundation namespace. The JsonResponse and RedirectResponse vectors
are recorded by `scripts/record.sh http-foundation-json-redirect`
(`oracle/recorders/http_foundation_json_redirect.php`).

The UploadedFile and FileBag vectors are recorded by `scripts/record.sh
http-foundation-files` (`oracle/recorders/http_foundation_files.php`); the
fixtures in `tests/fixtures/file` are upstream's `Tests/File/Fixtures`.

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

Not implemented: general Request/Response objects, `File\Stream`,
JSON payload access, sessions, response cache
negotiation, streamed/binary responses, general proxy handling, routing,
HttpKernel events, a service container or bundle configuration. The next
milestone should choose a small additional compatibility contract and port
its upstream tests, rather than treating these omissions as supported.

MIT; derived from Symfony Demo (copyright Fabien Potencier) and the existing
Rust port. See the repository's LICENSE and source mappings. Unofficial;
not affiliated with or endorsed by Symfony.
