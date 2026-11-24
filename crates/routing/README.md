# Symfony routing subset

Private workspace crate used by both the demo and request-lab. This is a
typed subset, not a complete port of Symfony Routing. MIT; Symfony source
and tests copyright Fabien Potencier. See the repository LICENSE. This is
unofficial and is not affiliated with or endorsed by Symfony.

| Contract | Source / evidence |
|---|---|
| Registered route order, captures and method matching | `Matcher/UrlMatcher.php`; selected upstream tests |
| HEAD matching GET, explicit HEAD routes, ordered unique Allow values | Same source and method assertions; PHP vectors |
| Safe-method slash redirects, raw encoding and fallback precedence | `Matcher/RedirectableUrlMatcher.php`; selected upstream tests and PHP vectors |
| Positive signed 64-bit controller ids, ASCII slugs, nonempty segments | Existing demo constraints; original demo routing assertions and request-lab HTTP test |
| Application predicates, e.g. allowed locales | Typed `Requirement::Custom`; demo registrations |
| Path/query escaping | Existing `Generator/UrlGenerator.php` subset extracted without changing its algorithms; demo encoding assertions |
| `route::Route`: path/host normalization, inline `{!name:argument.path<requirement>?default}` settings, `_route_mapping`, requirement anchors and the empty-requirement error, lowercased schemes, uppercased methods, options with `compiler_class`, the `_locale` rule for localized routes, `__serialize()` / `__unserialize()` with its type errors and Stringable refusal | `Route.php`; RouteTest 24 of 24; 2,000 generated routes, 1,000 requirements and 131 `unserialize()` payloads recorded from PHP 8.4.27 (`tests/data/route.json`) |
| `route_compiler::RouteCompiler`: static prefix, path and host regex, tokens and variables, separators and possessive quantifiers, optional trailing variables, `!` important variables, the `utf8` option and its errors, `_locale` substitution, capturing-group rewriting, name errors | `RouteCompiler.php`, `RouteCompilerInterface.php`; RouteCompilerTest 10 of 10; the recorded routes' `compile()` |
| `compiled_route::CompiledRoute` accessors and serialization | `CompiledRoute.php`; CompiledRouteTest 2 of 2 |
| `route_collection::RouteCollection`: replacement by name, priorities, aliases with circular-reference and deprecation handling, remove, `addCollection()`, `addPrefix()`, `addNamePrefix()` (canonical routes, local and external aliases), bulk setters, resources (`config` feature) | `RouteCollection.php`, `Alias.php`; RouteCollectionTest 24 of 24; 300 recorded collection scripts |
| `request_context::RequestContext`: constructor defaults, base URL without trailing slashes, uppercased method, lowercased host and scheme, ports, query string (null is `''`), parameters, `isSecure()`, `fromUri()` (URIs with a backslash before `?`/`#`, an end byte up to space, or CR/LF/tab read as `''`; the path as base URL; a port for the scheme it names, compared before lowercasing). `fromRequest()` is not ported (it needs HttpFoundation's Request) | `RequestContext.php`; RequestContextTest 16 of 18 (1 `n/a`, `testFromRequest` pending); 3,000 `fromUri()` and 10 `setBaseUrl()` calls recorded (`tests/data/request_context.json`) |
| `exception`: every routing exception class with its PHP ancestry (`Error::is_a()`), MethodNotAllowedException's uppercased allowed methods and MissingMandatoryParametersException's route name, parameters and message (`Error::detail()`) | `Exception/*.php` (no upstream test class; used by the matcher and generator stages) |
| `url_generator::UrlGenerator`: `generate()` with localized routes (`_locale` from the parameters, the context or the default locale, `_canonical_route`, aliases), defaults and context parameters, optional and important variables, requirement checks anchored as a whole after removing look-arounds (GHSA-72xp-p242-47p9) in strict, logging (`Some(false)`) and unchecked (`None`) modes, path encoding with dot segments, host tokens, scheme requirements, the four reference types, extra and `_query` parameters compared with defaults by `==` and built with `http_build_query()` (RFC 3986), objects turned into their public properties or `__toString()` (circular references refused), fragments; `getRelativePath()`. The logger receives context values as strings; a subclass's `$decodedChars` is not ported | `Generator/UrlGenerator.php`, `UrlGeneratorInterface.php`, `ConfigurableRequirementsInterface.php`; UrlGeneratorTest 91 of 91; 3,000 `generate()` and 2,000 `getRelativePath()` calls recorded (`tests/data/url_generator.json`) |

`RouteCollection<T>` holds ordered `Definition<T>` registrations. Each
definition contains literal/parameter segments and a startup-created target
factory. A target implements `Target`, supplying canonical uppercase method
names, or `None` for any method. Factories receive named `Parameters` and
return the application's own typed route. There is no route-pattern parsing
or regex compilation at request time. The Axum server remains the HTTP
adapter. The crate itself has no demo, database, locale-enum or template types.

symfony-demo-rs's registrations live in `src/routing/collection.rs`; `src/routing.rs`
keeps its route enum, URL layouts and public matching facade. The independent
lab registers its own routes. Method refusals hold an ordered vector so
multiple registrations for one path can aggregate their Allow values.

`tests/upstream.rs` preserves names and relevant assertions from 16 methods
of `UrlMatcherTest.php` / `RedirectableUrlMatcherTest.php` at the pinned (v8.1.11)
commit `f3fefc6cad3748d538be970ca81b71f06d7aa8b3`. Rust calls use
canonical method tokens rather than PHP's lowercase constructor arguments;
the expected methods, route names, parameter values and redirect paths are
preserved. This does not claim all upstream tests have been ported.

An additional test compares 90 outcomes recorded directly from that installed
PHP matcher: `tests/data/matcher.json`. Regenerate from a symfony-demo-rs
checkout, with the original running, then copy the file here:

```sh
uv run python compare/routing_vectors.py
cargo test -p symfony-routing
```

The ported Route classes (`route`, `compiled_route`, `route_compiler`,
`request_context`, `route_collection`, `alias`, `url_generator`) are Symfony's, and are not yet used by the
typed matcher and generator above. A collection owns its routes (PHP
shares Route objects between handles), and `compiler_class` is chosen at
build time with `Route::compile_with::<C>()`. Their tests are
`tests/route_test.rs`, `route_compiler_test.rs`, `compiled_route_test.rs`,
`route_collection_test.rs`, `request_context_test.rs`,
`url_generator_test.rs`, plus the replays `route_recorded.rs`
(`scripts/record.sh routing-route`), `request_context_recorded.rs`
(`scripts/record.sh routing-request-context`) and
`url_generator_recorded.rs` (`scripts/record.sh routing-url-generator`). Not reproduced, and refused instead:
the "called in" suffix of a TypeError for a non-string requirement, a
non-array `_route_mapping` default when a pattern adds a mapping, and a
`_canonical_route` default that cannot be concatenated in
`addNamePrefix()`.

Not implemented in the typed matcher and generator: arbitrary PCRE requirements, adjacent/greedy parameters,
optional-parameter/default inference, host or scheme routing, expression
conditions, route loaders, route aliases, locale-aware generation, complete
named URL generation, relative URLs, or HttpKernel events. Application
factories explicitly provide defaults; URL layouts remain application-owned.
`PositiveInt64` deliberately describes the demo's fixed-width controller
argument, rather than claiming to implement an unbounded PHP regex integer.

All lookup is in memory and case-sensitive. No filesystem-dependent resource
lookup is introduced. Focused tests cover these contracts; there is no
extensive OS/filesystem test matrix.
