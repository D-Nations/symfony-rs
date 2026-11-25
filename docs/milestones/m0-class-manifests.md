# Class manifests

November 25, 2026. Base: `1e5b0f4`. Branch: `claude/project-thread-strajz`.

## Why

The test manifests (`parity/<package>.tsv`) count upstream test methods.
They do not say how much of the upstream code has a Rust port, so the
progress figure of November 24 (about 8%) was an estimate. The class
manifests give that figure from data.

Before the work: main and the remote branches were checked for manifest
work (`m0/crate-manifests` and `m0/missing-manifests` are merged, so every
pinned package already has a test manifest). On the owner's computer the
only other worktree with manifest changes is `symfony-rs-wt-converter`
(an uncommitted psr/container pin); it is not touched, and psr/container
gets no manifest here.

## What

- `oracle/classes.php`: reads the paths of a package from standard input
  and prints `<path>\t<classes>\t<code lines>` with PHP's tokenizer
  (nothing loaded). Code lines are lines with a token other than
  whitespace, comments, docblocks and the open tag. Class names are the
  declared classes, interfaces, traits and enums; a name that is not
  UTF-8 (symfony/cache's `\xA9` class in `Traits/ValueWrapper.php`) is
  written with `\x` escapes.
- `scripts/classes.py`:
  - `generate PACKAGE...|--all`: the file list is the package's dist
    archive (`git archive HEAD` of the pinned checkout, which honours
    `.gitattributes` export-ignore), without top-level `Tests/`, `tests/`
    and `vendor/`. So twig/twig's `extra/` (the separate twig/*-extra
    packages), doctrine's `ci/` and `static-analysis/`, and intl's
    `Resources/bin` build scripts are out, as Composer installs them.
    Runs the oracle (Docker) or `SYMFONY_RS_PHP`. Keeps hand-set `n/a`
    rows.
  - `sync`: sets `ported` / `pending` and the Rust files from
    `parity/sources.tsv`, with no PHP and no `upstream/`. Run it after
    `scripts/sources.sh` (AGENTS.md step 3).
  - `check`: every pinned package has a manifest with the pinned
    reference; statuses are known; `ported` rows are exactly the files
    `parity/sources.tsv` maps (test paths in sources.tsv are skipped).
    `scripts/dev_check.py parity` (and `full`) runs it.
  - `report [--markdown]`: files and code lines, ported and total, per
    package, for all files and without `Resources/data/`.
- `scripts/parity-check.sh` and `scripts/parity_check.py` skip
  `parity/classes/`.
- `scripts/test_classes.py`: check, sync, report and the skip in both
  test-manifest checkers.

## Result

69 manifests, 5,771 files. Code lines of mapped files:

| Scope | Files | Ported files | Code lines | Ported lines | Share |
|---|---:|---:|---:|---:|---:|
| All | 5,771 | 435 | 700,315 | 37,486 | 5.4% |
| Without `Resources/data/` | 4,462 | 433 | 305,584 | 34,798 | 11.4% |
| Symfony, without data | | | 221,712 | 34,724 | 15.7% |
| Twig | | | 17,514 | 0 | 0% |
| Doctrine | | | 66,266 | 0 | 0% |

`Resources/data/` is generated tables: symfony/intl's ICU data alone is
391,884 code lines in 1,306 files, more than all other packages together.

The share is an upper bound: a file is `ported` when one Rust module
names it, also when the module ports part of it (for example the
deferred Config resources, decision 17). The test manifests show what is
still open in those files. Per package: `python scripts/classes.py report`.

## Checks

- `python -m unittest discover -s scripts -p "test_*.py"`: 35 tests, OK.
- `bash scripts/sources.sh --check`, `bash scripts/parity-check.sh`,
  `python scripts/classes.py check`: pass.
- No Rust change, so cargo fmt, clippy and test were not run again.
- Not run here: `generate` through Docker (the manifests were made with a
  local PHP 8.3 tokenizer; the oracle has 8.4.27). The tokens used are the
  same in both versions; a Docker regeneration should give the same files.

## Note for open branches

A branch that adds `//! Upstream:` lines must also run
`python scripts/classes.py sync`, or `classes.py check` fails after the
merge with "is pending but ... ports it; run scripts/classes.py sync".
