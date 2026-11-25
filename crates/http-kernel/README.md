# Symfony HttpKernel

Private workspace crate; MIT, with Symfony's source copyright retained in
the implementation. This is an unofficial partial port, not a complete
HttpKernel, bundle system or dependency-injection container. It has two
parts: the port of `HttpKernel.php` on HttpFoundation's Request and
Response (modules `http_kernel`, `event`, `controller`, `exception`), and
the earlier typed Axum lifecycle subset (`Kernel<C>`, below), which the
converter fixture still uses. request-lab runs on the port, served by
`symfony-runtime`'s HTTP adapter.

## The HttpKernel port

| Module/API | Source behavior | Evidence | Limits |
|---|---|---|---|
| `http_kernel` | Symfony `HttpKernel` (v8.1.11): `handle()` (request stack push/pop, `kernel.request`, controller and argument resolution, `kernel.controller`, `kernel.controller_arguments`, `kernel.view`, `kernel.response`, `kernel.finish_request`, `kernel.exception` with PHP's status and header rules, `\Error`s passed through unless all throwables are handled, request exceptions as `BadRequestHttpException`, `$catch`), `terminate()`, `terminateWithException()`; `HttpKernelInterface`, `TerminableInterface`, `KernelEvents` | HttpKernelTest 31 of 37 (5 `n/a`: PHP callable forms and a trace line; 1 pending: StreamedResponse); 1,500 recorded lifecycles with every listener call (`tests/data/lifecycle.json`) | The kernel is given its argument resolver (`argument_resolver::ArgumentResolver` is the default one); no `StreamedResponse` callback wrapping; the kernel is an `Arc` (events hand it to listeners) |
| `event` | The kernel events: `KernelEvent`, `RequestEvent`, `ControllerEvent` (controller replacement, `_controller_attributes`), `ControllerArgumentsEvent` (`getNamedArguments()`), `ControllerMetadata`, `ControllerArgumentsMetadata`, `ViewEvent`, `ResponseEvent`, `FinishRequestEvent`, `TerminateEvent`, `ExceptionEvent` | HttpKernelTest; the recorded lifecycles | `evaluate()` (ExpressionLanguage) and `ViewEvent`'s deprecated `controllerArgumentsEvent` property are not ported; attributes match by exact class name |
| `controller` | `ControllerResolverInterface`, `ArgumentResolverInterface`; a controller is a closure with what PHP would reflect: its name, its attributes, and each parameter's name, type, nullability, default, variadic and attributes; the request as a controller argument (`RequestObject`) | HttpKernelTest | PHP callable strings and arrays are resolved by `controller_resolver`; a closure is named `{closure}` unless given a name |
| `controller_resolver` | Symfony `ControllerResolver` (v8.1.11) and the `AsController` attribute: `getController()` for every `_controller` shape (`Class::method`, a class or function name, `[class or object, method]` arrays of any shape, objects, closures, scalars coerced to strings; `false` and the logger's warning when it is missing), `createController()` (the static-method fallback and its `callable` return type), `instantiateController()` (`new $class()` and PHP's errors: missing class, abstract class, interface, trait, private or protected constructor, missing constructor arguments, a constructor that throws), `getControllerError()` (alternatives by `levenshtein()` and `asort()`, `get_class_methods()` order and visibility, `method_exists()` and its `TypeError`s), `checkController()` (`_check_controller_is_allowed`, `allowControllers()` types by `is_a()` and attributes by name, the `@anonymous` class-name rewrite, the `BadRequestException`); `is_callable()` as PHP answers it from outside the class (`__invoke`, `__call`, `__callStatic`, inherited private methods, case-insensitive names, one leading backslash) | ControllerResolverTest 15 of 15; 4,000 recorded scripts (`tests/data/controller_resolver.json`) | Nothing is reflected: the PHP program is declared in a `ClassRegistry` (classes with kind, parent, interfaces, attribute names, methods with visibility, staticness and a `Controller` body; functions; closures as `ClosureObject`s with what reflection reports). Objects of undeclared classes have no methods; PHP's built-in functions and classes other than `stdClass` and `Closure` are unknown; `Closure::bind()`, `bindTo()`, `call()` and `fromCallable()` resolve but throw a `LogicException` when called with enough arguments; the `ArgumentCountError` for constructor arguments names `with_source_file()`'s path (default `vendor/symfony/http-kernel/Controller/ControllerResolver.php`); `ContainerControllerResolver` is not ported |
| `controller_metadata` | `ArgumentMetadata` (nullability from the type and a `null` default, `getDefaultValue()`'s `LogicException`, `getAttributes()`, `getAttributesOfType()` with `IS_INSTANCEOF`), `ArgumentMetadataFactory` | ArgumentMetadataTest 5 of 5, ArgumentMetadataFactoryTest 11 of 12 (1 `n/a`: reflection of a missing attribute class) | The factory reads the declared parameters, not reflection; `IS_INSTANCEOF` knows the `ValueResolver` hierarchy only |
| `argument_resolver` | `ArgumentResolver` (each argument offered to the value resolvers in order, near misses as reasons, `#[ValueResolver]` pinning a named resolver or disabling one by class or key, the at-most-one-value rule, upstream's messages), `ValueResolverInterface`, the `ValueResolver` attribute | ArgumentResolverTest 27 of 33 (1 `n/a`: a Request subclass; 5 pending: sessions); 4,000 recorded scripts (`tests/data/argument_resolver.json`) | Named resolvers are a `NamedResolvers` list in place of a service container |
| `value_resolver` | The default value resolvers: `RequestAttributeValueResolver` (route parameters coerced to `string`, `bool`, `int`, `float` as PHP's coercive mode does, else a 404), `RequestValueResolver` (with its near miss), `SessionValueResolver`, `DefaultValueResolver`, `VariadicValueResolver` | RequestAttributeValueResolverTest 10 of 10, RequestValueResolverTest 5 of 6 (1 `n/a`: a Request subclass); the recorded scripts | Sessions are not ported, so `SessionValueResolver` never resolves (SessionValueResolverTest pending); PHP's deprecation for a fractional float given to `int` is not reproduced |
| `router_listener` | Symfony `RouterListener` (v8.1.11): `onKernelRequest()` (the routing context from the request, an `UnexpectedValueException` as `BadRequestHttpException`, routing skipped when `_controller` is set, `matchRequest()` before `match()`, the `Matched route` log, `_route_mapping` with PHP's array and string offset rules, `_route_params`, `NotFoundHttpException` and `MethodNotAllowedHttpException` with upstream's messages), `onKernelFinishRequest()` (the parent's context restored after a sub-request, or recomputed without an event), `onKernelException()` (the welcome page in debug mode when no route is configured), `getSubscribedEvents()`; `UrlMatcherInterface` and `RequestMatcherInterface`, implemented by routing's `UrlMatcher` and `RedirectableUrlMatcher` | RouterListenerTest 10 of 11 (1 pending: ValidateRequestListener and ErrorListener); 2,000 recorded scripts (`tests/data/router_listener.json`) | The context is a shared `Handle<RequestContext>`; routing's matchers keep theirs by value, so the listener is given one; PHP's warnings on odd mappings are not reproduced; the welcome page's project directory uses `/` |
| `response_listener` | Symfony `ResponseListener` (v8.1.11): for the main request only, the kernel's charset when the response has none, `Content-Language` from the request's locale when enabled (not for informational or empty responses, nor over an existing header), `Vary: Accept-Language` added when `_vary_by_language` is truthy, then `Response::prepare()`; `getSubscribedEvents()` | ResponseListenerTest 7 of 7; 1,500 recorded scripts (`tests/data/response_listener.json`); six mutations of the rules, each caught | — |
| `exception` | `HttpException` with `fromStatusCode()`, its 16 subclasses (`Allow`, `Retry-After`, `WWW-Authenticate` headers), `ControllerDoesNotReturnResponseException`, PHP's built-in throwables; `instance_of()` for the kernel's `\Error`, `RequestExceptionInterface` and `HttpExceptionInterface` checks | HttpExceptionTest and its 16 subclass tests, 86 of 86 | Routing's exceptions answer `instance_of()` too. A throwable is an `Arc` (`===` by identity); other Rust errors count as `\Exception`; `ControllerDoesNotReturnResponseException`'s file, line and trace are not set |

Requests, responses and the two controller events are object handles
(php-value `Handle<T>`): the kernel, the request stack and listeners see
the same objects, as in PHP. Listeners are the EventDispatcher's; a
listener downcasts the `&mut dyn EventObject` it is given. The lifecycle
vectors are recorded by `scripts/record.sh http-kernel-lifecycle`
(`oracle/recorders/http_kernel_lifecycle.php`), the RouterListener
vectors by `scripts/record.sh http-kernel-router-listener`, the
ArgumentResolver vectors by `scripts/record.sh http-kernel-argument-resolver`,
the ControllerResolver vectors by `scripts/record.sh
http-kernel-controller-resolver`.

`resources/welcome.html` is upstream's `Resources/welcome.html.php`
rendered in the oracle (`scripts/record.sh http-kernel-welcome`), with
placeholders for the hue and the project directory. Its icons are from
Tabler Icons (MIT, Paweł Kuna); see `resources/LICENSE-tabler-icons`.

## The typed Axum lifecycle subset

The first concrete add-on is the demo's existing security-header policy.
It now implements `Listener<C>::response`, keeping startup-built CSP hashes,
the feed's separate policy and the rule that existing headers win. It runs
around Axum's `Next` target, covering static files as well as dynamic pages.
The demo's controller errors, firewall and session finalization retain their
existing application-specific handling inside that target.

The request lab used this subset until it moved to the HttpKernel port
(M2-A, `docs/milestones/m2-a-axum-adapter-request-lab.md`); the converter
fixture is its remaining user.

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
| A non-HTTP exception (`Exception::from_throwable`) reaches exception listeners as a 500 that they can downcast, as `ExceptionEvent::getThrowable()` allows; `is_http()` is `instanceof HttpExceptionInterface` | `HttpKernel::handleThrowable`, `ExceptionEvent::getThrowable` | Throwable-inspection test; converter fixture's 422 case |
| A successful exception response inherits the failure's status and headers unless custom codes are allowed; redirects and existing error responses keep their status | `HttpKernel::handleThrowable` | Upstream handling, redirection, HTTP exception and custom-status tests |
| Failure in exception listeners propagates; failure filtering an already handled error does not recursively dispatch exceptions | `HttpKernel::handleThrowable` | Stage failure test |
| Higher priorities first; ties retain registration order; stop affects only its event | `EventDispatcher::sortListeners`, `callListeners` | Priority/order and per-event propagation tests |

The source is the pinned (v8.1.11) Symfony HttpKernel commit
`460658504e26a2794aa614b4fbe6cf9bac2076fe`. Ten supported methods from its
[`Tests/HttpKernelTest.php`](https://github.com/symfony/http-kernel/blob/460658504e26a2794aa614b4fbe6cf9bac2076fe/Tests/HttpKernelTest.php)
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

The port depends on the workspace's php-value, symfony-http-foundation,
symfony-event-dispatcher and symfony-deprecation-contracts; base64 and
serde_json are test dependencies. No new registry dependency. Run
`cargo test -p symfony-http-kernel` or the workspace's ordinary
formatting, Clippy and test commands.
