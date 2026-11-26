# M2-A: Session, stage 2a: save handlers

November 26. Branch `m2/session-handlers`. Crate touched:
`symfony-http-foundation`. It gains the new `session::storage::handler`
module, `session::session_utils`, `php_session` (the state of PHP's
session extension, with its header functions), `\DomainException` in
`exception::Class`, and `SetCookie` in `php::OutputCall`.

No new dependency. Oracle: a new recorder,
`http_foundation_session_handlers.php`.

## Scope

| PHP files (symfony/http-foundation v8.1.11) | Rust | Upstream tests |
|---|---|---|
| `Session/Storage/Handler/AbstractSessionHandler.php`, PHP's `\SessionHandlerInterface`, `\SessionUpdateTimestampHandlerInterface`, `\SessionIdInterface` | `session/storage/handler/mod.rs`, `abstract_session_handler.rs` | through the tests below; AbstractSessionHandlerTest is 2b |
| `StrictSessionHandler.php` | `strict_session_handler.rs` | StrictSessionHandlerTest 18 of 18 |
| `NullSessionHandler.php` | `null_session_handler.rs` | NullSessionHandlerTest is 2b |
| `IdentityMarshaller.php`, symfony/cache's `MarshallerInterface` (the interface only) | `identity_marshaller.rs`, `marshaller_interface.rs` | IdentityMarshallerTest 3 of 3 |
| `MarshallingSessionHandler.php` | `marshalling_session_handler.rs` | MarshallingSessionHandlerTest 11 of 14 (3 `n/a`) |
| `MigratingSessionHandler.php` | `migrating_session_handler.rs` | MigratingSessionHandlerTest 9 of 11 (2 `n/a`) |
| `Session/SessionUtils.php` | `session/session_utils.rs` | through `destroy()`'s cookie, which is 2b |

The `n/a` tests:

- `testCannotBeSerialized` and `testCannotBeUnserialized` give the
  handler to `serialize()` and `unserialize()`. A Rust handler is not a
  PHP object, so there is nothing to give them.
- `testWriteDoesNotMarshallIgbinaryEmptyData` needs igbinary. The oracle
  image does not have it, so PHPUnit skips the test there too.

Some upstream tests run PHP's `session_start()` in a PHP server
(`Tests/Session/Storage/Handler/Fixtures/*.php`). They move to stage 2b,
together with `NativeSessionStorage`, the proxies,
`NativeFileSessionHandler` and the emulation of `session_start()`. They
are AbstractSessionHandlerTest and NullSessionHandlerTest. The database
and cache handlers (PDO, Redis, Memcached, MongoDB) and
`SessionHandlerFactory` are later stages.

## Prior art and choice

The search for Rust ports was not repeated (stage 1's row). tower-sessions
has a store trait, but not PHP's handler contract: there is no
`validateId()`/`updateTimestamp()`, and no prefetch.

The handlers are ported as they are:

- **PHP's interfaces** are traits. `\SessionUpdateTimestampHandlerInterface`
  is found through `as_update_timestamp()`, the port of `instanceof`.
- **`AbstractSessionHandler`'s subclasses** give it a `Backend`: the
  abstract `doRead`/`doWrite`/`doDestroy`, their class and their
  `destroy()`, which is the one `write()` calls.
- **Session extension state:** `php_session` keeps it per PHP request:
  the status, the id and the `session.*` ini values with PHP's defaults,
  checked in the oracle. `header()` and `setcookie()` go to the
  request's output calls.

## Evidence

- **Upstream tests:** 41 ported, 5 `n/a`. No GitHub advisory covers these
  files.
- **Recorded vectors:** 1,200 scripts recorded from PHP 8.4.27
  (`scripts/record.sh http-foundation-session-handlers`, 10,259 answers),
  all matching.
  - **The chain:** each script builds a chain over scripted leaves. A
    leaf is either an `AbstractSessionHandler` subclass whose
    `doRead`/`doWrite`/`doDestroy` answer from queues, or a bare
    `\SessionHandlerInterface` under `StrictSessionHandler`. The chain
    may put a `MarshallingSessionHandler` on top of the leaf, with the
    identity marshaller or a scripted one. Or it may be a
    `MigratingSessionHandler` over two leaves.
  - **What the queues answer:** reads give data, `''`, `false` or an
    exception. Writes and destroys give `true` or `false`. `gc()` gives a
    count or `false`. The scripted marshaller can return a non-string, a
    missing key, `$failed` set either way, `null`, an array, a float, or
    a `\DomainException` or `\RuntimeException`.
  - **Operations:** 3 to 14 per script, from `open`, `close`,
    `validateId`, `read`, `write`, `destroy`, `updateTimestamp` and
    `gc`.
  - **Compared:** each result or exception, and the calls each leaf and
    the marshaller saw.

  There are also 60 `IdentityMarshaller::marshall()` arrays of mixed
  values, and the three constructor errors.
- **Found by the vectors:**
  - **`$failed` in `MarshallingSessionHandler::write()`:** upstream checks
    `isset($failed['data'])`, a key, but symfony/cache's marshallers list
    the failed keys as values (`$failed[] = $id`). So in PHP a failure
    listed that way does not stop the write, and the write then fails
    with a `TypeError` for the missing `'data'`. `$failed` is now PHP's
    array, and the check is the `isset()`.
  - **The `TypeError` for that missing `'data'`** names the class that
    declares `write()`: `AbstractSessionHandler::write()`, not the
    interface (`write_class()`).
- **Mutations:** 26 tried, 25 caught by the tests and vectors. Among
  them:
  - each prefetch condition;
  - `newSessionId` kept or not;
  - empty data destroying;
  - Strict's `doDestroy` flag;
  - the marshalling `\DomainException` and empty-data paths;
  - each handler Migrating calls, and the result it returns;
  - the identity marshaller's check;
  - the two constructor checks;
  - the `$failed` key check;
  - `write_class()`.

  The one not caught is the `Cache-Control` header that `open()` sends
  when `session.cache_limiter` is empty, which nothing can see from the
  CLI. Stage 2b's PHP-server tests cover it.

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **Not yet proven against PHP:** `AbstractSessionHandler::destroy()`'s
  session cookie and `open()`'s `Cache-Control` header are ported, but
  only stage 2b's PHP-server tests can show them against PHP. The README
  does not claim them yet.
- **The marshaller interface:** only `MarshallerInterface` comes from
  symfony/cache. Its `DefaultMarshaller` and the other marshallers are
  not ported.
- **Next, stage 2b:** `NativeSessionStorage`, `session_start()`'s
  emulation, the proxies, `NativeFileSessionHandler`,
  `PhpBridgeSessionStorage`, then `Request`'s session methods.
