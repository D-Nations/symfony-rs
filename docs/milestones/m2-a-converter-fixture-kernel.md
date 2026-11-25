# M2-A: the converter fixture on the ported kernel

November 25. Branch `m2/converter-fixture-kernel`. Crates touched:
`symfony-http-kernel` (`response_listener`, `container_controller_resolver`,
`controller_resolver`, `controller`), `psr-container` (new),
`symfony-routing` (`redirectable_url_matcher`), `converter-fixture-app`.
Oracle: the ControllerResolver recorder gains a container mode.

The HTTP adapter stage moved request-lab onto the ported kernel; the
converter fixture (M1-D) still ran on the typed `Kernel<C>` with local
stand-ins for the listeners and HttpFoundation. This stage ports what the
fixture's Symfony app runs that the kernel did not have, and moves the
fixture onto `symfony-runtime`'s front controller and the ported
`HttpKernel`.

## Scope

| PHP file / behaviour | Rust | Upstream tests |
|---|---|---|
| symfony/http-kernel `EventListener/ResponseListener.php` (v8.1.11) | `crates/http-kernel/src/response_listener.rs` | ResponseListenerTest 7 of 7 |
| symfony/http-kernel `Controller/ContainerControllerResolver.php` | `crates/http-kernel/src/container_controller_resolver.rs` | ContainerControllerResolverTest 21 of 21 (6 own, 15 inherited) |
| psr/container 2.0.2 (`ContainerInterface.php` and the two exception interfaces) | `crates/psr-container` (new) | none upstream |
| `$this` in a declared method's body | `Method::with_this` in `controller_resolver.rs` (Upstream: the resolver's) | `tests/controller_resolver_this.rs` (5, not upstream) |
| `$this->context` in `redirect()` | `RedirectableUrlMatcherInterface::redirect_in` | `tests/redirect_in.rs` (3, not upstream) |
| symfony/framework-bundle `Routing/RedirectableCompiledUrlMatcher.php` `redirect()` and `Controller/RedirectController.php` `urlRedirectAction()` | `fixtures/converter-app/src/runtime/framework_bundle.rs` (local until M3) | none run (FrameworkBundle is M3) |
| The fixture | `fixtures/converter-app` | its 3 tests |

## Prior art and choice

No new search for the HttpKernel files (PLAN.md's HttpKernel row: no
Rust port). For PSR-11, crates.io has DI containers with their own APIs
(shaku, di, inject), none an untyped id-to-value interface; psr/container
is ported as a workspace crate, as psr/log was (PLAN.md stack row
"HttpKernel's `ContainerControllerResolver.php` and psr/container").

PHP's subclass overrides `instantiateController()`. Compared: a trait with
overridable methods (every caller of the resolver generic) and the parent
holding an optional container, calling the override where PHP's
`$this->instantiateController()` runs. Chosen: the second, so every parent
rule stays one code path; `ContainerControllerResolver` dereferences to
the parent. PHP's `$container instanceof DependencyInjection\Container`
is the closed enum `ControllerContainer` (`Psr`, `DependencyInjection`
with `getRemovedIds()`), not a downcast to a DI type that does not exist
before M3.

For `$this`: a class registry built per request with closures over the
request scope would fake it for one object per class; giving the method
the object it is called on is what PHP does, and every real controller
needs it. `Target::Method` now carries the object, and a method declared
`with_this` is run on it; reflection (name, parameters, attributes) is
the declared body's, so argument resolution is unchanged.

## Evidence

- ResponseListener: ResponseListenerTest 7 of 7; 1,500 scripts recorded
  from PHP 8.4.27 (`scripts/record.sh http-kernel-response-listener`), all
  matching; six mutations of the rules, each caught.
- ContainerControllerResolver: ContainerControllerResolverTest runs the
  parent's 15 methods on the container resolver as PHPUnit does (their
  bodies moved to `tests/common/controller_resolver_test_class.rs` behind
  a `TestClass` trait; each test file names every method it runs). The
  ControllerResolver recorder's container mode (`CR_MODE=container`,
  `scripts/record.sh http-kernel-container-controller-resolver`) recorded
  2,000 scripts with DI containers (services set, removed ids with values
  `true`, `false`, `null`, `1`) or other PSR-11 containers, services that
  are objects, closures or a `stdClass` under class names in any spelling
  or other ids, and `_controller` values naming them; all match,
  including the container's `has()`, `get()` and `getRemovedIds()` calls
  in order. Seven mutations, each caught; the one upstream's tests miss
  (catching every throwable, not only `\Error`) fails the recorded
  scripts.
- The fixture: all 16 responses recorded from the PHP app match in
  status, headers and body through the front controller. Four mutations
  of its rules (the redirect's default port, its query string, the
  subscriber's exception headers, the 422 status), each caught.

## Found on the way

- **ControllerResolver:** `Closure` has a private `__construct`, so
  `method_exists($closure, '__construct')` is true and PHP answers
  "should be public and non-abstract" (checked in PHP 8.4.27). The port
  said "Expected method". Found by the container mode, whose services
  include closures; fixed.
- **Re-recorded ControllerResolver vectors:** the 4,000 default-mode
  scripts were re-recorded; only the counter at the end of
  `class@anonymous` names moved, because the recorder now declares two
  more classes. The replay reads those names from the recording, and all
  4,000 still match.
- **`Content-Length`:** axum writes it for a body of known size; PHP's
  built-in server, which the fixture's recording used, closes the
  connection instead (`Connection: close`, already left out), so the
  recording has none. The fixture test checks that the header equals the
  body's length, then leaves it out of the header comparison. This is a
  change to the comparison rule, stated here and in the fixture README.

## Runtime gaps left in the fixture

| Gap | Destination |
|---|---|
| `RedirectableCompiledUrlMatcher::redirect()`, `RedirectController::urlRedirectAction()` (`parse_url()` refused for a path not starting with `/`) | a FrameworkBundle crate (M3) |
| `implode()`, `ucfirst()` | `php-value`, with recorded vectors |
| `ErrorListener` and the production error page; FrameworkBundle's other listeners | M2-A; none changes a recorded case |
| The compiled container's removed ids | M3 (the DI container) |

## Converter contract 0.3

The request's container is PSR-11 and request-scoped services are
`Send + Sync`; exceptions are shared `Throwable`s; the boundary is the
front controller, routes a routing `RouteCollection`, controllers declared
classes resolved from the container with `Method::with_this`, bodies
synchronous ([docs/converter-contract.md](../converter-contract.md)).

## Advisories

`parity/advisories.tsv` lists none for the files ported here.

## Checks

On Windows at `118eb86`: `cargo fmt --check` clean; `cargo clippy
--all-targets -- -D warnings` clean; `cargo test --workspace
--no-fail-fast` 3,413 passed, 0 failed; `scripts/sources.sh --check` and
`scripts/parity-check.sh` pass.

Linux at `118eb86`, fresh containers: `scripts/converter-fixture.sh`
(fmt, clippy and every test of the fixture and the crates it links, from
an empty directory) passes, 1,388 tests, 0 failed; `scripts/linux-test.sh
-p symfony-http-kernel -p symfony-routing -p psr-container -p
symfony-runtime -p request-lab -p converter-fixture-app` passes, 687
tests, 0 failed.

symfony-demo-rs comparison: **not run**, paused by the owner (November
23). Acceptance criteria that name it stay open.
