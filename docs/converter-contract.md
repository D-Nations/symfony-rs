# Converter contract (provisional, version 0.1)

What code generated from a Symfony application may assume about this
runtime, and what the runtime may assume about that code. It is the
interface between the symfony2rust converter (a php2rust workstream) and
the crates in this workspace.

**Status: provisional.** Version 0.1 is written before any converter output
exists (PLAN.md, M1-D). The converter fixture pins the version it was built
against. Each change to this document bumps the version and adds a row to
the change log at the end. Nothing here is stable before M8.

The fixture that exercises this contract is a hand-authored,
generated-code-shaped Rust crate until symfony2rust emits it. Hand-authored
output proves the runtime interface only; the conversion gate stays open
until the converter's real output builds and passes without manual edits.

## Prior art

No tool converts a Symfony application into Rust. The projects below do
related things, and each one settles one of the questions this contract
has to answer.

| Project | What it does | Advantages | Disadvantages for us |
|---|---|---|---|
| [PeachPie](https://github.com/peachpiecompiler/peachpie) | Compiles PHP to .NET assemblies. Generated code takes a per-request [`Context`](https://docs.peachpie.io/api/ref/context/) and passes the dynamic [`PhpValue`](https://docs.peachpie.io/api/ref/phpvalue/) almost everywhere | Runs unmodified applications (WordPress, Laravel); one uniform calling convention | Dynamic values everywhere cost speed and hide types from review; the runtime reimplements the PHP engine, which decision 8 rules out here |
| [KPHP](https://vkcom.github.io/kphp/kphp-internals/kphp-architecture/internals-runtime.html) | Compiles PHP to C++ with type inference. Classes become structs behind [`class_instance<T>`](https://vkcom.github.io/kphp/kphp-language/static-type-system/instances.html) (reference semantics); `mixed` is a tagged union | Typed code is fast; dynamic values only where code is dynamic | Refuses code it cannot type ([instances do not mix with `mixed`](https://vkcom.github.io/kphp/kphp-language/static-type-system/instances.html)); no framework support |
| Symfony's own `PhpDumper` | Compiles the DI container into a PHP class with one factory method per service, at cache warm-up | The behaviour to reproduce: the service graph is fixed before the first request, and errors in it are reported at build time | Under PHP-FPM the container is rebuilt per request, so a Rust container shared between requests must not leak mutated state |
| [Quarkus ArC](https://quarkus.io/guides/cdi-reference) | Resolves CDI injection at build time and generates the wiring code, in place of reflection at start-up | Build-time wiring, as AGENTS.md requires; unsatisfied dependencies fail the build | Java scopes (`@RequestScoped` proxies) have no direct Rust form |
| [shaku](https://docs.rs/crate/shaku), [fundle](https://docs.rs/crate/fundle/0.3.5/source/README.md), [injectable-rs](https://docs.rs/injectable-rs) | Rust DI libraries. shaku checks wiring at compile time but resolves trait objects from a container at run time; fundle and injectable-rs make missing dependencies compile errors | Compile-time checking without hand-written wiring | A generic container would duplicate what the converter already knows from Symfony's compiled container; each one is a new dependency |

**Choice.** Code is typed wherever the PHP is typed, following KPHP.
`php_value::Value` appears only where PHP is genuinely dynamic: `mixed`,
untyped arrays and values crossing into a component that takes PHP
values. PeachPie's dynamic-everywhere convention is rejected for speed
and for reviewability. The container is Symfony's compiled container,
written as Rust by the converter, so it follows `PhpDumper` and ArC. No DI
crate is adopted, because the converter already has the full service
graph. Per-request isolation follows PHP-FPM, not a long-running worker.

## 1. Crate layout

- The converter emits one application crate. It depends only on crates
  from this workspace and on the stack PLAN.md approves (Axum, Tokio,
  sqlx). It never depends on converter crates (PLAN.md decision 3).
- Configuration, routing attributes, service definitions and templates
  are read by the converter or by `build.rs`. Nothing is parsed per
  request beyond what the request carries (AGENTS.md).
- Unsupported PHP is refused at conversion time, with its file and line.
  Generated code never contains a stub that approximates PHP behaviour.

## 2. Values

- A typed PHP parameter, property or return value becomes the matching
  Rust type: `int` → `i64`, `float` → `f64`, `bool` → `bool`, `string` →
  `php_value::PhpString` (PHP strings are bytes), `?T` → `Option<T>`, a
  class → that class's Rust type (section 3), `array` →
  `php_value::PhpArray`. Arrays with a shape the converter can prove may
  become Rust structs; this is an optimisation, not a requirement.
- `mixed`, untyped values and union types the converter cannot map
  become `php_value::Value`.
- Scalar coercion at a typed boundary follows the calling file's
  `declare(strict_types=1)` setting, which the converter knows statically.
  The coercion and its `TypeError` message come from `php-value`, written
  from recorded PHP behaviour. The converter never writes its own.
- Conversions between Rust types and `Value` go through `From` / `TryFrom`
  implementations in `php-value`. A failed conversion is an error value
  (section 5), never a panic.

## 3. Objects

PHP objects are handles: assigning one aliases it, and `===` compares
identity.

- A class whose instances are never written after construction (readonly
  classes, value objects, and classes with no property writes outside the
  constructor) becomes `Arc<T>`.
- Any other class becomes a shared mutable handle, `Arc<Mutex<T>>`
  (standard library), wrapped in a generated newtype. Generated code never
  holds a lock across `.await`, and the application crate turns on
  `clippy::await_holding_lock`.
- A class whose objects cross into a component taking PHP values
  implements `php_value::PhpObject`.
- Identity (`===`, `spl_object_id`) is `Arc::ptr_eq` / the pointer's
  address. No global object registry exists.

This section will change once the fixture reaches DI and Doctrine (entity
identity maps).

## 4. Services and request lifetime

- The converter compiles Symfony's container (after its compiler passes,
  as `debug:container` reports it) into a generated `Container` struct
  with one field or factory per service. The graph is fixed when the
  application crate is built.
- **Application-scoped services** are built once at start-up and shared
  between concurrent requests as `Arc<T>`, where `T: Send + Sync`. A
  service is application-scoped only if the converter proves that nothing
  writes its state after construction.
- **Request-scoped services** are every other shared service. Each request
  builds its own instances, so one request's writes are never visible to
  another, as under PHP-FPM. They need `Send`, not `Sync`.
- Non-shared services (`shared: false`) are constructed at each injection
  point, as in Symfony.
- A service that cannot be classified is refused at conversion time.

## 5. Errors

- A PHP function that can throw returns `Result<T, E>`, where `E` is a
  generated error type covering the exception classes that function can
  throw. Each exception carries its class name, message, code and
  previous exception, so `getMessage()` and the error pages read the same.
- `HttpExceptionInterface` exceptions convert into
  `symfony_http_kernel::Exception` with their status and headers. Any
  other uncaught exception reaches the kernel as a 500. The kernel's
  exception listeners decide what the response says (the kernel never
  writes `detail` to a response on its own).
- Generated code does not panic. No `unwrap` or `expect` is used on
  request, database, file or network data (AGENTS.md).

## 6. Requests, controllers and responses

- The HTTP boundary is Axum's `Request` and `Response`, run through
  `symfony_http_kernel::Kernel::handle`: request listeners, controller
  resolution, controller listeners, invocation, then response or
  exception listeners.
- Routes come from the route table Symfony compiles (`debug:router`,
  attributes and loaders), matched by `symfony-routing`.
- A controller action becomes `async fn(&Container, RequestScope, Request,
  args) -> Result<Response, E>`, with arguments resolved by the ported
  argument resolvers. The returned future must be `Send`, because Axum
  may move it between worker threads.
- Request data is read through `symfony-http-foundation`'s bags, which
  keep PHP's parsing of query strings, bodies and cookies. The fixture
  links those crates only after their provenance rewrite (PLAN.md,
  request implementation provenance gate).

## 7. Concurrency summary

| Item | Bound |
|---|---|
| `Container` and application-scoped services | `Send + Sync`, immutable after start-up |
| Request scope and its services | `Send` |
| Controller and listener futures | `Send` |
| Kernel listeners | `Send + Sync`, registered at start-up |
| `php_value::Value`, `PhpArray`, `Object` | `Send + Sync` (already true) |

## Change log

| Version | Change |
|---|---|
| 0.1 | First provisional contract (M1-D), before any converter output |
