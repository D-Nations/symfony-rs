# Symfony EventDispatcher

Private workspace crate: a port of `symfony/event-dispatcher` v8.1.5
(`7458da64`), as installed by symfony/demo. MIT; Symfony source and tests
copyright Fabien Potencier. Unofficial, not affiliated with or endorsed by
Symfony.

```rust
use std::sync::Arc;
use symfony_event_dispatcher::*;

let dispatcher = EventDispatcher::new();
dispatcher.add_listener("kernel.request", Listener::closure(|event, name, _| {
    if let Some(event) = event.downcast_mut::<Event>() {
        event.stop_propagation();
    }
    assert_eq!(name, "kernel.request");
    Ok(())
}), 10).unwrap();
let mut event = Event::new();
dispatcher.dispatch(&mut event, Some("kernel.request")).unwrap();
assert!(event.is_propagation_stopped());
```

| Contract | Source | Evidence |
|---|---|---|
| Priority order (higher first), ties in registration order; `dispatch()` stops before the next listener once propagation is stopped | `EventDispatcher.php` | EventDispatcherTest (29), run on the base class and on a subclass (ChildEventDispatcherTest, 29) |
| Listener forms: closures, `[$object, 'method']`, invokable objects, lazy `[$factory, 'method']` / `[$factory]` resolved once and in place | `EventDispatcher.php` | Lazy-listener upstream tests |
| `===` and closure `==` when finding a listener: `$o->m(...)`, `$o(...)` and `$o->__invoke(...)` are equal, method names case-insensitive; anonymous closures equal only themselves | `EventDispatcher.php`, PHP 8.4 | `testNamedClosures`; oracle vectors |
| `removeListener()` removes every match at every priority; `getListenerPriority()` returns the first match in bucket order, which is insertion order until a sort | `EventDispatcher.php` | Oracle vectors |
| `getListeners()` lists events in the order they were first sorted; empty events are left out | `EventDispatcher.php` | Upstream tests; oracle vectors |
| A dispatch calls the listeners of its start: one added meanwhile runs next time, one removed meanwhile still runs | `EventDispatcher.php` | Oracle vectors |
| The base class resolves lazy listeners on first call; a subclass resolves them when sorting | `EventDispatcher.php` (`$optimized`) | `testDispatchLazyListener` on both |
| Default event name is the event's PHP class; non-stoppable events reach every listener; a listener's exception ends the dispatch and is returned | `EventDispatcher.php`, PSR-14 | Oracle vectors |
| Subscribers: `'m'`, `['m', $priority]`, `[['m1', $p], ['m2']]` | `EventDispatcher.php`, `EventSubscriberInterface.php` | Subscriber upstream tests |
| `GenericEvent` subject, ordered arguments, `ArrayAccess`, iteration, `Argument "%s" not found.` | `GenericEvent.php` | GenericEventTest (13) |
| `ImmutableEventDispatcher` delegates reads and refuses changes with `Unmodifiable event dispatchers must not be modified.` | `ImmutableEventDispatcher.php` | ImmutableEventDispatcherTest (7) |

Oracle vectors: `scripts/record.sh event-dispatcher` runs
`oracle/recorders/event_dispatcher_edges.php` on PHP 8.4.27 and writes
`tests/data/edges.txt`; the tests after the upstream ones in
`tests/event_dispatcher.rs` pin each line.

## Rust shape

- Events implement `EventObject` (`class()`, and `is_propagation_stopped()`
  for PSR-14's `StoppableEventInterface`) and are passed as
  `&mut dyn EventObject`; listeners downcast them. PHP returns the event
  from `dispatch()`; here the caller keeps it.
- PHP objects in listeners implement `Callable`, which calls a method by
  name and returns `UndefinedMethod` (`Call to undefined method C::m()`)
  for one it lacks. Identity is the `Arc` allocation.
- PHP's `getListeners(?string)` is `get_listeners(name)` and
  `get_all_listeners()`. Mutators return `Result` because the immutable
  dispatcher throws from them. Priorities are `i64`, as PHP's `int`.
- A PHP subclass that overrides `callListeners()` is
  `EventDispatcher::with_call_listeners(impl CallListeners)`.
- The dispatcher is `Send + Sync`. No lock is held while a listener or a
  lazy factory runs, so either may call back into it, as in PHP. Two
  threads dispatching a lazy listener for the first time may both call its
  factory; factories must be idempotent (service getters are).

## Not ported yet

- `Debug/TraceableEventDispatcher.php`, `Debug/WrappedListener.php`
  (36 tests): need Stopwatch and a PSR-3 logger.
- `DependencyInjection/RegisterListenersPass.php`, `AddEventAliasesPass.php`
  and `Attribute/AsEventListener.php` (26 tests): container compilation, M3.
- String callables (`'var_dump'`, `['Class', 'staticMethod']`).
- symfony-http-kernel still has its own typed dispatcher: its events borrow
  the request, and `EventObject` needs `'static` types. It moves onto this
  crate when HttpKernel's events own their request (M2).
