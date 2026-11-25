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
| [`symfony-http-foundation`](crates/http-foundation/README.md) | `symfony/http-foundation` (+ PHP request parsing) | Query, cookie and multipart parsing; ParameterBag, InputBag, ServerBag, HeaderBag, HeaderUtils, Cookie, IpUtils, AcceptHeader; File, UploadedFile and FileBag; Request; Response, JsonResponse, RedirectResponse; PHP-FPM's superglobals, uploads and output (`sapi`) |
| [`symfony-routing`](crates/routing/README.md) | `symfony/routing` | Route, CompiledRoute, RouteCompiler, RouteCollection; typed registrations, matcher and redirectable matcher, URL escaping |
| [`symfony-http-kernel`](crates/http-kernel/README.md) | `symfony/http-kernel` | HttpKernel's lifecycle on Request and Response (events, exceptions, terminate), the HttpException family, RouterListener, the ArgumentResolver, the ControllerResolver; the typed Axum hook subset |
| [`symfony-runtime`](crates/runtime/README.md) | `symfony/runtime` | HttpKernelRunner; the Axum adapter serving a ported kernel as nginx and PHP-FPM serve a front controller |
| [`symfony-mime`](crates/mime/README.md) | `symfony/mime` | MimeTypes and its guessers (the `file` command, and finfo on a pure-Rust libmagic) |
| [`symfony-config`](crates/config/README.md) | `symfony/config` | Definition, Builder, dumpers, XmlUtils, loaders, resources, ConfigCache |
| [`symfony-console`](crates/console/README.md) | `symfony/console` | The parts the Yaml and Dotenv commands use |
| [`symfony-translation`](crates/translation/README.md) | `symfony/translation` | MessageCatalogue, XLIFF and Qt loaders |
| [`symfony-string`](crates/string/README.md) | `symfony/string` | Byte, code point and Unicode strings, LazyString, inflectors |
| [`symfony-filesystem`](crates/filesystem/README.md) | `symfony/filesystem` | The `dumpFile()` subset Config's caches use |
| [`symfony-event-dispatcher`](crates/event-dispatcher/README.md), [contracts](crates/event-dispatcher-contracts/README.md) | `symfony/event-dispatcher` | Dispatcher, listeners, subscribers |
| [`twig`](crates/twig/README.md) | `twig/twig` | M4 started: Source, errors, tokens, token stream, Lexer, nodes, extensions, expression and token parsers, Parser |
| [`symfony-yaml`](crates/yaml/README.md), [`symfony-dotenv`](crates/dotenv/README.md), [`symfony-finder`](crates/finder/README.md), [`symfony-css-selector`](crates/css-selector/README.md), [`symfony-options-resolver`](crates/options-resolver/README.md), [`symfony-stopwatch`](crates/stopwatch/README.md) | the same names | Complete: no pending upstream test (`parity/done.txt`) |
| [`symfony-deprecation-contracts`](crates/deprecation-contracts/README.md), [`psr-log`](crates/psr-log/README.md) | `symfony/deprecation-contracts`, `psr/log` | The contracts |
| [`php-value`](crates/php-value/README.md), [`php-dom`](crates/php-dom/README.md), [`php-pcre`](crates/php-pcre/README.md) | — (PHP itself) | PHP values and engine rules, the DOM extension subset, `preg_*` patterns; written from PHP's recorded behaviour, no PHP source |
| [`request-lab`](examples/request-lab/README.md) | — | Independent example application on the ported kernel (RouterListener, ControllerResolver, ArgumentResolver, HttpKernel) through the HTTP adapter |

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

Class manifests: `parity/classes/<package>.tsv` lists every PHP source file
of the package's dist archive (no tests), the classes it declares, its
code lines and its status: `ported` when `parity/sources.tsv` maps it,
`pending`, or `n/a: <reason>`. `python scripts/classes.py generate
<package>` (re)writes one from `upstream/` with the oracle (or a local
`php` in `SYMFONY_RS_PHP`); `python scripts/classes.py sync` updates the
status columns after `scripts/sources.sh`, without PHP; `check` (in
`scripts/dev_check.py parity`) fails when they disagree; `report` prints
the size-weighted share. A file is `ported` as soon as one Rust module
names it, so the share is an upper bound; the test manifests show what is
still open.

