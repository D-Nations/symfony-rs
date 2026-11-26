# M2-A: BinaryFileResponse

November 26. Branch `m2/binary-file-response`. Crate touched:
`symfony-http-foundation`:

- new: `binary_file_response`, `file_stream`;
- changed: `response`, `file` (the `FileLike` trait), `php` (seek and
  size on the in-memory stream).

New dependency for the crate: `twox-hash`, already in the workspace for
Config, so its PLAN stack row is extended. Oracle: a new recorder,
`http_foundation_binary_file.php`.

## Scope

| PHP file | Rust | Upstream tests |
|---|---|---|
| symfony/http-foundation `BinaryFileResponse.php` (v8.1.11) | `crates/http-foundation/src/binary_file_response.rs` | BinaryFileResponseTest 32 of 32, with ResponseTestCase's test |
| `File/Stream.php` | `crates/http-foundation/src/file_stream.rs` | `testStream` |

## Prior art and choice

The PLAN's `Response.php` row covers Rust ports of Symfony's responses
(none found). Rust crates that serve files with `Range` and `If-Range`
exist: http-serve and http-fs, and serdir on top of them. They implement
the HTTP rules, not Symfony's. Symfony's differ:

- only one range is accepted, and a list of ranges is ignored;
- `If-Range` is compared to the ETag or to the formatted
  `Last-Modified`;
- a suffix longer than the file selects the whole file;
- 416 is answered only when the start is past the end.

The behaviour is ported as it is. Hashing stays below the behaviour layer:
`hash('xxh128')` is `twox-hash`, chosen for Config (stack row), with the
same feature.

The response is a `Response` carrying a `BinaryFile` state, as
StreamedResponse carries a `Stream`. PHP's subclasses of `File` become a
`FileLike` trait: upstream's own `File\Stream` and the test's `FakeFile`
override some of its methods, as an application's subclass would.
`\SplTempFileObject` is the crate's in-memory stream under the name
`php://temp`, which is the name PHP gives it.

## Evidence

- **Upstream tests:** BinaryFileResponseTest 32 of 32, including the test
  it inherits from ResponseTestCase. No GitHub advisory covers these
  files.
- **Recorded vectors:** 1,500 scripts recorded from PHP 8.4.27
  (`scripts/record.sh http-foundation-binary-file`, 10,946 answers), all
  matching. Each script recreates its files, with contents, names
  (non-ASCII, `%`, a space) and modification times from pools. They cover:
  - every kind of file argument, including missing paths and `file://`
    URLs;
  - statuses from 200 to 500;
  - twenty `Range` values (suffixes, overflowing numbers, a trailing
    newline, lists, other units);
  - `If-Range` matching and not;
  - `X-Sendfile-Type` trusted or not, with every type and five
    `X-Accel-Mapping` values;
  - dispositions and fallbacks that are invalid, ISO-8859-1, or hold `%`
    or control characters;
  - chunk sizes from 0 to 16 KiB, a 20,000-byte file sent in several
    chunks, delete after send, a second `sendContent()` after the file is
    gone, and an aborted client.

  Each echo's length and the CRC32 of all of them are compared.
- **Found by the vectors:** `getRealPath()` of a `file://` URL is `false`
  in PHP, so `X-Sendfile` gets the URL itself. The finder's `realpath()`
  resolves it. `FileLike::get_real_path` now gives `None` for any URL, and
  `File::get_real_path` follows it.
- **Mutations, each caught** (differences of 10,946):
  - a long suffix not clamped (2);
  - the end not clamped (the upstream tests);
  - ranges on any method (163);
  - `If-Range` ignored (67);
  - `Accept-Ranges: bytes` on every method (106);
  - sendfile types matched with case (48);
  - the accel path not URL-encoded (9);
  - `%` kept in the fallback (the upstream tests);
  - the fallback made per byte for UTF-8 (unit tests);
  - no delete after an unsuccessful send (29);
  - the abort ignored (18);
  - a disposition of `'0'` set (126, of 9,961);
  - HEAD sending the body (131);
  - the real path of URLs (7).

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **Through the HTTP adapter:** `X-Accel-Redirect` is hidden as nginx
  hides it, but nothing serves the file (PLAN.md decision 20(e), not
  decided).
- **Not reproduced:**
  - `ignore_user_abort(true)`, which has no counterpart;
  - a file that cannot be opened reports only "No such file or directory"
    or "Permission denied";
  - stream wrappers other than `file://` are not read.
- **Next:** the rest of HttpFoundation's open files (RequestMatcher,
  Session, RateLimiter).
