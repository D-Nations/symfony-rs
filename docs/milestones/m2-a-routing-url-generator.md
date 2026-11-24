# M2-A routing stage 3: UrlGenerator

November 24. Branch `m2/routing-url-generator`. Crates touched:
`symfony-routing`, `php-value`.

Built on stage 2 (RequestContext and the routing exceptions, PR #36). A
matcher session works on `Matcher/` at the same time; this stage does not
touch it.

## Scope

Upstream symfony/routing at the pin (v8.1.11,
`f3fefc6cad3748d538be970ca81b71f06d7aa8b3`).

| PHP file | Rust | Upstream tests |
|---|---|---|
| `Generator/UrlGenerator.php`, `Generator/UrlGeneratorInterface.php`, `Generator/ConfigurableRequirementsInterface.php` | `crates/routing/src/url_generator.rs` | UrlGeneratorTest 91 of 91 |

`php-value` gains `compare::loose_equals()` (PHP's `==`, which
`doGenerate()` uses through `array_udiff_assoc()` to drop query
parameters equal to defaults) and `url::http_build_query_with()` (the
`PHP_QUERY_RFC3986` encoding and float values). Both are written from the
PHP manual and recorded answers (no PHP source, decision 8).

Prior art: no Rust port of Symfony's UrlGenerator was found (searched
November 24; the routing stack row covers the router crates), and no Rust
crate implements PHP's `==` (PLAN.md stack row). symfony-console and
symfony-finder each have a partial copy, which can move to
`php_value::compare` later.

The existing `generator.rs` (the demo's typed subset) is unchanged. Its
two unit tests named after upstream tests are renamed
(`encodes_relative_path_segments`, `encodes_chained_relative_path_segments`),
since the full upstream tests now live in `tests/url_generator_test.rs`.

## Port notes

- `generate()` takes `&self`. A route not compiled in the collection is
  compiled on a copy for the call (PHP caches the compiled route on the
  shared object); requirement patterns are compiled once per generator.
- The look-around stripping regex uses PCRE recursion (`(?1)`), which
  php-pcre refuses; its alternatives start with different bytes, so it is
  a small hand parser with the same matches.
- `!preg_match()` treats a pattern PHP cannot compile, a non-UTF-8 subject
  under `/u` and other match failures as "no match", as PHP does.
- Parameters keep PHP's coercions: an array subject is a TypeError from
  `preg_match()`, an array in the path is `"Array"`, an object without
  `__toString()` an `\Error`, a non-string locale is cast before
  `strstr()`.
- The logger gets context values as strings (psr-log's `ContextValue`);
  PHP passes the raw value. The recorder logs the string form to match.
- A subclass changing `$decodedChars` is not ported (the table is
  upstream's constant).

## Behaviour found by recording

- `==` between two integer strings of which one overflows an int
  compares bytes (`"9223372036854775807" != "9223372036854775808"`), while
  an int and that string compare as numbers.
- NAN equals no string, not even `"NAN"`.
- A `\Stringable` object against a string compares under string rules,
  numeric strings included (`Text('1') == '1.0'`).
- `http_build_query()` writes floats as `(string)` does (`1.0E+20`, `NAN`),
  skips nulls and empty arrays, and encodes nested keys.

## Evidence

| What | Cases | Result |
|---|---|---|
| Upstream tests | UrlGeneratorTest 91 of 91 (data providers run as loops) | all pass |
| `generate()` (`crates/routing/tests/data/url_generator.json`, `oracle/recorders/routing_url_generator.php`) | 3,000 generated collections (inline settings, defaults of every type, requirements with look-arounds and alternations, hosts, schemes, localized variants and aliases), contexts, strictness and default locales, one call each: 1,139 URLs (69 through the logger), the rest exceptions of 7 classes, compared by class and message | all match |
| `getRelativePath()` (same file) | 2,000 generated path pairs | all match |
| `==` (`crates/php-value/tests/data/loose_equals.json`, `oracle/recorders/php_value_loose_equals.php`) | 8,649 ordered pairs of 93 values | all match |
| `http_build_query()` (`crates/php-value/tests/data/http_build_query.json`, `oracle/recorders/php_value_http_build_query.php`) | 511 arrays, both encodings | all match |

Mutation checks: comparing extra parameters with `===` instead of `==`
fails 4 recorded calls; not removing look-arounds fails 21; the pre-fix
requirement pattern fails the advisory's upstream test.

## Advisories

GHSA-72xp-p242-47p9 (UrlGenerator requirement check with an unanchored
alternation) is now `covered`: the requirement is anchored as a whole
(`#^(?:requirement)$#`), pinned by the fix's upstream test
`testGenerateForRouteWithAlternationRequirementRejectsSubstringMatch`. The
oracle shows the pre-fix check `preg_match('#^en|fr|vi|de$#i',
'/evil.com')` matching and the pinned code throwing. GHSA-h5x3-xfc9-m39h
(dot segments) stays `covered`; its upstream tests now also run against
the full generator.

## Checks

See the PR. `scripts/linux-test.sh -p symfony-routing -p php-value` runs
before merge.

## Not done

`CompiledUrlGenerator.php` and `Dumper/CompiledUrlGeneratorDumper.php`
(CompiledUrlGeneratorDumperTest, 21 tests pending).

## Demo comparison

Not run: paused by the owner (November 23). The demo uses the typed
generator, which this stage does not change.
