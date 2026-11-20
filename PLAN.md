# symfony-rs plan

This is the maintained runtime plan. The demo keeps a dated snapshot in
`../symfony-demo-rs/docs/runtime-plan.md`. Paths in "Existing demo code to
promote" are in symfony-demo-rs. Dependency staging was revised with the
owner's approval on November 19, 2026; new gates remain pending until proven.

## From the Symfony Demo port to a Rust Symfony runtime

## Context

`symfony-demo-rs` reproduces symfony/demo (`f42980c`, Symfony 8.1.10, PHP 8.4)
within its documented accepted differences; phases 1–13 are done.
Phases 11–13 started pulling reusable parts
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
this runtime's `php-value`, shared by Yaml, Config, DI and Twig (owner's
decision 3). Its PHP 8.4 behavior is checked against oracle vectors; missing
semantics stay explicit. The early converter fixture must exercise the
boundary with php2rust's values, including object identity and ownership
across asynchronous calls.

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
5. A complete package is done when: no `pending` rows, vectors pass,
   `cargo fmt --check` / `clippy -D warnings` / `cargo test` pass, the demo's
   92 pages + 127 live cases still match within its documented accepted
   differences, and the README contract table is updated.

CI gates the manifests: a component marked done with `pending` rows fails,
and so does a Rust test claiming an upstream name that isn't in the manifest.

A bounded stage can finish while its package remains partial: name the PHP
files and upstream tests in that stage, pass all of them, and list every
deferred integration with its dependency and destination stage below. Those
manifest rows stay `pending`; waiting for another component is not an `n/a`
reason. Only complete packages enter `parity/done.txt`. Test-method counts
describe tracked coverage, not a percentage of framework compatibility.

## Upstream tracking and security

- **Pins:** each package's exact version and reference from the demo's
  `composer.lock`, recorded in `parity/packages.tsv`. Use the split package
  repositories with their tests: the Symfony monorepo `v8.1.10` tag differs
  from 24 installed packages (M0 finding below).
  `scripts/fetch-upstream.sh` clones them into ignored `upstream/`; agents
  read those checkouts without editing them.
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

M0–M8 identify areas of work, not an instruction to finish every package
in one area before entering the next. Execute bounded stages in dependency
order, one PHP file and its tests at a time. Each stage ends with the checks
above, a log under `docs/milestones/`, and fresh comparisons for the apps it
affects. The demo's original phase order and comparison rules stay in force.

### Delivery order and deferred integrations

| Stage | Prerequisites | Exit evidence / deferred work |
|---|---|---|
| M1-A: foundation cores | M0 | Finish the current Config Definition/Builder step with its tests and oracle cases. Yaml and Dotenv cores remain partial packages until their commands pass. |
| M1-D: early conversion proof and second-app inventory | M1-A; M2 request rewrite before linking request crates | Provisional converter contract and executable fixture; pinned second app and ranked usage inventory. Complete before broad M2 expansion; this is the next scheduling priority after the current Config step. |
| M1-B: minimum Console and process support | Existing demo Console behavior; core contracts | Extract the command infrastructure needed for Yaml lint and Dotenv commands; add required Process behavior. Run the 11 Yaml and 19 Dotenv command tests. The rest of Console remains M7 work. |
| M1-C: Config loaders and XML | Config builders/dumpers, Filesystem/Finder; XML gates X1–X3 | Close resources, loaders and XmlUtils by supported loader; do not gate non-XML Config work on unfinished XML. |
| M2-A: HTTP behavior cores | Required M1 cores; request rewrite gate | Request/response, matcher/generator and lifecycle cores with explicit contracts. Payload mapping remains pending for Serializer/Validator. |
| M3-A: container and kernel integration | Config loaders actually used, M2-A, minimum Console | DI compiler and EventDispatcher DI passes / AsEventListener (26 pending tests); debug commands using the available Console subset. |
| M4-A/B: Twig language and runtime | php-value; existing HTTP adapters for initial fixtures | Twig gates T1–T2. Bridge functions wait for their named providers in M5/M6. |
| M5-A: application providers | Corresponding M1–M4 cores | Serializer/Validator then HTTP payload mapping; Form/Translation/Security then Twig integrations. Entity-backed integrations wait for M6. |
| M6-A/B/C: Doctrine | php-value and the container integration each stage uses | Doctrine gates D1–D3; then close entity providers, EntityType, UniqueEntity and EntityValueResolver. |
| M4-C / M5-B: demo framework replacement | All providers required by the original Twig templates and demo flows | Twig gate T3 and the M5 demo acceptance below; run after the relevant M6 integrations when required. |
| M7 / M8: remaining integration and API stabilization | Accepted earlier stages | Extend the second app's flows and converter corpus; stabilize the API already exercised since M1-D. |

