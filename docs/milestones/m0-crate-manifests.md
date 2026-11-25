# Manifests for crates without one

November 25, 2026. Base: `7828feb`. Worktree: `../symfony-rs-wt-manifests`;
branch: `m0/crate-manifests`.

## Scope

Three packages that have a crate in this repository had no parity manifest.
No crate, generator or check script changes.

| Package | Crate | Pin | Upstream tests |
|---|---|---|---:|
| psr/log | `crates/psr-log` | 3.0.2 `f16e1d5` | 0 |
| symfony/deprecation-contracts | `crates/deprecation-contracts` | v3.7.1 `f3202fa` | 0 |
| symfony/event-dispatcher-contracts | `crates/event-dispatcher-contracts` | v3.7.1 `c7de7a0` | 0 |

Generated with `scripts/manifest.sh` on checkouts from
`scripts/fetch-upstream.sh`. None of the three has a `Tests/` or `tests/`
directory at its pin (psr/log 3.x dropped its `Test/` helpers, which were
not PHPUnit tests either), so `oracle/manifest.php` lists nothing and each
manifest is its header line alone. That header-only form is the convention
for packages without upstream tests (PLAN.md, M0 progress).

The three crates have no tests directory and no `//! Upstream:` line naming
a test path, so no row is `ported`. `parity/done.txt` is unchanged.

## Checks

See the PR description for the real output of `python scripts/dev_check.py
full`.

The symfony-demo-rs comparison was not run (paused by the owner, November 23).
