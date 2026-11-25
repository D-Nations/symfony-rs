# M2-A: the HTTP adapter, and request-lab on the ported kernel

November 25. Branch `m2/axum-adapter-request-lab`. Crates touched:
`symfony-runtime` (new), `symfony-http-foundation` (`sapi`, `multipart`,
`cookies`, `query`, `php`), `php-value` (`vars::register_value()`),
`symfony-http-kernel` (README only), `request-lab`. Oracle: a PHP-FPM
image (`oracle/fpm.Dockerfile`, compose service `php-fpm`).

Built on HttpKernel stages 1–4. Until now request-lab ran on the typed
Axum subset (`Kernel<C>`); the ported kernel had no way to be served over
HTTP. This stage adds the HTTP adapter and moves request-lab onto the
ported kernel. The converter fixture still uses `Kernel<C>`; it moves next.

## Scope

| PHP file / behaviour | Rust | Upstream tests |
|---|---|---|
| symfony/runtime `Runner/Symfony/HttpKernelRunner.php` (v8.1.9) | `crates/runtime/src/http_kernel_runner.rs` | none covers it (the runtime's 19 PHPUnit tests are for `SymfonyRuntime` and the FrankenPHP runner: pending) |
| nginx's `fastcgi_params` and Symfony's documented location; the response nginx makes of PHP-FPM's | `crates/runtime/src/server.rs` (Upstream: none) | — |
| PHP-FPM: superglobals, uploads, `request_parse_body()`, `header()` and output | `crates/http-foundation/src/sapi.rs` (Upstream: none) | RequestFunctionalTest 2 of 2; ResponseFunctionalTest `testCookie` 5 of its 6 fixtures (row pending) |
| request-lab | `examples/request-lab/src/lib.rs` | its 4 HTTP tests, unchanged |

## Prior art and choice

Searched GitHub, crates.io and docs.rs (PLAN.md stack row "The HTTP
adapter"):

| Option | What it does | For | Against |
|---|---|---|---|
| ext-php-rs embed SAPI, ripht-php-sapi | Run php-src in-process; fill `$_SERVER` from a request | Real PHP behaviour | FFI, `unsafe`, php-src (PLAN.md decision 8, AGENTS): excluded; nothing to port |
| fastcgi-client (its Axum example) | Proxy an Axum request to a running PHP-FPM | Shows the HTTP → FastCGI mapping | Needs PHP at run time; the application here is Rust |
| symfony/psr-http-message-bridge `HttpFoundationFactory` | PSR-7 request → `Request` directly, server params as given | Simple, no globals | Skips `createFromGlobals()` (no `request_parse_body()`, `$_FILES` layout, `PHP_AUTH_*`, `argv`), so a Request PHP would never build |
| symfony/runtime `SymfonyRuntime` + `HttpKernelRunner` | `createFromGlobals()`, `handle()`, `send()`, `fastcgi_finish_request()`, `terminate()` | The production path of a Symfony app | Needs superglobals and a SAPI |
| FrankenPHP / RoadRunner (Go) | Embed PHP, fill superglobals per request in worker mode | Confirms the per-request global model | Go, embeds PHP |

Chosen: emulate the deployment itself. nginx's parameters (from its
documentation) feed an emulation of PHP-FPM (from the manual and
recordings) that fills the superglobals; `createFromGlobals()` and a port
of `HttpKernelRunner` run the kernel; what `Response::send()` recorded is
applied as PHP-FPM applies it, and turned into the HTTP response as nginx
turns PHP-FPM's. Each PHP request runs on its own blocking thread (a
PHP-FPM worker), which keeps the per-thread superglobals of one request
together; the response is sent at `fastcgi_finish_request()` and
`terminate()` runs after it. New dependency: tokio `rt` and `sync`
(already in the lock through axum).

## What the adapter maps

