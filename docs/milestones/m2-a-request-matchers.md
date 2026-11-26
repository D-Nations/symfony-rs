# M2-A: RequestMatchers

November 26. Branch `m2/request-matchers`. Crate touched:
`symfony-http-foundation`. New modules: `request_matcher_interface`,
`chain_request_matcher`, and `request_matcher` with its eleven matchers.

The crate now depends on `symfony-expression-language` (this workspace's
port; no new external crate). Oracle: a new recorder,
`http_foundation_request_matchers.php`.

PLAN.md decision 20(e), which the owner accepted as recommended, is
recorded in this change.

## Scope

| PHP file | Rust | Upstream tests |
|---|---|---|
| symfony/http-foundation `RequestMatcherInterface.php` (v8.1.11) | `crates/http-foundation/src/request_matcher_interface.rs` | through the matchers' tests |
| `ChainRequestMatcher.php` | `crates/http-foundation/src/chain_request_matcher.rs` | none upstream; the recorded scripts |
| `RequestMatcher/*RequestMatcher.php` (11 classes) | `crates/http-foundation/src/request_matcher/*_request_matcher.rs` | 20 of 20 |

## Prior art and choice

No new search: these classes are small, use only what HttpFoundation
already ports, and the PLAN's HttpFoundation row found no Rust port.

`matches()` returns the `bool` or what the request's getters threw (an
untrusted host's `SuspiciousOperationException`, for one). Getters cache,
so the request is taken mutably.

For `ExpressionRequestMatcher`, the `request` variable is
expression-language's `MethodObject` over a copy of the request:

- the common getters and tests;
- its bags, with `all()`, `keys()`, `get()`, `has()` and `count()`;
- for `query`, `request` and `cookies`, the `InputBag`'s own `get()`, so a
  value that is not a scalar throws `BadRequestException`, as in PHP.

## Evidence

- **Upstream tests:** 20 of 20 for the eleven matchers. There is no
  ChainRequestMatcherTest upstream. No GitHub advisory covers these
  files.
- **Recorded vectors:** 1,500 scripts recorded from PHP 8.4.27
  (`scripts/record.sh http-foundation-request-matchers`), 9,000 answers,
  all matching. Each script builds a request from pools: URIs with encoded
  characters and queries, methods, hosts (one invalid), `SERVER_PORT` as an
  int or a string, HTTPS, client addresses, headers, bodies, attributes
  (including a closure) and a locale. Six matchers built from pools then
  ask it whether it matches. They cover:
  - lists with spaces, tabs and empty items;
  - patterns that do not compile;
  - 40 expressions over the request object, including results of null,
    arrays, strings and floats;
  - chains.

  Two fixed scripts come first. They set no host (`HTTP_HOST` null and the
  URI `/`), so `getPort()` returns `SERVER_PORT` as given, and `'8000'` is
  not `=== 8000`.
- **Mutations, each caught** (differences of 9,000):
  - list items trimmed at the ends (4);
  - empty items dropped (71);
  - GET without HEAD (26);
  - query keys in any order (7);
  - hosts matched with case (20);
  - the path not decoded (11);
  - ports compared loosely (1, a fixed script);
  - non-string attributes matched (39);
  - a null expression result taken as false (91);
  - empty IP lists matching (13);
  - header bag keys compared with case (8).

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- **The expression's `request`:** methods it lacks are "Unable to call
  method" errors. A method that changes it changes only the copy.
- **Next:** Session, then RateLimiter.