Before starting each stage, record its exact upstream test/file set and any
new prerequisite exposed by implementation. Additional dependencies change
the stage order explicitly; they never justify silently dropping coverage.

### Early conversion proof and second application (M1-D)

- Write `docs/converter-contract.md` with a provisional, versioned Rust API:
  service construction/injection, typed attributes and `php-value`
  conversion, request/response and error boundaries, and ownership plus
  `Send + Sync` requirements across asynchronous calls. Track API changes
  with the fixture before promising a stable public interface at M8.
- In php2rust, the converter remains its own workstream. Pair a tiny pinned
  PHP Symfony app with a Rust fixture in this runtime: an injected service,
  scalar/array/object values, one successful request and one handled error.
  Start with an explicitly labelled hand-authored generated-code-shaped
  fixture if converter support is unavailable; that proves the runtime
  interface only. The conversion gate remains open until symfony2rust
  actually emits and builds the fixture without manual edits.
- Record PHP responses and relevant side effects, build the Rust fixture,
  and compare them. Repeat from an empty working directory on Linux, run
  locally in a removed container (owner, November 20: no paid CI jobs).
  Extend this fixture at DI, Twig, Form/Security and Doctrine stages; do not
  wait until M8 to discover a converter/runtime mismatch.
- Select and pin a second open-source Symfony app before broad M2 work.
  Check its licence and whether its dependencies fit the chosen Symfony
  version line. Record routes, package/API use, templates, configuration,
  persistence, queues and external integrations in
  `docs/acceptance/second-app.md`. Recommended: the EasyAdmin demo
  (decision 10; wallabag is on Symfony 5.4, and the Fast Track guestbook
  was not adopted).
- Rank component work by the demo plus that inventory: required for the
  next recorded workflow, required later, or not yet demanded. Capture a
  small initial workflow as an oracle, then add flows as providers arrive.
  Distinguish fixtures recorded from PHP, manually ported Rust flows and
  genuinely converted flows. M7 expands acceptance to async/integration
  scenarios rather than introducing the second app for the first time.

### Request implementation provenance gate (before M1-D HTTP linkage)

Rewrite `multipart.rs`, `query.rs` and `cookies.rs` from PHP documentation
and recorded observations, with no php-src-derived implementation, as the
owner required in decision 8. Retain and pass the existing 156 multipart
vectors, query/cookie tests and demo request comparisons; add vectors for
uncovered behavior. Record the replacement sources and provenance review
in the stage log. This gate precedes linking these request crates into the
converter fixture and is required before M2-A acceptance or any runtime
release. **Passed November 19** ([docs/milestones/m1-d-request-rewrite.md](docs/milestones/m1-d-request-rewrite.md)):
the three modules are rewritten from the PHP manual and 274 multipart, 160
query, 54 cookie and 12 content-type cases recorded from PHP 8.4.27; the
demo's comparisons are identical to a control run on its pinned revision.

### XML acceptance gates (M1-C)

- **X1, parser:** enumerate the DOM operations XmlUtils needs. Pass recorded
  valid/malformed XML cases including namespaces, encodings and diagnostics
  (text, level, code, line and column). Test external entity/DTD rejection
  without network or filesystem fetches, and bounded depth/entity expansion.
- **X2, schema validation:** inventory the XSD features used by the pinned
  DI, Routing and XLIFF 1.2/2.0 schemas. Record valid and invalid documents
  against each in the oracle. Unsupported features fail with a location;
  they must never silently bypass validation. Parser acceptance alone does
  not claim XSD support.
- **X3, consumers:** port XmlUtils tests, then enable each XML/XLIFF loader
  only after its schema and error-path comparisons pass. Keep the approved
  hand-written `php-dom` approach; record stage results and resource limits
  before broadening it. No general libxml compatibility is claimed.

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

