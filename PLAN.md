# symfony-rs plan

Copied from symfony-demo-rs `docs/runtime-plan.md` at the start; this file
is now the maintained version. Paths in "Existing demo code to promote"
are in symfony-demo-rs.

## From the Symfony Demo port to a Rust Symfony runtime

## Context

`symfony-demo-rs` reproduces symfony/demo (`f42980c`, Symfony 8.1.10, PHP 8.4)
byte for byte, phases 1–13 done. Phases 11–13 started pulling reusable parts
out into private workspace crates (`crates/http-foundation`, `crates/routing`,
`crates/http-kernel`), each a documented *subset* with upstream tests and
PHP-recorded vectors.

The owner wants this to grow into a comprehensive Rust rewrite of Symfony.
Answers settled on November 19:

- **Purpose:** the runtime that php2rust's planned `symfony2rust` converter
  targets. Converted client apps link against it and must behave like real
  Symfony. Proof comes from Symfony itself, as in this repo and php2rust.
- **Breadth:** usage-ranked tiers. Every component a typical Symfony app uses,
  not dev tooling or PHP-runtime glue.
- **Templates:** use an existing Rust Twig port if one exists, and the same
  for every other component (findings below).
- **Layout:** a new repository; symfony-demo-rs becomes its first consumer and
  acceptance test.

The pasted survey from November 18 (Serenade, Larust, Suprnova, php-rust,
etc.) found none with demonstrated upstream tracking. This plan answers that
directly: pinned upstream versions, machine-checked parity manifests, and a
defined process for following upstream security fixes.

