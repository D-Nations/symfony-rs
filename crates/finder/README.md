# Symfony Finder

Private workspace crate: a port of `symfony/finder` v8.1.10 (`2f4c0f26`),
as installed by symfony/demo. MIT; Symfony source and tests copyright
Fabien Potencier. No PHP or PCRE source code is used: the PHP built-ins
(`src/php.rs`, `src/date.rs`) and the PCRE pattern translation
(`src/pcre.rs`) are written from documentation and PHP's observed
behaviour. Unofficial, not affiliated with or endorsed by Symfony or PHP.

```rust
use symfony_finder::NumberComparator;

let size = NumberComparator::new(Some(b"> 10Ki")).unwrap();
assert!(size.test(10241));
assert_eq!(size.get_target(), b"10240");
```

| Contract | Source | Evidence |
|---|---|---|
| Comparators: operators, targets as PHP strings, PHP 8 comparison of ints and numeric strings, upstream's messages | `Comparator/*.php` | ComparatorTest (3), NumberComparatorTest (2), DateComparatorTest (2); 63 NumberComparator and 1,156 comparison vectors |
| `preg_match` on delimited patterns: delimiters, modifiers `imsxuADUSXn`, PCRE's `$`, `\Z`, `{` and byte semantics; unsupported features refused | (PHP behaviour) | 137 patterns x 62 subjects recorded from PHP |

Vectors: `scripts/record.sh finder` runs `oracle/recorders/finder_providers.php`
(upstream's provider rows) and `finder_vectors.php` on PHP 8.4.27.

## Differences

- `DateComparator` reads the date subset in `src/date.rs` (ISO dates and
  times, `@timestamp`, `now`/`today`/`yesterday`/..., relative amounts with
  `ago`, `last`, `next`), in UTC. Other formats PHP accepts (month and
  weekday names, time zones, ordinals) are refused with
  `"..." is not a valid date, or not a date format this port supports.`
  (PLAN.md, decisions for the owner).
- PCRE features the translator does not handle are refused with
  `Error::UnsupportedPattern` (recursion, conditionals, verbs, `\K`,
  `\X`, `\C`, branch reset, callouts, `/J`, `/r`, POSIX classes under
  `/u`). PHP's backtracking limit is approximated by `fancy-regex`'s
  (same 1,000,000 figure, counted differently).
