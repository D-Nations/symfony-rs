# Symfony DependencyInjection

Private workspace crate: a port of `symfony/dependency-injection` v8.1.10
(`9b34cad8`), as installed by symfony/demo, for the build-time container
compiler of PLAN.md ("M3: DependencyInjection"). MIT; Symfony source and
tests copyright Fabien Potencier. Unofficial, not affiliated with or
endorsed by Symfony.

The crate grows from the November 24 trial (`spikes/di-trial`), which
stays as it is until this crate's compiler and Rust emitter replace it.

## Value model

- Values are php-value `Value`s, and arrays are `PhpArray`s with PHP's
  keys (a removed method call leaves a gap, as `unset()` does).
- `Definition` (with `ChildDefinition`), `Alias` and the argument objects
  are shared PHP objects (`object::Shared`): a cloned handle is the same
  object (`===`, `ptr_eq`), `clone_object()` is PHP's `clone`, and the
  handle is an object `Value`, so an inline definition can sit in an
  argument array. `Reference` (with `TypedReference`), `Parameter`,
  `Variable` and `EnvClosureArgument` are immutable objects.
- `ChildDefinition` and `TypedReference`/`TaggedIteratorArgument` are the
  base type with extra data; their class names and `instanceof` answers
  are upstream's.
- Identifiers (service ids, class names, parameter names, tag and method
  names) are UTF-8 `String`s.
- One `Error` type; its `ErrorKind` gives the exception class, and
  `instance_of()` answers for the component's classes, their SPL parents,
  `ExceptionInterface` and PSR-11's interfaces.

## Contract

| Contract | Source | Evidence |
|---|---|---|
| Exceptions: classes, hierarchy, messages (`ServiceNotFoundException` alternatives, `ParameterNotFoundException`'s rebuilt message and setters, circular-reference paths, `EnvParameterException`, `InvalidParameterTypeException` from a described parameter, `AutowiringFailedException`'s lazy message) | `Exception/*.php` | unit tests; the tests below |
| `ContainerInterface` invalid-reference constants | `ContainerInterface.php` (constants only) | DefinitionTest |
| `Reference`, `TypedReference` (name kept only when id = type), `Parameter`, `Variable` | `Reference.php`, `TypedReference.php`, `Parameter.php`, `Variable.php` | ReferenceTest (1), ParameterTest (1) |
| `Alias`: public, deprecation template checks and message | `Alias.php` | AliasTest (7) |
| `Definition`: every setter and getter, change tracking order, factory and configurator normalization, decorated service, argument bounds messages, method calls with PHP keys, tags, resource tags, bindings (key normalization and `BoundArgument` wrapping), errors (lazy ones rendered once), deprecation | `Definition.php` | DefinitionTest (33) |
| `ChildDefinition`: parent, `index_N` arguments, `$name` arguments | `ChildDefinition.php` | ChildDefinitionTest (11) |
| Arguments: `AbstractArgument`, `BoundArgument` (process-wide usage sequence), `EnvClosureArgument`, `IteratorArgument`, `TaggedIteratorArgument` (both constructor shapes, default method names, deprecations), `ServiceClosureArgument`, `ServiceLocatorArgument`; `ArgumentInterface` values | `Argument/*.php` (not the run-time classes) | TaggedIteratorArgumentTest (6) |
| `ParameterBag` and `FrozenParameterBag`: add/set (numeric names refused), get with alternatives and the nested-array hint, deprecations, non-empty parameters, resolve, `resolveValue()` (keys too), `resolveString()` (`%%`, whole-value placeholders, circular references), escape/unescape | `ParameterBag/ParameterBag.php`, `FrozenParameterBag.php`, `ParameterBagInterface.php` | ParameterBagTest (24), FrozenParameterBagTest (7) |

Not yet ported: `EnvPlaceholderParameterBag`, `ContainerBag`, `Container`,
`ServiceLocator`, `ContainerBuilder`, the compiler passes, the loaders,
the dumpers (the Rust emitter replaces `PhpDumper`), `EnvVarProcessor`,
the attributes and the kernel classes.
