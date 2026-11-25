# M2-A HttpKernel stage 4: the ControllerResolver

November 25. Branch `m2/http-kernel-controller-resolver`. Crates touched:
`symfony-http-kernel`, `php-value` (`get_debug_type()` of anonymous
classes, `value_name()`).

Built on HttpKernel stages 1 and 3. The kernel was given a controller
resolver until now; this stage ports Symfony's own. It turns the
`_controller` request attribute into a callable.

## Scope

Upstream symfony/http-kernel at the pin (v8.1.11).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Controller/ControllerResolver.php`, `Attribute/AsController.php` | `crates/http-kernel/src/controller_resolver.rs` | ControllerResolverTest 15 of 15 |

Pending: ContainerControllerResolverTest (21 tests). It needs a service
container, which is not ported.

Prior art: the HttpKernel stack row (no Rust port). The PLAN.md row for
these files records how the PHP program is declared.

## Port notes

- **Declarations, not reflection.** PHP instantiates classes and asks
  `is_callable()`, `method_exists()`, `get_class_methods()` and
  reflection. Here the PHP program is declared in a `ClassRegistry`:
  - classes, abstract classes, interfaces and traits, with parent,
    interfaces and attribute names;
  - methods, with visibility, staticness and a `Controller` body (none
    when abstract). The constructor is the method `__construct`: its
    body's parameters give its arity, and a failing body is a
    constructor that throws;
  - functions;
  - closures, as `ClosureObject`s holding what `\ReflectionFunction`
    reports (name, called class, anonymous, attributes).

  Instances are `Instance` objects. `resolve()` returns PHP's callable
  (the same value, or the array with the new object) and the kernel's
  `Controller`, named as `ArgumentMetadataFactory` names it.
- **PHP's lookup rules**, as recorded:
  - class and function names are ASCII case-insensitive, and one
    leading backslash is ignored (two are not);
  - the method table lists the class's own methods, then what its parent
    adds, then its interfaces;
  - `method_exists()` on an object sees a parent's private methods; on a
    class name it does not;
  - from outside a class, a public method is callable. A public
    non-static method is not callable statically, even with
    `__callStatic`. An inaccessible or missing method goes to `__call`
    (on an object) or `__callStatic` (on a class name), and a trampoline
    closure takes the magic method's attributes.
- **Surprises kept:**
  - a private static method reached by `Class::method` fails
    `createController()`'s `callable` return type (a `TypeError`), not
    the "should be public" message;
  - a static `Class::method` string under `_check_controller_is_allowed`
    is a `ReflectionException` (`new \ReflectionFunction('C::m')`);
  - an `InvalidArgumentException` thrown by a constructor is wrapped like
    createController's own;
  - `method_exists()` refuses ints, floats, bools, null and arrays as its
    first argument, with PHP's `TypeError`.
- **Anonymous classes.** A class is named as PHP names it (`Parent@anonymous`,
  NUL, file, line, number). `get_debug_type()` (php-value) cuts it at
  `@anonymous`, and the message rewrite of `checkController()` is the
  upstream pattern, matched by hand.
- **Not kept identical** (PLAN.md decision 19): the `ArgumentCountError`
  for constructor arguments names `with_source_file()`'s path. The built-in
  `Closure` methods resolve but are not run.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | ControllerResolverTest 15 | all pass |
| Scripts (`crates/http-kernel/tests/data/controller_resolver.json`, `oracle/recorders/http_kernel_controller_resolver.php`) | 4,000 seeded programs, described below | all match |

In each script PHP declares a generated program in its own namespace. The
program is written to a file and required, so anonymous class names are
PHP's own. It has:
- 1–4 classes, abstract classes, interfaces and traits, with parents and
  interfaces;
- constructors that are public, protected or private, need 0–2 arguments
  (optional and variadic ones too), or throw one of six classes;
- methods of every visibility, static or abstract, with `__invoke`,
  `__call` and `__callStatic`;
- attributes (`AsController`, a lower-case look-alike, others) on
  classes, methods, functions and closures;
- functions, an anonymous class, and closures: anonymous, from functions,
  from methods (trampolines too).

Each script then sets:
- a `_controller`: `Class::method` and class names in any case, with one
  or two leading or a trailing backslash; function names; arrays of every
  shape (`[class, method]`, `[object, method]`, string keys, three
  elements, reversed keys, null, int and array methods, scalar classes);
  objects; closures; falsy values or none; ints, floats, bools;
- maybe `_check_controller_is_allowed` (true, false, `1`, `'yes'`, `'0'`,
  null) and `allowControllers()` calls (types in any case, `Closure`,
  unknown names; attributes).

Compared:
- each closure made of the program's callables: what reflection reports
  and what calling it returns;
- `false` and the logger's warning;
- or the callable returned (given objects and closures by identity), what
  calling it returns and its `ArgumentMetadataFactory` name;
- or the exception: class, message, code and previous.

Mutation checks on the vectors:

| Mutation | Scripts that differ |
|---|---|
| Magic methods listed | 303 |
| `__call` ignored | 297 |
| Class names looked up case-sensitively | 218 |
| createController's InvalidArgumentException not wrapped | 189 |
| `_check_controller_is_allowed` must be `true` | 114 |
| Leading backslash not ignored | 108 |
| `__callStatic` ignored | 50 |
| Allowed types ignored | 41 |
| Attributes compared case-insensitively | 37 |
| Inherited methods listed before the class's own | 29 |
| Array: no callable fallback after a failed `new` | 29 |
| Constructor arity always "exactly" | 16 |
| Anonymous class names not rewritten | 11 |
| No static fallback after a failed `new` | 10 |
| Levenshtein bound strict | 8 |
| A parent's private method never found | 7 |
| Alternatives not sorted | 1 |

## Advisories

None is listed for these files.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner).