### M1: Foundations (cores, then dependency-gated integrations)
Contracts (service/event/translation/cache/http-client traits),
EventDispatcher (full, generalizing http-kernel's dispatcher), Yaml (full
port; saphyr/yaml-rust2 implement YAML 1.2, not Symfony's dialect), Dotenv
(port; dotenvy expands differently), Config (TreeBuilder + processing, at
build time), String (complete `u()`/`b()` APIs, inflectors, slugger), Clock,
Uid (wrap `uuid`/`ulid` crates, Symfony's formats), Stopwatch (needed by
EventDispatcher's Debug classes and the profiler), Filesystem, Finder (walker
crate + ported filters/ordering), OptionsResolver, PasswordHasher (bcrypt
already in; add RustCrypto argon2/pbkdf2; "migrate from" logic), Process.
Also the Attributes derive and shared `php-value` integration (decision 3).
Console support needed by foundation commands is pulled forward in M1-B;
remaining Console features stay in M7. Package completion follows the
delivery table, including later DI integrations.

### M2: HTTP cores and later provider integrations
HttpFoundation (Request/Response/HeaderBag/ParameterBag/Cookie/
JsonResponse/StreamedResponse/BinaryFileResponse, RequestStack, sessions with
file/PDO/Redis handlers and PHP's session serialization as data only, so
sessions can be shared with PHP during a migration), ExpressionLanguage (full
port; evalexpr has another grammar), Routing (PCRE requirements (decision
5), defaults/optional params, host/scheme/conditions, loaders at build time,
full UrlGenerator), HttpKernel (sub-requests, argument and value resolvers
incl. `MapQueryString`/`MapRequestPayload`, `kernel.view`,
finish/terminate, fragments/ESI, HttpCache), ErrorHandler renderers and
FlattenException (no PHP error-hook parts). M2-A acceptance: request-lab and
the demo's kernel glue use the accepted core contracts and pass fresh
comparisons. MapQueryString/MapRequestPayload tests requiring Serializer or
Validator stay pending until M5-A. XML loaders require X3; complete-package
claims wait for every deferred integration.

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

M3-A closes EventDispatcher's DI integrations and uses the Console subset
from M1-B for debug commands; it does not require all of M7's Console work.

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
Acceptance is staged:

- **T1, language/compiler:** lexer/parser/precedence and Rust emission pass
  their upstream fixtures, including located failures for unsupported input.
- **T2, runtime:** inheritance, macros, includes, escaping, whitespace,
  dynamic values and extension calls pass the supported upstream fixtures.
  A small compiled-template application exercises the converter contract.
- **T3, bridges and demo:** wait for each Form, Translation, Security,
  Asset and Doctrine provider actually used. The demo renders the original
  `.twig` files unchanged; Askama is removed; the README's "whitespace
  between tags" difference and its `compare/diff.sh` normalization are
  deleted, and all 92 pages match.

T1/T2 completion does not mark Twig or TwigBridge complete; unimplemented
extensions and sandbox support retain pending coverage until accepted.

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
M5-A accepts providers and their integrations one at a time. M5-B acceptance,
after the demo's required M6 integrations: the demo's `src/` holds only the
PHP app's own code (controllers, entities, forms, commands, subscribers,
Twig extensions). Every framework part comes from crates.

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
and the database-diff recording from phase 9 is identical. Stage it as:

- **D1, DBAL:** accept SQLite types, schema/query behavior and transactions
  against the oracle first. PostgreSQL and MySQL/MariaDB remain committed
  targets, each requiring its own database-backed comparison suite before
  that platform is claimed supported.
- **D2, ORM:** record and pass identity-map, UnitOfWork, hydration, lazy
  association, DQL, pagination, rollback and lifecycle-event scenarios.
  Define the allowed build-time DQL inputs and located refusals explicitly.
- **D3, application integration:** enable DoctrineBundle/Bridge, migrations
  and fixtures as their tests pass; close deferred M5 entity integrations,
  run the demo database recording, and replay the second app's relevant
  persistence flows. DBAL completion alone does not claim ORM parity.

### M7: Async and integration
Messenger (buses, middleware, retries, failure transport; transports
sync/in-memory/Doctrine/Redis/AMQP via `lapin`; PHP-serialized envelopes
decoded as data only, so PHP and Rust workers can share a queue during a
migration), Scheduler (cron rules from dragonmantank/cron-expression),
Lock, Semaphore, RateLimiter (ported algorithms; governor uses another),
Workflow, Notifier (core + the most-used bridges, by client demand),
Webhook/RemoteEvent, Console complete (QuestionHelper, ProgressBar, etc.).
Acceptance: extend the second app selected in M1-D with recorded async and
integration workflows that exercise the components being accepted. The PHP
copy runs as an oracle only. Finish remaining Console behavior and close
its pending tests; command infrastructure was introduced in M1-B.

