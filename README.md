# symfony-rs

Rust crates that behave like Symfony components, proven against Symfony
itself. They are the runtime that converted Symfony applications link
against (php2rust's planned `symfony2rust`), and they are grown component by
component from the [symfony-demo-rs](https://github.com/D-Nations/symfony-demo-rs)
port, which stays their first consumer and acceptance test.

**Unofficial.** Not affiliated with or endorsed by Symfony SAS or the Symfony
project. "Symfony" is a trademark of Symfony SAS; the crate names are working
names for a private repository (see [PLAN.md](PLAN.md), decision 1).

## Status

Every crate is a documented, tested *subset*. Its README lists exactly which
contracts it supports, the PHP source each comes from, and the evidence.
Nothing outside those tables is claimed.

| Crate | Symfony package | Contract |
|---|---|---|
| [`symfony-http-foundation`](crates/http-foundation/README.md) | `symfony/http-foundation` (+ PHP request parsing) | Query, cookie and multipart parsing, method/format selection, Accept-Language, trusted HTTPS scheme |
| [`symfony-routing`](crates/routing/README.md) | `symfony/routing` | Typed registrations, matcher and redirectable matcher, URL escaping |
| [`symfony-http-kernel`](crates/http-kernel/README.md) | `symfony/http-kernel` | Request, controller, response and exception hooks with priorities |
| [`request-lab`](examples/request-lab/README.md) | — | Independent example application using the three crates |

Upstream pins: Symfony `v8.1.10`, Twig `v3.30.0`, Doctrine ORM `3.7.5` /
DBAL `4.5.0`, Symfony UX `3.6.0` (from the demo's `composer.lock`).

## Working on it

Read [AGENTS.md](AGENTS.md) and [PLAN.md](PLAN.md). Checks:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
scripts/sources.sh --check
scripts/parity-check.sh   # source map is current
```

Pinned upstream sources (with their tests), for porting and for parity
manifests: `scripts/fetch-upstream.sh` clones them into the ignored
`upstream/` directory. `scripts/manifest.sh <package>` (re)generates
`parity/<package>.tsv`, the list of every upstream test with its status,
using the PHP oracle in `oracle/` (Docker).

| Package | Upstream tests | Ported | Pending |
|---|---:|---:|---:|
| symfony/event-dispatcher | 140 | 0 | 140 |
| symfony/http-foundation | 945 | 1 | 944 |
| symfony/http-kernel | 1,040 | 10 | 1,030 |
| symfony/routing | 871 | 18 | 853 |

The crates' contracts are subsets; the pending rows are the work list.

Test vectors from real PHP: `scripts/record.sh routing|multipart|all`
(recorders in `oracle/recorders/`). Upstream security: `python
scripts/upstream_watch.py` (weekly in CI) lists advisories and newer patch
releases for the pinned packages; `parity/advisories.tsv` holds the triage.

## Licence

MIT, see [LICENSE](LICENSE). Code derived from Symfony keeps Fabien
Potencier's copyright. Code derived from Twig (BSD-3-Clause) will carry
Twig's notice in its own crate.
