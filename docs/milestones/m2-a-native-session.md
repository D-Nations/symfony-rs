# M2-A: Session, stage 2b: native storage and PHP's session extension

November 27. Branch `m2/native-session`. Crate touched:
`symfony-http-foundation`. Its `php_session` module grows from stage 2a's
state and header functions into the session extension (`lifecycle`,
`serializers`, `files`). The crate also gains the native storages, the
proxies and `NativeFileSessionHandler`, and `Class::Thrown` in
`exception` (a PHP throwable from a save handler, passed on unchanged).

No new dependency. Oracle: a new recorder,
`http_foundation_native_session.php`, with its router
`http_foundation_native_session_server.php`.

## Scope

| PHP files (symfony/http-foundation v8.1.11) | Rust | Upstream tests |
|---|---|---|
| `Session/Storage/NativeSessionStorage.php` | `session/storage/native_session_storage.rs` | NativeSessionStorageTest 21 of 22 (1 `n/a`) |
| `PhpBridgeSessionStorage.php` | `php_bridge_session_storage.rs` | PhpBridgeSessionStorageTest 2 of 2 |
| `NativeSessionStorageFactory.php`, `PhpBridgeSessionStorageFactory.php` | `native_session_storage_factory.rs`, `php_bridge_session_storage_factory.rs` | through the tests above |
| `Proxy/AbstractProxy.php` | `proxy/abstract_proxy.rs` | AbstractProxyTest 8 of 8 |
| `Proxy/SessionHandlerProxy.php` | `proxy/session_handler_proxy.rs` | SessionHandlerProxyTest 14 of 14 |
| `Handler/NativeFileSessionHandler.php` | `handler/native_file_session_handler.rs` | NativeFileSessionHandlerTest 4 of 4 |
| `Handler/AbstractSessionHandler.php`, `NullSessionHandler.php` (stage 2a's code) | stage 2a's | AbstractSessionHandlerTest 1 of 1, NullSessionHandlerTest 4 of 4 |
| PHP's session extension and `\SessionHandler` | `php_session.rs`, `php_session/{lifecycle,serializers,files}.rs` | through all of the above |

The `n/a` test is `testCreatingManyInstancesDoesNotLeakMemory`. It
measures `memory_get_usage()` over PHP objects. A Rust storage is freed
when it is dropped, so there is nothing to measure.

AbstractSessionHandlerTest runs 13 PHP scripts
(`Tests/Session/Storage/Handler/Fixtures/*.php`) under a PHP server and
compares their output with the `.expected` files. The Rust test runs each
script's steps on the emulation and compares with the same `.expected`
text.

## Prior art and choice

The search for a Rust port of Symfony's session classes was not repeated
(stage 1's row). For PHP's session extension, these were compared:

| Project | Approach | For | Against |
|---|---|---|---|
| lelaphp/lela | A PHP runtime translated from php-src | Complete | Derived from php-src, which decision 8 excludes |
| rphp | A PHP interpreter in Rust | Independent code | Has no session extension |
| muesli | Rust crate for PHP's session formats | `php`/`php_serialize` text | Formats only, with no lifecycle; a second `serialize()` beside php-value's |
| buzzs, p8 | PHP runtimes in Rust | Have session code | No licence, so they cannot be used |

**Choice:** an independent implementation on php-value, written from the
PHP manual and the oracle's answers. It needs no new dependency.

## Evidence

- **Upstream tests:** 55 ported, 1 `n/a`. No GitHub advisory covers these
  files.
- **Recorded vectors:** 1,500 requests recorded from PHP 8.4.27's built-in
  web server (`scripts/record.sh http-foundation-native-session`), all
  matching. They hold 11,277 operations and 28,516 log entries. The
  session cookie and the cache limiter's headers exist only in a web
  server, so each scenario is one request to `php -S`.
  - **A scenario** has:
    - a request cookie: an id the store or a session file has, one it
      lacks, a short one, one that is not a file name, or none;
    - the `session.*` ini settings;
    - a save handler: `files`, PHP's own; a scripted
      `AbstractSessionHandler`; bare, strict-wrapped, `validateId`-only
      and timestamp handlers; `NullSessionHandler`; or
      `MigratingSessionHandler`;
    - the handler calls that answer `false`;
    - a mode: raw `session_*()` calls (512), `NativeSessionStorage` with
      options (730), or `PhpBridgeSessionStorage` on a session PHP may
      have started (258);
    - 3 to 12 operations, with the clock moving between them when an
      operation says so.
  - **Compared:**
    - every handler call and its result, in order;
    - each operation's result or exception;
    - warnings and notices;
    - the response headers and `Set-Cookie` lines;
    - the final status, id and `$_SESSION`;
    - the store and the session files' names, modes and contents.

    Generated ids are numbered in order of appearance, and dates are
    reduced to their shape.

  Replay is `tests/native_session_recorded.rs`. `NATIVE_SESSION_DUMP=n`
  prints scenario `n` and both transcripts.
- **Found by the vectors:** the differences went from 547 scenarios to
  0. These behaviours were learned from them:
  - **Handler methods:** PHP calls `create_sid()` and `validateId()`
    whenever the handler object has the method, whether or not it
    declares the interface.
  - **The session cookie** is sent before `read()`, and only for an id
    that did not come from the request cookie. A later one replaces it
    in place, not appended.
  - **Lazy write** calls `updateTimestamp()` only for data that did not
    change and is not empty. Unchanged empty data still goes to
    `write()`.
  - **`session_id('')`** counts as an id given, so the request cookie is
    not read.
  - **Garbage collection** runs after `read()` and before decoding.
  - **Failures:** a decode failure destroys the session; a failed
    `open()` or `read()` closes and forgets the id.
  - **`session_regenerate_id()`:**
    - in strict mode it validates up to three new ids, then takes a
      fourth unchecked;
    - a read failure throws `\Error` "Failed to create(read) session ID".
  - **`session_reset()`** validates the id in strict mode, and sends the
    new id's cookie.
  - **`session_encode()`** needs a `session_start()` that set the session
    up, and returns `false` for an empty session. `session_destroy()`
    undoes that.
  - **The write-failure warning** names `{Class}::write`, but only `write`
    after a decode failure.
  - **`NativeSessionStorage::save()`'s error handler** rewrites
    `session_write_close()`'s warnings as they are raised. The rewritten
    text is not html-escaped. Reading an unset `$_SESSION` there warns
    three times.
  - **Header and notice messages:**
    - a second `session_start()` gives an `E_NOTICE` "Ignoring
      session_start()…";
    - `setcookie()` and `header()` after output warn "Cannot modify
      header information…";
    - `session_id()` refuses after output only with `use_cookies`.
- **Stage 2a's open items:** `AbstractSessionHandler::destroy()`'s
  session cookie and `open()`'s `Cache-Control` header are now in the
  recorded requests, and the README claims them.
- **Mutations:** 24 tried, 24 caught by the tests and vectors. They
  covered:
  - lazy write's two conditions;
  - the cookie id read again after `session_id()`;
  - the cookie for a new id;
  - strict mode's validation;
  - a read failure keeping the id;
  - gc after decoding;
  - regenerate not destroying;
  - four tries made one;
  - a decode failure not destroying;
  - a non-array `$_SESSION` written;
  - the `private` cache limiter;
  - the cookie's `SameSite`;
  - `ini_set()` while active;
  - numeric and `|` keys encoded;
  - the `files` id checks in reading and in strict validation;
  - `save()` keeping empty bags or not putting the data back;
  - the storage's own id check;
  - the bridge's `clear()`;
  - the proxy's save handler name;
  - `NativeFileSessionHandler`'s `;` count.

  One mutant, lazy write on empty data, survives the upstream fixtures
  alone: their `.expected` text is rewritten for PHP 8.6, which hides it.
  The vectors catch it.

## Checks

See the PR for the output of the full gate on the final commit, and of
`scripts/linux-test.sh -p symfony-http-foundation`.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **Back-references in `php` session data:** PHP numbers the values of
  all entries together, so an `r:`/`R:` in one entry may point into
  another. php-value's `serialize()` numbers one value at a time. Such
  data is refused (`Class::NotPorted`) rather than read or written
  differently. Supporting it needs shared numbering in php-value
  (php2rust-compatibility-tools); see the decision below.
- **`php_binary`** is refused (`Class::NotPorted`).
- **`Last-Modified`:** the `private_no_expire` and `public` limiters do
  not send it. No recorded request's `headers_list()` has one, so
  nothing shows when PHP sends it or with what date. It is not modelled.
- **File and line:** the file and line in "headers already sent" and
  "session started" messages are `""` and `0`. The recorder normalizes
  them.
- **Locks:** a locked session file is not waited for. PHP blocks until
  the lock is released; the recorded requests run one at a time.
- **A `$_SESSION` replaced** by a string, `false` or an object and then
  used as an array is refused.
- **Next:** `Request`'s session methods (`getSession()`, `hasSession()`,
  `hasPreviousSession()`, three RequestTest tests). Then the database
  and cache handlers and `SessionHandlerFactory`.

## Decisions for the owner

- **Shared back-reference numbering in php-value.** Recommended: add a
  serializer state to php-value that numbers values across calls. The
  `php` session format could then read and write references between
  entries, as PHP does. Until then such sessions are refused. This
  repository's code is not changed for it.
