# M2-A stage 1: HttpFoundation bags and HeaderUtils

November 22. Branch `m2/http-foundation-bags`.

## Scope

Upstream symfony/http-foundation at the pin (v8.1.10,
`79325f87c3a2acc7564ddce9f457046e980e7255`):

| PHP file | Rust | Upstream tests |
|---|---|---|
| `ParameterBag.php` | `crates/http-foundation/src/parameter_bag.rs` | ParameterBagTest: 39 ported, 2 `n/a`, 1 pending (`testFilter`) |
| `InputBag.php` | `crates/http-foundation/src/input_bag.rs` | InputBagTest: 23 ported, 2 `n/a` |
| `Exception/BadRequestException.php`, `UnexpectedValueException.php`, `RequestExceptionInterface.php` | `crates/http-foundation/src/exception.rs` | (through the bag tests) |
| `HeaderUtils.php` without `makeDisposition()` and `parseQuery()` | `crates/http-foundation/src/header_utils.rs` | HeaderUtilsTest: 6 ported, 7 pending |

PHP built-ins the bags call, added to `php-value` (written from the PHP
manual and recorded answers, no PHP source, decision 8):
`filter::filter_var()` for five filters, `object::backed_enum_from()`,
`object::Closure`, `object::StringableObject`, `PhpObject::to_php_string()`
and `numeric::numeric_value()`.

The `n/a` tests (testFilterArrayWithCycle and
testFilterArrayWithSharedScalarReference in both bag tests) build PHP
references (`&$value`) into the array; php-value arrays are plain values
(decision 3). `testFilter` uses `FILTER_VALIDATE_EMAIL` and
`FILTER_VALIDATE_URL`, which php-value refuses; its other assertions run
in `filter_assertions_without_email_and_url`, which also checks that the
two filters are refused.

## Evidence recorded from PHP 8.4.27 (oracle image)

- `oracle/recorders/filter_var.php` → `crates/php-value/tests/data/filter.json`:
  22,932 `filter_var()` calls (113 values × the five filters × flag and
  option sets, callbacks by name) and the extension's constants. All match.
- `oracle/recorders/http_foundation_bags.php` →
  `crates/http-foundation/tests/data/bags.json`: 2,450 ParameterBag and
  InputBag calls (every getter, `all()`, `getEnum()` with int-, string-
  and unit enums, `filter()`) over 53 stored values and defaults. All
  match.
- `oracle/recorders/http_foundation_header_utils.php` →
  `crates/http-foundation/tests/data/header_utils.json`: 3,000 generated
  `split()` calls and 633 `quote()`/`unquote()` strings. All match.

Run with `scripts/record.sh php-value` and `scripts/record.sh
http-foundation`.

## What the recordings found

- `filter_var()`: with `FILTER_FLAG_ALLOW_HEX`/`ALLOW_OCTAL` the digits are
  read as an unsigned 64-bit number (`0xffffffffffffffff` is `-1`);
  without `FILTER_NULL_ON_FAILURE`, a valid `false` from
  `FILTER_VALIDATE_BOOL` is replaced by the `default` option; a wrong
  shape (array vs scalar) ignores `default`; `FILTER_CALLBACK` without a
  `flags` entry walks arrays; range options are cast to int.
- `ParameterBag::all($key)` gives `[]` for a stored null (`?? []`).
- `BackedEnum::from()` on a string-backed enum truncates a float that fits
  in an int (`1.5` finds the case `'1'`) and takes a `\Stringable` as its
  string; on an int-backed enum a `\Stringable` is a `TypeError`.
- `HeaderUtils::split()`: with a whitespace separator (`' '`, `"\t"`) the
  separator branch's `\s*` gives back whitespace until a separator
  follows, and a value's inner whitespace excludes separators. The
  earlier tokenizer (in `accept_language.rs`) treated every whitespace run
  alike, which the Accept-Language path (`,;=`) never exposed. A
  one-separator group whose first value unquotes to `''` recurses with no
  separators left; PHP warns ("Uninitialized string offset 0") and takes
  the innermost branch.
- `unquote()` keeps a backslash before a newline (PCRE's `.`); `quote()`
  passes a token followed by one final `\n` (no `D` flag).

## Advisories

`parity/advisories.tsv`, from `scripts/upstream_watch.py` on November 22:

- GHSA-r925-mgvr-h9cc (`HeaderUtils::split` ReDoS): still `covered`. The
  fix's test moved with the tokenizer to
  `tests/header_utils_test.rs::testSplitIsNotAmbiguousAboutWhitespaceBeforeSeparators`;
  the new whitespace-separator handling skips a separator-free whitespace
  run in one step, so the scan stays linear
  (`long_whitespace_runs_split_in_linear_time`, 200,000 bytes). The
  pre-fix check against v8.1.6 was done in M0's drill.
- GHSA-38cx-cq6f-5755 (`IpUtils::PRIVATE_SUBNETS`): still `not-ported`;
  IpUtils waits for decision 16.
- GHSA-3rg7-wf37-54rm (PATH_INFO): still `not-ported` (Request).

The watch also lists v8.1.11, which changes `HeaderUtils::makeDisposition()`
(`/D` anchor) and IpUtils (`PRIVATE_RANGES`). Neither is ported; decision
16 asks the owner to move the pins before they are.

## Checks

The five AGENTS.md checks and symfony-demo-rs's checks and comparison
against this checkout are in the PR description.

## Next

AcceptHeader and AcceptHeaderItem as an API (accept_language.rs keeps its
own copy until then), HeaderBag, ServerBag, ResponseHeaderBag, Cookie,
FileBag (needs `File/UploadedFile.php`). IpUtils, `makeDisposition()` and
`setTrustedProxies()` after decision 16. `parseQuery()` with `parse_str()`
in php-value. HttpKernel does not yet answer a
`RequestExceptionInterface` with 400.
