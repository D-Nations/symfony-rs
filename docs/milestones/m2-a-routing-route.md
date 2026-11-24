# M2-A routing stage 1: Route, RouteCompiler, RouteCollection

November 24. Branch `m2/routing-route`. Crates touched: `symfony-routing`,
and one additive method in `php-value`.

## Scope

Upstream symfony/routing at the pin (v8.1.11,
`f3fefc6cad3748d538be970ca81b71f06d7aa8b3`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Route.php` | `crates/routing/src/route.rs` | RouteTest 24 of 24 |
| `CompiledRoute.php` | `crates/routing/src/compiled_route.rs` | CompiledRouteTest 2 of 2 |
| `RouteCompiler.php`, `RouteCompilerInterface.php` | `crates/routing/src/route_compiler.rs` | RouteCompilerTest 10 of 10 |
| `RouteCollection.php` | `crates/routing/src/route_collection.rs` | RouteCollectionTest 24 of 24 |
| `Alias.php` | `crates/routing/src/alias.rs` | (no upstream test class; recorded) |
| `Exception/ExceptionInterface.php`, `InvalidArgumentException.php`, `RuntimeException.php`, `RouteCircularReferenceException.php` | `crates/routing/src/exception.rs` | through the above |

The crate's existing typed matcher and generator (the demo's subset) are
unchanged and do not use these classes yet; they come with the matcher
and generator stages.

Prior art (GitHub and crates.io, November 24): no Rust port of Symfony
Routing exists. Router crates (matchit, path-tree, route-recognizer, and
the routers of axum and actix) have their own pattern languages, without
inline requirements and defaults, optional trailing variables,
separators, host patterns or PCRE requirements. Recorded in PLAN.md's M2
stack table. No new dependency: php-pcre, php-value, symfony-config
(optional, `config` feature) and symfony-deprecation-contracts are
workspace crates.

## Shape of the port

- A Route is a value. A RouteCollection owns its routes: `add()` and
  `add_collection()` take them, `get_mut()` changes one, `clone()` is
  `__clone()`'s deep copy. PHP shares Route objects between the holders of
  a handle; no ported caller depends on that sharing after a route is
  added.
- Defaults, requirements, options, schemes, methods, tokens and
  variables are PHP arrays (`php_value::PhpArray`), so their keys follow
  PHP's rules and `__serialize()` writes them as PHP does.
  CompiledRoute's tokens stay arrays because the class accepts any array
  (its test passes `['tokens']`).
- `compiler_class` is resolved when the program is built:
  `Route::compile()` knows RouteCompiler, and `Route::compile_with::<C>()`
  names another `RouteCompilerInterface`. Any other class is PHP's
  `Class "%s" not found`. A subclass that only changes `SEPARATORS` is
  `RouteCompiler::compile_with_separators()`.
- The inline-settings pattern and the compiler's helper patterns are
  upstream's, run through php-pcre. The compiler writes the route's own
  regular expressions as strings and never runs them.
- `Route::unserialize()` takes the `unserialize()` allow-list
  (`Route::classes()` plus anything the data holds) and the CompiledRoute
  subclasses to accept.
- Resources need the optional `config` feature (symfony/config is in
  upstream's require-dev only), so the demo does not build symfony-config.

## Behaviour found by recording

- `findNextSeparator()` with a UTF-8 pattern whose next static character
  is a newline returns null from a function declared `: string`. PHP throws
  `TypeError: Symfony\Component\Routing\RouteCompiler::findNextSeparator():
  Return value must be of type string, null returned` (6 of the 2,000
  generated routes). The port throws the same.
- A preceding text made only of newlines makes `preg_match('/.$/u')` fail.
  The separator is then null, which `str_contains()` reads as `''`, so the
  compiler emits an empty text token and a null separator. Kept.
- `__unserialize()` of a value of the wrong type into a typed property is
  PHP's `Cannot assign %s to property %s::$%s of type %s`, where a bool is
  named `true` or `false` (not `bool`). Scalars assigned to string
  properties are converted, and a missing key is null.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | 60 of the 60 methods of the four test classes | all pass |
| Routes (`crates/routing/tests/data/route.json`, `oracle/recorders/routing_route.php`) | 2,000 generated paths and hosts (inline requirements, defaults, mappings and `!`, duplicate, digit-first and over-long names, `_fragment`, `_locale` with `_canonical_route`, UTF-8 and non-UTF-8 bytes, newlines), with defaults, requirements, `utf8` options, schemes and methods: construction (375 throw), `compile()` (894 compile, 731 throw), the route after compiling, `serialize()` bytes | all match |
| Requirements | 1,000 generated through `setRequirement()` | all match |
| Collections | 300 generated scripts (1,373 steps) of add with priorities, aliases, alias deprecations, remove, `addNamePrefix()`, `addPrefix()`, `addCollection()`, each followed by `all()`, the aliases and `get()` of 7 names with their deprecations | all match |
| `unserialize()` | 131 Route and CompiledRoute payloads with another type (or none) in each property | all match |

Each replay was checked by breaking a rule on purpose. Writing the regex
flags as `Ds` instead of `sD` failed the routes replay, and naming a bool
`TRUE` failed the unserialize replay.

## Checks

`cargo fmt --check` ok, `cargo clippy --all-targets -- -D warnings` clean,
`scripts/sources.sh --check` ok, `scripts/parity-check.sh` ok (routing 78
ported, 818 pending).

`cargo test` on the Windows host: 2,434 passed, 1 failed,
`symfony-filesystem`'s `testReadNonExistentFile`. That test expects
Linux's "No such file or directory" and Windows says "The system cannot
find the file specified.". It is unchanged by this branch and passes in
`scripts/linux-test.sh -p symfony-filesystem`. `scripts/linux-test.sh -p
symfony-routing -p php-value` on the committed branch: see the PR.

## Advisories

The upstream watch lists two symfony/routing advisories, both triaged
before this stage. GHSA-h5x3-xfc9-m39h (UrlGenerator dot segments) is
`covered` and unchanged. GHSA-72xp-p242-47p9 (UrlGenerator requirement
check with unanchored alternation) stays `not-ported`, with a new note:
Route now stores requirement regexes, but the generator's check is not
ported. The generator stage must pin the fix. The unserialize hardening
tests (`testUnserializeRejectsObjectInTypedScalarProperty` in RouteTest
and CompiledRouteTest) are ported.

## Not reproduced (refused, not approximated)

- The `", called in %s on line %d"` suffix of PHP's TypeError for a
  requirement that is not a string (it names the calling PHP file).
- A `_route_mapping` default that is not an array, when a pattern adds a
  mapping, and a `_canonical_route` default that `.` cannot concatenate
  (an array or a non-Stringable object) in `addNamePrefix()`: both
  `Class::NotPorted`.
- A php-pcre `MatchError` (backtracking limit) in the helper patterns,
  where PHP's `preg_*` returns null: `Class::NotPorted`.

## Not done in this stage

The matchers, generator, dumpers, loaders, Router, RequestContext and
attribute classes (818 pending tests). The next routing stage is
`RequestContext.php` and `Matcher/UrlMatcher.php` on these Route objects.

## Demo comparison

Not run: the owner paused the symfony-demo-rs comparison step on
November 23 (AGENTS.md, "Checks"). The demo uses only the crate's
existing typed matcher and generator, which this branch does not change.
