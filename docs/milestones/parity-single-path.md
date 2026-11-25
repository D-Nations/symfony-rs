# Parity test-path guard and YAML unreadable-file case

November 24, 2026. Base: `2b04f94`; implementation: `d9344bc`.
Worktree: `../symfony-rs-wt-parity-gap`; branch: `fix/parity-single-path`.

## Scope and resulting behavior

Only **symfony-yaml tests and its contract** are touched among crates.
No runtime source, dependency, Cargo profile or lockfile changes.

Both the Bash and Python parity validators previously required another
space after the first mapped source path. A single `Tests/...` mapping
therefore bypassed the check that every upstream-named Rust test has a
ported manifest claim. The corrected patterns recognize a test path in
any position, including a single path, nested directories, lowercase
`tests/` and horizontal whitespace, without matching unrelated lines or
production directories such as `NotTests/`.

Regression tests compare both validators on these inputs. A read-only
negative control restores the old YAML manifest row in memory and confirms
that the corrected checker rejects exactly the formerly missed function.

## Real unreadable-file test

Pinned upstream: symfony/yaml v8.1.9, `6107ee26a25215c3df3af0a3315e56bb4fd0b7db`.
Read `Parser::parseFile()`, ParserTest's unreadable-file method, and its
setup/cleanup. The oracle vendor copy was verified against that checkout
with `scripts/fetch-upstream.sh --verify ../symfony-demo/vendor symfony/yaml`.

The old `testParsingNotReadableFilesThrowsException` replay had a skipped
PHP row with no calls. It is replaced by a live Unix test under the same
upstream name: copy the existing fixture into a private temporary
directory, prove it parses, chmod to upstream's 0200, require a real
PermissionDenied from opening it, assert ParseException and upstream's
unchanged message regex plus the exact PHP-recorded message, restore 0644
and prove it parses again. A guard removes the private directory on error
or panic. Shared crate/upstream fixtures are never chmodded.

`oracle/recorders/yaml_unreadable.php` runs in the existing pinned PHP
8.4.27 oracle as UID/GID 1000, with repositories mounted read-only.
`scripts/record.sh yaml-unreadable` records readable, unreadable and restored
results into `crates/yaml/tests/data/unreadable_file.json`; normal
`yaml`/`all` recording includes it too. Temporary paths are normalized,
but Symfony's message is otherwise unchanged. Repeated recordings are
byte-for-byte identical (SHA256
`737debba2b4fc0f6f017b0898a114c9608301fd13893594f334af15162feff81`).
A root control fails explicitly instead of producing a skipped vector.

The manifest row now names the real Unix test as ported. ParserTest is
172/172; the package is 341 ported, 1 n/a, 0 pending out of 342 upstream
methods. The remaining n/a belongs to InlineTest and is unchanged.

Windows does not compile this Unix permission test, matching upstream's
Windows restriction. Linux acceptance runs it as an unprivileged user.
Consequently Windows has one fewer listed Rust test than before: the old
assertion-free replay no longer reports a fictitious pass there.

## Verification

All checks below ran against implementation d9344bc, in debug profiles.

| Check | Actual result |
|---|---|
| python scripts/dev_check.py full (Windows) | exit 0 |
| Python tooling discovery | 26 passed |
| cargo fmt --all --check | exit 0 |
| cargo clippy --locked --workspace --all-targets -- -D warnings | exit 0 |
| cargo test --locked --workspace --no-fail-fast (Windows) | 2,717 passed, 0 failed, 0 ignored |
| scripts/sources.sh --check (batch backend) | exit 0; sources.tsv unchanged |
| scripts/parity-check.sh (batch backend) | exit 0 |
| scripts/parity-check.sh (standalone Bash implementation) | exit 0; same package summaries |
| scripts/linux-test.sh (fresh complete workspace) | exit 0; formatting/Clippy pass; 2,728 tests passed, 0 failed, 0 ignored; 437.30 s wall time |
| Linux live unreadable-file test | passed directly and again in the full suite |
| Non-root PHP recording, repeated | byte-for-byte identical |
| Root PHP recording control | nonzero exit; unreadability precondition refuses root |
| In-memory old manifest control | exit 1 on exactly the formerly missed YAML test, as intended |
| bash -n scripts/record.sh scripts/parity-check.sh | exit 0 |

Logs are ignored under oracle/out/: parity-gap-host.log,
parity-gap-linux.log, parity-gap-bash.log, parity-gap-root-control.log
and parity-gap-manifest-control.log. Cargo totals include unit, integration
and doc-test result groups. The new live Unix case is explicitly present
as passed in the Linux log; it no longer consults the legacy skipped row.
The Windows test command took 332.97 s in this new worktree.

The base has no open conflicting PRs. No new dependency/stack selection
was needed for this existing-component test fix. All oracle and Linux
containers started for this task exited and were removed.

symfony-demo-rs checks/page comparisons are not run, as paused by the owner;
their acceptance criteria remain open. Only this YAML recorder is rerun;
other committed oracle vectors are unchanged. No paid CI or new images
were added; oracle containers use --rm.

