# M1-D: converter fixture

The fixture in PLAN.md ("Early conversion proof and second application"):
a tiny pinned PHP Symfony app, its responses recorded from PHP, a
hand-authored generated-code-shaped Rust crate in this runtime, and a
comparison, repeated on Linux from an empty directory.

**Hand-authored, not converter output.** This proves the runtime interface
only. The conversion gate stays open until symfony2rust emits
`fixtures/converter-app` and it builds and passes without manual edits.

## Prior art

How to shape generated code was settled by the converter contract's
comparison (PeachPie, KPHP, Symfony's `PhpDumper`, Quarkus ArC, the Rust
DI crates; [converter-contract.md](../converter-contract.md)). The
fixture's method, recording the original over HTTP and replaying the
requests through the Rust app, is the one the symfony/demo port and the
second app's W1 already use; no new tool was needed.

## PHP app

[oracle/apps/converter-fixture](../../oracle/apps/converter-fixture):
Symfony 8.1.10 (FrameworkBundle only) on symfony/demo's pinned `vendor/`,
no `composer.json`, read-only, cache in `/tmp`, `APP_ENV=prod`. It has an
injected service (`Greeter`: string, int and array parameters plus a
service), a readonly value object (`Greeting`), a mutable shared service
(`VisitCounter`), one action (`GET /greet/{name}` with a `[a-z]+`
requirement and a `punctuation` query parameter) and a `kernel.exception`
subscriber that answers `text/plain` for its own `GreetingException` (422)
and for HTTP exceptions.

`scripts/record.sh converter-fixture` (`oracle/recorders/converter_fixture.py`)
serves it with PHP 8.4.27's built-in server in the oracle image and
records 16 responses ([responses.json](../../oracle/apps/converter-fixture/responses.json)):

| Cases | What they reach |
|---|---|
| `greet`, `greet-again` | Success; the repeat still reports `visits: 1` (no state shared between requests) |
| `greet-query`, `greet-query-empty` | `InputBag::getString()` with and without a value |
| `greet-quarter`, `greet-max-length` | `(string) $float` (`0.41666666666667`, `0.25`), and `12 / 12` giving int `1` coerced to float |
| `greet-head` | HEAD on a GET route; `Response::prepare()` empties the body |
| `too-long` | `GreetingException` → the subscriber's 422 |
| `requirement-uppercase`, `requirement-encoded`, `no-route-referer` | RouterListener's 404 messages, with the raw path and with `(from "...")` |
| `method-not-allowed` | 405 with `Allow: GET`, copied by the subscriber from the exception's headers |
| `query-non-scalar` | `BadRequestException` → `HttpKernel` 400 with the same message |
| `trailing-slash`, `trailing-slash-query`, `trailing-slash-port` | RedirectableCompiledUrlMatcher → `RedirectController::urlRedirectAction`: 301 without `Cache-Control`, raw query string kept, non-default port kept |

## Rust crate

[fixtures/converter-app](../../fixtures/converter-app), workspace member,
`publish = false`, no new third-party dependency. One module per PHP
class under `app`, the compiled container (`Container` application-scoped,
`RequestScope` built per request), the compiled routes on
`symfony-routing`, and the kernel wiring on `symfony-http-kernel` with
Symfony's listener priorities. `cargo run -p converter-fixture-app`
serves it with Axum.

Results (`cargo test -p converter-fixture-app`):

- `every_recorded_response_matches`: all 16 responses identical in
  status, headers and body. Headers are compared as a multiset without
  those PHP's built-in server adds (`Host`, `Connection`, `X-Powered-By`)
  and without `Date`. Corrupting three recorded values (a body, a header
  name, a status) made exactly those three cases fail, so the comparison
  can fail.
- `request_scoped_state_is_not_shared`, `concurrency_bounds_hold`: the
  contract's section 4 and 7 bounds.

`scripts/converter-fixture.sh` runs the fixture's fmt, clippy and tests,
with the crates it links, on Linux: `git archive HEAD` unpacked into an
empty directory of a removed `rust:1.99-slim` container (pinned by
digest), which downloads the locked crates itself. It runs locally, not in
CI (owner, November 20: no paid CI jobs).

## Runtime gaps found

What the fixture needed that the workspace crates do not have. Each is in
`fixtures/converter-app/src/runtime`, ported from the named upstream file
for the recorded cases only; other input is refused (`NotPorted`, a 500)
rather than approximated. Generated code is not meant to carry these: they
are M2-A work.

