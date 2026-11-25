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

| Crate | Package | Contract |
|---|---|---|
| [`symfony-http-foundation`](crates/http-foundation/README.md) | `symfony/http-foundation` (+ PHP request parsing) | Query, cookie and multipart parsing; ParameterBag, InputBag, ServerBag, HeaderBag, HeaderUtils, Cookie, IpUtils, AcceptHeader; File, UploadedFile and FileBag; Request; Response (sending recorded for the HTTP adapter) |
| [`symfony-routing`](crates/routing/README.md) | `symfony/routing` | Route, CompiledRoute, RouteCompiler, RouteCollection; typed registrations, matcher and redirectable matcher, URL escaping |
| [`symfony-http-kernel`](crates/http-kernel/README.md) | `symfony/http-kernel` | HttpKernel's lifecycle on Request and Response (events, exceptions, terminate), the HttpException family; the typed Axum hook subset |
| [`symfony-mime`](crates/mime/README.md) | `symfony/mime` | MimeTypes and its guessers (the `file` command, and finfo on a pure-Rust libmagic) |
| [`symfony-config`](crates/config/README.md) | `symfony/config` | Definition, Builder, dumpers, XmlUtils, loaders, resources, ConfigCache |
| [`symfony-console`](crates/console/README.md) | `symfony/console` | The parts the Yaml and Dotenv commands use |
| [`symfony-translation`](crates/translation/README.md) | `symfony/translation` | MessageCatalogue, XLIFF and Qt loaders |
| [`symfony-string`](crates/string/README.md) | `symfony/string` | Byte, code point and Unicode strings, LazyString, inflectors |
| [`symfony-filesystem`](crates/filesystem/README.md) | `symfony/filesystem` | The `dumpFile()` subset Config's caches use |
| [`symfony-event-dispatcher`](crates/event-dispatcher/README.md), [contracts](crates/event-dispatcher-contracts/README.md) | `symfony/event-dispatcher` | Dispatcher, listeners, subscribers |
| [`symfony-yaml`](crates/yaml/README.md), [`symfony-dotenv`](crates/dotenv/README.md), [`symfony-finder`](crates/finder/README.md), [`symfony-css-selector`](crates/css-selector/README.md), [`symfony-options-resolver`](crates/options-resolver/README.md), [`symfony-stopwatch`](crates/stopwatch/README.md) | the same names | Complete: no pending upstream test (`parity/done.txt`) |
| [`symfony-deprecation-contracts`](crates/deprecation-contracts/README.md), [`psr-log`](crates/psr-log/README.md) | `symfony/deprecation-contracts`, `psr/log` | The contracts |
| [`php-value`](crates/php-value/README.md), [`php-dom`](crates/php-dom/README.md), [`php-pcre`](crates/php-pcre/README.md) | — (PHP itself) | PHP values and engine rules, the DOM extension subset, `preg_*` patterns; written from PHP's recorded behaviour, no PHP source |
| [`request-lab`](examples/request-lab/README.md) | — | Independent example application using HttpFoundation, Routing and HttpKernel |

