# M2-A HttpKernel stage 3: the ArgumentResolver

November 25. Branch `m2/http-kernel-argument-resolver`. Crate touched:
`symfony-http-kernel`.

Built on HttpKernel stage 1. The kernel was given an argument resolver
until now; this stage ports Symfony's own: it turns request attributes
into a controller's arguments.

## Scope

Upstream symfony/http-kernel at the pin (v8.1.11).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Controller/ArgumentResolver.php`, `Controller/ValueResolverInterface.php`, `Attribute/ValueResolver.php` | `crates/http-kernel/src/argument_resolver.rs` | ArgumentResolverTest 27 of 33 |
| `ControllerMetadata/ArgumentMetadata.php`, `ArgumentMetadataFactory.php`, `ArgumentMetadataFactoryInterface.php` | `crates/http-kernel/src/controller_metadata.rs` | ArgumentMetadataTest 5 of 5, ArgumentMetadataFactoryTest 11 of 12 |
| `Controller/ArgumentResolver/RequestAttributeValueResolver.php`, `RequestValueResolver.php`, `SessionValueResolver.php`, `DefaultValueResolver.php`, `VariadicValueResolver.php` | `crates/http-kernel/src/value_resolver.rs` | RequestAttributeValueResolverTest 10 of 10, RequestValueResolverTest 5 of 6 |
| `Exception/NearMissValueResolverException.php`, `ResolverNotFoundException.php` | `crates/http-kernel/src/exception.rs` | through ArgumentResolverTest |

`n/a`:
- `testGetArgumentsInjectsExtendingRequest` and
  `testResolveSupportsExtendingRequest`: a PHP subclass of `Request`. The
  port has none.
- `testMultipleAttributes`: reflection drops an attribute whose class does
  not exist. A Rust controller declares only the attributes it has.

Pending: the five ArgumentResolverTest session tests, and
SessionValueResolverTest. Sessions are not ported.

The other value resolvers (BackedEnum, DateTime, QueryParameter,
RequestHeader, RequestPayload, Service, Uid, UploadedFile,
NotTaggedController, Traceable) are a later stage.

Prior art: the HttpKernel stack row (no Rust port). The PLAN.md row for
these files records how a controller declares what PHP reflects.

## Port notes

- **Reflection.** PHP's `ArgumentMetadataFactory` reflects the
  controller. Here the controller declares what reflection would report:
  - its name (`Class::method`, a function's name, `{closure}` by default);
  - each `Parameter`: name, type as PHP prints it, nullability (`?T`, a
    union with `null`, or a `null` default), default, variadic, and
    attributes as objects.

  The ported factory turns these into `ArgumentMetadata`. A converter can
  generate the declarations from the PHP signature at build time.
- **Attributes.** `getAttributesOfType()` compares class names. With
  `IS_INSTANCEOF` it also matches any `ValueResolver` attribute object,
  whose class may be a subclass's name (`MapEntity`).
- **The request.** The request passed as an argument is a PHP value that
  holds the request's handle (`RequestObject`), so `assertSame()` holds.
- **Named resolvers.** PHP's container is a `NamedResolvers` list. A
  service locator lists its names in `ResolverNotFoundException`; another
  container does not.
- **Coercion.** `RequestAttributeValueResolver` coerces `int` and `float`
  as PHP's coercive mode passes a value to a typed parameter: numeric
  strings, floats within the int range (truncated), bools. Overflow,
  leading-numeric strings, null, arrays and objects become a 404. PHP's
  deprecation for a fractional float given to `int` is not reproduced.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | ArgumentResolverTest 27, ArgumentMetadataTest 5, ArgumentMetadataFactoryTest 11, RequestAttributeValueResolverTest 10, RequestValueResolverTest 5 | all pass |
| Scripts (`crates/http-kernel/tests/data/argument_resolver.json`, `oracle/recorders/http_kernel_argument_resolver.php`) | 4,000 seeded scripts, described below | all match |

In each script PHP evaluates a closure with a generated signature:
- 0–4 parameters;
- no type, scalar types, `mixed`, `iterable`, unions, nullable types;
- `Request`, a look-alike `\Foo\Request`, `stdClass`, `SessionInterface`;
- defaults, a variadic last parameter, `#[ValueResolver]` attributes that
  pin, disable (by class or by the chain's key) or name an unknown
  resolver.

Each script then sets generated request attributes:
- numeric, padded, overflowing and leading-numeric strings, bool tokens;
- ints, floats (`INF`, `NAN`, beyond the int range), bools, null, arrays,
  `stdClass`.

The resolver is then one of:
- the default resolvers;
- a shuffled custom chain (some entries with string keys) that includes
  resolvers yielding two values and throwing near misses;
- either of these, with or without named resolvers.

Compared:
- the metadata built from the declared parameters, against what PHP's
  reflection reported;
- the arguments (the request by identity), or the exception's class and
  message.

Mutation checks on the vectors:

| Mutation | Scripts that differ |
|---|---|
| Pinning ignored | 172 |
| A variadic gets `null` from DefaultValueResolver | 56 |
| No at-most-one-value check | 56 |
| `string` accepts null | 43 |
| More than one pin allowed | 32 |
| `int` rejects bools | 24 |
| Reasons not numbered | 18 |
| No `null` for a nullable argument | 18 |
| RequestValueResolver does not defer to an attribute | 16 |
| Disabling ignored | 11 |
| `int` takes floats beyond its range | 8 |
| `float` takes leading-numeric strings | 2 |
| Disabling by the chain's key ignored | 1 |

Coercing union types too changes nothing: no union is one of the four
coerced types, so the result is the same.

## Advisories

None is listed for these files.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner).
