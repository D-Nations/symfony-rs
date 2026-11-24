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
| Array order and appending: assignment keeps an existing key's place; append (`push()`, `next_index()`) uses one more than the largest int key ever used (not lowered by removal; negative keys count, PHP 8.3+); `Cannot add element to the array as the next element is already occupied` after `PHP_INT_MAX` | 9 scenarios |
| `(int)`, `(float)` of strings (longest numeric prefix, whitespace, saturation, INF → 0) and `is_numeric()` | 55 vectors |
| `(string)` of floats (`precision = 14`), `var_export()` and `json_encode()` of floats (shortest digits) | 338 vectors |
| `hexdec()`, `octdec()`: invalid characters skipped, float after overflow | 19 vectors |
| `filter::filter_var()` for `FILTER_UNSAFE_RAW` (with `FILTER_FLAG_STRIP_LOW`), `FILTER_VALIDATE_INT` (hex, octal, `min_range`/`max_range`), `FILTER_VALIDATE_BOOL`, `FILTER_SANITIZE_NUMBER_INT` and `FILTER_CALLBACK`; `FILTER_REQUIRE_SCALAR`, `FILTER_REQUIRE_ARRAY`, `FILTER_FORCE_ARRAY`, `FILTER_NULL_ON_FAILURE`, `default`; the extension's constants. Any other filter, flag or option is refused (`Unsupported`) | 22,932 recorded calls (`tests/data/filter.json`, oracle/recorders/filter_var.php) and the constants' values |
| `object::backed_enum_from()`: `BackedEnum::from()` in coercive mode (int- and string-backed), its `TypeError`/`ValueError` messages; the `E_DEPRECATED` for a truncated float is not reproduced | symfony-http-foundation's 2,450 recorded bag calls |
| `clock::time()`, with `clock::mock()` fixing it per thread as ClockMock does | Unit test; symfony-http-foundation's time-sensitive tests |
| `datetime::gmdate()` and the `D l M F j w N` format characters; a `GMT` zone | 13 recorded timestamps (`tests/data/dates.json`) |
| `dateparse::strtotime()` and `dateparse::create_from_format_rfc2822()`: HTTP, cookie, asctime and ISO dates, `@timestamp`, relative offsets, single words; PHP's `false` where the reason is visible; any other text refused (`Unsupported`). Default time zone UTC | 173 + 209 recorded answers (`tests/data/dates.json`, oracle/recorders/http_foundation_dates.php); the refused texts are listed in `tests/dates.rs` |
| `net::{validate_ip, inet_pton, inet_ntop, ip2long}`: `FILTER_VALIDATE_IP`'s address grammar (no leading zeros, one `::` for at least one group, IPv4 tail), glibc's `inet_ntop()` text, the `ValueError` for a NUL byte | 388 strings and 629 packed addresses recorded (symfony-http-foundation's `tests/data/ip_utils.json`) |
| `numeric::{leading_numeric_value, compare_string_with_int, int_argument}`: a string in arithmetic, `$string <=> $int` and an `int` parameter under PHP 8's rules (a leading-numeric string is its prefix in arithmetic, compares as a string, and is a TypeError as an argument) | symfony-http-foundation's IpUtils netmask vectors |
| `path::{to_path, to_os_str, from_path, from_os_str, from_os_string}`: PHP's byte-string paths as std `Path`/`PathBuf` and back; exact on Unix; outside Unix a path that is not UTF-8 names no file (`None`) | Unit tests; symfony-finder's, symfony-filesystem's, symfony-config's and symfony-translation's file tests |
| `path::{basename, dirname}` (and `_str` forms): PHP's `basename()` (no suffix) and `dirname()` (one level) with `/` as `DIRECTORY_SEPARATOR` | 16 cases in a unit test (from the PHP manual, not yet recorded in the oracle); symfony-finder's, symfony-filesystem's, symfony-config's and symfony-dotenv's tests |
| `url::{urlencode, rawurlencode, urldecode, rawurldecode}` on bytes | Unit test; symfony-http-foundation's Cookie vectors |
| `url::{http_build_query, bin2hex, hex2bin}`: `http_build_query($data, '', $separator)` (RFC 1738) for strings, ints, bools, null and nested arrays; a float or object entry is refused (`None`) | symfony-http-foundation's HeaderUtilsTest `parseQuery` tests and 3,000 recorded `parseQuery()` calls |
| `vars::{register, parse_str, top_level_key}`: PHP's input-variable rules (NUL ends a name, leading spaces dropped, `.`/space/unclosed `[` to `_`, `[...]` groups, int keys, `[]` appends, `max_input_nesting_level` 64) and `parse_str()` (`max_input_vars` 1000, stops at a raw NUL byte); PHP's warnings are not reproduced | 254 recorded `parse_str()` inputs (`tests/data/parse_str.json`, oracle/recorders/php_value_parse_str.php); symfony-http-foundation's 160 query, 54 cookie and 274 multipart vectors |
| `base64::decode()` (non-strict `base64_decode()`: bytes outside the alphabet skipped, trailing bits dropped) | 15 recorded answers |
| `PhpArray::ksort()`, `array::compare_keys()` (PHP 8's `<=>` on keys) | symfony-http-foundation's header-bag vectors |
| `object::Closure` (a `\Closure` over a Rust function), `StringableObject`, `PhpObject::to_php_string()` (`\Stringable`), `numeric::numeric_value()` | Unit use in dependants |

Arrays have value semantics: a clone shares storage until one side is
written (`Arc::make_mut`). Removing a key is O(n). Objects are handles
(`Arc<dyn PhpObject>`); concrete classes (stdClass, DateTimeImmutable,
enum cases) and `serialize()`/`unserialize()` come with the components
that need them.
