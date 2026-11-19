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
   assertions. Record vectors from real PHP for what tests leave open.
5. Update the crate README's contract table. Nothing outside it is claimed.

## Behaviour rules

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
```

Then, for any change to a crate symfony-demo-rs uses, run that repository's
checks and page comparison against this checkout (its `[patch]` override).
An unexplained difference is a failure, not a note. Do not weaken, skip or
delete a test or comparison rule to get a pass; if a check is wrong, say so
and ask. Debug builds only.

## Housekeeping

- Commit after each ported file or coherent step, naming the PHP file
  (`feat(HeaderBag.php): ...`). Feature branches and reviewable PRs; do not
  change repository visibility or publish crates without the owner.
- Never commit `target/`, `upstream/` or `oracle/out/`.
- Stop oracle containers when finished; remove superseded images by hand.
- Keep the MIT licence and Symfony copyright lines. No wording that suggests
  Symfony endorses this work.
