# Instructions for agents

This repository ports Symfony components to Rust, behaviour for behaviour.
Read [PLAN.md](PLAN.md) first: milestones, decisions, and the reasons for the
stack. These rules say how to do the work.

## Goal

A converted Symfony application must behave like the original: same status
codes, headers that affect behaviour, cookies, messages, error texts and
stored formats. The specification is the pinned upstream source and its
tests (`upstream/`, fetched by `scripts/fetch-upstream.sh`), and the output
of real PHP running it. **Read upstream, never edit it.**

## Per-component loop

1. Before starting a component, search GitHub and crates.io for an existing
   Rust port or primitive. Record options, choice and reason in PLAN.md's
   stack table. Adopt a crate only below the behaviour layer (transport,
   hashing, ids) unless it reproduces Symfony's behaviour on recorded
   vectors.
2. Generate the component's parity manifest (`parity/<package>.tsv`): every
   upstream PHPUnit test method, each `ported`, `n/a: <reason>` or
   `pending`.
3. Port one PHP file at a time. Read the whole file and its tests first.
   Each Rust module's first doc line names the PHP file it ports
   (`//! Port of Symfony\Component\X\Y.php`); `scripts/sources.sh` builds
   `parity/sources.tsv` from those lines.
4. Port the upstream tests in the same change, with the same names and
   assertions. Record vectors from real PHP for what tests leave open
   (a recorder in `oracle/recorders/`, run by `scripts/record.sh`).
5. Update the crate README's contract table. Nothing outside it is claimed.

## Behaviour rules

- No PHP source code in this repository, under any licence (php-src
  included, even its BSD-3-Clause branch; PLAN.md decision 8). PHP's own
  behaviour is learned by running it in a container and from its
  documentation, then written independently. Symfony, Twig and Doctrine
  sources (MIT/BSD) are the specification and are ported as before.

- Copy behaviour, not style. If PHP behaviour is surprising, keep it and pin
  it with a test. If it cannot be kept, stop and record it under "Decisions
  for the owner" in PLAN.md rather than choosing silently.
- Use upstream's text for messages; never retype or reword one.
- What PHP does by reading config or reflecting at run time is done at build
  time (`build.rs`, proc-macros). Nothing is parsed per request beyond what
  the request itself needs.
- Unsupported input at build time is refused with a location, never
  approximated.

## Code rules

- Security before speed and features. Prefer fewer dependencies.
- SQL through sqlx with bound parameters only.
- No `unsafe`. No `unwrap`/`expect` on anything derived from a request, a
  database, a file or the network; return an error.
- Enums for closed sets, with each value's string form defined once.
- For compatibility problems (paths, OS differences, encodings), use the
  standard, idiomatic Rust solution, as long as it does not disturb the
  port's progress (owner, November 24). Do not solve one again in each
  crate.
- File paths: keep PHP's bytes for every path PHP shows (messages, return
  values, keys), and use `std::path::Path`/`PathBuf` for every system call.
  Convert only with `php_value::path` (`to_path`, `from_path`, …), and use
  its `basename()`/`dirname()`; do not add a crate's own copy or
  `#[cfg(unix)]` conversion.
- No new dependency without a PLAN.md stack row saying what it was compared
  with and why it won.
- Every crate keeps `publish = false` until the owner decides naming and
  publication (PLAN.md decision 1).

## Checks

Run before calling any step done, and report the real output:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
scripts/sources.sh --check
scripts/parity-check.sh
```

The symfony-demo-rs step (that repository's checks and page comparison
against this checkout, for a change to a crate it uses) is paused by the
owner (November 23): do not run it until the owner turns it back on. Stage
logs say it was not run; acceptance criteria that name these comparisons
stay open meanwhile. When it runs, an unexplained difference is a failure,
not a note. Do not weaken, skip or
delete a test or comparison rule to get a pass; if a check is wrong, say so
and ask. Debug builds only.

Tests that only run on Unix are run on a Windows host with
`scripts/linux-test.sh [-p CRATE...]`, in a removed Linux container. There
is no hosted CI for these checks (owner: no paid CI jobs), so they run
locally before every merge.

## Parallel sessions

Several sessions may work on this repository at once. To keep them apart:

- Work in your own git worktree beside the repository, never in the shared
  main checkout: `git worktree add -b <branch> ../symfony-rs-wt-<name>
  origin/main`. Fill its ignored `upstream/` with `scripts/fetch-upstream.sh`
  (php-src is not fetched; never read it).
- Pick a stage whose crates no open branch is changing (check
  `gh pr list` and the remote branches first). Say in the PR which crates
  it touches.
- Before merging, `git fetch && git rebase origin/main`. Conflicts in
  PLAN.md, `parity/sources.tsv` and `Cargo.lock` usually need both sides
  kept; regenerate `sources.tsv`. Re-run the full checks after the rebase
  only if it brought in code (crates, `Cargo.*`, parity manifests, the
  check scripts); for documentation-only changes, `scripts/sources.sh
  --check` and `scripts/parity-check.sh` are enough (owner, November 23).
- symfony-demo-rs comparisons run one at a time across all sessions: they
  share that checkout, its `Cargo.toml` `[patch]` edit and the
  `compare-original` container. Before starting, check that no
  `compare-original` container is running and that `Cargo.toml` there has
  no `[patch]` edit. Point the patch at your worktree, confirm with
  `cargo tree` that the demo builds against it, revert the edit when done,
  and commit nothing there.
- A control run (the comparison on the demo's pinned revision) depends only
  on the symfony-demo-rs commit, the symfony-rs revision its `Cargo.toml`
  pins, and the `compare-original` image id. Reuse the latest
  `compare/out/run-control-*` whose three values still match, and record
  them in the stage log; run a new control only when one has changed.
- Stop only the containers and processes you started.

## Running PHP

- Never install PHP, Composer or PHP extensions on the host. PHP runs only
  in a container, removed when it exits (`--rm`).
- Anything that loads the pinned packages or produces a vector or a test
  expectation runs in the oracle image (`oracle/`): its PHP version, libc
  and extensions are what the recorded vectors mean. Add to that image only
  what a recorder needs.
- A quick check of plain PHP behaviour that loads no packages may use the
  smallest official image of the pinned version instead, e.g.
  `docker run --rm -i php:8.4.27-cli-alpine php < probe.php`. Its answer is
  a lead, not evidence: confirm in the oracle before relying on it.
- Mount repositories read-only. Probe scripts go through stdin or into the
  ignored `oracle/out/`, never into `upstream/` or `../symfony-demo`.

## Upstream advisories

Every advisory the upstream watch lists for a ported package gets a row in
`parity/advisories.tsv`: `covered` (name the Rust test that pins the fixed
behaviour, ideally the fix's own upstream test), `not-ported` (say which
milestone must revisit it) or `affected` (fix it first; it blocks other
work). Check a `covered` verdict against the oracle: the pre-fix PHP should
fail the test's inputs.

## Housekeeping

- Commit after each ported file or coherent step, naming the PHP file
  (`feat(HeaderBag.php): ...`). Feature branches and reviewable PRs; do not
  change repository visibility or publish crates without the owner.
- Never commit `target/`, `upstream/` or `oracle/out/`.
- Test fixtures taken from a third party (not Symfony upstream) keep that
  party's licence file next to them and are named in the crate README.
- A test that reads `upstream/` skips, with a note on stderr, when the
  checkout is not fetched.
- Stop oracle containers when finished; remove superseded images by hand.
- Keep the MIT licence and Symfony copyright lines. No wording that suggests
  Symfony endorses this work.
