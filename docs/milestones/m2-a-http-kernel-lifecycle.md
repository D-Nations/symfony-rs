# M2-A HttpKernel stage 1: the lifecycle on Request and Response

November 25. Branch `m2/http-kernel-lifecycle`. Crates touched:
`symfony-http-kernel`, `symfony-http-foundation`, `php-value`,
`symfony-event-dispatcher` (one `cfg`).

Built on HttpFoundation stages 5 and 6 (Request, Response) and the ported
EventDispatcher. This is the second half of the HTTP path: the kernel
that turns a Request into a Response through listeners, as a Symfony
application runs it.

## Scope

Upstream symfony/http-kernel and symfony/http-foundation at the pin
(v8.1.11).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `HttpKernel.php`, `HttpKernelInterface.php`, `TerminableInterface.php`, `KernelEvents.php` | `crates/http-kernel/src/http_kernel.rs` | HttpKernelTest 31 of 37 (5 `n/a`, 1 pending) |
| `Event/KernelEvent.php`, `RequestEvent.php`, `ControllerEvent.php`, `ControllerArgumentsEvent.php`, `ControllerMetadata.php`, `ControllerArgumentsMetadata.php`, `ViewEvent.php`, `ResponseEvent.php`, `FinishRequestEvent.php`, `TerminateEvent.php`, `ExceptionEvent.php` | `crates/http-kernel/src/event.rs` | through HttpKernelTest; the event tests (attributes, `evaluate()`) are a later stage |
| `Controller/ControllerResolverInterface.php`, `Controller/ArgumentResolverInterface.php` | `crates/http-kernel/src/controller.rs` | through HttpKernelTest |
| `Exception/HttpException.php`, `HttpExceptionInterface.php`, the 16 subclasses, `ControllerDoesNotReturnResponseException.php` | `crates/http-kernel/src/exception.rs` | HttpExceptionTest and its 16 subclass tests, 86 of 86 |
| HttpFoundation `RequestStack.php`, `Exception/SessionNotFoundException.php` | `crates/http-foundation/src/request_stack.rs`, `exception.rs` | RequestStackTest 5 of 5 |

`n/a` in HttpKernelTest:
- Four tests resolve PHP callable forms: an invokable object, a function
  name, `[$object, 'method']` and a static method array. A Rust
  controller is a closure, as in `testHandleWhenTheControllerIsAClosure`.
- `testHandleWhenTheControllerDoesNotReturnAResponse` reads the source
  line from the exception's trace. The exception's class and message are
  pinned by the extra `controller_result_messages` test instead.

Pending: `testVerifyRequestStackPushPopWithStreamedResponse`, which waits
for StreamedResponse.

The existing typed Axum subset (`Kernel<C>`, `tests/lifecycle.rs`)
stays: request-lab and the converter fixture use it. Its test file no
longer claims HttpKernelTest's rows; the port's file does.

Prior art: see the stack row (none found; borrowed events cannot cross
the type-erased dispatcher).

## Port notes

- **Object handles.** PHP passes objects by handle: the request a
  listener changes is the one on the request stack, and the
  `ControllerEvent` the metadata holds is the one later listeners
  change. php-value gains `Handle<T>` (an `Arc<Mutex<T>>`; cloning gives
  the same object, `same()` is `===`). Requests, responses and the two
  controller events are handles, and the `RequestStack` is one too. The
  kernel never holds a lock while it dispatches.
- **Throwables.** A throwable is an `Arc`, so a listener can see the
  kernel's exception and the kernel can rethrow the same object. This is
  what `assertSame($exception, $e)` and the terminate-exception weak
  reference rely on. A listener that rethrows boxes the `Arc`; the
  kernel unwraps it. `instance_of()` reports each known class's ancestry,
  for the kernel's three checks:
  - `\Error`: passed through unless `$handleAllThrowables` is set;
  - `RequestExceptionInterface`: wrapped in a `BadRequestHttpException`;
  - `HttpExceptionInterface`: status and headers copied to the response.

  Any other Rust error is an `\Exception`.
- **Controllers.** PHP reflects a callable for its parameters (named
  arguments) and attributes. A Rust `Controller` is a closure together
  with those, declared up front. `setController()` compares with `==` for
  closures, which here is identity of the callable.
- **The kernel in events.** Events hand listeners the kernel, so
  `HttpKernel::new()` returns an `Arc` built with `new_cyclic`.
- **Output.** `X-Php-Ob-Level` is the buffer count in `php::Output`.
  `terminateWithException()` sends through Response's recorded output and
  reads `headers_sent()` from it.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | HttpKernelTest 31 of 37, HttpExceptionTest and subclasses 86 of 86, RequestStackTest 5 of 5 | all pass |
| Lifecycles (`crates/http-kernel/tests/data/lifecycle.json`, `oracle/recorders/http_kernel_lifecycle.php`) | 1,500 seeded scripts: controllers that return a response, return another value or throw, or are missing; argument resolvers that return or throw; `$catch` and `$handleAllThrowables`; 0–6 prioritized listeners that log and act. Compared: every listener call with what its event showed, the response or throwable, the stack afterwards, `terminate()` / `terminateWithException()` with their listener calls and output, and 71 throwables from `terminate()` given back to `terminateWithException()` | all match |

Outcomes covered: 569 responses and 931 throwables of 13 classes; the
eight events fire 3,085 times in all, `kernel.view` 42 times.

Mutation checks on the lifecycle vectors:

| Mutation | Answers that differ |
|---|---|
| No 500 for a non-HTTP throwable | 18 |
| No `BadRequestHttpException` wrapping | 153 |
| Ignoring `$catch` | 149 |
| Catching `\Error`s | 76 |
| Forgetting that a redirect keeps its status | 50 |
| Breaking the terminate-exception identity | 32 |

Clearing the terminating flag during `terminate()`'s own dispatch changes
nothing observable. The flag is read only by exception events dispatched
during that call, and the kernel dispatches none itself.
`testTerminateWithExceptionThrownByTerminateListener` covers the flag's
use.

## Advisories

None concerns these files. GHSA-6439-2f28-8p8q, the attribute listeners,
stays `not-ported`.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner).
