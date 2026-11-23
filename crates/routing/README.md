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

Not implemented: arbitrary PCRE requirements, adjacent/greedy parameters,
optional-parameter/default inference, host or scheme routing, expression
conditions, route loaders, route aliases, locale-aware generation, complete
named URL generation, relative URLs, or HttpKernel events. Application
factories explicitly provide defaults; URL layouts remain application-owned.
`PositiveInt64` deliberately describes the demo's fixed-width controller
argument, rather than claiming to implement an unbounded PHP regex integer.

All lookup is in memory and case-sensitive. No filesystem-dependent resource
lookup is introduced. Focused tests cover these contracts; there is no
extensive OS/filesystem test matrix.
