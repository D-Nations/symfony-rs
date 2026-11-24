# Symfony Filesystem

Private workspace crate: a port of the part of `symfony/filesystem`
v8.1.9 (`43d109c8`), as installed by symfony/demo, that Config's caches
use. MIT; Symfony source and tests copyright Fabien Potencier. No PHP
source code is used: the PHP built-ins (`src/php.rs`) are written from
documentation and PHP's recorded answers. Unofficial, not affiliated with
or endorsed by Symfony or PHP.

```rust
use symfony_filesystem::{Filesystem, Path};

assert_eq!(Path::canonicalize(b"/css/../style.css").unwrap(), b"/style.css");
assert!(Path::is_absolute(b"vfs://root/file.txt"));
let _ = Filesystem::new();
```

| Contract | Source | Evidence |
|---|---|---|
| `dumpFile()`: the atomic write (temporary file from `tempnam()` in the target's directory, content, the target's mode or `0666 & ~umask()`, `rename()` over the target, the temporary file removed on failure), missing directories created, symbolic links followed (absolute, relative, chains, dangling; a link whose target is `"0"` replaced), `file://` URLs; upstream's IOException and InvalidArgumentException messages with PHP's warnings | `Filesystem.php` | FilesystemTest's dumpFile tests (5); recorded `filesystem` (22 dumpFile calls) |
| `readFile()` (including "No such file or directory" for missing files on Windows), `chmod()` (mode and umask, several files), `mkdir()`, `rename()`, `readlink()`, `tempnam()` (local and `file://` directories, the 63-character prefix, the system's temporary directory as fallback) | `Filesystem.php` | FilesystemTest (10 more); recorded `filesystem` (17 calls) |
| `Path::canonicalize()`, `normalize()`, `getRoot()`, `isAbsolute()` / `isRelative()` (`parse_url()` giving anything but null), `makeAbsolute()`, `getHomeDirectory()` | `Path.php` | PathTest (12 of its tests, the rows for `/` as `DIRECTORY_SEPARATOR`) |
| Exceptions: IOException (with `getPath()`), InvalidArgumentException, RuntimeException | `Exception/*.php` | the tests above |

## Rust shape

- Paths and contents are bytes. `DIRECTORY_SEPARATOR` is `/` on every
  system (Linux behaviour is canonical, as in symfony-finder): Path's
  Windows branches (`\`, `C:`) are not ported. On Windows a mode only
  sets or clears the read-only attribute, as PHP's `chmod()` does there,
  and `umask()` is 0.
- `umask()` is read (from `/proc/self/status` on Linux, from a probe file
  elsewhere on Unix), never set: Rust has no safe call for it. So
  testDumpFile expects `0666 & ~umask()` for the umask the tests run with.
- Path reads `HOME`, `HOMEDRIVE` and `HOMEPATH` through an `Env` (the
  process's by default) because Rust cannot change its environment
  safely; upstream's tests use `putenv()`. Its canonical-path buffer is a
  cache only and is not ported.
- `tempnam()` names are the prefix and 19 random characters (13 from
  `[a-z0-9]`, 6 from `[a-zA-Z0-9]`), the form PHP 8.4's take, from the
  OS random source (`getrandom`); files are created with `O_EXCL` and
  mode 0600.
- Missing files and paths through a file (`a.txt/x`) read as "No such
  file or directory" on every host, as the pinned PHP tests expect.
  Other failure reasons retain the host's native error text.
- Content is a string; a stream resource has no counterpart.
- Refused with `Class::Unsupported`: stream wrappers other than `file://`
  (`compress.zlib://`, `gs://`, user wrappers) and renaming a directory
  (upstream's `mirror()` fallback).

## Not ported yet

`copy()`, `exists()`, `touch()`, `remove()`, `chown()`, `chgrp()`,
`symlink()`, `hardlink()`, `mirror()`, `makePathRelative()`,
`isAbsolutePath()`, `appendToFile()`, `tempnam()` with a suffix or another
stream wrapper, the rest of Path, and ExceptionTest: 130 upstream tests
pending in `parity/symfony/filesystem.tsv`.

Vectors: `scripts/record.sh config-cache` records the Filesystem calls
with Config's caches (`crates/config/tests/data/cache.json`, replayed by
`crates/config/tests/recorded_cache.rs`).
