# M2-A HttpFoundation stage 7: JsonResponse and RedirectResponse

November 25. Branch `m2/http-foundation-json-redirect`. Crates touched:
`symfony-http-foundation`, `php-value`.

Built on HttpFoundation stage 6 (Response). These are the two responses
a converted controller returns most often after a plain one: a JSON
document (`$this->json()`) and a redirect (`$this->redirectToRoute()`).
Their content is text PHP writes, so the port needed PHP's
`json_encode()` and `htmlspecialchars()`, written in php-value.

## Scope

Upstream symfony/http-foundation at the pin (v8.1.11).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `JsonResponse.php` | `crates/http-foundation/src/json_response.rs` | JsonResponseTest 22 of 22 |
| `RedirectResponse.php` | `crates/http-foundation/src/redirect_response.rs` | RedirectResponseTest 8 of 8 |
| (PHP's `json_encode()`) | `crates/php-value/src/json/encode.rs`, `json::encode` | covered by the recorded calls |
| (PHP's `json_decode()` to objects) | `crates/php-value/src/json/decode.rs`, `json::decode_object` | covered by the recorded documents |
| (PHP's `htmlspecialchars()`) | `crates/php-value/src/html.rs` | covered by the recorded redirects |

Nothing in the two test files is left `pending` or `n/a`.

Prior art: the `Response.php` stack row's search (November 25) covers
Symfony's responses; there is no Rust port. For `json_encode()`,
serde_json (already a test dependency) and the config crate's scalar
`json_encode_with()` were compared; neither has PHP's flags, errors or
values. See the new stack row. No dependency was added.

## Port notes

- **Subclasses.** Both types hold a `Response` and dereference to it
  (`Deref`/`DerefMut`), as `ResponseHeaderBag` does to `HeaderBag`, so
  every inherited method is there; `From<…> for Response` hands the
  response on. Methods inherited from Response return `&mut Response`,
  not the subtype.
- **Headers as PHP's array.** The constructors take `(name, values)`
  pairs, not a built `ResponseHeaderBag`: RedirectResponse must know
  whether the array itself had a `cache-control` key
  (`array_change_key_case()`), which the bag's computed Cache-Control
  hides.
- **Errors.** `JsonResponseError` is this crate's `Error`
  (`\TypeError`, `\InvalidArgumentException`, `\JsonException` with its
  code, Response's errors), or what a `jsonSerialize()` threw
  (`php_value::Thrown`), or a `\JsonException` whose previous exception
  is that. `setData()`'s special case for a plain `\Exception` named
  `Failed calling …` rethrows the same exception on PHP 8.4, so it is a
  pass-through.
- **`\JsonSerializable`.** `PhpObject` gains `json_serialize()`, `None`
  by default. A Rust object returns the value to encode or a `Thrown`.
- **JSON text.** `new JsonResponse($data, json: true)` accepts strings,
  ints, floats (as `(string)`, precision 14) and `\Stringable`s, as PHP's
  `is_string() || is_numeric() || instanceof \Stringable` and the
  coercion to `setJson(string)` do. The `\TypeError` message is
  upstream's with `__METHOD__` written out.
- **The callback pattern** is upstream's literal as PHP evaluates the
  single-quoted string, compiled once through php-pcre. A name that is
  not UTF-8 makes `preg_match()` false, so it is invalid. `$` still
  matches before a final newline, as in PCRE: `"cb\n"` is accepted and
  written into the content, as PHP does.

## Behaviour found by recording

`json_encode()` was written from the manual and five probes, then
corrected against the recorded calls:

- The depth is checked when a level is closed, not opened: an error
  inside an array that is too deep is reported, not the depth.
- Without `JSON_PARTIAL_OUTPUT_ON_ERROR`, an invalid string value, a
  unit enum case or a level too deep ends the encoding; INF/NAN and an
  invalid member name only set the error, and encoding goes on (a later
  error replaces it). With partial output nothing stops, the last error
  is kept, and a level too deep is written in full.
- `JSON_THROW_ON_ERROR` alone leaves `json_last_error()` untouched;
  with `JSON_PARTIAL_OUTPUT_ON_ERROR` it is ignored. When a
  `jsonSerialize()` throws after INF set the error, PHP throws a
  `\JsonException` for INF, with the user's exception as previous.
- A property name starting with NUL (a mangled private or protected
  name) is left out of an object, `ArrayObject` included; in an array it
  is kept.
- Invalid UTF-8 is cut as PHP cuts it: a byte that cannot start a
  sequence is one piece, a lead byte with up to its length of
  continuation bytes is one piece even when overlong, a surrogate or
  above U+10FFFF (so `\xed\xa0\x80` is one U+FFFD, `\xc0\xaf` two).
- `JSON_NUMERIC_CHECK` keeps a numeric string whose number is infinite
  (`"1e500"`) as a string.
- `json_decode()` to objects raises `JSON_ERROR_INVALID_PROPERTY_NAME`
  as soon as the member is complete, before a later scanner error.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | JsonResponseTest 22 of 22, RedirectResponseTest 8 of 8 | all pass |
| `json_encode()` (`crates/php-value/tests/data/json_encode.json`, `oracle/recorders/php_value_json.php`) | 15,747 calls: every scalar and string vector under 19 flag sets, hand-picked structures under 21 flag sets and 5 depths, nestings around each depth limit (to 513), 6,000 random values (arrays, `stdClass`, `ArrayObject`, enum cases, `JsonSerializable` objects that return or throw) under random flags and depths. Text, `json_last_error()` code and message, or the exception with its previous | all match |
| `json_decode()` to objects (same file) | 8,154 documents (hand-picked NUL member names around every other error, depth edges, 2,000 generated documents and their mutations), with and without `JSON_BIGINT_AS_STRING` | all match |
| JsonResponse and RedirectResponse (`crates/http-foundation/tests/data/json_redirect.json`, `oracle/recorders/http_foundation_json_redirect.php`) | 1,500 JsonResponse scripts (324 failed constructions, 4,841 calls, final status, content, options and headers without Date), 3,255 `setCallback()` names (375 accepted), 1,000 RedirectResponse constructions (423 succeed, 103 of them then call `setTargetUrl()`): 11,772 answers | all match |
| Existing php-value vectors (`php.json`'s `json_encode()` strings, `json_decode.json`) | unchanged | all match |

Mutation checks (each rule broken alone, the replay run, the rule
restored), 19 of 19 caught:

| Rule broken | Answers that fail |
|---|---|
| callback reserved words ignored | 334 of 11,772 |
| callback pattern without `$` | 399 of 11,772 |
| `text/javascript` kept when the callback is removed | 207 of 11,772 |
| `setEncodingOptions()` decoding to arrays | 74 of 11,772 |
| `setData()` checking the error under `JSON_THROW_ON_ERROR` | 11 of 11,772 |
| a 301 keeping its default Cache-Control | 60 of 11,772 |
| the `cache-control` key matched case-sensitively | 36 of 11,772 |
| the status checked before the URL | 36 of 11,772 |
| `htmlspecialchars()` keeping `'` | 51 of 11,772 |
| `htmlspecialchars()` keeping invalid UTF-8 | 55 of 11,772 |
| depth checked when a level opens | 341 of 15,747 |
| invalid UTF-8 replaced byte by byte | 963 of 15,747 |
| an invalid member name ending the encoding | 141 of 15,747 |
| INF ending the encoding | 589 of 15,747 |
| mangled property names kept | 72 of 15,747 |
| `JSON_NUMERIC_CHECK` converting to INF | 24 of 15,747 |
| `\JsonException` without the previous exception | 13 of 15,747 |
| U+2028 raw under `JSON_UNESCAPED_UNICODE` alone | 35 of 15,747 |
| the property name check skipped in `json_decode()` | 640 of 8,154 |

## Limits

- `JSON_ERROR_RECURSION` and `JSON_ERROR_UNSUPPORTED_TYPE` do not arise:
  a `Value` holds no reference cycle and no resource.
- `json_encode()` recurses once per level; partial output goes on past
  the depth limit as PHP does, so a value nested deeper than the stack
  allows is the caller's (a `Value` that deep already overflows on drop).

## Advisories

The upstream watch lists no advisory for `JsonResponse.php` or
`RedirectResponse.php`.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner, November 23); M2-A's acceptance by
comparison stays open.