### M8: Test tooling and the symfony2rust contract
BrowserKit, DomCrawler, CssSelector (CSS→XPath semantics; evaluate
`scraper`/`selectors` first), FrameworkBundle's KernelBrowser and
`WebTestCase` assertions, so converted client apps keep their functional
tests. Stabilize and semver-check the generated-code API first exercised in
M1-D. Expand its fixture corpus, requiring actual symfony2rust output to
build without manual fixes and pass the recorded workflows. The converter
itself remains php2rust's workstream.

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
3. **PHP value semantics.** Use symfony-rs's shared `php-value` layer. The
   original recommendation to depend directly on php2rust's `php-rt` was
   superseded by the owner's decision below. Document the runtime boundary
   in php2rust's approach: converted apps link the MIT/BSD-derived Symfony,
   Twig and Doctrine runtime; converter crates retain their own policy.

   **Owner, November 19 (at the Yaml start):** symfony-rs owns the value
   layer. A new `php-value` crate holds PHP's values (null, bool, int,
   float, byte strings, ordered arrays with PHP's key rules, objects such
   as stdClass, DateTimeImmutable, enum cases and component classes like
   TaggedValue), `Send + Sync`, written from recorded PHP behaviour with no
   php-src code. Yaml, Config, DI and Twig share it; php-rt can adopt it or
   convert to it later. Reason: php-rt has no objects, is `Rc`-based, and
   its `round()` follows pre-8.4 PHP. Also decided: PHP's `serialize()` /
   `unserialize()` are ported (Yaml's `PARSE_OBJECT` / `DUMP_OBJECT`),
   with unserialization limited to an allow-list of classes the
   application registers.
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
   3.01. The initial alternatives (BSD-3-Clause source or a policy exception)
   were superseded by the owner's decision below; neither is the chosen
   implementation path.

   **Owner, November 19:** no PHP source code at all in the rewritten
   projects, under any licence, including php-src's BSD-3-Clause main
   branch. Behaviour comes only from running PHP (in containers) and from
   documentation. So `multipart.rs`, `query.rs` and `cookies.rs` must be
   rewritten from their recorded vectors without php-src (the explicit
   provenance gate now precedes M1-D HTTP linkage and M2-A acceptance).
   Where PHP's behaviour cannot be matched that way, the difference is recorded.

9. **XML (ext-dom / libxml).** Config's `Util/XmlUtils.php` (and through
   it the DI and Routing XML loaders, Translation's XLIFF loader, Validator
   and Mime) needs `DOMDocument::loadXML`, `schemaValidateSource` and
   libxml's error list.

   **Owner, November 19 (at the Config start):** a hand-written port, in a
   `php-dom` crate in this workspace beside `php-value` (PHP behaviour,
   shared by several components), not a separate repository. Written from
   behaviour recorded in the oracle (decision 8): libxml's error texts,
   levels, codes, lines and columns are recorded vectors. `LIBXML_NONET`
   is enforced by refusing external entities and DTD fetches outright.
   XSD validation covers the features used by the schemas Symfony ships
   (DI, Routing, XLIFF 1.2/2.0); a schema using anything else is refused
   with its location. Needed at `XmlUtils` (Config stage 4); the parser
   comes first, validation after.

