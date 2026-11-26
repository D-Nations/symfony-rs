# M2-A: flushed output delivered as nginx passes it on

November 26. Branch `m2/streamed-delivery`. Crates touched:
`symfony-runtime` (`server`, `http_kernel_runner`; new direct dependency
`http-body`), `symfony-http-foundation` (`php`: the `flush()` hook;
`sapi`: `take_body()`; `response`, `streamed_response`: `flush()`
through the hook). No oracle change.

The previous stage left the front controller replaying every recorded
output call at the end, so a `StreamedResponse` reached the client whole.
This stage passes output on while the script runs, as nginx in front of
PHP-FPM does with Symfony's documented location.

## What nginx does (from its documentation)

- **Start of the response:** nginx answers the client once PHP-FPM has
  sent the headers. PHP sends them with the first output, or at
  `flush()`.
- **Buffering:** `fastcgi_buffering` is on by default. A response with
  `X-Accel-Buffering: no` (EventStreamResponse sets it) turns it off, and
  is then "passed to a client synchronously, immediately as it is
  received". With buffering on, nginx reads the response into
  `fastcgi_buffers`, one memory page (4 KiB) each by default.
- **Hidden headers:** `fastcgi_hide_header`'s default hides `Status` and
  the `X-Accel-*` headers from the client.
- **Framing:** a body without `Content-Length` goes to an HTTP/1.1 client
  chunked.

## Prior art and choice

No new search for the adapter itself (the PLAN's HTTP adapter row). For
the streaming body, compared:
- axum's `Body::from_stream`, which needs `futures-core`'s `Stream` trait
  as a direct dependency;
- implementing `http_body::Body`, which needs `http-body` for `Frame`
  (axum re-exports the trait only);
- `tokio-stream`'s `ReceiverStream`, a new crate in the lock.

Chosen: `http-body` 1.1. It is the trait axum's own `Body` is built on,
it is already in the lock through axum and hyper, and it depends only on
`bytes` and `http`. The PLAN has a stack row for it.

For when output is passed on, compared:
- always at once;
- only at the end (the previous adapter);
- nginx's documented buffering (chosen).

## Design

- **`php::flush()`:** records the call and runs a per-thread hook. The
  three places that record `flush()` (`Response::send()`,
  `streamed_response::flush()` and the runner) call it.
- **The live SAPI:** the front controller keeps a `Sapi` for the request
  (`Delivery`). At each `flush()`, at `fastcgi_finish_request()` and at
  the end, `Delivery` applies the calls made since the last time and
  drains them, so the recorded output stays small.
- **Passing it on:** once the headers are sent, the head goes to the
  client through the existing oneshot, and the body through a bounded
  channel. `blocking_send` gives backpressure, as a slow client slows
  PHP-FPM. With buffering on, a piece is passed on once 4,096 bytes
  wait, and the rest at the end.
- **Before any output:** a response that ends before its headers were
  sent is answered whole, as before.
- **Hidden headers:** `into_response` hides the five `X-Accel-*`
  headers. `X-Accel-Buffering` is acted on; the other four are logged as
  not applied.

## Evidence

- `tests/server.rs`, four new tests. The script blocks on a channel the
  test controls, so ordering does not depend on timing:
  - with `X-Accel-Buffering: no`, the flushed part arrives while the
    script still waits; the header is hidden and there is no
    `Content-Length`;
  - with buffering on, a small flushed part is held until the end;
  - a full 4,096-byte buffer is passed on before the end;
  - the five `X-Accel-*` headers are hidden, and another `X-Accel-`
    name is kept.
- The existing server tests pass unchanged, including the response sent
  before `terminate()` ends and the streamed response of the last stage.
- Mutations, each caught: `X-Accel-Buffering` ignored, never buffering,
  no full-buffer pass, the head sent only at the end, `X-Accel-*` shown.

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **Framing of a response that ends before any `flush()`** (PLAN
  decision 20 (d)): nginx would send it chunked, but the adapter keeps
  axum's `Content-Length`. The converter fixture's comparison rule counts
  on that header, so the change is the owner's call.
- **Approximations:** where nginx cuts buffered output (one page per
  piece), and PHP-FPM's own 8 KiB write buffer between flushes, which is
  not modelled.
- **`headers_sent()`:** it still answers the scripted flag. It does not
  turn true after output was sent.
- **The other `X-Accel-*` headers:** `X-Accel-Redirect`, `-Expires`,
  `-Limit-Rate` and `-Charset` are hidden but not applied.
- **Next:** `StreamedJsonResponse.php` and `EventStreamResponse.php`.
