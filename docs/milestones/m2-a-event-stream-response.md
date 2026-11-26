# M2-A: EventStreamResponse

November 26. Branch `m2/streamed-json-event`. Crates touched:
`symfony-http-foundation` (`event_stream_response` and `server_event`,
new; `streamed_response`, `response`, `php`), `symfony-runtime`
(`server`). Oracle: a new recorder, `http_foundation_event_stream.php`.

## Scope

| PHP file / behaviour | Rust | Upstream tests |
|---|---|---|
| symfony/http-foundation `EventStreamResponse.php` (v8.1.11) | `crates/http-foundation/src/event_stream_response.rs` | EventStreamResponseTest 14 of 14 |
| `ServerEvent.php` | `crates/http-foundation/src/server_event.rs` | through EventStreamResponseTest |
| `connection_aborted()` | `php::Output::connection_aborted`, set by the front controller when the client is gone | `tests/server.rs` |

`StreamedJsonResponse.php` is left for its own stage. It walks PHP data
holding generators and `JsonSerializable` objects, which needs a
php-value model for them.

## Prior art and choice

No new search: the PLAN's `Response.php` row found no Rust port of
Symfony's responses, and the shape is the previous stage's. An
`EventStreamResponse` is a `Response` whose `Stream` carries an
`EventStream`, holding the subclass's `$retry` and `$lastRetry`. The
callback is given that handle, as PHP's callback is given `$this`, and
returns its events as an iterator. A PHP generator may sleep between
events; the iterator is just as lazy. No new dependency.

## Evidence

- **Upstream tests:** EventStreamResponseTest 14 of 14, including both
  field-injection tests. No advisory is listed for these files (checked
  against GitHub's advisory database).
- **Recorded vectors:** 1,200 scripts recorded from PHP 8.4.27
  (`scripts/record.sh http-foundation-event-stream`, 5,444 answers), all
  matching. They cover:
  - header keys in any case: PHP's `+=` adds a default under the exact
    key, so a lowercase key is later replaced by the default;
  - retries null, 0, negative and positive;
  - `lastRetry` across events, including the response's retry set
    between events sent directly;
  - comment, id and type of `''`, `'0'` and values with line breaks;
  - data of `''`, `'0'`, CR, LF and CRLF, and lists of lines;
  - `connection_aborted()` turning true after a given event;
  - `send()` twice; `getCallback()`; `(string)`; and a response without
    a callback (`LogicException`).
- **The non-CLI flush path:** a test of its own, since `PHP_SAPI` is a
  constant and the oracle's is the CLI's. It checks
  `closeOutputBuffers(0, true)` and `flush()` after each part.
- **Server tests:** each event reaches the client before the generator is
  let go on, and `X-Accel-Buffering` is hidden. A client that goes away
  ends the event loop.
- **Mutations, each caught** (differences of 5,444):
  - `'0'` taken as truthy (325);
  - a retry of 0 or less written (260);
  - CRLF split as two line breaks (175);
  - the retry added to every event (106);
  - header keys compared without case (494);
  - no abort check (55);
  - no flush outside the CLI (the non-CLI test);
  - `connection_aborted()` never set by the server (the server test).

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **Late event callbacks:** an event callback given to `setCallback()`
  once one is set is called without `$this` and its result ignored. A PHP
  callback that declares the parameter throws an `ArgumentCountError`
  instead; that is not reproduced (crate README).
- **Generator data:** data from a generator is collected before the event
  is written. The bytes are the same.
- **Next:** `StreamedJsonResponse.php`.