Measured size of the work (non-test PHP lines in the demo's `vendor/`):
about 330k for the installed Symfony packages (form 26k, DI 26k, console 23k,
validator 20k, http-kernel 16k, cache 16k, http-foundation 15k, ...; `intl` is
mostly data), Twig 27k, Doctrine about 125k (ORM 58k, DBAL 50k). Messenger,
Serializer, Workflow, Notifier etc. are not installed in the demo and come on
top.

## Prior art found (searched November 19)

| What | Found | Verdict |
|---|---|---|
| Twig in Rust | [Nercury/twig-rs](https://github.com/Nercury/twig-rs), [colin-kiegel/twig-rust](https://github.com/colin-kiegel/twig-rust), their merge [rust-web/twig](https://github.com/rust-web/twig): all alpha, 0.1.0 crate about 10 years old, "not usable" | **No usable port.** Read for design ideas only |
| Twig parsers | [ludtwig-parser](https://lib.rs/crates/ludtwig-parser) (lossless parser for a formatter/linter, maintained), oak-twig | Evaluate ludtwig as a parser front-end at M4 start; likely rejected (formatting grammar, not Twig's extension-defined tags/operators) |
| Jinja engines | [MiniJinja](https://github.com/mitsuhiko/minijinja) | Jinja2 semantics (Python truthiness, other filters), templates read at run time. Not Twig |
| Symfony-shaped Rust frameworks | [Serenade](https://github.com/Interchouette-ITC/Serenade) (Apache-2.0; kernel, DI, events, bundles, forms, security; "conceptual" inspiration, no behaviour parity claimed) | Reference for crate split and bundle API only |
| Laravel-shaped (Larust, Rustlavel, Rustavel, Suprnova) | Own frameworks, no parity tracking | Take Suprnova's per-feature parity map idea |
| Run real Symfony on a Rust PHP VM | [php-rust](https://github.com/francescotinti/php-rust): Symfony http-kernel tests pass, pinned PHP oracle | Excluded by php2rust's licence policy (PHP License 3.01). Take its pinned differential-testing method |
| Individual components (ExpressionLanguage, Yaml, DI, Messenger, Serializer, Workflow, Validator, Doctrine/DQL) | No Rust ports. Look-alikes only: evalexpr, saphyr/yaml-rust2, dotenvy, garde/validator, serde, apalis, sqlmodel-session, lref, ferrocat-icu / formatjs ICU parser | Port the behaviour. Adopt Rust crates only below the behaviour layer (transport clients, hashing, ids). See candidates per milestone |

**Conclusion:** nothing can be adopted as a Symfony- or Twig-compatible base.
Each component is ported from its pinned PHP source (MIT; Twig BSD-3-Clause),
keeping copyright notices. Existing Rust crates are used only for transport
or primitives, where Symfony's observable behaviour does not depend on them.
Per AGENTS.md and the global CLAUDE.md, every milestone still starts with its
own GitHub/crates.io search for that component, recorded as a stack-table row.

## Architecture

**Repository.** New private repo (working name `symfony-rs`, see decision 1).
`crates/http-foundation`, `routing`, `http-kernel` move there with history
(`git subtree split` per directory, merged into the new repo). symfony-demo-rs
depends on it by pinned git rev, with a `[patch]` path override for local
work. `examples/request-lab` moves along as the minimal second consumer.

**Crate per Symfony package**, same names as Composer (`symfony-yaml`,
`symfony-form`, `twig`, `doctrine-orm`, ...), each with: its source mapping
(module header names the PHP file, kept from AGENTS.md), README contract
table (as the three existing crates already have), `parity/` manifest, and
upstream tests under their original names.

**Three layers in every component:**
1. *Behaviour core:* plain Rust, typed, no I/O policy. Ported file by file.
2. *Build-time compilers:* anything PHP does by reading config or reflecting
   at run time (YAML/XML/PHP config, attributes, route loading, container
   compilation, Twig templates, DQL, validation/serializer metadata) runs in
   `build.rs` or proc-macros and emits Rust. Nothing is parsed per request,
   which keeps the repo's existing rule.
3. *Adapters:* axum/tower for HTTP (as now), sqlx for databases, and named
   client crates for transports.

**Dynamic values.** Twig, Form, PropertyAccess, Serializer and
ExpressionLanguage need PHP semantics: loose `==`, truthiness, string
conversion of floats, `.` attribute lookup (`getX`/`isX`/`hasX`/property/key).
Typed Rust structs get a `#[derive(Attributes)]` fast path. Untyped values use
php2rust's `php-rt` `Value`, which already reproduces those semantics against
real PHP (decision 3).

**Bundles.** A `Bundle` trait: typed config tree (ported `Config`
TreeBuilder rules, so defaults and error messages match), container
extension, compiler passes, routes, Twig paths and listeners. This extends
the axum `Router` + tower `Layer` + config struct shape already chosen in
PLAN.md "Proposed: Symfony bundles as Rust modules".

**Existing demo code to promote** (move into crates, keep tests and vectors):

| Demo module | Becomes |
|---|---|
| `src/console/` (ArgvInput, SymfonyStyle, Table, descriptor, width table) | `symfony-console` |
| `src/i18n/` (XLIFF `build.rs` loader, ICU MessageFormat, intl data extraction) | `symfony-translation`, `symfony-intl`, `twig-intl-extra` |
| `src/html_sanitizer/` | `symfony-html-sanitizer` |
| `src/html.rs` (Twig escaping) | `twig` escaper runtime |
| `src/security/` (session, CSRF, remember-me, password, voter) | `symfony-security-*`, `symfony-password-hasher`, HttpFoundation session |
| `src/form/`, `src/validator.rs` | `symfony-form`, `symfony-validator` |
| `src/live_component/`, `src/icon.rs`, `src/asset_mapper.rs` | `symfony-ux-live-component`, `symfony-ux-icons`, `symfony-asset-mapper` |
| `src/slugger.rs` + `data/slugger/` | `symfony-string` |
| `src/markdown.rs` | `twig-markdown-extra` |
| `src/request/multipart.rs`, `src/middleware/` | already in `http-foundation` / `http-kernel` |
| `compare/*.py`, `data/*/extract.php` vector recorders | the new repo's `oracle/` |

## Per-component loop (every component, every milestone)

1. Search GitHub/crates.io for an existing port or primitive; add a stack row
   (options compared, choice, reason), as AGENTS.md requires.
2. Generate the parity manifest: `parity/<package>.tsv` lists every PHPUnit
   test method at the pinned tag, extracted by PHP's own tokenizer in the
   oracle container. Status per row: `ported` / `n/a: <reason>` / `pending`.
3. Port one PHP file at a time with its tests under the same names
   (AGENTS.md rules: no `unsafe`, no `unwrap` on external data, enums for
   closed sets, sqlx bound parameters only).
4. Record vectors from the pinned PHP for anything tests don't cover (parsers,
   formatters, error messages). For parsers, also run cargo-fuzz with the
   corpus replayed against the oracle (php-rust's approach, run locally).
5. Done when: no `pending` rows, vectors pass, `cargo fmt --check` / `clippy
   -D warnings` / `cargo test` pass, the demo's 92 pages + 127 live cases
   still match, and the README contract table is updated.

CI gates the manifests: a component marked done with `pending` rows fails,
and so does a Rust test claiming an upstream name that isn't in the manifest.

## Upstream tracking and security

- **Pins:** Symfony `v8.1.10` (monorepo tag, so tests are included; the
  vendored dist packages have none), Twig `v3.30.0`, Doctrine ORM `3.7.5` /
  DBAL `4.5.0`, UX `3.6.0`, taken from the demo's `composer.lock`.
  `upstream/fetch.sh` clones them read-only into an ignored directory.
- **Source map:** module headers → generated `parity/sources.tsv` (Rust
  module ↔ PHP file ↔ pinned commit).
- **Weekly scheduled CI job:** diff the pinned tag against the newest patch
  release for every mapped PHP file, and check GitHub advisories for
  `symfony/*`, `twig/twig`, `doctrine/*`. Either one opens an issue listing
  the affected Rust modules.
- **Security fix procedure:** triage (is the code ported? does the Rust
  design already exclude it?) → reproduce as a failing test from the
  upstream fix's test → port → note it in the component's CHANGELOG with the
  CVE/GHSA id. Response target is set by the owner (decision 4).
- **Version moves:** 8.1 → each 8.x minor → 8.4 LTS (Nov 2027). Each move
  diffs mapped files, re-records vectors, and re-runs manifests.

## Milestones

Each milestone ends with the checks above, a log in
`docs/milestones/NN-name/README.md`, and fresh demo comparisons. Order is
by dependency. Within a milestone, components go one at a time.

### M0: New repository and parity tooling (no new behaviour)
- Create the private repo (owner confirms at that moment, per AGENTS.md).
  Move the three crates and request-lab with history; licence (MIT + Symfony
  and Demo copyright lines; Twig crates add the BSD-3 notice); "unofficial,
  not endorsed" wording.
- Point symfony-demo-rs at it by git rev; demo checks and 92-page comparison
  unchanged.
- `oracle/`: Docker image of PHP 8.4 + the pinned packages + `run.php`, which
  runs JSON-described cases against a component and writes vectors. Generalize
  the existing recorders (`compare/routing_vectors.py`,
  `multipart_vectors.py`, `data/*/extract.php`).
- `parity/` manifest generator, CI gate, `sources.tsv`, upstream watch job,
  Linux CI (extend the existing `.github/workflows/checks.yml`).
- Backfill manifests for http-foundation, routing and http-kernel. Their
  `pending` rows become the M2 work list.

### M1: Foundations (self-contained, easy upstream tests)
Contracts (service/event/translation/cache/http-client traits),
EventDispatcher (full, generalizing http-kernel's dispatcher), Yaml (full
port; saphyr/yaml-rust2 implement YAML 1.2, not Symfony's dialect), Dotenv
(port; dotenvy expands differently), Config (TreeBuilder + processing, at
build time), String (complete `u()`/`b()` APIs, inflectors, slugger), Clock,
Uid (wrap `uuid`/`ulid` crates, Symfony's formats), Filesystem, Finder (walker
crate + ported filters/ordering), OptionsResolver, PasswordHasher (bcrypt
already in; add RustCrypto argon2/pbkdf2; "migrate from" logic), Process.
Also the Attributes derive and the php-rt integration (decision 3).

### M2: HTTP core complete
HttpFoundation (Request/Response/HeaderBag/ParameterBag/Cookie/
JsonResponse/StreamedResponse/BinaryFileResponse, RequestStack, sessions with
file/PDO/Redis handlers and PHP's session serialization as data only, so
sessions can be shared with PHP during a migration), ExpressionLanguage (full
port; evalexpr has another grammar), Routing (PCRE requirements (decision
5), defaults/optional params, host/scheme/conditions, loaders at build time,
full UrlGenerator), HttpKernel (sub-requests, argument and value resolvers
incl. `MapQueryString`/`MapRequestPayload`, `kernel.view`,
finish/terminate, fragments/ESI, HttpCache), ErrorHandler renderers and
FlattenException (no PHP error-hook parts). Acceptance: request-lab and the
demo's kernel glue run on the full crates.

### M3: DependencyInjection, Kernel, FrameworkBundle core
DI as a build-time compiler: definitions, parameters, `%env()%` processors
(evaluated at run time), tags, autowiring by type, decorators, service
locators, lazy services, compiler passes; YAML/XML/PHP config loaders at
build time (the PHP loader reads only the array/fluent subset; the rest is
refused with a location, php2rust's "refuse, don't guess" rule). Kernel
(environments, `APP_ENV`/`APP_DEBUG`, bundles). FrameworkBundle config tree,
grown with each later component. `debug:router` / `debug:container` /
`debug:config` / `debug:event-dispatcher` JSON output. These are also the
interface symfony2rust reads. Acceptance: the demo's services are wired by the
container, and those `debug:*` JSON outputs match real Symfony's for the demo
(normalized).

### M4: Twig (build-time compiler)
Port Twig 3.30's Lexer, TokenStream, Parser, token parsers, Node tree and
operator precedence. The compiler emits Rust (quote + prettyplease, as
php2rust does), not PHP. Runtime: Environment, extension registry
(filters/functions/tests), EscaperRuntime (from `src/html.rs`), inheritance
(`extends`, blocks, `embed`, `use`, macros, `include` with dynamic names
resolved from the compiled registry), whitespace control, sandbox later.
Twig's `Fixtures/*.test` files are data-driven and port almost directly; add
them to the manifest. TwigBridge (path, url, asset, csrf, trans, is_granted,
form functions), TwigBundle, twig extras (intl, markdown, html, string).
**Acceptance:** the demo renders the original `.twig` files unchanged;
Askama is removed; the README's "whitespace between tags" difference and its
`compare/diff.sh` normalization are deleted, and all 92 pages match.

### M5: Application components (the demo's remaining stack, made complete)
Translation (all loaders at build time, intl-icu domains, legacy `|`
plurals, fallbacks), Intl (data from Symfony's `Resources/data`, the method
that beat ICU4X earlier), Validator (all constraints, groups, sequences,
violation paths, XLIFF messages; egulias email rules), Form (all core types
and extensions, CSRF, validator integration, Twig form themes incl.
`bootstrap_5_layout`), Security (Core/Http/Csrf/Bundle: firewalls, form/json
login, remember-me, http-basic, login-link, access-token, custom
authenticators, memory/chain/entity providers, role hierarchy, access_control,
switch_user, login throttling, logout), Asset/AssetMapper (complete),
HtmlSanitizer (complete), Mime + Mailer (ported Mime serializer; SMTP
transport via `lettre` if its wire output passes vectors, otherwise ported),
Cache (array/filesystem/Redis/tag-aware, `get()` stampede protection;
`moka`/`redis` underneath), HttpClient (hyper-util underneath; retries,
scoping, MockHttpClient), Serializer + PropertyAccess/PropertyInfo/TypeInfo
(metadata from derives), Monolog bridge (line format parity only if needed),
UX (StimulusBundle, TwigComponent, LiveComponent complete, Icons, Turbo).
Acceptance: the demo's `src/` holds only the PHP app's own code (controllers,
entities, forms, commands, subscribers, Twig extensions). Every framework part
comes from crates.

### M6: Doctrine
Collections, EventManager, Persistence, Inflector, DBAL (SQLite, PostgreSQL,
MySQL/MariaDB platforms (decision 6), types with PHP-compatible storage,
schema manager, QueryBuilder; sqlx drivers underneath), ORM (attribute
metadata at build time, UnitOfWork, identity map, hydration, explicit lazy
proxies, DQL compiled to SQL at build time, Paginator, lifecycle events),
Migrations, DoctrineBundle, DoctrineBridge (entity user provider,
UniqueEntity, EntityValueResolver, EntityType), Fixtures. SeaORM/Diesel stay
rejected, for the same reasons the demo rejected them and because they lack
unit-of-work semantics. Acceptance: the demo's repositories run on the ORM,
and the database-diff recording from phase 9 is identical.

### M7: Async and integration
Messenger (buses, middleware, retries, failure transport; transports
sync/in-memory/Doctrine/Redis/AMQP via `lapin`; PHP-serialized envelopes
decoded as data only, so PHP and Rust workers can share a queue during a
migration), Scheduler (cron rules from dragonmantank/cron-expression),
Lock, Semaphore, RateLimiter (ported algorithms; governor uses another),
Workflow, Notifier (core + the most-used bridges, by client demand),
Webhook/RemoteEvent, Console complete (QuestionHelper, ProgressBar, etc.).
Acceptance: a second open-source Symfony app that uses these, run as an
oracle only (candidates chosen at M7 start after a licence check, e.g. the
"Fast Track" guestbook, wallabag).

### M8: Test tooling and the symfony2rust contract
BrowserKit, DomCrawler, CssSelector (CSS→XPath semantics; evaluate
`scraper`/`selectors` first), FrameworkBundle's KernelBrowser and
`WebTestCase` assertions, so converted client apps keep their functional
tests. Write the stable API that generated code may use (versioned and
semver-checked). Add a fixture corpus that symfony2rust (in php2rust) builds
against. The converter itself is php2rust's work, not this plan's.

**Out of scope:** WebProfilerBundle, DebugBundle/VarDumper (beyond a minimal
`dump()`), MakerBundle, Flex, PHPUnit bridge, polyfills, Runtime (replaced by
`main`), ErrorHandler's PHP hooks, VarExporter's lazy ghosts, APCu, Panther,
the PSR-7 bridge, and Notifier bridges nobody asks for.

## Decisions for the owner (recommendations, used unless changed)

1. **Name.** "Symfony" is a trademark. Recommended: keep `symfony-*` crate
   names while private; choose a neutral public name before any publication.
2. **Twig approach.** With no usable port, recommended: the build-time
   compiler (M4). Alternative: MiniJinja + a Twig compatibility layer
   (faster to start, but Jinja2 semantics and run-time template parsing).
3. **PHP value semantics.** Recommended: depend on php2rust's `php-rt` by
   pinned git rev for the dynamic layers, rather than writing a second copy
   of PHP's type juggling. Needs a line in php2rust's `docs/approach.md`:
   symfony2rust's target runtime contains MIT/BSD-derived Symfony, Twig and
   Doctrine code (php2rust's rule bans copying it into *converter* crates).
4. **Security response target.** Recommended: triage within 2 working days of
   an upstream advisory; fix in ported code before the next client release.
5. **Regex** (route requirements, `Regex` constraint). Recommended: `regex`
   crate, with `fancy-regex` for lookaround/backrefs. PCRE features neither
   supports are refused at build time. No pcre2 FFI.
6. **Doctrine platforms.** Recommended: SQLite, PostgreSQL, MySQL/MariaDB.
   This reverses PLAN.md's "out of scope" for the runtime only; the demo
   stays SQLite.
7. **Older client Symfony versions (5.4/6.4/7.4).** Recommended: one
   runtime version line. Client apps are upgraded on the PHP side first
   (Rector + upgrade guides, their own tests green), then converted.
   Alternative: version-gated features, which multiply every manifest.

8. **php-src-derived code** (found in M0). `symfony-http-foundation`'s
   `multipart.rs` ports `main/rfc1867.c` and `query.rs`/`cookies.rs` follow
   `main/php_variables.c` of PHP 8.4.27, released under the PHP License
   3.01. php2rust's `docs/approach.md` allows php-src-derived code only from
   BSD-3-Clause source. Recommended: before symfony2rust links this crate,
   either re-derive those modules from the BSD-3-Clause main branch (same
   functions, behaviour re-checked against the 156 recorded vectors) or
   record an explicit exception in php2rust. Nothing changes until the owner
   decides.

## M0 progress

- [x] Repository created from symfony-demo-rs with the history of
      `crates/http-foundation`, `crates/routing`, `crates/http-kernel` and
      `examples/request-lab` (`git subtree`), November 19.
- [x] Upstream pins: `parity/packages.tsv` (68 Composer packages at the exact
      commits symfony/demo installed) and `upstream.tsv` (php-src
      `php-8.4.27`). `scripts/fetch-upstream.sh --verify ../symfony-demo/vendor`
      confirms all 68 checkouts are file-for-file identical to the installed
      code. **Finding:** the Symfony monorepo tag `v8.1.10` is *not*: 24
      packages whose last split release is older (e.g. `symfony/asset`
      v8.1.0) carry later edits there. Pins therefore use each package's own
      repository and locked reference, as phases 12 and 13 already did.
- [x] Source map: `//! Upstream:` lines in every Rust file,
      `parity/sources.tsv` generated and checked by `scripts/sources.sh`
      (also in CI).
- [ ] Oracle image (`oracle/`): PHP 8.4 + the pinned packages + `run.php`
      for JSON-described cases. Needs Docker.
- [ ] Parity manifest generator (PHP tokenizer over each package's `Tests/`,
      run in the oracle image) and its CI gate; backfill manifests for
      http-foundation, routing, http-kernel.
- [ ] Move the vector recorders from symfony-demo-rs (`compare/routing_vectors.py`,
      `compare/multipart_vectors.py`, `data/*/extract.php`) into `oracle/`.
- [ ] Weekly upstream watch job (newer patch releases of mapped files, GitHub
      advisories) and one replayed past security fix as a drill.

## Verification

- Per component: manifest has no `pending` rows (CI-enforced); upstream tests
  under their original names pass; PHP-recorded vectors pass; fuzz corpus
  replay against the oracle shows no differences.
- Workspace: `cargo fmt --check`, `cargo clippy --all-targets -- -D
  warnings`, `cargo test` in the new repo and in symfony-demo-rs.
- Application: after every component the demo uses, run
  `docker compose -f compare/compose.yaml up`, `compare/capture.sh` /
  `compare/diff.sh`: 92/92 pages, 124/127 live cases (the three documented
  filesystem-case artifacts). From M4 on, the same with no whitespace
  normalization. From M6 on, the database-diff recording is identical too.
- `debug:*` JSON output matches real Symfony's for the demo (M3 on); the
  second app's recorded responses match (M7).
- Upstream process drill at M0 end: replay one past Symfony security fix
  through the watch job → issue → failing test → fix.
- Housekeeping per AGENTS.md: commit per ported file naming the PHP source,
  debug builds only, stop comparison containers, never commit databases,
  `target/`, `static/assets/` or `compare/out/`.
