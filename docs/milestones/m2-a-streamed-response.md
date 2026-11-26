# M2-A: StreamedResponse

November 26. Branch `m2/streamed-response`. Crates touched:
`symfony-http-foundation` (`streamed_response`, new; `response`, `php`,
`sapi`, `json_response`, `redirect_response`), `symfony-http-kernel`
(`http_kernel`), `symfony-runtime` (`http_kernel_runner`). Oracle: a new
recorder, `http_foundation_streamed_response.php`.

## Scope

| PHP file / behaviour | Rust | Upstream tests |
|---|---|---|
| symfony/http-foundation `StreamedResponse.php` (v8.1.11) | `crates/http-foundation/src/streamed_response.rs`, with `Response`'s checks for it in `response.rs` | StreamedResponseTest 13 of 13 |
| `HttpKernel::handle()`'s `finally`: a streamed response's callback re-pushes the request | `crates/http-kernel/src/http_kernel.rs` | HttpKernelTest `testVerifyRequestStackPushPopWithStreamedResponse` |
| `@ob_flush()` in the process model | `OutputCall::ObFlush`, `Sapi::ob_flush` | through the recorded scripts |

## Prior art and choice

No new search: the PLAN's `Response.php` row found no Rust port of
Symfony's responses. The design question is how a subclass that
overrides methods `Response` itself calls fits a kernel that holds every
response as a `Response`. Compared:

- **A type of its own converting into `Response`** (how `JsonResponse`
  and `RedirectResponse` were done). Once the kernel holds it as a
  `Response`, the overrides are gone: `prepare()` for a HEAD request and
  `setNotModified()` call `setContent(null)`, which marks a streamed
  response as sent, and `send()` calls `sendHeaders()` and
  `sendContent()`.
- **A trait object for every response.** Every caller changes, and the
  kernel, listeners and tests read fields through a vtable.
- **`Response` carrying the subclass's state (chosen).** `Response` holds
  an optional `Stream` (the subclass's three properties). The five
  methods the subclass overrides check for it, as PHP's late binding
  dispatches.

`JsonResponse` and `RedirectResponse` override nothing that `Response`
calls, so they stay wrappers.

API changes that follow from PHP's signatures:
- `setContent()` now returns a `Result`: it throws on a streamed response.
- `getContent()` returns `Option`: it is `string|false` in PHP.
- `sendContent()` and `send()` return a throwable, since a callback may
  throw anything. The runner passes it on unwrapped.
- `JsonResponse::get_content()` and `RedirectResponse::get_content()`
  stay strings.

## Evidence

- StreamedResponseTest 13 of 13, and HttpKernelTest's streamed-response
  test (the request is on the stack while the callback runs, and off it
  before and after).
- 1,500 scripts recorded from PHP 8.4.27 (`scripts/record.sh
  http-foundation-streamed-response`, 17,305 answers), all matching.
  Each script builds a response from a callback (echoing parts with
  `flush()` between some, counting its calls, or throwing), from a list
  of chunks, or from null. Its calls then go through every method the
  subclass changes: `prepare()` for GET, HEAD and POST, `setNotModified()`,
  `isNotModified()`, `sendHeaders()` (including informational codes),
  `sendContent()`, `send()`, `setContent()`, `getContent()`, `(string)`,
  `getCallback()` called, `setCallback()`, `setChunks()`, and status and
  header changes. Compared: each result, the process calls in order
  (`echo` between `header()`, `ob_flush()` and `flush()`), and the final
  state.
- Mutations, each caught:
  - by the recorded scripts: informational `sendHeaders()` marking the
    headers sent (36 differences); `setContent(null)` not marking the
    content sent (294); `sendContent()` running the callback more than
    once (1,189); `getContent()` returning a string (1,956); no
    `ob_flush()` between chunks (436); headers sent more than once (572);
  - by the HttpKernelTest port: no request re-push.
- `tests/server.rs`: a chunked streamed response served through
  `FrontController`.

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- The front controller sends output whole, at `fastcgi_finish_request()`
  or at the script's end. PHP-FPM behind nginx sends a part at each
  `flush()`. The bytes are the same; the timing and the transfer encoding
  are not (runtime README). This is the next step.
- `StreamedJsonResponse.php` and `EventStreamResponse.php` (subclasses of
  this one; their tests are pending).
- Chunks from a PHP generator: a callback writes them.
