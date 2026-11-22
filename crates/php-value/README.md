# PHP values

Private workspace crate: PHP's values and the engine behaviour that
symfony-rs components depend on (PLAN.md decision 3). MIT. No PHP source
code is used: every rule here is written from PHP 8.4.27's observed
behaviour and checked against vectors recorded in the oracle container
(`scripts/record.sh php-value` → `tests/data/php.json`).

| Contract | Evidence |
|---|---|
| `Value`: null, bool, int, float, byte string, array, object; `===` as `PartialEq` | Unit use in dependants |
| Array keys: canonical decimal strings become ints (`"7"`, `"-3"`; not `"07"`, `"+1"`, `"-0"`, `" 1"`, out-of-range); `true`/`false` → 1/0; `null` → `""` | 58 key vectors |
| Array order and appending: assignment keeps an existing key's place; append uses one more than the largest int key ever used (not lowered by removal; negative keys count, PHP 8.3+); `Cannot add element to the array as the next element is already occupied` after `PHP_INT_MAX` | 9 scenarios |
| `(int)`, `(float)` of strings (longest numeric prefix, whitespace, saturation, INF → 0) and `is_numeric()` | 55 vectors |
| `(string)` of floats (`precision = 14`), `var_export()` and `json_encode()` of floats (shortest digits) | 338 vectors |
| `hexdec()`, `octdec()`: invalid characters skipped, float after overflow | 19 vectors |
| `filter::filter_var()` for `FILTER_UNSAFE_RAW` (with `FILTER_FLAG_STRIP_LOW`), `FILTER_VALIDATE_INT` (hex, octal, `min_range`/`max_range`), `FILTER_VALIDATE_BOOL`, `FILTER_SANITIZE_NUMBER_INT` and `FILTER_CALLBACK`; `FILTER_REQUIRE_SCALAR`, `FILTER_REQUIRE_ARRAY`, `FILTER_FORCE_ARRAY`, `FILTER_NULL_ON_FAILURE`, `default`; the extension's constants. Any other filter, flag or option is refused (`Unsupported`) | 22,932 recorded calls (`tests/data/filter.json`, oracle/recorders/filter_var.php) and the constants' values |
| `object::backed_enum_from()`: `BackedEnum::from()` in coercive mode (int- and string-backed), its `TypeError`/`ValueError` messages; the `E_DEPRECATED` for a truncated float is not reproduced | symfony-http-foundation's 2,450 recorded bag calls |
| `object::Closure` (a `\Closure` over a Rust function), `StringableObject`, `PhpObject::to_php_string()` (`\Stringable`), `numeric::numeric_value()` | Unit use in dependants |

Arrays have value semantics: a clone shares storage until one side is
written (`Arc::make_mut`). Removing a key is O(n). Objects are handles
(`Arc<dyn PhpObject>`); concrete classes (stdClass, DateTimeImmutable,
enum cases) and `serialize()`/`unserialize()` come with the components
that need them.
