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

Manifest snapshot, November 23, 2026 (test methods, not a compatibility
percentage; later changes are authoritative in `parity/`):

| Package | Upstream tests | Ported | N/A | Pending |
|---|---:|---:|---:|---:|
| symfony/config | 334 | 307 | 2 | 25 |
| symfony/console | 1,152 | 320 | 11 | 821 |
| symfony/css-selector | 85 | 80 | 5 | 0 |
| symfony/dotenv | 57 | 57 | 0 | 0 |
| symfony/event-dispatcher | 140 | 114 | 0 | 26 |
| symfony/filesystem | 157 | 27 | 0 | 130 |
| symfony/finder | 143 | 139 | 4 | 0 |
| symfony/http-foundation | 959 | 193 | 5 | 761 |
| symfony/http-kernel | 1,045 | 10 | 0 | 1,035 |
| symfony/options-resolver | 234 | 234 | 0 | 0 |
| symfony/routing | 896 | 18 | 0 | 878 |
| symfony/stopwatch | 33 | 33 | 0 | 0 |
| symfony/string | 266 | 235 | 1 | 30 |
| symfony/translation | 345 | 55 | 2 | 288 |
| symfony/yaml | 342 | 340 | 2 | 0 |
| **Total** | **6,188** | **2,162** | **32** | **3,994** |

Six packages have no pending rows and are listed in `parity/done.txt`:
Stopwatch, Yaml, Dotenv, CssSelector, OptionsResolver and Finder. M1 is
underway: Config has its Definition, Builder, dumpers, XmlUtils, loaders,
filesystem resources and ConfigCache ported (the reflection-based
resources, decision 17, are a later stage); String, Console (the subset the Yaml and
Dotenv commands use), Translation's XLIFF and Qt loaders and the XML
gates are partial. M2 has started with HttpFoundation. symfony/filesystem
has only the `dumpFile()` subset that ConfigCache needs. Deferred DI and
Console integrations keep their tests pending. Per-stage detail is in
[PLAN.md](PLAN.md#m1-progress) and `docs/milestones/`.

The maintained [PLAN.md](PLAN.md#delivery-order-and-deferred-integrations)
sets dependency stages: an early converter fixture and second-app inventory,
request-parser provenance gate, and separate XML, Twig and Doctrine
acceptance stages. These additions are planned work, not completed results.

Test vectors from real PHP: `scripts/record.sh <recorder>|all`
(recorders in `oracle/recorders/`). Upstream security: `python
scripts/upstream_watch.py` (weekly in CI) lists advisories and newer patch
releases for the pinned packages; `parity/advisories.tsv` holds the triage.

## Licence

MIT, see [LICENSE](LICENSE). Code derived from Symfony keeps Fabien
Potencier's copyright. Code derived from Twig (BSD-3-Clause) will carry
Twig's notice in its own crate.
