# M2-A: the listener seam

November 26. Branch `m2/listener-seam`. Crates touched:
`symfony-event-dispatcher` (`compiled_event_dispatcher`, new; two
crate-private hooks in `event_dispatcher`), `symfony-http-kernel` (tests
only), `converter-fixture-app`. No oracle change.

PLAN.md's static-specialization scope asks M2-A for a minimal listener
execution seam: a listener pipeline trait, the dynamic registration API
and default behaviour kept, a small static fixture, and priority,
equal-priority order, propagation, controller replacement, early response,
exceptions and response filtering tested in both modes. The ported
`HttpKernel` takes a dispatcher, so the seam is a dispatcher, not the
`Kernel<C, L>` the PLAN first sketched.

## Scope

| Behaviour | Rust | Evidence |
|---|---|---|
| A listener set settled before the first request, dispatched as the pinned `EventDispatcher.php` dispatches it | `ListenerMap`, `CompiledEventDispatcher` in `crates/event-dispatcher/src/compiled_event_dispatcher.rs` (Upstream: none) | `tests/compiled_event_dispatcher.rs` |
| The seam generated code implements | `ListenerPipeline` (`has_listeners`, `dispatch`, `thaw`) | the typed-pipeline test in the same file |
| Anything but `dispatch()` and `hasListeners()` falls back to a plain dispatcher in the pinned one's state | `thaw()`: `EventDispatcher::add_slot` (shared slots) and `optimize` (the `krsort()` a first dispatch leaves) | the same 20,000 scripts |
| The kernel's lifecycle in both modes | `crates/http-kernel/tests/lifecycle_recorded.rs` | the 1,500 PHP-recorded scripts, plain and compiled |
| The fixture on it | `fixtures/converter-app/src/kernel.rs` | its 3 tests; converter contract 0.4 |

## Prior art and choice

The search found the upstream design itself. Symfony 8.2, which comes
after the pinned 8.1, adds `CompiledEventDispatcher`
([symfony/symfony#66122](https://github.com/symfony/symfony/pull/66122)).
The container hands it the listener map as data, sorted in run order. It
dispatches from that map, and anything but `dispatch()` and
`hasListeners()` copies the map into a plain `EventDispatcher` that every
later call goes through. Compared:

| Option | For | Against |
|---|---|---|
| Upstream 8.2 `CompiledEventDispatcher` | Upstream's own answer; M3's container output maps onto it; the fallback keeps every dynamic call exact | Not the pinned version: it deprecates mutation, and its thaw rebuilds the table with fresh lazy closures |
| [#66121](https://github.com/symfony/symfony/pull/66121), the `addListener()` calls folded into `EventDispatcher`'s constructor | No new class | Upstream rejected it: the closures stay, and it needs a flag on `EventDispatcher` |
| Drupal's `ContainerAwareEventDispatcher` (precompiled array, changed in place) | Fast, one table | Its own mutation code, a second implementation of the table's rules |
| [mod-events](https://docs.rs/crate/mod-events) (crates.io) | Typed handlers, priorities | Run-time registration behind a lock; no PHP identity, lazy or ordering semantics; a dependency |

Chosen: the 8.2 design, made to behave as 8.1. No deprecation notice is
triggered. The thaw rebuilds the state the pinned dispatcher would have:
- the same `addListener()` calls in the same order;
- each stored listener's slot shared with the compiled table, so a lazy
  listener resolved on either side is resolved on both, as PHP's
  by-reference lists are;
- the priority buckets of every event dispatched so far `krsort()`ed, as
  `optimizeListeners()` leaves them. `getListenerPriority()` and the
  table `TraceableEventDispatcher` reads see that order.

`as_event_dispatcher()` thaws, because the compiled dispatcher stands in
for an `EventDispatcher`. No new dependency.

## Evidence

- `compiled_dispatcher_runs_as_the_plain_one`: 20,000 generated scripts run
  on the plain `EventDispatcher` (whose behaviour is pinned by its upstream
  tests and the oracle edge vectors) and on the compiled one, compared call
  by call:
  - what they cover: dispatches (named and unnamed), `getListeners()`
    for one event and for all, `getListenerPriority()`, `hasListeners()`,
    adding and removing listeners and subscribers, and the raw table;
  - what the listeners are: methods, invokables, anonymous and bound
    closures, lazy listeners whose factories log their calls, duplicates
    at several priorities, and subscribers;
  - what the listeners do: stop propagation, throw, or add and remove
    listeners during a dispatch.

  All match, and 14,284 dispatches ran on the compiled path before any
  thaw. The first version failed on 37 scripts: a listener's change
  thawed the dispatcher mid-dispatch, and a lazy listener later in that
  same dispatch was resolved only in the compiled table. Shared slots
  fixed it.
- `a_typed_pipeline_runs_as_the_plain_dispatcher`: a hand-written
  pipeline calling its listeners' methods directly (the small static
  fixture), against the plain dispatcher.
- `lifecycles_run_as_in_php_on_a_compiled_dispatcher`: the 1,500 scripts
  recorded from PHP 8.4.27 with http-kernel v8.1.11 (prioritized
  listeners that return early responses, stop propagation, replace the
  controller or its arguments, give, replace or rethrow exceptions, and
  filter responses) pass on the compiled dispatcher, and none thaws it.
- Mutations, each caught:
  - by the generated scripts: no `krsort()` on thaw; ascending run
    order; unshared slots; ignoring propagation; `hasListeners()` always
    true; dispatches not noted for the thaw;
  - by the recorded lifecycles: ascending run order (519 differences);
    ignoring propagation (227).
- The converter fixture runs on a `CompiledEventDispatcher`; its 16
  recorded responses match.

## Checks

See the PR for the output of the full gate on the final commit.

The symfony-demo-rs comparison was not run: the owner paused it
(November 23).

## Left open

- M3-A generates per-event pipelines from the compiled container
  (`ListenerPipeline`), and lazy listeners from its service locator.
- The baseline measurements the PLAN asks for (fixture latency,
  allocations, and so on) are a separate item, still pending. No
  speed-up is claimed here.
