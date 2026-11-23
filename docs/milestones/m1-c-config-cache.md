# M1-C: ConfigCache, its factories and the resource checkers

Stage: the cache classes of symfony/config, with the subset of
symfony/filesystem they need (`dumpFile()` and what it calls). Started
and recorded November 23 on the oracle (PHP 8.4.27, glibc). Config's pin
(v8.1.10) did not move; symfony/filesystem is read at its pin in
`parity/packages.tsv` (v8.1.9, from the demo's lock file).

## 1. What was ported

| PHP file | Rust | Tests |
|---|---|---|
| `ConfigCacheInterface.php`, `ResourceCheckerConfigCache.php`, `ResourceCheckerInterface.php`, `ConfigCache.php` | `symfony_config::config_cache` | ResourceCheckerConfigCacheTest 12, ConfigCacheTest 6 (all) |
| `ConfigCacheFactoryInterface.php`, `ConfigCacheFactory.php`, `ResourceCheckerConfigCacheFactory.php` | `symfony_config::config_cache_factory` | ConfigCacheFactoryTest `n/a` (a non-callable cannot be passed: the callable is a Rust closure by type); 1 Rust test of the factories |
| `Resource/SelfCheckingResourceChecker.php`, `Resource/SkippingResourceChecker.php` | `symfony_config::resource` | through the cache tests |
| symfony/filesystem `Filesystem.php` (`dumpFile()`, `mkdir()`, `tempnam()`, `chmod()`, `rename()`, `readlink()`, `readFile()`), `Path.php` (`makeAbsolute()`, `canonicalize()`, `isAbsolute()` and their helpers), `Exception/*` used | new crate `symfony-filesystem` | FilesystemTest 15, PathTest 12 (27 of 157) |

symfony/config: 307 of 334 ported, 2 `n/a`, 25 pending.
symfony/filesystem: 27 of 157 ported, 130 pending (`parity/symfony/filesystem.tsv`).

Shape:

- Cache metadata is a slice of `Arc<dyn ResourceInterface>`. The meta
  file is PHP `serialize()` text written by php-value; the classes it may
  name are a `ResourceClasses` registry (Config's four filesystem
  resources by default), standing for PHP's autoloader.
  `unserialize()` allows `stdClass` and the registered classes only; an
  unknown class is `__PHP_Incomplete_Class` and makes the cache stale.
- The top-level `a:N:{...}` is written by hand, so that the integer keys
  of a resource's `__serialize()` array stay `i:`.
- `ConfigCache` writes the `.meta.json` beside the meta file with
  upstream's steps: the `Tracking\` prefix, the mangled `\0Class\0`
  names cut, `unserialize()` with no class allowed, `json_encode()`
  with unescaped slashes and unicode, and the `@type` key. A failed
  encode gives `{"resources":Array}`, as PHP's string conversion does.
- SelfCheckingResourceChecker's answers are shared in the process
  (`clear_cache()` empties them, as upstream's tests do by reflection).
- symfony-filesystem has the atomic write as upstream does it (PLAN.md
  stack row "Atomic file writes"): `tempnam()` in the target's directory
  (prefix plus 19 random characters from `getrandom`, `O_EXCL`, 0600;
  PHP's notice and the system temporary directory as fallback), the
  content, the target's mode or `0666 & ~umask()`, `rename()` (a copy on
  `EXDEV`), and the temporary file removed when a step fails. A symbolic
  link is followed. `umask()` is read from `/proc/self/status` on Linux.
- `parse_url()`'s scheme is three-state (`UrlScheme::{Scheme, Null,
  False}`) and moved from symfony-config's FileLocator to
  symfony-filesystem, so that `Path::isAbsolute()` tells null from false
  (`phar:///` gives false, so it is absolute).

## 2. Recorded

`oracle/recorders/config_cache.php` (`scripts/record.sh config-cache`)
writes `crates/config/tests/data/cache.json`; `tests/recorded_cache.rs`
(Unix) replays it in a base directory of the same length as the
recorder's (`/tmp/cc-00000000`), with umask 022:

- 27 writes: meta files, `.meta.json` files, modes and the IOException
  texts, for resources with public, protected, private, nested,
  integer-keyed and non-UTF-8 properties.
- 1,170 `isFresh()` calls: debug on and off, missing, empty, truncated,
  trailing-data, `b:0;`, unknown-class and non-resource meta files, for
  each checker set.
- 39 filesystem calls: `dumpFile()` on new, existing, read-only and
  linked targets (absolute, relative, dangling, `"0"`), `readFile()`,
  `chmod()` and `tempnam()`.

All match but 9, listed in crates/config/README.md ("Differences"):

- 1 write: an object met twice in a `__serialize()` array is written
  again where PHP writes `r:<n>;` (php-value has no references).
- 8 `isFresh()` calls: GlobResource's restore refuses any object as its
  prefix where PHP refuses only a `\Stringable`. This is workstream H's
  GlobResource; it is not changed here.

What the recording found, now ported: PHP 8.3+'s warning on trailing
data after the serialized value makes the cache stale; `b:0;` is a
failure; `file_get_contents()` on a path through a file gives "No such
file or directory"; `tempnam()`'s name shape; a resource with missing
properties fails only when it is checked.

Mutation check: with the meta-file writer or the freshness loop changed,
20 writes and 14 `isFresh()` calls fail.

Oracle notes (cloud session): `dockerd` was started by hand; the image
was built without the extra extensions (deb.debian.org is blocked), and
Composer was replaced by the autoload shim on the upstream checkouts.
The recorder needs no extension.

## 3. Differences and open items

Listed in crates/config/README.md and crates/filesystem/README.md:
TypeError messages lack ", called in <file> on line <n>"; OPcache has no
counterpart; stream wrappers other than `file://`, stream contents and
renaming a directory (upstream's `mirror()` fallback) are refused with
`Class::Unsupported`; the Windows branches of `Path` are not ported.

Pending in Config: ClassExistenceResource, ReflectionClassResource and
ComposerResource (23 tests; PLAN.md decision 17, accepted, a later stage);
Definition/Loader, Definition/Configurator and
Definition/Configuration.php (2 tests).

Advisories: the upstream watch was not run for symfony/filesystem in the
cloud session (no advisory API there); run `scripts/upstream_watch.py`
before the merge.

## 4. Checks

Run locally (GitHub CI was removed); see the PR for the output.
