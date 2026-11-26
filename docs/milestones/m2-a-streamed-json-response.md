# M2-A: StreamedJsonResponse

November 26. Branch `m2/streamed-json`. Crate touched:
`symfony-http-foundation` (`streamed_json_response` and `generator`,
new; `streamed_response`, `response`, `json_response`). Oracle: a new
recorder, `http_foundation_streamed_json.php`.

## Scope

| PHP file / behaviour | Rust | Upstream tests |
|---|---|---|
| symfony/http-foundation `StreamedJsonResponse.php` (v8.1.11) | `crates/http-foundation/src/streamed_json_response.rs` | StreamedJsonResponseTest 15 of 15 |
| PHP's `\Generator` under `foreach` | `crates/http-foundation/src/generator.rs` | unit tests; through the recorded scripts |

## Prior art and choice

The PLAN's `Response.php` row already covers Rust ports of Symfony's
responses (none found). For the new part, streaming JSON out of lazy
data, the Rust crates found were struson, streaming_serde_json and
rill-json:

- **struson** (a streaming reader and writer, with optional serde): it
  writes JSON by serde's rules.
- **streaming_serde_json** (iterators converted both ways): its README
  calls it experimental, and it also follows serde's rules.
- **rill-json** (a streaming parser): its serializer writes Rust values,
  not lazy data.

None of them reproduces `json_encode()`'s flags (`JSON_HEX_*`,
`JSON_NUMERIC_CHECK`, `JSON_PRESERVE_ZERO_FRACTION`, …). Symfony's output
also depends on encoding the structure once and splitting it on a
placeholder. So the port keeps upstream's algorithm on php-value's
`json_encode()`, which already has recorded vectors. No new dependency.

php-value already models `\JsonSerializable` (`PhpObject::json_serialize`)
and other `\Traversable` objects (`traverse()`, which gives every pair at
once). A generator must run lazily, so that what its body echoes lands
between the pairs. It is a boxed Rust iterator, the shape
EventStreamResponse's events already use, inside a `Generator` object
that keeps `foreach`'s rewind rules.

`Generator` lives in http-foundation, its only user. php-value is now a
separate repository; once a second crate needs generators, it should move
there.

The response is a `Response` whose `Stream` is marked as the
`StreamedJsonResponse` subclass. The marker is the new `Subclass` enum,
which also holds EventStreamResponse's state.

## Evidence

- **Upstream tests:** StreamedJsonResponseTest 15 of 15. No GitHub
  advisory covers this file.
- **Recorded vectors:** 1,500 scripts recorded from PHP 8.4.27
  (`scripts/record.sh http-foundation-streamed-json`, 5,105 answers), all
  matching. Each non-empty echo is compared as its own chunk, through an
  output buffer of chunk size 1. They cover:
  - generators keyed automatically or by int, string, bool, float, null
    or an array; generators that echo before each yield; generators
    that throw before an item or at the end;
  - `ArrayObject`, an `IteratorAggregate`, `JsonSerializable` objects
    (also `Traversable`, or throwing), and `stdClass`;
  - placeholders as values and keys, including three fixed scripts with
    the placeholder as an array's key, where PHP writes JSON whose parts
    no longer line up;
  - invalid UTF-8 (`JsonException`) and ten encoding options;
  - `Content-Type` headers of `''`, `'0'` and lowercase keys;
  - data that is not iterable (`TypeError`, naming `true` and `false`);
  - the callback called twice ("Cannot traverse an already closed
    generator", "Cannot rewind a generator that was already run").
- **Mutations, each caught** (differences of 5,105):
  - any int first key taken as a list (151);
  - `JSON_NUMERIC_CHECK` kept for keys (64);
  - a placeholder key not counted (34);
  - a placeholder value not counted (21);
  - generators read eagerly (297);
  - `JsonSerializable` iterables streamed (476);
  - a `Content-Type` of `'0'` kept (210);
  - an empty iterable without `[` (the upstream test);
  - a generator stopped at its first yield not giving it again (the unit
    test).

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **Recognising objects:** `\Traversable` and `\JsonSerializable` are
  what `PhpObject::instance_of` answers. An object that implements
  `json_serialize()` but does not claim the interface is streamed as a
  plain object.
- **Not reproduced:**
  - a generator key that is an array is written as `"Array"` without
    PHP's warning;
  - the `TypeError` has no "called in" suffix;
  - generators support only `foreach`, not `send()`, `getReturn()` or
    `yield from`.
- **Next:** `BinaryFileResponse.php`.
