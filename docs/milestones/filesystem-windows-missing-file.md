# Windows missing-file warning fix (November 24, 2026)

Scope: `symfony-filesystem` only (`src/php.rs` and the crate contract).
No dependency, public API, source mapping or parity status changes.

## Cause and change

On Windows, `std::io::Error` displays a missing file as "The system cannot
find the file specified.". The PHP warning expected by pinned
`FilesystemTest.php::testReadNonExistentFile` is "No such file or directory".
The existing Rust test failed before this fix with the reported mismatch.

`file_get_contents()` now handles `ErrorKind::NotFound` alongside its
existing `NotADirectory` mapping. This uses the standard Rust error kind,
keeps the original path bytes and preserves the existing exception text and
test assertions. It covers both a missing file and a missing parent path.
Other error handling is unchanged.

## Evidence

- Base: `origin/main` at `2612c63`; implementation commit: `b20e92f`.
- Upstream: symfony/filesystem v8.1.9, commit
  `43d109c803d22c00bb820ed45df250b6a8792ca2`.
- Windows before: the exact `testReadNonExistentFile` rerun failed (1 failed).
- Windows after: `cargo test -p symfony-filesystem` passed: 10 Filesystem
  tests and 12 Path tests, 0 failures.
- `scripts/sources.sh --check`: passed (exit 0), including a rerun after all pinned upstream checkouts were fetched.
- `scripts/parity-check.sh`: passed (exit 0).
- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test` (full Windows workspace, pinned fixtures fetched): passed,
  2,712 tests, 0 failures, 0 ignored (exit 0).
- `scripts/linux-test.sh -p symfony-filesystem`: passed against `b20e92f`:
  formatting, Clippy and 27 tests (15 Filesystem, 12 Path), 0 failures.
  The script used its pinned Rust image and an unprivileged user in a
  removed container; the pinned upstream checkout was mounted read-only.
- PHP oracle: `symfony-rs-oracle:8.4.27`, image `6593774ab6ae`.
  Pinned Symfony's `readFile()` on the nonexistent `Tests/invalid` returned:
  `Failed to read file "/work/upstream/symfony/filesystem/Tests/invalid": file_get_contents(/work/upstream/symfony/filesystem/Tests/invalid): Failed to open stream: No such file or directory`.
  The probe asserted that suffix and exited 0, with the worktree mounted
  read-only and the container removed on exit.

The symfony-demo-rs checks and page comparisons were not run: they remain
paused by the owner. No dependent application acceptance gate is closed.
The first full Windows run stopped in Finder because its upstream fixtures
were absent (65 fixture-setup failures). All pinned upstream checkouts were
then fetched with `scripts/fetch-upstream.sh` before rerunning the workspace
tests.