Class snapshot, November 25, 2026 (code lines of mapped files; later
changes are authoritative in `parity/classes/`): 34,798 of 305,584 code
lines (11.4%) outside `Resources/data/`, which holds generated tables
(symfony/intl's ICU data is 391,884 lines); 37,486 of 700,315 (5.4%) with
them. By vendor, without data: Symfony 15.7%, Twig 0%, Doctrine 0%,
psr/log 80.4%.

Manifest snapshot, November 25, 2026 (test methods, not a compatibility
percentage; later changes are authoritative in `parity/`):

| Package | Upstream tests | Ported | N/A | Pending |
|---|---:|---:|---:|---:|
| doctrine/dbal | 2,878 | 0 | 0 | 2,878 |
| doctrine/orm | 3,208 | 0 | 0 | 3,208 |
| psr/log | 0 | 0 | 0 | 0 |
| symfony/asset-mapper | 160 | 0 | 0 | 160 |
| symfony/asset | 34 | 0 | 0 | 34 |
| symfony/browser-kit | 206 | 0 | 0 | 206 |
| symfony/cache-contracts | 0 | 0 | 0 | 0 |
| symfony/cache | 1,080 | 0 | 0 | 1,080 |
| symfony/clock | 36 | 0 | 0 | 36 |
| symfony/config | 334 | 307 | 2 | 25 |
| symfony/console | 1,152 | 320 | 11 | 821 |
| symfony/css-selector | 85 | 80 | 5 | 0 |
| symfony/debug-bundle | 11 | 0 | 0 | 11 |
| symfony/dependency-injection | 1,436 | 0 | 0 | 1,436 |
| symfony/deprecation-contracts | 0 | 0 | 0 | 0 |
| symfony/doctrine-bridge | 377 | 0 | 0 | 377 |
| symfony/dom-crawler | 222 | 0 | 0 | 222 |
| symfony/dotenv | 57 | 57 | 0 | 0 |
| symfony/error-handler | 114 | 0 | 0 | 114 |
| symfony/event-dispatcher-contracts | 0 | 0 | 0 | 0 |
| symfony/event-dispatcher | 140 | 114 | 0 | 26 |
| symfony/expression-language | 106 | 0 | 0 | 106 |
| symfony/filesystem | 157 | 27 | 0 | 130 |
| symfony/finder | 143 | 139 | 4 | 0 |
| symfony/form | 2,734 | 0 | 0 | 2,734 |
| symfony/framework-bundle | 1,089 | 0 | 0 | 1,089 |
| symfony/html-sanitizer | 73 | 0 | 0 | 73 |
| symfony/http-client-contracts | 0 | 0 | 0 | 0 |
| symfony/http-client | 941 | 0 | 0 | 941 |
| symfony/http-foundation | 959 | 479 | 10 | 470 |
| symfony/http-kernel | 1,045 | 200 | 8 | 837 |
| symfony/intl | 209 | 0 | 0 | 209 |
| symfony/mailer | 125 | 0 | 0 | 125 |
| symfony/mime | 395 | 28 | 1 | 366 |
| symfony/monolog-bridge | 75 | 0 | 0 | 75 |
| symfony/monolog-bundle | 108 | 0 | 0 | 108 |
| symfony/options-resolver | 234 | 234 | 0 | 0 |
| symfony/password-hasher | 87 | 0 | 0 | 87 |
| symfony/process | 168 | 0 | 0 | 168 |
| symfony/property-access | 227 | 0 | 0 | 227 |
| symfony/property-info | 143 | 0 | 0 | 143 |
| symfony/routing | 896 | 372 | 1 | 523 |
| symfony/runtime | 19 | 0 | 0 | 19 |
| symfony/security-bundle | 323 | 0 | 0 | 323 |
| symfony/security-core | 158 | 0 | 0 | 158 |
| symfony/security-csrf | 109 | 0 | 0 | 109 |
| symfony/security-http | 503 | 0 | 0 | 503 |
| symfony/service-contracts | 0 | 0 | 0 | 0 |
| symfony/stimulus-bundle | 36 | 0 | 0 | 36 |
| symfony/stopwatch | 33 | 33 | 0 | 0 |
| symfony/string | 266 | 235 | 1 | 30 |
| symfony/translation-contracts | 0 | 0 | 0 | 0 |
| symfony/translation | 319 | 81 | 2 | 236 |
| symfony/twig-bridge | 2,021 | 0 | 0 | 2,021 |
| symfony/twig-bundle | 43 | 0 | 0 | 43 |
| symfony/type-info | 175 | 0 | 0 | 175 |
| symfony/ux-icons | 109 | 0 | 0 | 109 |
| symfony/ux-live-component | 258 | 0 | 0 | 258 |
| symfony/ux-twig-component | 212 | 0 | 0 | 212 |
| symfony/validator | 1,399 | 0 | 0 | 1,399 |
| symfony/var-dumper | 237 | 0 | 0 | 237 |
| symfony/var-exporter | 119 | 0 | 0 | 119 |
| symfony/web-profiler-bundle | 90 | 0 | 0 | 90 |
| symfony/yaml | 342 | 341 | 1 | 0 |
| twig/extra-bundle | 7 | 0 | 0 | 7 |
| twig/html-extra | 14 | 0 | 0 | 14 |
| twig/intl-extra | 17 | 0 | 0 | 17 |
| twig/markdown-extra | 6 | 0 | 0 | 6 |
| twig/twig | 778 | 0 | 0 | 778 |
| **Total** | **29,037** | **3,047** | **46** | **25,944** |

Every package pinned in `parity/packages.tsv` has a manifest; those
without a Rust port yet are all `pending`, and the contracts packages
that ship no PHPUnit tests have the header line alone. Six packages have no pending rows and are listed in `parity/done.txt`:
Stopwatch, Yaml, Dotenv, CssSelector, OptionsResolver and Finder. M1 is
underway: Config has its Definition, Builder, dumpers, XmlUtils, loaders,
filesystem resources and ConfigCache ported (the reflection-based
resources, decision 17, are a later stage); String, Console (the subset the Yaml and
Dotenv commands use), Translation's XLIFF and Qt loaders and the XML
gates are partial. M2 has started: HttpFoundation's bags, headers,
cookies, IpUtils, AcceptHeader, the File classes, Request, Response,
JsonResponse, RedirectResponse and
RequestStack, HttpKernel's lifecycle with its events and HTTP exceptions,
RouterListener, the ArgumentResolver with its value resolvers and the
ControllerResolver,
the HTTP adapter (symfony/runtime's HttpKernelRunner, nginx's FastCGI
parameters, PHP-FPM's superglobals, uploads and output recorded from
PHP-FPM) with request-lab running on the ported kernel,
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
