# M2-A HttpKernel stage 2: RouterListener

November 25. Branch `m2/http-kernel-router-listener`. Crates touched:
`symfony-http-kernel`, `psr-log` (a context value kind), `symfony-routing`
(`Class::ancestry()` made public, one test match arm).

Built on HttpKernel stage 1 (the kernel and its events) and routing's
RequestContext and matchers. RouterListener is what connects routing to
the kernel: it fills the routing context from each request and turns the
matched route into request attributes.

## Scope

Upstream symfony/http-kernel at the pin (v8.1.11).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `EventListener/RouterListener.php` | `crates/http-kernel/src/router_listener.rs` | RouterListenerTest 10 of 11 (1 pending) |
| `Resources/welcome.html.php` | `crates/http-kernel/resources/welcome.html`, rendered in the oracle | `testNoRoutingConfigurationResponse` |

Pending: `testWithBadRequest`, which needs ValidateRequestListener and
ErrorListener.

Two tests hand the kernel `new ControllerResolver()` and
`new ArgumentResolver()`, which are not ported yet:
- In `testNoRoutingConfigurationResponse` routing fails first, so the
  resolvers are never reached.
- `testRoutingContextOfTheParentIsRestoredAfterNestedMainRequest`
  resolves closures in `_controller`; the test's resolver looks the closure
  up by the name the attribute holds.

Prior art: the HttpKernel stack row (no Rust port). The PLAN.md row for
this file records the choice about the shared context.

## Port notes

- **The shared context.** In PHP, the listener, the matcher and the URL
  generator share one `RequestContext` object, so the listener's
  `fromRequest()` is what they see. Here the listener holds a
  `Handle<RequestContext>`. `UrlMatcherInterface::get_context()` returns
  that handle, as does a request matcher that is also context-aware (the
  Router).
- **Routing's own matchers.** `UrlMatcher` and `RedirectableUrlMatcher`
  keep their context by value. As request matchers they match in a copy
  filled from the request, as their `matchRequest()` does, so the listener
  needs a context of its own for them.
- **Parent contexts (new in v8.1.11).** When a sub-request starts, the
  parent's context is saved; when its `FinishRequestEvent` arrives, the
  saved context is restored rather than computed again. Without an event
  (`null`), it is recomputed from the parent request. PHP keys the saved
  contexts by `spl_object_id()`; here each entry holds the request itself.
- **Route mapping.** `_route_mapping` follows PHP's array and string
  offset rules for whatever a matcher returns:
  - `isset()` on a string mapping;
  - `[$a, $b] =` on short arrays;
  - writes into a string attribute (a one-byte write, space padding,
    `Cannot assign an empty string to a string offset`);
  - `Cannot use a scalar value as an array`;
  - offsets that are not keys.

  PHP's warnings and deprecations along the way are not reproduced.
- **Logging.** The `Matched route` log passes PHP values, so psr-log's
  `ContextValue` gains a `Value` kind (PSR-3's `mixed`).
- **The welcome page.** Upstream's template is rendered once in the
  oracle, with placeholders for the hue (`random_int(0, 360)`, here a
  random value from the standard library's hash keys) and the project
  directory. The directory is `realpath()` (finder's, as config uses) plus
  `/`, which is Linux PHP's `DIRECTORY_SEPARATOR`.

## Upstream checkout found at v8.1.10

This checkout's `upstream/` had http-foundation, http-kernel and routing
at their v8.1.10 commits, not the v8.1.11 pins in `parity/packages.tsv`.
The oracle mounts these three checkouts, so vectors recorded here could
come from v8.1.10.

Between the two versions, `Request.php` and `RouterListener.php` change;
`Response.php` and `HttpKernel.php` do not. After `fetch-upstream.sh`, the
Response, lifecycle and Request vectors were re-recorded at the pins and
are byte-identical. `scripts/record.sh` now refuses to run while a moved
package's checkout is not at its pinned commit.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | RouterListenerTest 10 of 11 | all pass |
| Scripts (`crates/http-kernel/tests/data/router_listener.json`, `oracle/recorders/http_kernel_router_listener.php`) | 2,000 seeded scripts, described below | all match |

Each script builds a listener on one of three matchers: a URL matcher, a
request matcher, or one that is both, with or without its own context. It
then runs one to three nested requests through `onKernelRequest()` and
unwinds them with `onKernelFinishRequest()`, given the event or null. The
matcher returns generated parameters (`_route`, `_controller`,
`_route_mapping` of any shape) or throws one of five exceptions.

Compared:
- the constructor's exception;
- each call's exception (class, message, code, status, headers,
  previous);
- the request attributes;
- the matcher's calls;
- the logger's calls;
- the context after every step;
- `onKernelException()`: whether it sets a response, its status, and
  whether its page is the recorded welcome page.

Outcomes covered:
- 254 constructor refusals;
- 413 `NotFoundHttpException`s, 276 `MethodNotAllowedHttpException`s and
  306 `BadRequestHttpException`s;
- 120 TypeErrors and 13 Errors from route mappings;
- 155 other throwables passed through;
- 408 welcome pages.

Mutation checks on the vectors:

| Mutation | Scripts that differ |
|---|---|
| No parent context restore | 865 |
| Saving the context for main requests too | 1,141 |
| No `BadRequestHttpException` wrapping | 293 |
| Debug mode ignored | 175 |
| A null `_route` not logged as `n/a` | 128 |
| A falsy referer appended | 58 |
| No string offset write | 8 |
| A string mapping not indexed | 5 |
| A loose `'' !==` | 3 |

## Advisories

None concerns this file. GHSA-6439-2f28-8p8q, the attribute listeners,
stays `not-ported`.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner).
