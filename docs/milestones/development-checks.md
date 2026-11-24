# PR #46: development checks and Linux parity runs

Verified November 24, 2026 (Asia/Tokyo), in the isolated
`../symfony-rs-wt-pr46` worktree on `review/pr46-validated`.

## Scope

Original PR head: `945fb2227889fddcbd18c3eda907cbf1eedb6991`.
Current main `c9288fd` was merged without rewriting the original PR commits.
The only crate edits are rustfmt changes in **php-value** and
**symfony-options-resolver**, inherited from main's failing format check.
No runtime behavior, dependency, lockfile, source mapping or parity status
was changed.

The runner finds Git Bash on Windows, reports command timings, runs the
tooling tests in the full gate and still runs doctests after nextest fails.
Affected selection includes committed, staged, unstaged and untracked paths;
regressions cover reverse-dependency cycles and optional/build/dev/target
path dependencies.

Source/parity scripts opt into a standard-library Python batch backend
through the runner's interpreter. The original Bash implementations remain
available when SYMFONY_RS_PYTHON is unset. Fixture tests compare exit codes,
summaries, diagnostics and generated source maps, including deliberately
broken and malformed manifests.

The Linux runner uses a prepared image based on the existing pinned Rust
digest. It caches tool installation; its default acceptance mode still has
empty Cargo downloads/build directories. --cached explicitly reuses two
named volumes per image recipe and worktree for development. All checks run
as an unprivileged user, with archived HEAD and read-only upstream mounts;
containers use --rm. No global Cargo/environment settings were changed.

## Verification

Host: Windows, Rust/Cargo 1.99.1, Python 3.15, Git Bash 5.3.15,
Docker Desktop 4.99.0 / Engine 29.9.3. Pinned upstream fetched with
scripts/fetch-upstream.sh, without php-src.

Rust acceptance tree: `09fa4b2`. Follow-up `14a43d3` only tightens the
Python backend's reproduction of Bash's malformed-TSV parsing and adds two
negative subcases; all tooling tests and source/parity checks were rerun.

| Check | Actual result |
|---|---|
| python scripts/dev_check.py full | exit 0 |
| Tooling unittest discovery | 24 tests passed; final run 9.80 s |
| cargo fmt --all --check | exit 0 |
| cargo clippy --locked --workspace --all-targets -- -D warnings | exit 0 |
| cargo test --locked --workspace --no-fail-fast (Windows) | 2,716 passed, 0 failed, 0 ignored; 323.03 s |
| scripts/sources.sh --check, batch backend | exit 0; generated map matches parity/sources.tsv |
| scripts/parity-check.sh, batch backend | exit 0; summaries match Bash reference |
| bash scripts/linux-test.sh, fresh whole workspace | exit 0; format and Clippy pass; 2,726 tests passed, 0 failed, 0 ignored; 375.07 s wall time |
| Scoped check -p php-value --timings | exit 0; Cargo HTML timing report generated |
| cargo test --locked -p php-value --profile dev-full --no-run | exit 0; full-symbol test binaries built |
| python scripts/dev_check.py audit | exit 0 |
| Bash syntax and invalid Linux runner arguments | syntax passes; invalid options/package text exit 2 before Docker |
| Cached Linux filesystem checks | exit 0 twice; all 27 filesystem/path tests rerun |

The Windows/Linux counts include unit, integration and doc-test result
groups. Both replay the complete fetched MIME upstream set and the other
committed PHP vectors. Existing recorded skips inside replay harnesses
are not Cargo ignored tests (see the gap below).

nextest is not installed: its failure-then-doctest control flow is covered
by runner tests, but the optional nextest binary was not exercised.

Raw logs and Cargo timing artifacts remain ignored under oracle/out/ and
target/. All containers started for these checks exited and were removed.
Only this worktree's two development cache volumes are retained.

## Observed timings

These are illustrative local readings, with concurrent verification work,
not controlled benchmark results or claims about total compile speed.

| Workload | Original / first run | Batch / warm run |
|---|---:|---:|
| Source-map check | original Bash 43.65 s | batch 1.17 s; later warm 0.24-0.25 s |
| Parity manifest check | original Bash 60.41 s | batch 2.19 s; later warm 0.36-0.39 s |
| Cached Linux filesystem suite at the same HEAD `95599d1` | empty cache and image preparation 20.22 s | populated cache 8.68 s |

Cache reuse does not suppress tests or reuse PHP answers. The full fresh
Linux suite above was run independently of the cached development tests.
The default line-table profiles were retained; no before/after compile-time
claim is made for those settings.

## Existing parity-rule gap awaiting owner decision

The original Bash test-mapping pattern requires an additional space after
the first source path, so a single `Tests/...` path is not recognized for
the unclaimed-test check. The batch backend deliberately preserves that
rule until the owner approves a correction, as AGENTS.md requires asking
when a check is wrong.

A read-only audit with corrected detection exposes one existing case:
`crates/yaml/tests/parser_test.rs::testParsingNotReadableFilesThrowsException`.
The manifest marks it n/a because the recording was skipped as root/on
Windows. Its recording contains no calls and its Rust replay returns
success without assertions. Fixing detection alone would therefore fail
the gate; the unreadable-file case needs a real test/expectation before
being claimed as ported. No test was deleted, skipped or relabeled here.

symfony-demo-rs checks/page comparisons were **not run**, as paused by the
owner. Their acceptance criteria remain open. Oracle recordings were not
regenerated for this tooling-only change.