- **Request (nginx):** `QUERY_STRING`, `REQUEST_METHOD`, `CONTENT_TYPE`,
  `CONTENT_LENGTH` (a chunked body's length once buffered), `SCRIPT_NAME`,
  `REQUEST_URI`, `DOCUMENT_URI`, `DOCUMENT_ROOT`, `SERVER_PROTOCOL`,
  `REQUEST_SCHEME`, `HTTPS`, `GATEWAY_INTERFACE`, `SERVER_SOFTWARE`,
  `REMOTE_ADDR`/`PORT` (from `ConnectInfo`), `SERVER_ADDR`/`PORT`,
  `SERVER_NAME`, `REDIRECT_STATUS`, `SCRIPT_FILENAME`, then `HTTP_*`
  headers (underscored names and `Proxy` dropped; repeats joined).
  `client_max_body_size` (413), and Symfony's location refusing
  `/index.php...` and `*.php` (404).
- **Request (PHP-FPM, recorded):** `$_SERVER` (environment, parameters
  last first with PHP-FPM's duplicate rule, `FCGI_ROLE`, `PHP_SELF`,
  `PHP_AUTH_USER`/`PW`/`DIGEST`, request times, `argv`/`argc`); `$_GET`;
  `$_COOKIE`; `$_POST` and `$_FILES` for POST only (exact method), within
  `post_max_size`; uploaded files written as `php` + 19 characters,
  registered for `is_uploaded_file()` and removed after the request;
  every `UPLOAD_ERR_*` PHP gave (no file, ini size, `MAX_FILE_SIZE`,
  partial), the type cut at `;`, the basename rule, nameless parts, the
  stop at a malformed name, `max_file_uploads`; `$_REQUEST`'s recursive
  merge; `php://input` (empty after reading a multipart body);
  `request_parse_body()` for PUT, DELETE, PATCH and QUERY (form,
  multipart, `[[], []]` past `post_max_size`, a throw otherwise).
- **Response (PHP-FPM, recorded):** `header()` with status lines,
  `$replace`, `$response_code`, `Location`/`WWW-Authenticate` statuses, the
  `Content-Type` charset rewrite and the refusals; `header_remove()`;
  `http_response_code()`; `headers_list()`; `headers_sent()`;
  `setcookie()`/`setrawcookie()` without expiry; `echo`, output buffers,
  `flush()`, `fastcgi_finish_request()`; `ini_set()` of
  `default_mimetype`/`default_charset`; the CGI response: `Status:` (from
  the status line, a `Status:` header, or code and reason), the default
  content type, 304 without one, no body for HEAD; an uncaught throwable
  as a 500.
- **Response (nginx):** the status from `Status:`, the other lines as
  headers.

## What it does not map

See PLAN.md decision 20: reason phrases (hyper writes canonical ones),
`X-Powered-By` (off by default), invalid CGI header lines (dropped and
logged; nginx gives 502 for some). Also: warnings are logged and never
displayed; `setcookie()` with an expiry is refused; no static files; the
web server's 404 has no HTML page; `headers_sent()` that `Response` reads
is the value at the start of the request (output before `send()` is not
seen there); FastCGI parameters are written from nginx's documentation,
not recorded from nginx (nginx is not in the oracle). Sessions, streamed
responses and HTTP/2 specifics are not part of the adapter.

## request-lab

Routes are a `RouteCollection` (`_controller` defaults naming
`App\Controller\LabController::*`), matched by a `RedirectableUrlMatcher`
whose `redirect()` is the lab's `slashRedirect` action, through
`RouterListener`; `LabController` is declared in a `ClassRegistry` for the
`ControllerResolver`; the `ArgumentResolver` passes `Request $request` and
`string $path`; `HttpKernel` runs it (`handle_all_throwables` on); a
`kernel.exception` listener answers `BadRequestHttpException` with JSON and
other HTTP exceptions with an empty body. The item requirement is a
pattern for 1 to `PHP_INT_MAX`. JSON uses `JSON_FORCE_OBJECT |
JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE |
JSON_INVALID_UTF8_SUBSTITUTE`, which writes the bytes the lab wrote before.

Its tests (`examples/request-lab/tests/requests.rs`) pass unchanged. A
throwaway harness sent 29 requests to the old and the new lab (both
inspect paths, forms for POST/PUT and multipart, invalid UTF-8 in the
query, the scalar error, `/error`, `/redirect`, slash redirects for GET,
HEAD and POST, 405s, items at the bounds, `/`, unknown and odd paths):
status and body are identical for all 29, and so is every header the old
lab sent. The new lab adds Symfony's `Cache-Control: no-cache, private` and
`Date` to every response and `Content-Type: text/html; charset=UTF-8` to
bodiless ones. The harness also found that `/index.php/inspect` reached
`/inspect` (Symfony's base URL detection strips the front controller)
until the adapter refused the front controller's path as Symfony's nginx
location does; it is a 404 again, as before.

## Evidence

| What | Cases | Result |
|---|---|---|
| FastCGI requests (`crates/http-foundation/tests/data/sapi.json`, `oracle/recorders/runtime_fpm.php` with `runtime_fpm_index.php`) | 115: methods, query strings (incl. `max_input_vars`), cookies, headers, 13 `Authorization` forms, environment overrides, `PATH_INFO`, forms and JSON, multipart with files of every outcome and 18 odd field names, bodies past and at `post_max_size`, PUT/PATCH/DELETE/QUERY bodies | `$_SERVER`, `$_GET`, `$_POST`, `$_COOKIE`, `$_REQUEST`, `$_FILES`, uploaded file contents, `php://input` and `request_parse_body()` all match |
| Output scripts (same file) | 946: 173 named for GET and HEAD (status lines, content types, bad lines, removals, cookies, `Response::send()` shapes) and 600 generated | every CGI response byte for byte, every answer, warning (without "output started at"), `headers_list()` and final status match |
| RequestFunctionalTest, ResponseFunctionalTest | 2 tests (3 methods each); 5 cookie fixtures against upstream's `.expected` | pass |
| HttpKernelRunner and the adapter (`crates/runtime/tests/server.rs`) | parameters, responses, early response before `terminate()`, a 500, refused paths, 413, upload cleanup, each SAPI's finish | pass |

The FPM image is `php:8.4.27-fpm-bookworm` (the CLI oracle's version and
libc), no php.ini, `display_errors` off; the recorder is its own FastCGI
client in PHP. Re-recording differs only in the container's `HOSTNAME`.

Mutation checks on the recorded vectors (each made once in `sapi.rs` or
`multipart.rs`, then `sapi_recorded` and `functional` run): all 27 caught.

| Mutation |
|---|
| `$replace` ignored |
| `Location` keeps 201 → keeps 200 instead |
| `WWW-Authenticate` sets 403 |
| `;charset=` rewrite with a space |
| default content type without the space |
| 304 keeps its content type |
| `$response_code` keeps the status line |
| every `Status:` header written |
| HEAD gets a body |
| 418 gets a reason |
| parameters in sent order |
| empty `PHP_AUTH_PW` set |
| `argv` split at `&` |
| `post` read as POST |
| multipart POST keeps `php://input` |
| `MAX_FILE_SIZE` inclusive |
| basename at `/` only |
| malformed name skips one file |
| nameless counter not advanced |
| `$_REQUEST` not merged recursively |
| `max_file_uploads` off by one |
| file type not cut at `;` |
| `post_max_size` inclusive |
| `setcookie()` not encoding |
| bracket keys keep leading spaces |
| `flush()` after finish does not end the script |
| `Content-Type` does not stop the default |

## Found on the way

- The uploaded-file list (`php::register_uploaded_file()`) was
  process-wide: with concurrent requests one could `move_uploaded_file()`
  another's upload, and one request's end cleared the other's list. It is
  now per thread, as a request runs on one.
- The multipart parser treated a file part after 20 non-empty filenames
  as a failure only when it was itself non-empty; PHP stops at any file
  part then (recorded), and reads no later file.

## Listener seam (PLAN.md)

The deferred listener seam waited for request-lab and the fixture to
move to the ported kernel. request-lab has moved; its listeners are the
EventDispatcher's, so the seam belongs on the ported kernel's dispatch,
not on `Kernel<C>`. It is built once the converter fixture moves (next).

## Advisories

GHSA-fqc7-9xjw-jrh3 (symfony/runtime): `SymfonyRuntime`'s argv handling,
not ported; triaged `not-ported` (the M3 runtime stage must port the fix,
as `sapi` fills `argv` as PHP does).

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner).