10. **Second application (M1-D).** Recommended:
    [EasyCorp/easyadmin-demo](https://github.com/EasyCorp/easyadmin-demo)
    at `119c6df` (MIT, Symfony `^8.1`, PHP ≥ 8.4, SQLite). It was the only
    maintained candidate already on the pinned 8.1 line
    ([docs/acceptance/second-app.md](docs/acceptance/second-app.md) lists
    the others). Two points are open for the owner:
    - **Patch level.** Its lock has v8.1.11 (contracts v3.8.1) for 38 of
      the 60 packages pinned here, which come from symfony/demo at
      v8.1.10. Recommended: record this app with its own lock, naming the
      lock in each comparison, and keep the shared pins at the demo's
      until a deliberate version move.
    - **Production boot.** Its `when@prod` Doctrine config sets two options
      DoctrineBundle 3 removed, so `APP_ENV=prod` fails in Config's
      ArrayNode. Recommended: run the oracle copy (under `oracle/out/`,
      never `upstream/`) with only those two lines removed, keeping the
      patch in the repository and the reason in each recording, and offer
      the fix upstream once the owner agrees. Alternatives: record `dev`
      (profiler headers and toolbar) or `test` (mock sessions), or wait
      for an upstream fix.

    **Owner, November 19:** both recommendations accepted. The EasyAdmin
    demo is the second app; it is recorded with its own lock; the oracle
    copy applies `oracle/apps/easyadmin-demo/prod-boot.patch` (the two
    Doctrine lines) and nothing else.

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
- [x] Oracle image (`oracle/`, `php:8.4.27-cli-bookworm` + intl, pdo_sqlite;
      mounts this repo and `../symfony-demo` read-only). A generic `run.php`
      for JSON-described cases comes with the first recorder moved in.
- [x] Parity manifests: `oracle/manifest.php` lists upstream tests with PHP's
      tokenizer (no code loaded, so packages whose tests need uninstalled
      components work too), `scripts/manifest.sh` merges into
      `parity/<package>.tsv` keeping statuses, `scripts/parity-check.sh`
      gates CI (statuses, claimed Rust tests exist, every upstream-porting
      Rust `test*` fn is claimed, `parity/done.txt` packages have no
      pending rows). Checked against PHPUnit 11.5's own `--list-tests`:
      identical for event-dispatcher (140) and routing (871); PHPUnit cannot
      list http-foundation and http-kernel at all (their tests need
      RateLimiter and Serializer, not installed). Backfilled: routing 16 and
      http-kernel 10 ported; only the declaring class's row is marked, so
      inherited copies (e.g. Compiled matchers) stay pending.
- [x] Recorders for this repository's vectors moved from symfony-demo-rs
      into `oracle/recorders/` (`scripts/record.sh routing|multipart`); one
      oracle container each, and both re-recorded files are byte-identical
      to the committed ones (90 routing cases, 156 multipart bodies). The
      demo's other `data/*/extract.php` recorders move with their modules
      (M1, M5).
- [x] Upstream watch: `scripts/upstream_watch.py` (GitHub advisories whose
      range holds a pin; every advisory of the last 400 days for ported
      packages; newer patch releases and the mapped files they change),
      weekly in `.github/workflows/upstream-watch.yml`, which keeps one
      "Upstream watch" issue up to date. Triage lives in
      `parity/advisories.tsv`.
- [x] Drill (November 19), on real advisories rather than a staged one. The
      first run listed six advisories for ported packages, all fixed before
      the pins:
      - GHSA-r925-mgvr-h9cc (`HeaderUtils::split` ReDoS, fixed in 8.1.7):
        the fix's upstream test is ported under its name. The oracle shows
        v8.1.6 failing it (PCRE backtrack limit, empty results) and the pin
        giving exactly the Rust results.
      - GHSA-h5x3-xfc9-m39h (UrlGenerator chained dot segments): **a real
        gap**. The whole-path dot-segment step had not been ported, so a
        caller passing `a/../b` got a path that collapses off-route.
        Ported as `generator::encode_dot_segments` with the two upstream
        tests. The demo was not reachable (its parameters cannot be dot
        segments).
      - Four not-ported verdicts (IpUtils, PATH_INFO, `#[IsGranted]` HEAD,
        regex requirements), each with the milestone that must revisit it.
      The same run found **v8.1.11** of http-foundation, routing and
      http-kernel. Triage of the mapped files it changes:
      `HeaderUtils::makeDisposition` (`/D` anchor), `prepareBaseUrl` /
      `getUrlencodedPrefix` (client-chosen `//host` base URL) and
      `setTrustedProxies` (`PRIVATE_RANGES`) are none of them ported, so
      the Rust code is unaffected. Pins stay at the demo's `composer.lock`
      (the demo original is read-only); moving them is a version move.

## M1 progress

**Scheduling adjustment, November 19 (owner approved):** finish the current
Config Definition/Builder step, then establish the M1-D converter contract
and second-app inventory, doing the request rewrite before the HTTP fixture
links those crates. Use the delivery table for subsequent prerequisites.
These new gates are pending; this plan edit does not mark implementation,
conversion, XML validation or fresh comparisons complete.

Stack rows (searched November 19):

| Component | Options compared | Choice | Reason |
|---|---|---|---|
| Stopwatch | [fine_grained](https://docs.rs/crate/fine_grained/latest) (one stopwatch with laps, 2017), [ticky](https://docs.rs/ticky/1.0.2/src/ticky/lib.rs.html), [chronometer](https://docs.rs/crate/chronometer/latest), clock-core: single anonymous timers | Port, no dependency (`std::time` underneath) | None has named events, categories, sections, periods with memory, PHP's rounding or Symfony's messages |
| Yaml | No Rust port of Symfony Yaml exists. YAML 1.2 crates ([saphyr](https://docs.rs/serde-saphyr/latest/), [yaml-rust2](https://docs.rs/crate/yaml-rust2/0.11.1), [noyalib](https://github.com/sebastienrousseau/noyalib), [saneyaml](https://docs.rs/crate/saneyaml/latest)) implement the spec, not Symfony's dialect; serde_yaml is archived | Port on `php-value` | Symfony's booleans, `0o` octals, timestamps as ints, merge keys, `!php/*` tags, flags and messages are its own |
| Regex for ported PCRE patterns (owner, November 19) | `regex` 1.13 (RE2-style, linear time, no backtracking) vs hand-written matchers vs `fancy-regex` (lookaround, backtracking) | `regex`; patterns without its features (Yaml's `(?<!:):(?!:)` delimiter) matched by hand | Upstream patterns translate one for one and stay comparable; linear time rules out ReDoS; possessive quantifiers become greedy where the following class excludes what they consume |
| Dotenv | [dotenvy](https://crates.io/crates/dotenvy), [dotenvs](https://docs.rs/crate/dotenvs/0.2.1/source/README.md) (`${VAR:-default}`), [zenv](https://github.com/numToStr/zenv) (expansion only in double quotes), [dotenvx](https://docs.rs/crate/dotenvx/0.1.0) (encryption, shell commands): other parsers and expansion rules | Port on `php-value`, no dependency | None has Symfony's syntax errors, `.env.local` / `.env.$env` layering, `SYMFONY_DOTENV_VARS`, deferred cross-file resolution or `$_ENV` / `$_SERVER` / `getenv()` precedence |
| Config | No Rust port of Symfony Config found. Serde-struct config crates: [schematic](https://docs.rs/schematic) (per-field merge strategies, garde validation; the closest), [clapfig](https://docs.rs/crate/clapfig/latest) (layered files/env/CLI, post-merge hook), [confique](https://awesome.ecosyste.ms/projects/github.com%2Flukaskalbertodt%2Fconfique) / [einstellung](https://docs.rs/crate/einstellung/0.1.6) (derive-time defaults and validators), [rust-config-tree](https://docs.rs/crate/rust-config-tree/0.2.0) (include trees on confique + Figment), config-rs / figment (untyped layering) | Port on `php-value`, no dependency | All of them fix the schema as a Rust type. Symfony's is a node tree built in code by bundles (prototypes, `ExprBuilder` if/then rules, normalization keys, `canBeEnabled`, deep merge vs `performNoDeepMerging`, deprecations), with its own error paths and messages |
| XML for `XmlUtils` (owner, November 19; decision 9) | [roxmltree](https://docs.rs/roxmltree) / [quick-xml](https://docs.rs/quick-xml) (parsers, no validation); libxml2 bindings (C, FFI, the real libxml but `unsafe` underneath); pure-Rust XSD: [uppsala](https://docs.rs/crate/uppsala/0.4.0) (DOM, XPath, XSD 1.1), [xsd-schema](https://docs.rs/crate/xsd-schema/0.1.0), [xmlschema](https://docs.rs/xmlschema), [fastxml](https://docs.rs/crate/fastxml/0.1.0) | Hand-written `php-dom` crate, no dependency | None reproduces libxml's error messages, levels and positions, which `XmlUtils` puts into its exception texts; FFI conflicts with no-`unsafe`; scoping validation to Symfony's schemas keeps the surface small |
| EventDispatcher + contracts | [mod-events](https://docs.rs/crate/mod-events/latest) (typed per-event-type bus, priorities, no names, no propagation stop, failures collected rather than raised); [event_dispatcher 0.0.2](https://docs.rs/event_dispatcher/) (stoppable trait, unbuilt docs, abandoned); Serenade's events (conceptual only); evee (a Python port, design reference) | Port, no dependency | None reproduces named events, listener identity (`===` / closure `==`), lazy listeners or the observable ordering rules |
| Converter runtime contract (M1-D) | [PeachPie](https://github.com/peachpiecompiler/peachpie) (dynamic `PhpValue` everywhere, per-request `Context`), [KPHP](https://vkcom.github.io/kphp/) (inferred types, `class_instance<T>`, `mixed` only where dynamic), Symfony's `PhpDumper` and [Quarkus ArC](https://quarkus.io/guides/cdi-reference) (container compiled at build time), DI crates [shaku](https://docs.rs/crate/shaku), [fundle](https://docs.rs/crate/fundle/0.3.5/source/README.md), [injectable-rs](https://docs.rs/injectable-rs) | Typed code with `php-value` only where PHP is dynamic; the compiled container emitted as Rust; no DI crate | Types keep speed and reviewability; the converter already has Symfony's full service graph, so a generic container would duplicate it; PHP-FPM's per-request isolation decides which services may be shared ([docs/converter-contract.md](docs/converter-contract.md)) |
| PHP request parsing (`query`, `cookies`, `multipart`; M1-D provenance gate) | [serde_qs](https://github.com/nicklaswj/serde_qs) (Rack/qs brackets, repeated keys collected into a `Vec`), [serde-querystring](https://docs.rs/serde-querystring) ("brackets like PHP", no name rewriting or limits), `form_urlencoded` (pairs only), [multer](https://crates.io/crates/multer) (strict RFC 7578 multipart) | Rewritten here from the PHP manual and recorded PHP answers, no dependency | None has PHP's name rewriting (`.`/space/unclosed `[` to `_`), its integer-key and next-index rules, its separate query/body/cookie limits, cookie first-wins, or its lenient multipart reading (`--boundary--` not ending the body, partial-line delimiters); php-src is excluded by decision 8 ([docs/milestones/m1-d-request-rewrite.md](docs/milestones/m1-d-request-rewrite.md)) |
| Console (M1-B subset; searched November 20) | No Rust port of Symfony Console (searches find only the PHP original and Node ports such as [symfony-style-console](https://www.npmjs.org/package/symfony-style-console)); [clap](https://docs.rs/clap) (its own argv grammar, errors and help), [clap_complete](https://docs.rs/clap_complete) (its own completion protocol), [comfy-table](https://github.com/Nukesor/comfy-table) / tabled (their own layout and wrapping); symfony-demo-rs's `src/console` (a partial port arranged around the demo's commands) | Port on `php-value`, no dependency; move the demo's measured width table in | None reproduces Symfony's ArrayInput/ArgvInput binding and messages, SymfonyStyle's blocks and spacing, Table's layout, or `_complete`'s input model. Process: none of the three commands M1-B needs (Yaml `lint:yaml`, Dotenv `debug:dotenv` and `dotenv:dump`) uses symfony/process, so M1-B ports no Process behaviour |

- [ ] EventDispatcher. Ported: `EventDispatcher.php`,
      `EventDispatcherInterface.php`, `EventSubscriberInterface.php`,
      `GenericEvent.php`, `ImmutableEventDispatcher.php`, the contracts'
      `Event.php` / `EventDispatcherInterface.php` (crates
      `symfony-event-dispatcher`, `symfony-event-dispatcher-contracts`),
      and behind the `debug` feature `Debug/WrappedListener.php` and
      `Debug/TraceableEventDispatcher.php`: 114 of 140 upstream tests,
      plus oracle vectors for what the tests leave open
      (`scripts/record.sh event-dispatcher`). Pending: the DI passes and
      `#[AsEventListener]` (26 tests), which belong to M3's container
      compiler.
- [x] psr/log (`psr-log`, pinned 3.0.2 from the demo's lock file):
      LoggerInterface, LogLevel, NullLogger. No upstream tests.
      symfony-http-kernel keeps its own dispatcher until M2: its events
      borrow the request, and type-erased events must be `'static`.
- [x] Stopwatch (`symfony-stopwatch`): all 33 upstream tests; in
      `parity/done.txt`. StopwatchEventTest runs on a mocked clock, as
      upstream's `time-sensitive` group does. The PHP built-ins it needs
      (`round()`, `(int)`, `%d`, `%.2F`) are checked against 3,000+ vectors
      (`scripts/record.sh stopwatch`). `memory_get_usage()` has no Rust
      equivalent: the default host reports 0, and a host can supply a figure.
      `round()` is written from behaviour (no PHP source, per decision 8):
      half away from zero with half-way points computed as doubles. On
      171,025 recorded values it matches PHP below `2^52 / 10` and differs
      on 122 above it (crate README, "Differences"). Compared: rounding the
      shortest decimal form (3,270 differ, from 7e13) and php2rust's
      `php-rt` `round()` (14,571 differ, from 2.35: it implements the
      pre-8.4 15-digit pre-rounding, a php2rust bug to report).
- [ ] Yaml (`symfony-yaml`, values in `php-value`). Ported: every file
      but `Command/LintCommand.php` (after Console): 329 of 342 upstream
      tests, 2 `n/a` with their reasons in the manifest, 11 pending
      (LintCommandTest). ParserTest, DumperTest, YamlTest and
      YamlTestSuiteTest replay every call each upstream test makes to the
      parser and the dumper, recorded on the oracle while the upstream
      test runs and passes (`oracle/recorders/yaml_test_calls.php`);
      YamlTestSuiteTest runs on yaml-test-suite at the commit symfony/yaml
      pins (`upstream.tsv`), 200 cases, upstream's skip list.
- [ ] Dotenv (`symfony-dotenv`): `Dotenv.php` and the exceptions, all 38
      DotenvTest tests (also run on Linux: the `$(...)` cases skip on
      Windows, as upstream's do) and 231 recorded `parse()` inputs. PHP's
      globals are a `Globals` value; `putenv()` is an overlay, since
      setting the process environment is `unsafe`. `.env.local.php` is
      read as dump-env's literal array, not run. Upstream loops forever on
      a value ending in `$(`; the port fails instead (crate README,
      "Differences"). Pending: the two commands (19 tests) after Console.

- [ ] Config (`symfony-config`, values in `php-value`). Ported: the
      Definition nodes (`BaseNode.php` and the interfaces, VariableNode,
      ScalarNode, BooleanNode, NumericNode, IntegerNode, FloatNode,
      StringNode, EnumNode, ArrayNode, PrototypedArrayNode), the Definition
      exceptions, `Exception/LogicException.php` and `Processor.php`
      without `processConfiguration()`: 75 of 334 upstream tests, plus
      1,938 recorded cases (`scripts/record.sh config`). Placeholders are
      thread-local; PHP's built-ins it needs are written from documentation
      and checked by those cases. `trigger_deprecation()` is the new
      `symfony-deprecation-contracts` crate. Stage 2: `Definition/Builder/*`
      (TreeBuilder, NodeBuilder, the node definitions, ExprBuilder and the
      merge, normalization and validation builders),
      `processConfiguration()` and ConfigurationInterface: 92 more tests
      (167 of 334), plus 4,624 recorded cases (`config_builder.php`),
      which also found that a float `$children` is truncated to an int,
      not cast to a string (fixed in PrototypedArrayNode too).
      `docUrl()` reads Composer's versions from a registry the application
      fills. This completes M1-A's Config Definition/Builder step; next
      is M1-D (scheduling adjustment below). Config stays partial:
      deferred are the reference dumpers and `ArrayShapeGenerator` (no new
      prerequisite; before M1-C, which needs them), `Definition/Loader`,
      `Definition/Configurator` and `Definition/Configuration.php` (they
      need Config's loaders; M1-C), and loaders, resources and `XmlUtils`
      (Filesystem, Finder, `php-dom`, decision 9; M1-C). Their 167 rows
      stay `pending`.
- [ ] M1-D. Done: provisional converter contract 0.1
      ([docs/converter-contract.md](docs/converter-contract.md)), with
      prior art compared (stack row above); the second app recommended
      (decision 10, EasyAdmin demo pinned in `upstream.tsv`) and
      inventoried in `prod` with the two-line Doctrine overlay
      ([docs/acceptance/second-app.md](docs/acceptance/second-app.md):
      142 routes, 872 services, ranked component work, first workflow W1).
      Decision 10 accepted by the owner. The request provenance gate
      passed (rewrite of `query.rs`, `cookies.rs`, `multipart.rs`; stage
      log [docs/milestones/m1-d-request-rewrite.md](docs/milestones/m1-d-request-rewrite.md)),
      which was needed because `symfony-routing` links
      `symfony-http-foundation`. W1 recorded from the second app (40
      responses, `scripts/record.sh second-app-w1`). Fixture done, hand-authored
      ([docs/milestones/m1-d-converter-fixture.md](docs/milestones/m1-d-converter-fixture.md)):
      PHP app `oracle/apps/converter-fixture` (16 recorded responses),
      Rust crate `fixtures/converter-app` matching all 16, a local Linux
      run from an empty directory (`scripts/converter-fixture.sh`), contract
      0.2, and `Exception::from_throwable` in `symfony-http-kernel` (demo
      comparisons unchanged against a control). Its `runtime` module lists
      the HttpFoundation/HttpKernel/FrameworkBundle gaps M2-A must close.
      The conversion gate stays open until symfony2rust emits the fixture.

## Verification

- Per complete package: manifest has no `pending` rows (CI-enforced);
  upstream tests under their original names pass; PHP-recorded vectors pass; fuzz corpus
  replay against the oracle shows no differences.
- Per bounded stage: all named files/tests and vectors pass; deferred
  integrations retain `pending` rows, dependencies and destination stages.
  A stage exit never overrides the complete-package CI gate.
- Workspace: `cargo fmt --check`, `cargo clippy --all-targets -- -D
  warnings`, `cargo test` in the new repo and in symfony-demo-rs.
- Application: after every component the demo uses, run
  `docker compose -f compare/compose.yaml up`, `compare/capture.sh` /
  `compare/diff.sh`: 92/92 pages, 124/127 live cases (the three documented
  filesystem-case artifacts). At T3 remove whitespace normalization only
  after the original templates match; at D3 the database-diff recording
  must also match. Use the candidate runtime checkout via the demo's local
  `[patch]` override, recording both revisions and overrides in the log;
  testing only the demo's older pinned revision does not validate a runtime
  change.
- `debug:*` JSON output matches real Symfony's for the demo (M3-A on).
  The converter fixture and second app begin at M1-D, with supported flows
  extended at each provider stage. Record whether output is hand-authored
  or actually generated; only the latter closes a conversion gate.
- Upstream process drill at M0 end: replay one past Symfony security fix
  through the watch job → issue → failing test → fix.
- Housekeeping per AGENTS.md: commit per ported file naming the PHP source,
  debug builds only, stop comparison containers, never commit databases,
  `target/`, `static/assets/` or `compare/out/`.
