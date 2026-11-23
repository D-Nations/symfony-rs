# M1-C: Config loaders, FileLocator and filesystem resources

Stage: the non-XML loaders and resources of symfony/config, plus the
Definition tests (Normalization, Merge, Finalization) for code already
ported. Started November 22, recorded and merged November 23 on the
oracle (PHP 8.4.27, glibc). Config's pin did not move.

## 1. What was ported

| PHP file | Rust | Tests |
|---|---|---|
| `Exception/LoaderLoadException.php`, `FileLoaderImportCircularReferenceException.php`, `FileLocatorFileNotFoundException.php` | `symfony_config::exception`; `Class::LoaderLoad`, `Class::FileLoaderImportCircularReference`, `Class::FileLocatorFileNotFound` (`Error::get_paths()`) | LoaderLoadExceptionTest 9 of 9 |
| `FileLocator.php`, `FileLocatorInterface.php` | `symfony_config::file_locator` (`parse_url_scheme()` for `isAbsolutePath()`) | FileLocatorTest 5 of 5 |
| `Loader/LoaderInterface.php`, `LoaderResolverInterface.php`, `DirectoryAwareLoaderInterface.php`, `Loader.php`, `LoaderResolver.php`, `DelegatingLoader.php`, `FileLoader.php`, `GlobFileLoader.php`, `ParamConfigurator.php` | `symfony_config::loader` | LoaderTest 7, LoaderResolverTest 3, DelegatingLoaderTest 5, FileLoaderTest 9 (all) |
| `Resource/ResourceInterface.php`, `SelfCheckingResourceInterface.php`, `FileResource.php`, `FileExistenceResource.php`, `DirectoryResource.php`, `GlobResource.php` | `symfony_config::resource` | FileResourceTest 7, FileExistenceResourceTest 4, DirectoryResourceTest 16, GlobResourceTest 13 of 14 (testPhar `n/a`) |
| (Definition, already ported) | | NormalizationTest 6, MergeTest 5, FinalizationTest 1 |

symfony/config: 289 of 334 ported, 1 `n/a`, 44 pending.

Shape: loaders are `Arc<dyn LoaderInterface>`; Loader and FileLoader are
structs a concrete loader embeds, holding a weak `$this` given by
`Arc::new_cyclic`. FileLoader's static `$loading` is per thread. Paths
are bytes through symfony-finder's `fs` (`/` separators, stream
wrappers). New dependencies: `php-pcre`, `symfony-finder` (workspace)
and `twox-hash` for `hash('xxh128')` (PLAN.md stack row).

symfony-finder gained `fs::glob()`, PHP's `glob()` for files and
directories with or without `GLOB_BRACE`; `glob_dirs()` (Finder's
`in()`) is unchanged and Finder's tests still pass.

## 2. Recorded

`oracle/recorders/config_loaders.php` (`scripts/record.sh
config-loaders`, also run by `scripts/record.sh config`) copies
`crates/config/tests/fixtures` and writes
`crates/config/tests/data/loaders.json`; `tests/recorded_loaders.rs`
replays it on its own copy, every case matching:

- `parse_url($name, PHP_URL_SCHEME)` and `isAbsolutePath()`: 94 names.
- LoaderLoadException: 432 messages (18 resource kinds x 6 previous
  messages x source x type), and 4 circular-reference messages.
- `FileLocator::locate()`: 432 calls (6 path sets x 12 names x 3 current
  paths x first), results, messages and `getPaths()`.
- GlobResource: 1,680 cases (35 patterns x 3 prefixes x recursive x
  forExclusion x 3 excluded sets, with and without `GLOB_BRACE`), paths
  and `__toString()`; `hash('xxh128')` on 6 inputs.
- `FileLoader::import()`: 456 calls (19 resources x 6 exclusions x
  ignoreErrors x type).

What the recording found, now ported:

- `parse_url()`'s scheme: a `:` followed by one to five digits ending the
  string or reaching `/` is a port; after `scheme://` the authority needs
  a host and a numeric port up to 65535; an empty authority is refused
  unless the scheme is `file` and a path follows. `0:` is a scheme, but a
  falsy one for `isAbsolutePath()`.
- glibc's `glob()` matches `.` and `..` with a segment starting with a
  dot, so `/Resource/.*` (recursive) walks the parent directory.
- `realpath()` knows no stream wrappers, so FileResource keeps a
  `file://` path as given; `realpath('')` is the working directory.
- `json_encode(1.0)` in LoaderLoadException is `1`.

## 3. Differences and open items

Listed in crates/config/README.md ("Differences", "Not ported yet"):
`phar://` refused, the iterator's values are paths, `class_exists()` is
supplied by the caller, `glob()` collects before `import()` loads, SPL's
directory-open message not recorded.

Pending: ConfigCache, ResourceCheckerConfigCache, their factories and
the resource checkers (19 tests; symfony/filesystem's `dumpFile()` and
`serialize()` meta files, the next stage); ClassExistenceResource,
ReflectionClassResource and ComposerResource (23 tests; PLAN.md decision
17, for the owner); Definition/Loader, Definition/Configurator and
Definition/Configuration.php (2 tests).

## 4. Checks

Run on the rebased branch; see the PR for the output.