Upstream pins are each package's exact version and reference from the demo's
`composer.lock`, recorded in `parity/packages.tsv`; split Symfony packages
can have different versions. The M0 findings in [PLAN.md](PLAN.md#m0-progress)
explain why a single monorepo tag is insufficient.

## Working on it

Each crate's README states its own contract and remaining work.

Read [AGENTS.md](AGENTS.md) and [PLAN.md](PLAN.md). Checks:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
scripts/sources.sh --check
scripts/parity-check.sh   # manifest claims and completion status
```

## Faster local checks (Windows and Linux)

Use Python 3.10+ (standard library only), Git, and Cargo. On Windows,
the full check finds Bash beside Git automatically. Use --bash PATH to
select another executable; the Windows WSL launcher is avoided.
Run these from PowerShell or a Unix shell:

```text
python scripts/dev_check.py list
python scripts/dev_check.py check
python scripts/dev_check.py test
python scripts/dev_check.py test --nextest
python scripts/dev_check.py check -p php-value
python scripts/dev_check.py full
python scripts/dev_check.py parity
python -m unittest discover -s scripts -p "test_*.py"
```

The default base is origin/main; use --base to select another ref. Refresh
the base with git fetch when appropriate. Affected mode includes changes
in commits, the index, the working tree, and untracked files. It includes
transitive workspace consumers of each changed crate using Cargo metadata,
including optional, dev, build and target-specific path dependencies.
Unknown paths, root/configuration or manifest changes, data changes, an
empty change set, and metadata failures select the full workspace.
Explicit -p selects only the named packages, not reverse dependents.
This is a development convenience, not a proof of impact: external inputs,
cross-package include paths and runtime integrations can require full checks.

Optional nextest: install cargo-nextest using your preferred installation
method (e.g. cargo install cargo-nextest --locked). The development runner
also runs Cargo doctests, even if nextest fails; nextest does not run them. If nextest is not
installed, use ordinary test mode. The full gate always uses Cargo test
and includes the existing source/parity scripts. No tests are suppressed.
Windows-specific failures must still be verified with the existing
scripts/linux-test.sh; it checks committed HEAD in a fresh Linux container.
Oracle recordings, upstream source retrieval, generated-table checks and
converter/second-app acceptance remain separate and unchanged. Do not run
the paused symfony-demo-rs comparison until the owner resumes it.

Each command reports elapsed time. The full gate also tests the tooling.
The source/parity scripts use a standard-library Python batch backend when
the runner supplies SYMFONY_RS_PYTHON. This checks the same manifests,
source mappings and test names without launching grep/awk/sed per row.
Run python scripts/parity_check.py sources --check and python
scripts/parity_check.py parity directly to avoid Bash entirely. The original
Bash implementations remain available when SYMFONY_RS_PYTHON is unset;
the tool tests compare both on valid and deliberately broken fixtures.

For repeated Linux parity replays:

```sh
bash scripts/linux-test.sh --cached -p symfony-filesystem
bash scripts/linux-test.sh --cached -p symfony-filesystem -p php-value
bash scripts/linux-test.sh                 # fresh pre-merge acceptance
```

All modes check committed HEAD, as before; commit edits before testing.
The small scripts/linux-check build context caches the pinned Rust image
and rustfmt/clippy installation. Default runs still start with empty Cargo
downloads and build directories. --cached is for development: Docker volumes
reuse downloads and build artifacts inside the Linux VM, avoiding a Windows
target bind mount. Names include the image recipe and worktree path, so
concurrent worktrees have independent caches. Tests still run in a removed,
unprivileged container with upstream mounted read-only. The script prints
the two volume names; use docker volume rm NAME... when they are no longer
needed. Do not remove another session's volumes.

Full demo comparisons remain paused. Existing recorded-vector Rust tests
benefit from the same Cargo caches; oracle recordings still use the pinned
PHP image and are never replaced with cached answers.

Normal dev/test profiles now emit line tables instead of full debug info.
For full debugger symbols use cargo test --profile dev-full or cargo build
--profile dev-full. Profile changes cause an initial rebuild; later builds
reuse compatible artifacts. Keep target directories separate per concurrent
worktree; sccache can share compatible compiler results without target locks.
Set RUSTC_WRAPPER=sccache in your environment if installed. Incremental Rust
compilation is not cacheable by sccache; compare warm incremental builds
with CARGO_INCREMENTAL=0 plus sccache before choosing a local setting.
Do not change global environment settings from project scripts.

To inspect the cost rather than guess:

```text
python scripts/dev_check.py audit
python scripts/dev_check.py audit --timings
python scripts/dev_check.py test --timings
sccache --show-stats
```

The audit prints duplicate dependency versions and feature activation;
timings are Cargo HTML reports under the configured target directory.
A lockfile entry is not a compilation unit: test targets, build scripts,
proc macros and feature combinations can compile the same package more
than once. Review unused features and duplicate versions only after
measuring and verifying parity. No runtime dependency change is needed
for these tools. Profile and cache guidance follows the
[Cargo profiles reference](https://doc.rust-lang.org/cargo/reference/profiles.html)
and [Docker cache guidance](https://docs.docker.com/build/cache/optimize/).

Pinned upstream sources (with their tests), for porting and for parity
manifests: `scripts/fetch-upstream.sh` clones them into the ignored
`upstream/` directory. `scripts/manifest.sh <package>` (re)generates
`parity/<package>.tsv`, the list of every upstream test with its status,
using the PHP oracle in `oracle/` (Docker).

Manifest snapshot, November 25, 2026 (test methods, not a compatibility
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
| symfony/http-foundation | 959 | 447 | 10 | 502 |
| symfony/http-kernel | 1,045 | 117 | 5 | 923 |
| symfony/mime | 395 | 28 | 1 | 366 |
| symfony/options-resolver | 234 | 234 | 0 | 0 |
| symfony/routing | 896 | 372 | 1 | 523 |
| symfony/stopwatch | 33 | 33 | 0 | 0 |
| symfony/string | 266 | 235 | 1 | 30 |
| symfony/translation | 345 | 81 | 2 | 262 |
| symfony/yaml | 342 | 341 | 1 | 0 |
| **Total** | **6,583** | **2,932** | **43** | **3,608** |

Six packages have no pending rows and are listed in `parity/done.txt`:
Stopwatch, Yaml, Dotenv, CssSelector, OptionsResolver and Finder. M1 is
underway: Config has its Definition, Builder, dumpers, XmlUtils, loaders,
filesystem resources and ConfigCache ported (the reflection-based
resources, decision 17, are a later stage); String, Console (the subset the Yaml and
Dotenv commands use), Translation's XLIFF and Qt loaders and the XML
gates are partial. M2 has started: HttpFoundation's bags, headers,
cookies, IpUtils, AcceptHeader, the File classes, Request, Response and
RequestStack, HttpKernel's lifecycle with its events and HTTP exceptions,
symfony/mime's MIME types and guessers, and Routing's Route,
RouteCompiler, RouteCollection, RequestContext, both URL generators and
both matchers (route conditions wait for ExpressionLanguage).
symfony/filesystem has only the `dumpFile()` subset that
ConfigCache needs. Deferred DI and
Console integrations keep their tests pending. Per-stage detail is in
[PLAN.md](PLAN.md#m1-progress) and `docs/milestones/`.

The maintained [PLAN.md](PLAN.md#delivery-order-and-deferred-integrations)
sets dependency stages: an early converter fixture and second-app inventory,
request-parser provenance gate, and separate XML, Twig and Doctrine
acceptance stages. These additions are planned work, not completed results.

Test vectors from real PHP: `scripts/record.sh <recorder>|all`
(recorders in `oracle/recorders/`). Upstream security: `python
scripts/upstream_watch.py` (a weekly scheduled workflow; the other checks
run locally, there is no hosted CI) lists advisories and newer patch
releases for the pinned packages; `parity/advisories.tsv` holds the triage.

## Licence

MIT, see [LICENSE](LICENSE). Code derived from Symfony keeps Fabien
Potencier's copyright. Code derived from Twig (BSD-3-Clause) will carry
Twig's notice in its own crate.
