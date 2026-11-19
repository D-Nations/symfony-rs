# Symfony HttpKernel lifecycle subset

Private workspace crate; MIT, with Symfony's source copyright retained in
the implementation. This is an unofficial partial port, not a complete
HttpKernel, bundle system or dependency-injection container.

The first concrete add-on is the demo's existing security-header policy.
It now implements `Listener<C>::response`, keeping startup-built CSP hashes,
the feed's separate policy and the rule that existing headers win. It runs
around Axum's `Next` target, covering static files as well as dynamic pages.
The demo's controller errors, firewall and session finalization retain their
existing application-specific handling inside that target.

The independent request lab supplies `Match<Route>` as its controller target
and converts an explicit controller failure to JSON in an exception listener.
It has no demo dependency, database, templates, translations or assets.

```rust
use axum::{body::Body, extract::Request, response::IntoResponse};
use symfony_http_kernel::{Kernel, Listener, ResponseEvent, Exception};

struct Headers;
impl<C> Listener<C> for Headers {
    fn response(&self, event: &mut ResponseEvent<'_>) -> Result<(), Exception> {
        event.response.headers_mut().entry("x-content-type-options")
            .or_insert(axum::http::HeaderValue::from_static("nosniff"));
        Ok(())
    }
}

# async fn example() -> Result<(), Exception> {
let mut kernel = Kernel::default();
kernel.add_listener(0, Headers);
let response = kernel.handle(
    Request::new(Body::empty()),
    |_| Ok(()),
    |_, ()| async { Ok("hello".into_response()) },
).await?;
assert_eq!(response.headers()["x-content-type-options"], "nosniff");
# Ok(())
# }
```

| Contract | Source | Evidence |
|---|---|---|
| Request event before resolution; early response skips resolution and invocation but runs response hooks | `HttpKernel::handleRaw`, `RequestEvent::setResponse` | Selected upstream early-response test and focused lifecycle test |
| Mutable typed controller target after resolution and before asynchronous invocation | `ControllerEvent::setController` | Replacement and body-preservation test |
| Response modification or replacement, including early and handled-error responses | `HttpKernel::filterResponse`, `ResponseEvent` | Selected upstream response-listener test and focused lifecycle test |
| Explicit failures from request, resolver, controller hooks, invocation or response filtering enter exception hooks | `HttpKernel::handle`, `handleThrowable` | Stage-by-stage failure test |
| Exception replacement; setting a response stops that event | `ExceptionEvent`, `RequestEvent` | Replacement/propagation test |
| A successful exception response inherits the failure's status and headers unless custom codes are allowed; redirects and existing error responses keep their status | `HttpKernel::handleThrowable` | Upstream handling, redirection, HTTP exception and custom-status tests |
| Failure in exception listeners propagates; failure filtering an already handled error does not recursively dispatch exceptions | `HttpKernel::handleThrowable` | Stage failure test |
| Higher priorities first; ties retain registration order; stop affects only its event | `EventDispatcher::sortListeners`, `callListeners` | Priority/order and per-event propagation tests |

The source is the installed Symfony HttpKernel commit
`924974dbdcdf4cc40d7bb2c65ae83bb6dc67cff1`. Ten supported methods from its
[`Tests/HttpKernelTest.php`](https://github.com/symfony/http-kernel/blob/924974dbdcdf4cc40d7bb2c65ae83bb6dc67cff1/Tests/HttpKernelTest.php)
retain their names and relevant assertions in `tests/lifecycle.rs`.
Six additional tests cover the Rust integration boundaries, including
HttpFoundation's surprising `isRedirect()` classification: 201 is retained,
whereas 300 and 304 are not. This follows `Response.php`, not a generic
300-399 status test.

Listeners are synchronous, `Send + Sync`, registered and sorted at startup.
Each registration has one priority; register separate listeners when different
hooks need different priorities. There is no listener mutation or sorting
during a request. Propagation state is local to each event. Controllers own
the request and may consume its streaming body; the kernel never reads it.
Response and exception listeners receive a metadata snapshot from before
invocation, including changes made by earlier hooks. Mutations inside the
controller are not reflected back into that snapshot.

Only explicit `Exception` results are handled. Panics are not caught, internal
error details are never rendered automatically, and unhandled failures return
to the application. The demo logs them and uses a generic 500; the lab renders
only its intentionally public fixed example error. Hook code must keep
blocking I/O out of the request thread.

Not implemented: subrequests, a request stack, argument reflection/resolution,
`kernel.view`, finish/terminate events, PHP output buffering, streamed-response
stack restoration, a `catch=false` switch, async listeners, service discovery,
YAML configuration or bundle loading. Axum remains responsible for transport
and HTTP response preparation; the lab retains its explicit HEAD-body rule.

No new registry dependency. Run `cargo test -p symfony-http-kernel` or the
workspace's ordinary formatting, Clippy and test commands.
