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

Upstream pins are each package's exact version and reference from the demo's
`composer.lock`, recorded in `parity/packages.tsv`; split Symfony packages
can have different versions. The M0 findings in [PLAN.md](PLAN.md#m0-progress)
explain why a single monorepo tag is insufficient.

## Working on it

M1 also contains `symfony-event-dispatcher` and its contracts, `psr-log`,
`symfony-stopwatch`, `php-value`, `symfony-yaml`, `symfony-dotenv`,
`symfony-deprecation-contracts` and `symfony-config`. Each crate's README
states its own contract and remaining work.

Read [AGENTS.md](AGENTS.md) and [PLAN.md](PLAN.md). Checks:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
scripts/sources.sh --check
scripts/parity-check.sh   # manifest claims and completion status
```

Pinned upstream sources (with their tests), for porting and for parity
manifests: `scripts/fetch-upstream.sh` clones them into the ignored
`upstream/` directory. `scripts/manifest.sh <package>` (re)generates
`parity/<package>.tsv`, the list of every upstream test with its status,
using the PHP oracle in `oracle/` (Docker).

Manifest snapshot, November 19, 2026 (test methods, not a compatibility
percentage; later changes are authoritative in `parity/`):

| Package | Upstream tests | Ported | N/A | Pending |
|---|---:|---:|---:|---:|
| symfony/config | 334 | 75 | 0 | 259 |
| symfony/dotenv | 57 | 38 | 0 | 19 |
| symfony/event-dispatcher | 140 | 114 | 0 | 26 |
| symfony/http-foundation | 945 | 1 | 0 | 944 |
| symfony/http-kernel | 1,040 | 10 | 0 | 1,030 |
| symfony/routing | 871 | 18 | 0 | 853 |
| symfony/stopwatch | 33 | 33 | 0 | 0 |
| symfony/yaml | 342 | 329 | 2 | 11 |

M0 tooling is recorded complete; M1 foundation work is underway. Config's
Definition/Builder step is the current implementation work. Stopwatch is
the only package currently listed in `parity/done.txt`. Deferred DI and
Console integrations keep their tests pending.

The maintained [PLAN.md](PLAN.md#delivery-order-and-deferred-integrations)
sets dependency stages: an early converter fixture and second-app inventory,
request-parser provenance gate, and separate XML, Twig and Doctrine
acceptance stages. These additions are planned work, not completed results.

Test vectors from real PHP: `scripts/record.sh routing|multipart|all`
(recorders in `oracle/recorders/`). Upstream security: `python
scripts/upstream_watch.py` (weekly in CI) lists advisories and newer patch
releases for the pinned packages; `parity/advisories.tsv` holds the triage.

## Licence

MIT, see [LICENSE](LICENSE). Code derived from Symfony keeps Fabien
Potencier's copyright. Code derived from Twig (BSD-3-Clause) will carry
Twig's notice in its own crate.
