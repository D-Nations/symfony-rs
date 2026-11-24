# M2-A routing stage 4: CompiledUrlGenerator and its dumper

November 24. Branch `m2/routing-compiled-url-generator`. Crates touched:
`symfony-routing`.

Built on stage 3 (UrlGenerator, PR #40). It does not touch `Matcher/`.

## Scope

Upstream symfony/routing at the pin (v8.1.11,
`f3fefc6cad3748d538be970ca81b71f06d7aa8b3`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Generator/CompiledUrlGenerator.php` | `crates/routing/src/compiled_url_generator.rs` | CompiledUrlGeneratorDumperTest (below) |
| `Generator/Dumper/CompiledUrlGeneratorDumper.php`, `Generator/Dumper/GeneratorDumper.php` | `crates/routing/src/compiled_url_generator_dumper.rs` | CompiledUrlGeneratorDumperTest 21 of 21 |

`Generator/Dumper/GeneratorDumperInterface.php` (only `dump()` and
`getRoutes()`) is not ported as a trait.

## Decision (owner, November 24)

Upstream's `dump()` writes the compiled routes as PHP code, which
CompiledUrlGenerator gets back with `require`. The owner chose "data now,
Rust later": `getCompiledRoutes()` and `getCompiledAliases()` are ported
as data (`CompiledRoutes`), `CompiledUrlGenerator` takes that data, and
`dump()` stays unported until the route loaders arrive and it can become a
`build.rs` emitter of Rust (PLAN.md's build-time layer). `compiled()` is
what the dumped file returns, so the upstream tests use it where they
`require` the file. This avoids `CompiledUrlMatcherDumper::export()` (in
`Matcher/`, the matcher session's area). Values `export()` refuses
(objects without a PHP export form) are kept as they are.

## Port notes

- `CompiledUrlGenerator` holds a `UrlGenerator` over an empty collection
  for the parent's state (context, logger, strictness) and `doGenerate()`,
  as the PHP subclass does. The locale lookup (`?? ?:` and the `strstr()`
  loop) and the `_canonical_route` / `_locale` step are shared helpers.
- The dumper compiles a route on a copy unless it was compiled in the
  collection.
- Alias cycle detection uses `array_search()`, a loose comparison.
- The compiled routes keep PHP's key order: assigning an existing name
  keeps its place.

## Behaviour found by recording

- `RouteCollection::get()` (ported in stage 1) compared visited alias
  names by bytes, while upstream's `array_search()` compares loosely:
  `010 -> 10` is a circular reference in PHP, since `"010" == "10"`.
  Fixed in `route_collection.rs`; the plain-generator replay found it.
- The compiled and plain generators give different answers for 82 of the
  3,690 recorded calls: 42 by outcome (mostly RouteNotFoundException from
  the compiled one where the plain one, which keeps the last localized
  route it looked up even without a matching `_canonical_route`, goes on),
  40 by URL, message or deprecations. Both sides are recorded and both
  match.
- The first alias's deprecation is collected, not triggered, at dump time;
  the deprecations of the aliases it leads through are triggered then too.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | CompiledUrlGeneratorDumperTest 21 of 21 | all pass |
| Dump (`crates/routing/tests/data/compiled_url_generator.json`, `oracle/recorders/routing_compiled_url_generator.php`) | 1,500 generated collections (routes with inline settings, localized names; aliases with chains, cycles, missing targets, deprecations, names equal under `==`; name prefixes through `addCollection()`): 1,230 arrays as `require` returns them, the rest exceptions; deprecations triggered | all match |
| `generate()` (same file) | 3,690 calls, each through CompiledUrlGenerator on the dump and UrlGenerator on the collection: URL or exception, deprecations | all match |

Mutation checks: dropping a localized alias's `_canonical_route` fails 8
recorded cases; not triggering chained deprecations fails 46; comparing
visited alias names strictly fails 2 dumps; the `RouteCollection::get()`
byte comparison failed 1 call before the fix.

## Advisories

No advisory in the upstream watch names these files.

## Checks

`cargo fmt --check` ok, `cargo clippy --all-targets -- -D warnings`
clean, `scripts/sources.sh --check` ok, `scripts/parity-check.sh` ok
(routing 204 ported, 1 n/a, 691 pending). `cargo test` on the Windows
host: 2,711 passed, 1 failed, the known Windows-only `symfony-filesystem`
`testReadNonExistentFile` (OS error text), which this branch does not
touch. `scripts/linux-test.sh -p symfony-routing` on the committed branch
(fmt, clippy, tests, as a non-root user): 217 passed, 0 failed.

## Not done

- `dump()` and `GeneratorDumperInterface` (owner's decision above).
- `RouteCollection::add()` scanned its lists for the name, so adding n
  routes was quadratic: `testDumpWithTooManyRoutes` (32,769 routes) spent
  8.1 s adding and 0.6 s compiling in a debug build. Fixed afterwards
  (branch `m2/routing-collection-index`): the collection keeps its arrays
  as PHP does, with a hash index (`ordered.rs`), and adding them takes
  0.15 s.

## Demo comparison

Not run: paused by the owner (November 23). The demo does not use these
classes.
