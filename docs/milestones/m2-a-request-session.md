# M2-A: Session, stage 3: Request's session methods

November 27. Branch `m2/request-session`. Crate touched:
`symfony-http-foundation`. Two of its files change, `request.rs` and
`request_stack.rs`. No other crate calls the changed
`RequestStack::get_session()`.

No new dependency. Oracle: a new recorder,
`http_foundation_request_session.php`.

## Scope

| PHP files (symfony/http-foundation v8.1.11) | Rust | Upstream tests |
|---|---|---|
| `Request.php`: `getSession()`, `hasPreviousSession()`, `hasSession()`, `setSession()`, `setSessionFactory()` | `request.rs` | RequestTest's testHasSession, testGetSession, testHasPreviousSession: RequestTest is now 116 of 120 |
| `RequestStack.php`: `getSession()` | `request_stack.rs` | none (RequestStackTest has no session test) |

The one RequestTest test still pending is `testIntlLocale`, which needs
intl.

## Prior art and choice

The search was not repeated: stage 1's stack row covers the Session
classes, and the Request row covers `Request.php`.

**Choice:** port the methods as written.

- **The session property:** PHP's `SessionInterface|\Closure|null` is a
  three-way enum: no session, a factory, or a session.
- **Sharing:** the session is an `Arc`, as PHP's object handle is. A
  clone, `duplicate()` included, shares it, because upstream's
  `__clone()` leaves `$session` alone.
- **The factory** returns `Result<Option<Arc<dyn SessionInterface>>>`.
  `Ok(None)` stands for a factory that returns `null`. `getSession()`
  turns that into `setSession()`'s `\TypeError`, with PHP's text up to the
  "called in" location, and keeps the factory in place.

## Evidence

- **Upstream tests:** the three session tests, with the same assertions.
  testGetSession's `assertInstanceOf(Session::class, ...)` checks that
  the `Session` given is the one returned.
- **Recorded vectors:** 800 scripts recorded from PHP 8.4.27
  (`scripts/record.sh http-foundation-request-session`), 8,120
  operations, all matching on the first replay.
  - **A script:** a `Request::create()` with 0 to 3 cookies from a name
    pool, and a `RequestStack`.
  - **The operations,** on requests picked by index:
    - `hasSession()` with either argument, `getSession()` and
      `hasPreviousSession()`;
    - `setSession()` with a new or an earlier session;
    - `setSessionFactory()` with a new factory or one already set on
      another request. A factory returns the same session each time, a
      new session each call, `null`, or throws a `\RuntimeException`;
    - `duplicate()` and `clone`;
    - a cookie set or removed;
    - a session's `setName()`;
    - `push()`, `pop()` and the stack's `getSession()`.
  - **Compared:** each result (a boolean, the session's label, the
    request's index, or the exception's class and message), and how many
    times each factory was called.
- **Mutations:** 8 tried, 8 caught:
  - `hasSession(true)` true for a factory;
  - the factory's session not kept;
  - `hasPreviousSession()` not calling the factory;
  - `hasPreviousSession()` ignoring the cookie;
  - a `null` factory accepted;
  - `duplicate()` dropping the session;
  - the stack using the main request instead of the current one;
  - the stack skipping factories.

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **Other non-session values:** a factory that returns something else
  that is not a session (a string, an array) cannot be written. In PHP
  it gives the same `\TypeError` with that type's name.
- **Next:** the database and cache session handlers and
  `SessionHandlerFactory`, then HttpKernel's session listeners
  (`AbstractSessionListener`, `SessionListener`), which call
  `setSessionFactory()`.
