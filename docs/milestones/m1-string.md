# M1: symfony/string (November 22)

Stage log for the String port beyond Console's subset (PLAN.md, M1
progress). Pinned `symfony/string` v8.1.10 (`6c611097`); oracle PHP
8.4.27, ICU 72.1, PCRE2 10.44.

## Scope

Ported, one PHP file per Rust module: `AbstractString.php` and
`TruncateMode.php` (`abstract_string.rs`, the trait), `ByteString.php`,
`AbstractUnicodeString.php`, `CodePointString.php`, `UnicodeString.php`
(rewritten from Console's subset, its API kept), the three exceptions,
`Resources/functions.php` (`lib.rs`). Second step, after PR #13:
`LazyString.php` (`lazy_string.rs`), `Inflector/InflectorInterface.php`
(`inflector/mod.rs`), `EnglishInflector.php`, `FrenchInflector.php` and
`SpanishInflector.php`. Not this stage: `ascii()` and the `locale*()`
methods (ICU transliterators) and the Slugger that needs them.

Prior art (searched again): no Rust port of symfony/string; the stack row
for case mappings and patterns (PLAN.md) records the options compared.

## Prerequisite moved

Finder's PCRE translator became the `php-pcre` crate, so String's
`match()`, `replaceMatches()` and flagged `split()` run PHP patterns the
same way Finder does. symfony-finder re-exports it as
`symfony_finder::pcre`; its tests pass unchanged. One addition: `\p{..}`
inside a character class (upstream's `[^\pL0-9]++`, `[\p{Ll}0-9]`) is
passed through as it already was outside one.

## Evidence

| Upstream test | Ported | Notes |
|---|---|---|
| ByteStringTest | 68 / 68 | fromRandom() with a caller's random source (decision 15) |
| CodePointStringTest | 73 / 78 | 5 pending: `ascii()`, `locale*()` |
| UnicodeStringTest | 75 / 81 | 5 pending as above; 1 `n/a`: PHP `unserialize()` gadget |
| FunctionsTest | 3 / 3 | |
| LazyStringTest | 7 / 7 | callables given with their `getPrettyName()` name |
| EnglishInflectorTest | 4 / 4 | |
| FrenchInflectorTest | 2 / 2 | each row also with `ucfirst()` |
| SpanishInflectorTest | 3 / 3 | |

Total 235 of 266: 1 `n/a`, 30 pending (ICU transliterators and the
Slugger).

Every data provider is replayed from the oracle
(`oracle/recorders/test_providers.php`); the two `replaceMatches()`
closures are written in Rust. Upstream's own ICU-21661 skips apply.

Recorded for what the tests leave open (`scripts/record.sh string`):

- `string_case.php`: mbstring's lower, upper, title, fold and simple fold
  of every code point and ICU's NFKC_CF (11,881 that differ), plus the
  Cased and Case_Ignorable ranges of `mb_strtolower()`'s final sigma rule.
  `scripts/gen-string-tables.py` turns them into `src/case_tables.rs`.
- `string_builtins.php`: 59,808 answers of the `str*pos()`, `mb_*pos()`
  and `grapheme_*pos()` families (24 haystacks, 30 needles, 7 offsets),
  and 4,208 calls of `mb_substr()`, `grapheme_substr()`,
  `grapheme_extract()`, `grapheme_str_split()`, whole-string case mapping,
  `str_pad()`, `ucwords()`, `ucfirst()`, `lcfirst()`, `substr()`,
  `substr_replace()`, `trim()` with ranges and the normalizer forms; the
  Windows-1252 (mbstring) and windows-1250 (`iconv()`, which mbstring
  lacks) code pages.

- `string_inflector.php`: a corpus of 10,629 words (upstream's provider
  words and the inflectors' own table entries, read by reflection, each
  with case variants, prefixes, a trailing newline, `é` and bytes that are
  not UTF-8) through `singularize()` and `pluralize()` of each inflector:
  63,774 answers, all equal. Breaking the English vowel set on purpose
  makes 521 words differ, so the replay compares.
- `string_lazy.php`: 100 LazyString cases (each kind of return value,
  callable names in the TypeError, retries and memoization, the
  `fromCallable()` array check, `fromStringable()`, `resolve()`,
  `isStringable()`, `json_encode()`, `serialize()`): 97 replayed and
  equal, 3 listed as not expressible in the typed API.

## Findings

- `grapheme_strpos()` is ICU's collation-based search: it matches
  canonically equivalent text (`é` against `e` + U+0301), skips default
  ignorables (U+200B, U+200C) and folds case simply (`ß` never matches
  `ss`). A first exact-match implementation differed on 526 of 59,808
  answers; modelling those three rules leaves 19, of two kinds listed in
  the README (the U+FB01 ligature under `stripos`, and `strrpos` of an
  all-ignorable needle). UnicodeStringTest's `testReplace` row on a ZWNJ
  depends on the ignorable rule.
- mbstring's Windows-1252 maps the five bytes the code page leaves
  undefined to the C1 controls and accepts them under strict detection.
- `str_pad()` with an empty pad returns the string when no padding is
  needed; it only throws when it would pad.
- `grapheme_extract()` of `''` is `false`, not `''`.
- LazyString's coercion is that of a `string` return type without
  `strict_types`: `0.1 + 0.2` becomes `"0.3"` (`precision = 14`, as
  `(string)` gives it), null `""`, a Stringable its text; an array, a
  plain object or an enum is the TypeError naming its type. After that
  TypeError the callback is not called again; the next cast fails the
  same way. A callback that throws is called again, and a lazy-callable's
  object, once made, is kept.
- `fromCallable()` accepts any array of more than two items and fails
  only when cast ("Value of type int is not callable").
- The French and Spanish patterns have no `/u`: `/i` folds ASCII only, so
  `ÉMAUX` singularizes to `ÉMal` (the `ém` alternative misses `ÉM`; the
  `-al` rule's `m` matches), and `$` also matches before a final newline
  (`cheval` + newline pluralizes to `chevaux` + newline).

## Checks

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `scripts/sources.sh --check` and `scripts/parity-check.sh`
pass (output in the PR). symfony-demo-rs uses none of the crates changed
(symfony-string, symfony-console, symfony-finder, php-pcre), so its page
comparison is not affected.
