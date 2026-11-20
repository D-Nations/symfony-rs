# Symfony Finder

Private workspace crate: a port of `symfony/finder` v8.1.10 (`2f4c0f26`),
as installed by symfony/demo. MIT; Symfony source and tests copyright
Fabien Potencier. No PHP or PCRE source code is used: the PHP built-ins
(`src/php.rs`, `src/date.rs`, `src/fs.rs`) and the PCRE pattern
translation (`src/pcre.rs`) are written from documentation and PHP's
observed behaviour. Unofficial, not affiliated with or endorsed by
Symfony or PHP.

```rust
use symfony_finder::{Finder, NumberComparator};

let size = NumberComparator::new(Some(b"> 10Ki")).unwrap();
assert!(size.test(10241));
assert_eq!(size.get_target(), b"10240");

let mut finder = Finder::create();
finder.files().name("*.rs").depth("< 1").unwrap();
finder.r#in(concat!(env!("CARGO_MANIFEST_DIR"), "/src")).unwrap();
assert!(finder.has_results().unwrap());
```

| Contract | Source | Evidence |
|---|---|---|
| Comparators: operators, targets as PHP strings, PHP 8 comparison of ints and numeric strings, magnitudes, upstream's messages | `Comparator/*.php` | ComparatorTest (3), NumberComparatorTest (2), DateComparatorTest (2); 63 NumberComparator and 1,156 comparison vectors |
| `Glob::toRegex()` text for every flag and delimiter combination | `Glob.php` | GlobTest (13); 402 recorded globs |
| `Gitignore::toRegex()` / `toRegexMatchingNegatedPatterns()` text and matching | `Gitignore.php` | GitignoreTest (3, 535 provider rows); 37 recorded lines |
| Finder: `in()` (directories and `glob()` with braces), `append()`, depth, name, contains, path, size, date, exclude, dot and VCS files, `.gitignore` and `.git/info/exclude`, prune filters, sorting, `count()`, `hasResults()`, upstream's messages | `Finder.php`, `Iterator/*.php`, `SplFileInfo.php`, `Exception/*.php` | FinderTest (81), the other Iterator tests (21), VcsIgnoredFilterIteratorTest (15), GlobTest's 4 directory tests |
| Order of filters, callbacks and stat calls on stream wrappers (`fs::stream_wrapper_register()`) | `Finder.php`, `Iterator/ExcludeDirectoryFilterIterator.php`, `Iterator/RecursiveDirectoryIterator.php` | testFilterPrune's call log |
| `preg_match` on delimited patterns: delimiters, modifiers `imsxuADUSXn`, PCRE's `$`, `\Z`, `{` and byte semantics; unsupported features refused | (PHP behaviour) | 137 patterns x 62 subjects recorded from PHP |
| `strnatcmp()`, `strnatcasecmp()`, `strcasecmp()` | (PHP behaviour) | 2,500 recorded pairs |

The manifest (`parity/symfony/finder.tsv`) has 139 tests ported and 4
`n/a`: FinderOpenBasedirTest (incomplete upstream; `open_basedir`), two
FTP integration tests, and CustomFilterIteratorTest::testWithInvalidFilter
(a filter is a closure by type).

Vectors: `scripts/record.sh finder` runs `oracle/recorders/finder_providers.php`
(upstream's provider rows, which the ports of the provider tests replay)
and `finder_vectors.php` on PHP 8.4.27.

## Differences

- `DateComparator` reads the date subset in `src/date.rs` (ISO dates and
  times, `@timestamp`, `now`/`today`/`yesterday`/..., relative amounts with
  `ago`, `last`, `next`), in UTC. Other formats PHP accepts (month and
  weekday names, time zones, ordinals) are refused with
  `"..." is not a valid date, or not a date format this port supports.`
  (PLAN.md, decision 13).
- PCRE features the translator does not handle are refused with
  `Error::UnsupportedPattern` (recursion, conditionals, verbs, `\K`,
  `\X`, `\C`, branch reset, callouts, `/J`, `/r`, POSIX classes under
  `/u`). PHP's backtracking limit is approximated by `fancy-regex`'s
  (same 1,000,000 figure, counted differently). `fancy-regex` parses 64
  nested groups where PCRE takes 250; deeper patterns made of lookarounds
  followed by a group (what `Gitignore` builds) are flattened first
  (`src/pcre/nesting.rs`), other deep patterns fail to compile.
- `in()`'s `glob()` follows glibc's rules for local paths only; it is
  written here (no `glob(3)`), checked by the FinderTest glob tests.
- An appended Finder is taken by value; PHP keeps a reference, so later
  changes to it are seen by PHP and not here.

## Rust shape

- Linux behaviour is canonical on every system (as in symfony-console):
  the directory separator is `/`, `useUnixPaths()` changes nothing, and
  `realpath()` answers with `/` on Windows. On Windows, the tests that
  need symbolic links or Unix permissions do not run (upstream skips them
  there too); they pass on Linux (`rust:1.99-slim`, non-root).
- Paths and patterns are bytes (`Vec<u8>`), as PHP strings are.
- Iteration yields `Result<SplFileInfo, Error>`: an `Err` is what PHP
  throws while iterating; each file's key is its pathname.
- `string|array` parameters take anything `Into<Args>`; methods PHP lets
  throw return `Result`; the rest return `&mut Self`.
- Filters and sort callbacks are `Send + Sync` closures.
