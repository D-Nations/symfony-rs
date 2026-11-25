# Converter fixture (M1-D)

Private workspace crate; MIT. The Rust side of the converter fixture
(PLAN.md, "Early conversion proof and second application"): the Symfony app
in [oracle/apps/converter-fixture](../../oracle/apps/converter-fixture),
written against [converter contract 0.3](../../docs/converter-contract.md),
on the ported Symfony kernel served by `symfony-runtime`.

**Hand-authored, not converter output.** The crate has the shape
symfony2rust is to emit, and proves the runtime interface only. The
conversion gate stays open until symfony2rust emits this crate and it
builds and passes without manual edits.

| Module | PHP | Contract |
|---|---|---|
| `app::greet_controller` | `Controller/GreetController.php` | Section 6: a `PhpObject` service of a declared class; `greet()` declared with `Method::with_this` |
| `app::greeter` | `Service/Greeter.php` | Sections 2 and 4: typed arguments; request-scoped because it reaches `VisitCounter` |
| `app::greeting` | `Service/Greeting.php` | Section 3: readonly class as `Arc<T>`; `array` as `PhpArray`; `(string) $float` from `php-value` |
| `app::visit_counter` | `Service/VisitCounter.php` | Section 3: mutable class as `Arc<Mutex<T>>` newtype |
| `app::greeting_exception` | `Service/GreetingException.php` | Section 5: an exception class thrown as a shared `Throwable` |
| `app::error_subscriber` | `EventSubscriber/ErrorSubscriber.php` | Sections 5 and 7: `kernel.exception` subscriber that downcasts the throwable |
| `container` | compiled container | Section 4: application scope, per-request scope, and the request's PSR-11 container |
| `routes` | compiled routes | Section 6: a routing `RouteCollection` (`router.utf8`) |
| `kernel` | `public/index.php`, `Kernel.php`, FrameworkBundle's listeners | Section 6: `RouterListener`, `ResponseListener`, the subscriber, `ContainerControllerResolver`, `HttpKernel`, `FrontController` |
| `runtime` | FrameworkBundle's `RedirectableCompiledUrlMatcher::redirect()` and `RedirectController::urlRedirectAction()`; `implode()`, `ucfirst()` | Not generated: runtime gaps, listed in [docs/milestones/m2-a-converter-fixture-kernel.md](../../docs/milestones/m2-a-converter-fixture-kernel.md) |

## Evidence

| Claim | Test |
|---|---|
| The 16 responses recorded from PHP 8.4.27 (`scripts/record.sh converter-fixture`) match in status, headers and body | `tests/recorded.rs`, `every_recorded_response_matches` |
| A mutable service's state is not shared between requests | `request_scoped_state_is_not_shared`, and the recording's `greet` / `greet-again` |
| The containers, the subscriber and the controller are `Send + Sync` | `concurrency_bounds_hold` |
| The same on Linux from an empty directory | `scripts/converter-fixture.sh` (local, in a removed container) |

Requests go through the front controller (`router().oneshot()`), as a
server's would. Headers are compared as a multiset, without those PHP's
built-in server adds (`Host`, `Connection`, `X-Powered-By`) and without
`Date`, which Symfony sets and the HTTP server sets here. `Content-Length`,
which axum writes where PHP's built-in server closed the connection, is
checked against the body's length and then left out. Logs are not
compared.

`cargo run -p converter-fixture-app` serves the app on
`127.0.0.1:${FIXTURE_PORT:-8022}`.
