# M2-A routing stage 5: UrlMatcher and RedirectableUrlMatcher

November 25. Branch `m2/routing-url-matcher`. Crates touched:
`symfony-routing`, `php-pcre`.

Built on stages 1–4 (Route, RouteCompiler, RouteCollection,
RequestContext, the exceptions and both generators). This is the
matching half of the HTTP path. Next come HttpFoundation's `Request.php`,
then `matchRequest()` and `RequestContext::fromRequest()` on it.

## Scope

Upstream symfony/routing at the pin (v8.1.11,
`f3fefc6cad3748d538be970ca81b71f06d7aa8b3`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Matcher/UrlMatcher.php`, `Matcher/UrlMatcherInterface.php` | `crates/routing/src/url_matcher.rs` | UrlMatcherTest 83 of 91 |
| `Matcher/RedirectableUrlMatcher.php`, `Matcher/RedirectableUrlMatcherInterface.php` | `crates/routing/src/redirectable_url_matcher.rs` | RedirectableUrlMatcherTest 96 of 104 |

Pending:
- The six condition tests (`testCondition`, `testZeroCondition`,
  `testZeroConditionNextToOtherConditions`, `testRequestCondition`,
  `testRouteParametersCondition`, `testSiblingRoutes`) need
  ExpressionLanguage.
- The two `matchRequest()` tests need HttpFoundation's Request.
- RedirectableUrlMatcherTest inherits all eight, so they are pending there
  too.

Prior art (searched November 25): no Rust port of Symfony's UrlMatcher was
found. The router crates (matchit, path-tree, route-recognizer, axum's and
actix's) have their own pattern languages; see the routing stack row.
Three approaches were compared:
- Adopting a router crate would approximate Symfony's semantics.
- Porting `CompiledUrlMatcher` first would optimize before the matcher
  contract is proven (PLAN.md, M2-A row).
- Porting `UrlMatcher.php` on the ported routes and php-pcre was chosen.
  The compiled matcher can follow as a build-time emitter, as the compiled
  generator did.

## Port notes

- **Compilation.** `UrlMatcher::new()` compiles every route once. A route
  that does not compile fails the match that reaches it, as PHP's lazy
  `compile()` does. Path and host regexes are compiled once per matcher.
- **Regex failures.** A pattern PHP cannot compile is a non-match, as
  `!preg_match()` reads PHP's `false`. A pattern php-pcre does not
  translate is refused (`NotPorted`).
- **Conditions.** A route with a condition throws upstream's own
  `\LogicException` for a missing ExpressionLanguage component when it is
  reached. That is what PHP does without that component.
- **`$matches`.** `preg_match()`'s array is built as PHP builds it, with
  php-pcre's new `Pcre::group_names()`: each named group under its name and
  then its number, `''` for a group that did not take part, and the
  trailing groups that did not take part left out.
- **Redirects.** `RedirectableUrlMatcher<R>` stands for the abstract class
  and `R` for its `redirect()`. Upstream's `goto redirect_scheme` is a
  loop. Each retry is a fresh match, which resets the collected methods and
  schemes, on a copy of the context with the scheme changed, so the
  matcher's context is never modified. Only routing exceptions are caught
  on a retry.
- **Typed matcher.** The demo's typed matcher (`lib.rs`) is unchanged. The
  functions in `tests/upstream.rs` are renamed `typed_*`, and their 16
  manifest rows now name the ported tests.

## php-pcre: group names

`Pcre::group_names()` reports each group's name by number.

Group names under `/u` now follow PCRE2 10.44, as PHP 8.4.27 bundles it:
- Allowed: Unicode letters (`\p{L}`), decimal digits (`\p{Nd}`) and `_`,
  with no digit first.
- Refused: other numbers (`½`, `Ⅳ`, `〇`), marks and punctuation.
- Without `/u` a name is ASCII.

Before this change, php-pcre refused every non-ASCII name as a pattern PHP
rejects, which failed `testSlashVariant2` and `testUtf8VarName`. The
classes come from fancy-regex's Unicode tables, so there is no new
dependency. The probe answers are pinned in
`group_name_tests::unicode_group_names_follow_pcre2`.

## Behaviour found by recording

- `NoConfigurationException` is thrown for the path `/` whenever no
  method or scheme was collected, not only for an empty collection (278
  of the recorded requests).

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | UrlMatcherTest 83 of 91, RedirectableUrlMatcherTest 96 of 104 (inherited tests run on both matchers) | all pass |
| `match()` on both matchers (`crates/routing/tests/data/url_matcher.json`, `oracle/recorders/routing_url_matcher.php`) | 2,500 generated collections (inline requirements, defaults and mappings, important and adjacent variables, trailing slashes, UTF-8 routes, hosts with variables, schemes, methods, priorities, `_canonical_route`) and 48 edge collections (paths that differ only by a trailing slash, with schemes and methods, in both orders): 11,152 requests in generated contexts, 22,304 answers (attributes compared as whole arrays; exceptions by class, message and allowed methods) | all match |
| php-pcre group names | 17 names probed on PHP 8.4.27 | all match |

Mutation checks:
- Dropping the scheme check from the `NoConfigurationException` condition
  fails 65 recorded answers.
- Dropping either of the redirecting matcher's two scheme fallbacks after a
  slash retry fails the edge collections. They were added after the random
  collections alone did not catch the first of those.
- Letting null match values override defaults changes no answer. Match
  values are never null.

## Advisories

No advisory in the upstream watch concerns the matcher.

## Checks

See the PR for the commands' output. The symfony-demo-rs comparison was
not run (paused by the owner, November 23).