| Gap | Upstream | Destination |
|---|---|---|
| A non-HTTP exception that listeners can inspect | `HttpKernel::handleThrowable`, `ExceptionEvent::getThrowable` | **Closed** in `symfony-http-kernel`: `Exception::from_throwable`, `is_http()` |
| RouterListener: match, 404 and 405 messages, `Allow`, `Referer` | `EventListener/RouterListener.php`, `MethodNotAllowedHttpException.php` | `symfony-http-kernel` |
| ResponseListener and `Response::prepare()` (charset, HEAD, HTTP/1.0 `Pragma`/`Expires`) | `EventListener/ResponseListener.php`, `Response.php` | `symfony-http-kernel`, `symfony-http-foundation` |
| `Response` construction with `ResponseHeaderBag`'s computed `Cache-Control` (no caller-supplied directives yet; `Date` is left to the HTTP server) | `Response.php`, `ResponseHeaderBag.php` | `symfony-http-foundation` |
| `RedirectResponse` | `RedirectResponse.php` | `symfony-http-foundation` |
| `Request::getHost()` (with `isHostValid()`, host-name branch only), `getPort()`, `getHttpHost()`, `getUriForPath()`; scheme fixed to `http`, base URL `''`, no trusted proxies | `Request.php`, `RequestContext.php` | `symfony-http-foundation`, `symfony-routing` |
| `InputBag::getString()` and the request-exception 400 | `InputBag.php`, `HttpKernel.php` | `symfony-http-foundation`, `symfony-http-kernel` |
| `RedirectController::urlRedirectAction()` | FrameworkBundle `Controller/RedirectController.php` | a FrameworkBundle crate (M3) |
| `ErrorListener` and the production error page | `EventListener/ErrorListener.php`, ErrorHandler's `HtmlErrorRenderer` | M2-A; no recorded case reaches it |
| `implode()`, `ucfirst()`, `strtolower()`, `trim()`, `htmlspecialchars(ENT_QUOTES)` | PHP manual | `php-value`, with recorded vectors |

## Contract changes (0.1 → 0.2)

- Section 4: a service's state includes every object it reaches. The
  fixture's `Greeter` has only readonly properties but holds the mutable
  `VisitCounter`, so it is request-scoped.
- Section 5: non-HTTP exceptions reach listeners through
  `Exception::from_throwable`; request exceptions become 400 HTTP
  exceptions with the same message.

## Checks

This repository: `cargo fmt --check` clean, `cargo clippy --all-targets
-- -D warnings` clean, `cargo test` 809 passed and 0 failed,
`scripts/sources.sh --check` and `scripts/parity-check.sh` pass.
`scripts/converter-fixture.sh` at `1944e6a`: fmt, clippy and every test of
the fixture and the four crates it links pass in an empty Linux directory.

symfony-demo-rs (branch `plan/symfony-runtime`, `[patch]` pointing at this
checkout, restored afterwards, nothing committed there), because
`symfony-http-kernel` changed: `cargo fmt --check` and clippy clean,
`cargo test` 226 passed. Then the full comparison on fresh databases,
both servers with the original's `APP_SECRET`, once with this checkout and
once as a control with its commit before the kernel change (`1bfdcc4`):

| Comparison | This checkout | Control `1bfdcc4` |
|---|---|---|
| Pages (`capture.sh`, 4 identities) | 92 same, 0 differ | 92 same, 0 differ |
| Flows admin / locale / ties | 68 / 19 / 16 same, 0 differ | 68 / 19 / 16 same, 0 differ |
| Flows security | 2 same, 9 differ | 3 same, 8 differ |
| Multipart login cases | 92 same, 0 differ | 92 same, 0 differ |
| Live search | 124 same, 3 differ | 124 same, 3 differ |

Apart from the diff tool's timestamps, every diff file is identical
between the runs except one security step, `array_indexes/11-POST`: the
cleared `REMEMBERME` cookie's `expires` date is one second apart between
the original and the port, because they answered in different seconds.
An earlier run against this checkout (discarded because its page capture
had failed on a path conversion in the run script) gave 3 same and 8
differ, as the control did. The 8 are the accepted Arabic-slug
difference (`mqal-jdyd` / `mql-jdyd`, demo README) and the 3 live cases
are the documented case-insensitive-mount artifacts. The ties flow now
gives 16 same; the last stage log reported 14 and 2, and the control
gives 16 too, so that change predates this stage.

## Noticed, not fixed

- `oracle/apps/easyadmin-demo/w1.json` keeps the session's `Expires`
  header unmasked (`Expires: Thu, 19 Nov 2026 ...`), so re-recording W1
  always changes the file. `second_app_w1.py` should mask it as it masks
  `Date`.
