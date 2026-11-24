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

**Crates generally follow Symfony packages**, with names matching Composer (`symfony-yaml`,
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

### Progressive static specialization (cross-milestone plan, November 24)

Preserve PHP behaviour, not the runtime cost of its mechanisms where that
cost is unnecessary. This is an extension of the typed-core/build-time
compiler architecture above and the M1-D converter contract, not a new
blocking milestone. Continue the existing dependency-ordered ports.

**Design rules.** Prefer concrete types, generic traits for useful static
dispatch, and enums for closed sets. Keep trait objects for open extension
points and where specialization is not worthwhile. Keep `php-value` for
observable PHP semantics (mixed types, ordered arrays, byte strings, coercion,
object identity, reflection and dynamic dispatch). Convert at explicit
boundaries only when semantics are preserved; no silent approximation or
unsupported-input fallback that bypasses the refusal rule. Security, parity,
ownership and request isolation precede performance. Do not count generic
parameters or trait objects as a performance metric.

**Current refactor scope (now / next non-conflicting bounded stages):**
- [ ] Record these rules in the converter contract (coordinate with
      php2rust), and audit generated-code-shaped fixture types. Preserve the
      existing concrete container, typed controller enum, typed routes and
      generic `Kernel<C>::handle<R, I, F>`. Prefer declared scalar, nullable
      and class types; keep PHP coercion and error handling at boundaries.
- [ ] During the next HttpKernel stage (M2-A), introduce a minimal listener
      execution seam, e.g. `Kernel<C, L = DynamicListeners<C>>` with a
      listener pipeline trait. Keep the existing dynamic registration API
      and default behaviour; prove the seam with a small static fixture.
      If the kernel interface is still changing, defer the seam until its
      lifecycle contract is stable rather than blocking the current port.
      Test priority, equal-priority order, propagation, controller replacement,
      early response, exceptions and response filtering in both modes.
- [ ] Keep the general EventDispatcher's dynamic API and observable mutation,
      lazy loading and identity semantics. Document a typed alternative
      boundary, but do not rewrite that component or duplicate its logic now.
- [ ] Inventory dynamic `Value`, `PhpArray`, `Arc<dyn ...>`, boxed futures,
      locks and allocations on fixture request paths. Record why each is
      required or a candidate for later removal; no blanket rewrite.
- [ ] Establish reproducible baseline measurements on the fixture: latency,
      throughput, allocations, memory, binary size and build time. Use the
      same workloads, profiles and environment for comparisons. No new
      dependencies without the existing stack review.

**Implementation gates (all pending):**

| Existing milestone / trigger | Specialization work and exit evidence |
|---|---|
| M1-D and early converter work | Map declared types, nullability, concrete classes and safe closed unions to Rust; preserve PHP byte-string, array, overflow/coercion and identity semantics where needed. Use `Value` where types or semantics remain dynamic. Emit and build the actual fixture, compare PHP outcomes; hand-authored code cannot close the converter gate. |
| M2-A HTTP lifecycle and routing | Validate the listener seam and typed route/controller execution with oracle and differential tests. Keep dynamic behaviour available; do not optimize routing before the matcher contract is proven. |
| M3-A DI and event integration | Use the compiled service graph to emit concrete constructors, correct scopes, lazy services and typed references (incorporate the DI trial findings). Generate direct per-event listener pipelines only for provably closed sets; preserve priorities, registration/mutation semantics and dynamic escape paths. Test static vs dynamic and PHP, including failures. |
| M4–M6 provider stages | Extend conservative local/collection type inference and typed access to Twig, Form, Validator, Serializer and Doctrine as each provider becomes usable. Verify every specialization against its compatibility path and pinned PHP; heterogeneous arrays and uncertain calls remain dynamic. |
| Demo and second-app acceptance | Benchmark actual generated workflows including database/authentication, bundles and dynamic extensions. Optimize measured hot paths (dispatch, matcher, allocation, locking); require no parity regression. Demo comparisons remain paused by the owner until explicitly resumed; no dependent gate closes meanwhile. |
| M7–M8 integration and API stabilization | Expand whole-application call/service/event graph analysis where safe; verify dynamic interoperability. Stabilize documented interfaces after evidence from both applications. Track runtime gains against binary size, build time and maintainability. |

**Verification.** Each bounded implementation stage keeps the existing
AGENTS.md checks, parity manifests, source mappings and stage logs. Add
differential tests of static and dynamic paths against the pinned oracle,
including error text, ordering, mutation, ownership and asynchronous request
isolation. Record baseline and candidate revisions, workload, build profile,
hardware and repeated measurements; do not claim speedups without evidence.
Follow the debug-build-only rule; release-profile benchmarking and additional
profiling tools (e.g. cargo llvm-lines, cargo bloat, cargo asm, Criterion)
require owner approval where they change current rules or dependencies.
Do not add hosted paid CI. Defer broad specialization and allocation tuning
until representative workloads expose a measurable bottleneck.

### Code consolidation and idiomatic Rust (cross-milestone plan, November 24)

Reduce accidental complexity and maintain idiomatic Rust throughout the
port, starting now. This is distinct from static specialization: fewer
lines, fewer allocations and fewer generic instantiations are different
objectives. Preserve security, PHP/Symfony observable behaviour, source
traceability and independently useful package boundaries. Do not pause
the dependency-ordered port for a workspace-wide rewrite.

**Package boundaries (owner requirement, revised November 24).** Prefer
independently reusable crates where useful, but one Rust crate per PHP
package or dependency is not mandatory. Group cohesive components or helpers
when that reduces complexity and dependency overhead. Preserve standalone
consumption of useful components without the whole framework or converter. Keep crate names, public contracts and feature boundaries
explicit; keep application-specific code outside reusable crates.
Consolidation means sharing proven common implementations and allowing
cohesive grouping, not merging everything into a monolithic runtime. Lower-level PHP semantics and primitives
belong in narrowly scoped shared crates (such as the existing php-value,
php-pcre and php-dom); prefer the standard library where its semantics
match. Use optional dependencies for genuine integrations, keep dependency
direction acyclic and avoid umbrella dependencies, duplicated implementations
or unnecessary abstraction crates. Preserve `publish = false` until the
owner's existing naming/publication decision; future independent publication
is a design goal, not authorization to publish now.

**Start now:**
- [ ] Inventory source size by crate: handwritten production Rust, tests,
      generated source/tables, fixtures/oracle data and documentation
      separately. Record dependency counts and repeated implementations.
      Use this as a baseline, not a line-count quota; do not treat parity
      tests, required Unicode/ICU tables or recorded vectors as waste.
- [ ] Audit the existing shared helpers and obvious duplicate algorithms
      during the next affected component stages. Examples to investigate:
      byte/string conversions, path handling, PHP array operations,
      comparison/coercion, repeated parsers, error construction and lookup
      helpers. The existing path policy and php-value loose equality
      consolidation are precedents. Verify semantics before sharing code.
- [ ] Make the idiomatic Rust checklist below part of every new port and
      bounded refactor. Fix obvious local issues as files are touched;
      schedule cross-crate changes as separate reviewable stages, avoiding
      conflicts with active branches.
- [ ] Audit public APIs and crate dependency graphs for independent use:
      build/test each crate with its supported feature sets, including
      minimal features when applicable. Check for accidental dependencies
      on the converter, application fixtures or unrelated framework crates.

**Idiomatic Rust checklist (apply to existing and new code):**
- Model invariants with concrete structs, enums, newtypes, Option and
  Result; use exhaustive matching and typed errors. Avoid PHP-style
  inheritance emulation, string-based dispatch and dynamic values unless
  required for compatibility.
- Prefer borrowing, slices and suitable standard collections; use iterator
  methods, pattern matching, entry APIs and error propagation where they
  improve clarity. Simple loops are fine. Avoid gratuitous clone/to_owned,
  allocation, boxing, Arc, Mutex, redundant conversions and intermediate
  collections. Preserve ownership, identity and async isolation.
- Prefer standard traits and conversion APIs; use generics only where
  they simplify reuse or provide a justified specialization. Avoid
  speculative abstractions, wrappers that add no invariant, needless
  getters/setters and macros that hide important behaviour.
- Keep visibility narrow and modules cohesive. Share canonical helpers
  without erasing upstream source mappings or making independent crates
  depend on unrelated framework packages. Keep dependencies small and
  document alternatives under the existing stack-review rule.
- Follow rustfmt and Clippy; investigate warnings rather than suppressing
  them broadly. Document necessary compatibility exceptions. No unsafe
  code, panics on external inputs or weakening of tests.

**Scheduling and exit gates:**

| Stage | Scope and evidence |
|---|---|
| Now / ongoing bounded ports | Baseline inventory, idiomatic checklist, local simplification and obvious duplicate removal; parity and source checks remain mandatory. |
| M1/M2 foundations and HTTP | Consolidate shared semantics when two or more proven consumers need them; prefer existing shared crates. Keep native and compatibility paths sharing behaviour cores where practical. |
| M3–M6 integration | Review crate boundaries, feature dependencies, public contracts and converter-generated boilerplate as providers stabilize. Avoid premature API churn or rewrites. |
| Representative app acceptance | Profile actual generated applications before broader allocation/dispatch changes; compare code size, binary size, build time and runtime metrics separately. |
| M7–M8 stabilization | Review all reusable crate contracts, standalone consumption and documentation; check minimal dependency/feature combinations and remove obsolete internal duplication. Publication remains a separate owner decision. |

**Acceptance.** Every refactor must preserve the pinned upstream tests,
oracle vectors, security fixes, error messages and other observable
behaviour. Retain existing source mappings and explain shared
implementations in the affected crate contracts. Record before/after
measurements and rationale for cross-crate refactors in the stage logs;
no arbitrary LOC-reduction target. Run the existing local verification
checks, with affected-crate feature checks as appropriate. The demo
comparison pause stays in force until the owner resumes it; no dependent
acceptance gate closes meanwhile. Do not add paid hosted CI or change the
debug-build-only rule. Prefer deleting redundant code over adding a
second implementation unless the measured benefit or compatibility need
justifies maintaining both.

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

The gates run locally, before every merge (owner, November 23: no paid CI
jobs; the GitHub-hosted `.github/workflows/checks.yml` is removed, and
"CI" below means these local checks). Tests that only run on Unix are run
on a Windows host with `scripts/linux-test.sh`, in a removed Linux
container. The weekly upstream watch stays a scheduled workflow.

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
**Owner, November 23:** the symfony-demo-rs comparison step is paused until
the owner turns it back on (AGENTS.md, "Checks"); stages log that it was
not run, and no acceptance that requires it is marked passed meanwhile.

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
  Linux CI (extend the existing `.github/workflows/checks.yml`; removed
  November 23, the gates now run locally).
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

**Trial, November 24 (owner request):** [spikes/di-trial](spikes/di-trial/README.md)
emits symfony/demo's compiled container (548 services, recorded from PHP
after the compiler passes) as Rust in `build.rs`; every service resolves,
with shared, non-shared, lazy, request and application scopes and PHP's
order for cycles through method calls. It passes no gate and tests only
the back half (graph to Rust), not loaders or passes in Rust. Findings for
M3 (64% of services become request-scoped by propagation from
`kernel.reset`; XmlDumper's file is incomplete as an interface; declared
classes are not always the runtime type; Twig's call cycle; lazy services;
env processors) are in [docs/milestones/m3-di-trial.md](docs/milestones/m3-di-trial.md).

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

**Trial, November 24 (owner request):** [spikes/twig-trial](spikes/twig-trial/README.md)
compiles the demo's `blog/_post.html.twig` (with its include) at build time;
its output is byte-identical to Twig 3.30 for 5 cases and 10 language
snippets. It passes no gate. Findings for M4 (an attribute-lookup hook and
PHP `==` in `php-value`, build-time extension set, compile-time
`is_safe_callback`, lines on every node, strict variables) are in
[docs/milestones/m4-twig-trial.md](docs/milestones/m4-twig-trial.md).

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

   **Owner, November 22:** php-src is no longer fetched. `upstream.tsv`
   had listed it at php-8.4.27 as "reference only"; that row is removed
   and local `upstream/php-src` checkouts are deleted, so nothing in the
   workflow can read it.

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
      until a deliberate version move. (November 23: decision 16 moved
      http-foundation, http-kernel and routing to v8.1.11; the other
      shared pins stay at the demo's lock.)
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

11. **Non-UTF-8 command-line arguments (M1-B).** PHP's `$_SERVER['argv']`
    holds bytes; `symfony-console`'s ArgvInput tokens are `String`s, so
    `Input::argv_from_server()` refuses an argument that is not UTF-8
    (for example a Latin-1 file name passed to `lint:yaml`) with a
    `RuntimeException`, where PHP would accept it. Recommended: keep the
    refusal for M1-B (none of the recorded vectors uses such a name) and
    move tokens to `PhpString` when Console is completed in M7.
    Alternative: move them now, before the commands are ported.

12. **`callable` allowed type (OptionsResolver).** PHP's `is_callable()`
    accepts function-name strings (`'strlen'`) and `[object, 'method']`
    arrays; `php-value` has no function or method table, so
    `symfony-options-resolver` treats only closures as callable and
    rejects such strings and arrays where PHP accepts them. No upstream
    test or recorded case uses them. Recommended: keep this until the
    converter can emit a function/method registry, then answer
    `is_callable()` from it. Alternative: refuse a `callable` allowed type
    at build time.

13. **Finder's dates (`DateComparator`).** `new \DateTimeImmutable($s)`
    is PHP's whole date/time grammar. Recommended: the subset in
    `crates/finder/src/date.rs` (ISO dates and times, `@timestamp`,
    `now`/`today`/`yesterday`/..., relative amounts with `ago`, `last`,
    `next`; UTC), checked on 37 recorded strings, refusing anything else
    with its own message. Alternative: port the full grammar (month and
    weekday names, time zones, ordinals) into `php-value` now, where
    DI's `%env()%` and Twig's `date()` will need it later.

14. **XML refusals and the missing XML loaders (M1-C, November 22; accepted).**
    `php-dom` refuses, with a location, what libxml would read from a file
    or the network when `validateOnParse` is set (external DTD subset,
    external parameter entity, external entity referenced in content) and
    encodings PHP decodes through iconv. Every refused document has a
    document type or an unusual encoding: PHP's `XmlUtils::parse()` then
    throws "Document types are not allowed." or a parse error (after
    reading the file). Recommended for X3: `XmlUtils` turns a refusal
    into `XmlParsingException` with the refusal's text, a stated
    difference, rather than reading files. Also: at this pin
    symfony/dependency-injection and symfony/routing have no XML loaders,
    so decision 9's "DI and Routing XML loaders" no longer apply; X2's
    schema inventory is XLIFF 1.2/2.0/2.2 and Validator's
    constraint-mapping schema. **Accepted by the owner, November 22**:
    X3's `XmlUtils` reports a refusal as `XmlParsingException` carrying
    the refusal's text, listed as a difference in the crate README.

15. **`ByteString::fromRandom()`'s randomness (String; accepted).** PHP's
    `Random\Randomizer` reads the operating system's CSPRNG; std has no
    such API and no crate in the lock reaches it. Recommended: the caller
    passes the random bytes (`from_random(length, alphabet, random)`),
    each output byte still chosen uniformly from the alphabet, and the
    application or converter runtime supplies its OS source. Alternative:
    add `getrandom` (a stack row then) so the port reads the OS itself.
    **Owner, November 22: add `getrandom`.** `from_random()` reads the
    OS; `from_random_with()` keeps the caller's source for deterministic
    use (stack row "OS randomness").

16. **HttpFoundation's v8.1.11 hardening (M2-A, November 22; decided).** The pins
    are the demo's lock (v8.1.10). v8.1.11 adds a `/D` anchor to
    `HeaderUtils::makeDisposition()`'s ASCII check (at the pin a fallback
    filename ending in a newline passes it) and `IpUtils::PRIVATE_RANGES` with
    IPv4-mapped matching for `setTrustedProxies()`. Porting these files at
    the pin would bring in the weaker behaviour. Recommended: leave
    `makeDisposition()`, IpUtils and `setTrustedProxies()` unported until
    the pins move to v8.1.11 (a version move, with the demo's comparisons),
    then port the fixed code with its tests. Alternative: port them now at
    the pin and pin the pre-fix behaviour with a test.
    **Owner, November 22: move the pins to v8.1.11 now.** Done November 23
    ([docs/milestones/m2-a-pin-8.1.11.md](docs/milestones/m2-a-pin-8.1.11.md)):
    symfony/http-foundation, symfony/http-kernel and symfony/routing are
    pinned at v8.1.11 (none of their 8.1.11 releases needs another
    package's), past the demo's lock; the oracle mounts them over the
    demo's vendor copy. IpUtils and `makeDisposition()` are ported at
    v8.1.11; `setTrustedProxies()` comes with `Request.php`.

17. **Config's reflection-based resources (M1-C, November 23; accepted).**
    ClassExistenceResource, ReflectionClassResource and ComposerResource
    decide freshness by asking PHP at run time: whether a class can be
    autoloaded (running the autoloader, catching its errors), a hash of a
    class's reflected signatures (constants, properties, methods,
    parameters, attributes), and the mtimes of Composer's installed.json
    for the autoloaders declared. Their 23 tests load PHP code (`eval`,
    fixture classes) to change those answers. In a converted application
    classes are compiled in, so the answers are fixed at build time.
    Recommended: port them over a class registry the converter fills at
    build time (existence, a signature hash computed from the PHP source
    by the converter, Composer's vendor directories), keep their string
    forms and serialized formats, and mark the tests that change PHP code
    at run time `n/a`. Alternative: leave them unported, with DI's
    container cache relying on File and Glob resources only. The rows
    stay `pending` until the owner decides. **Owner, November 23: the
    build-time class registry.** The rows stay `pending` until that stage
    ports them; tests that change PHP code at run time become `n/a`.
18. **MIME type guessing (`File::getMimeType()`, M2-A, November 24).**
    symfony/mime's `MimeTypes` asks `FileinfoMimeTypeGuesser` first: PHP's
    `finfo`, which runs the libmagic copy and magic database bundled with
    PHP. `FileBinaryMimeTypeGuesser` (the `file` command) is upstream's
    fallback. Neither is reproducible from Rust without libmagic: on the
    472 files of the http-foundation and mime checkouts, the oracle's
    `finfo` and its `file` 5.44 differ on 13 (`application/x-empty` vs
    `inode/x-empty` for empty files, `text/x-php` vs
    `application/javascript` for 9 PHP files), and the `file` command
    differs by system and is absent on Windows. Options: (a) keep the
    `file` command as the only default guesser (done in this stage;
    Windows has none, so `getMimeType()` is upstream's LogicException
    there, and the MIME tests are skipped there as upstream skips them
    without fileinfo); (b) bind the system libmagic (`magic` 0.17 /
    `magic-sys`, FFI, needs libmagic on every host, answers follow the
    system's magic database, not PHP's); (c) a pure-Rust libmagic
    (`pure-magic` 0.5, `libmagic-rs` 0.12) with a magic database pinned to
    PHP 8.4's, checked against recorded `finfo` answers on a corpus.
    Recommended: (c) if one of them reproduces the recorded answers,
    else (a). **Owner, November 24: option (c), pure-Rust libmagic, in a
    separate PR.** Done in the M2-A finfo stage
    ([docs/milestones/m2-a-mime-fileinfo.md](docs/milestones/m2-a-mime-fileinfo.md)):
    `pure-magic` 0.5.2 with the `magic-db` 0.7.2 rules (both under their
    BSD-2-Clause option). `libmagic-rs` 0.12 has no `!:mime` support yet,
    so it gives no MIME types; `pure-magic` loading the `file` 5.45 magic
    sources (PHP 8.4's libmagic is 5.45) fails on 6 of 341 files and
    matched fewer answers than `magic-db`. No crate reproduces PHP's
    answers exactly: 12,525 of 12,843 upstream files and 107 of 124
    samples match, and each difference is listed and checked. `new
    MimeTypes()` registers it after the `file` command, as upstream, so it
    is asked first; on Windows it is the only guesser.

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
      (November 23: those three packages moved to v8.1.11, decision 16.)

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
| XML for `XmlUtils` (owner, November 19; decision 9; searched again November 22) | [roxmltree](https://docs.rs/roxmltree) / [quick-xml](https://docs.rs/quick-xml) (parsers, no validation); libxml2 bindings (C, FFI, the real libxml but `unsafe` underneath); [exml](https://docs.rs/crate/exml/latest) (libxml2 2.11.8 translated from C to Rust, parts still `unsafe`), [xmloxide](https://docs.rs/crate/xmloxide/latest) (pure Rust reimplementation of libxml2), [oxml](https://docs.rs/crate/oxml/latest) (parser, tree, XPath, entity budgets), [xot](https://docs.rs/crate/xot/latest) (tree, no DTD, UTF-8 only); pure-Rust XSD (searched again for X2): [uppsala](https://docs.rs/crate/uppsala/0.4.0) (DOM, XPath, XSD 1.1), [xsd-schema](https://docs.rs/crate/xsd-schema/0.1.0), [xmlschema](https://docs.rs/xmlschema), [fastxml](https://docs.rs/crate/fastxml/0.2.0) (validation tested against libxml, not its messages), [xsd](https://docs.rs/crate/xsd/0.5.0) (datatypes only) | Hand-written `php-dom` crate, no dependency | None reproduces libxml 2.9.14's error messages, levels, positions and recovery, which `XmlUtils` puts into its exception texts (exml follows 2.11.8, whose messages and recovery differ; 457 of 472 recorded documents match `php-dom`, the rest refused or listed, [docs/milestones/m1-c-xml-x1.md](docs/milestones/m1-c-xml-x1.md); no XSD crate gives libxml's validation codes, texts or expected-element lists, which XliffFileLoader's exceptions carry: 1,321 of 1,325 recorded validations match, [docs/milestones/m1-c-xml-x2.md](docs/milestones/m1-c-xml-x2.md)); FFI and translated `unsafe` conflict with no-`unsafe`; scoping validation to Symfony's schemas keeps the surface small |
| Translation catalogue operations (searched November 24) | GitHub search for Rust Symfony MessageCatalogue / MergeOperation found no behaviour-compatible port; [Serenade](https://github.com/Interchouette-ITC/Serenade) offers framework architecture; [rust-i18n](https://crates.io/crates/rust-i18n) and [fluent-bundle](https://crates.io/crates/fluent-bundle) use their own catalogue formats and resolution rules | Port the four `Catalogue/` files on the existing MessageCatalogue and php-value; no new dependency | Preserve lazy domain processing, fallback-aware membership, ICU/plain domain ordering and metadata; no alternative demonstrated these semantics on Symfony vectors |
| XLIFF loading (symfony/translation, November 23) | [xliff](https://crates.io/crates/xliff), [bilingua-xliff](https://crates.io/crates/bilingua-xliff), [observatory-xliff12](https://crates.io/crates/observatory-xliff12) (XLIFF readers on their own XML parsers); Rust i18n catalogues such as serenade-translation | Port of XliffFileLoader / XliffUtils / MessageCatalogue on `php-dom`, no dependency | None validates with libxml's errors (XliffFileLoader puts them in its exception messages) or reads the document as SimpleXML does (prefixed children ignored, direct text only, numeric keys, needs-translation rule, pgs units); 308 recorded cases, 306 matching and 2 listed ([docs/milestones/m1-c-xml-x3-xliff.md](docs/milestones/m1-c-xml-x3-xliff.md)) |
| EventDispatcher + contracts | [mod-events](https://docs.rs/crate/mod-events/latest) (typed per-event-type bus, priorities, no names, no propagation stop, failures collected rather than raised); [event_dispatcher 0.0.2](https://docs.rs/event_dispatcher/) (stoppable trait, unbuilt docs, abandoned); Serenade's events (conceptual only); evee (a Python port, design reference) | Port, no dependency | None reproduces named events, listener identity (`===` / closure `==`), lazy listeners or the observable ordering rules |
| Converter runtime contract (M1-D) | [PeachPie](https://github.com/peachpiecompiler/peachpie) (dynamic `PhpValue` everywhere, per-request `Context`), [KPHP](https://vkcom.github.io/kphp/) (inferred types, `class_instance<T>`, `mixed` only where dynamic), Symfony's `PhpDumper` and [Quarkus ArC](https://quarkus.io/guides/cdi-reference) (container compiled at build time), DI crates [shaku](https://docs.rs/crate/shaku), [fundle](https://docs.rs/crate/fundle/0.3.5/source/README.md), [injectable-rs](https://docs.rs/injectable-rs) | Typed code with `php-value` only where PHP is dynamic; the compiled container emitted as Rust; no DI crate | Types keep speed and reviewability; the converter already has Symfony's full service graph, so a generic container would duplicate it; PHP-FPM's per-request isolation decides which services may be shared ([docs/converter-contract.md](docs/converter-contract.md)) |
| PHP request parsing (`query`, `cookies`, `multipart`; M1-D provenance gate) | [serde_qs](https://github.com/nicklaswj/serde_qs) (Rack/qs brackets, repeated keys collected into a `Vec`), [serde-querystring](https://docs.rs/serde-querystring) ("brackets like PHP", no name rewriting or limits), `form_urlencoded` (pairs only), [multer](https://crates.io/crates/multer) (strict RFC 7578 multipart) | Rewritten here from the PHP manual and recorded PHP answers, no dependency | None has PHP's name rewriting (`.`/space/unclosed `[` to `_`), its integer-key and next-index rules, its separate query/body/cookie limits, cookie first-wins, or its lenient multipart reading (`--boundary--` not ending the body, partial-line delimiters); php-src is excluded by decision 8 ([docs/milestones/m1-d-request-rewrite.md](docs/milestones/m1-d-request-rewrite.md)) |
| Console (M1-B subset; searched November 20) | No Rust port of Symfony Console (searches find only the PHP original and Node ports such as [symfony-style-console](https://www.npmjs.org/package/symfony-style-console)); [clap](https://docs.rs/clap) (its own argv grammar, errors and help), [clap_complete](https://docs.rs/clap_complete) (its own completion protocol), [comfy-table](https://github.com/Nukesor/comfy-table) / tabled (their own layout and wrapping); symfony-demo-rs's `src/console` (a partial port arranged around the demo's commands) | Port on `php-value`, no dependency; move the demo's measured width table in | None reproduces Symfony's ArrayInput/ArgvInput binding and messages, SymfonyStyle's blocks and spacing, Table's layout, or `_complete`'s input model. Process: none of the three commands M1-B needs (Yaml `lint:yaml`, Dotenv `debug:dotenv` and `dotenv:dump`) uses symfony/process, so M1-B ports no Process behaviour |
| String's Unicode layer for Console (owner, November 20) | [unicode-normalization](https://docs.rs/unicode-normalization) + [unicode-segmentation](https://docs.rs/unicode-segmentation) (unicode-rs, no dependencies; the demo already uses unicode-segmentation) vs tables extracted from the oracle's ICU and PCRE2 with hand-written UAX #29 and NFC | The two crates, checked against the oracle | They match ICU 72.1 on all 13,233 NFC cases and 4,105 grapheme sequences; PCRE2's `\X` (used by `chunk()`) is reproduced by splitting their clusters where PCRE2 does, matching all 4,105. Remaining differences come from their newer Unicode data (95 code points assigned after 15.0, and Indic conjuncts in `length()`), listed in crates/string/README.md |
| OS randomness for `ByteString::fromRandom()` (owner, November 22; decision 15) | [getrandom](https://docs.rs/getrandom) (rust-random, the OS CSPRNG only; adds `cfg-if` everywhere and `libc` on Unix, `r-efi` only for UEFI targets), [rand](https://docs.rs/rand) / `rand_core::OsRng` (built on getrandom, adds generators the port does not need), reading `/dev/urandom` by hand (Unix only; Windows needs `BCryptGenRandom` through FFI, excluded by no-`unsafe`), the caller supplying the bytes (no dependency, differs from PHP's signature) | `getrandom` 0.4 | The smallest crate that reaches each platform's CSPRNG, as PHP's `Random\Randomizer` does, with no `unsafe` in this workspace; `rand` would add code for nothing. `from_random_with()` keeps a caller's source |
| Atomic file writes for ConfigCache (symfony-filesystem `dumpFile()`; November 23) | No Rust port of Symfony Filesystem found. [tempfile](https://docs.rs/tempfile) (`NamedTempFile::persist()`: its own random names and errors, no PHP `tempnam()` fallback or messages), [atomic-write-file](https://docs.rs/atomic-write-file) 0.3 (`O_TMPFILE` and `linkat` on Linux, fsync; no temporary file name to observe), [atomicwrites](https://docs.rs/atomicwrites) 0.4 and [write_atomic](https://docs.rs/write_atomic) 0.7 (temporary file plus rename, their own names and errors) | Port of `dumpFile()` and what it calls (`mkdir()`, `tempnam()`, `chmod()`, `rename()`, `readlink()`, `readFile()`) in a new `symfony-filesystem` crate, on `std::fs` and `getrandom` (already in the lock, decision 15) | Upstream's steps are observable: the temporary file in the target's directory with PHP's `tempnam()` name shape and 0600 mode, the target's mode or `0666 & ~umask()`, the link target written, the IOException texts with PHP's warnings, and the temporary file removed on failure. 39 recorded filesystem calls match (docs/milestones/m1-c-config-cache.md). No new crate in the lock |
| `hash('xxh128')` for Config's DirectoryResource and GlobResource (November 23) | [twox-hash](https://docs.rs/twox-hash) 2.1 (MIT, XXH3-128 with `default-features = false`, no dependencies), [xxhash-rust](https://docs.rs/xxhash-rust) 0.8 (BSL-1.0, no dependencies), a hand-written XXH3-128 (seven length paths and the long-input loop) | `twox-hash`, feature `xxhash3_128` only | Hashing is below the behaviour layer (AGENTS.md); MIT like this repository; matches PHP's `hash('xxh128')` on the recorded vectors (empty, short, 300-byte and invalid-UTF-8 inputs) |
| String's case mappings, NFKC_CF and patterns (November 22) | Rust's `char::to_lowercase()` / `to_uppercase()` (its own Unicode version, no title case or folding), [unicase](https://docs.rs/unicase) / [caseless](https://docs.rs/caseless) (folding only), [icu_casemap](https://docs.rs/icu_casemap) + icu_normalizer (ICU4X: complete, but a large dependency tree and newer data than the oracle's ICU 72.1), tables recorded from the oracle; for `match()`, `replaceMatches()` and flagged `split()`: Finder's PCRE translator | Tables of mbstring's lower, upper, title and fold mappings, ICU's NFKC_CF and the final sigma properties, recorded per code point (`scripts/gen-string-tables.py`); Finder's `pcre.rs` moved into a `php-pcre` crate both use | The tables are PHP's own answers for every code point (11,881 differ from identity), so case results cannot drift with a crate's Unicode version; no new crate in the lock. symfony-finder re-exports php-pcre unchanged; `\p{..}` inside a class is now translated as outside one |
| CssSelector (searched November 20) | No Rust port of Symfony CssSelector or Python cssselect. [css-to-xpath](https://docs.rs/css-to-xpath) (Servo's `selectors`/`cssparser`, its own XPath shapes, Generic/Html/Xhtml modes, no nested `:has()`); [selectors](https://docs.rs/selectors) / [scraper](https://docs.rs/scraper) (match against a DOM, no XPath); scrapling's translator (cssselect-like, tied to its crate); cssifier / xcss (XPath to CSS, the other direction) | Port on `php-value` and `regex` (bytes, `(?-u)`), no new dependency | None produces Symfony's XPath strings, node string forms, specificity, `ParseException` texts with token positions, PHP truthiness (`"0"`), byte-wise patterns or the LRU cache |
| OptionsResolver (searched November 20) | No Rust port found (crates.io/GitHub searches for "options resolver" find only module-path resolvers such as oxc_resolver); typed option structs with [serde](https://serde.rs) defaults or builder derives ([derive_builder](https://docs.rs/derive_builder)), and layered config crates (schematic, figment) | Port on `php-value` and `symfony-deprecation-contracts`, no new dependency | Typed structs fix the option set at compile time; Symfony's is built at run time by form types and bundles (lazy defaults reading other options, normalizers, allowed types/values, nested and prototype options, deprecations) with its own messages and resolution order |
| Finder (searched November 20) | No Rust port of Symfony Finder. Walkers: [walkdir](https://docs.rs/walkdir) (depth-first, sorting, symlink loops), [ignore](https://docs.rs/ignore) (ripgrep's walker with its own gitignore rules and parallelism), [jwalk](https://docs.rs/jwalk); globs: [globset](https://docs.rs/globset), [glob](https://docs.rs/glob), [wax](https://docs.rs/wax). Regex for the PCRE patterns Finder builds and accepts (`Glob`/`Gitignore` emit lookaround, possessive quantifiers, `$f`): `regex` (no lookaround), [fancy-regex](https://docs.rs/fancy-regex) 0.20 (backtracking, lookaround, atomic and possessive, a bytes mode with ASCII classes), [pcre2](https://docs.rs/pcre2) (FFI, excluded by decision 5) | Port, walking with `std::fs`; `fancy-regex` under a PHP-pattern translator (`crates/finder/src/pcre.rs`) | Finder's order of filters and stat calls (pinned by upstream's stream-wrapper log tests), stream wrappers, `SplFileInfo` relative paths, its own gitignore regexes and Glob's regex text are its behaviour, so no walker or glob crate reproduces them. `fancy-regex` is the regex engine decision 5 names for lookaround; the translator keeps PCRE's meaning (bytes without `/u`, `$` before a final newline, `/x`, `/U`, `/n`, literal `{`) and refuses what it does not translate; checked on 137 patterns x 62 subjects recorded from PHP |
| Console's OutputWrapper and Table cell patterns (November 21) | `regex` (no atomic groups or lookbehind; upstream's wrap pattern needs both), a hand-written wrapper (the demo's `wrap()`, its own rules), `fancy-regex` 0.20 (already Finder's engine, decision 5) | `fancy-regex`, upstream's pattern written for it; `preg_replace()`'s scan done by hand | The wrap pattern is upstream's text and stays comparable; checked on 2,500 recorded `wrap()` calls and SymfonyStyle's 27 fixtures. No new crate in the workspace |
| Config's YamlReferenceDumper values (`Inline::dump()`; November 22) | symfony-yaml (this workspace's port of symfony/yaml, upstream's own dependency for this class), a second inline dumper written into symfony-config | symfony-yaml, optional, behind symfony-config's `yaml` feature | Upstream calls symfony/yaml's `Inline::dump()`; symfony-yaml reproduces it on its recorded vectors, and a copy would drift from it. No new crate in the lock |
| composer.json for `dotenv:dump` and `debug:dotenv` (November 21) | A hand-written JSON reader in `php-value` (none exists; it would need PHP's `json_decode()` rules for depth, big integers and invalid UTF-8), `serde_json` (already in the lock as every crate's test dependency) | `serde_json`, optional, only with symfony-dotenv's `console` feature | The commands read `extra.runtime` from a file the project owns; integers that do not fit become floats as in PHP, invalid JSON is `null`. No new crate in the lock |
| Twig trial emission (spikes/twig-trial, November 24) | Rust source as hand-built strings (no dependency); `quote` + `proc-macro2` + `syn` 2 (already in Cargo.lock through other crates) with [prettyplease](https://docs.rs/prettyplease) 0.2 to lay out the file, as php2rust does; [genco](https://docs.rs/genco) (quasi-quoting with its own formatter) | `quote` + `syn` + `prettyplease`, in the trial compiler only (a build dependency) | It was the planned M4 approach ("quote + prettyplease"), the emitted tokens are checked by `syn` before they are written, and readable output makes generated templates reviewable. M4 repeats this row when the real `twig` crate starts |
| DI trial input and emission (spikes/di-trial, November 24) | Input: `debug:container --format=json` or the debug container XML it reads (both exist only with `kernel.debug` and are written before the removal passes), or `XmlDumper` on the final `ContainerBuilder` from a recorder; output: hand-built strings, or `quote` + `syn` + `prettyplease` as in the Twig trial; existing Rust DI crates ([shaku](https://docs.rs/crate/shaku), [lockjaw](https://docs.rs/crate/lockjaw/latest/source/userguide/src/setup.md), [fundle](https://docs.rs/crate/fundle/0.3.5), [injectable-rs](https://docs.rs/injectable-rs), [chassis](https://docs.rs/crate/chassis/0.2.0), [rustyinject](https://docs.rs/crate/rustyinject/latest), [dill](https://docs.rs/dill), [dependency-injector](https://docs.rs/crate/dependency-injector/2.1.0)) | `XmlDumper` output of the final graph (prod, no debug), read with `php-dom` and `XmlUtils::phpize()`; `quote` + `syn` + `prettyplease`, in the trial compiler only | It is the graph `PhpDumper` compiles, in Symfony's own format, read with workspace crates (no new dependency). The DI crates take their graph from hand-written Rust attributes and could not take Symfony's. M3 repeats this row when the real crate starts |

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
- [x] Yaml (`symfony-yaml`, values in `php-value`). Ported: every file,
      `Command/LintCommand.php` behind the `console` feature: 340 of 342
      upstream tests, 2 `n/a` with their reasons in the manifest, plus
      150 recorded `lint:yaml` runs; in `parity/done.txt`. The four
      advisories the watch lists are `covered`, checked against the
      releases before each fix ([m1-b-console.md](docs/milestones/m1-b-console.md)). ParserTest, DumperTest, YamlTest and
      YamlTestSuiteTest replay every call each upstream test makes to the
      parser and the dumper, recorded on the oracle while the upstream
      test runs and passes (`oracle/recorders/yaml_test_calls.php`);
      YamlTestSuiteTest runs on yaml-test-suite at the commit symfony/yaml
      pins (`upstream.tsv`), 200 cases, upstream's skip list.
- [x] Dotenv (`symfony-dotenv`): `Dotenv.php` and the exceptions, all 38
      DotenvTest tests (also run on Linux: the `$(...)` cases skip on
      Windows, as upstream's do) and 231 recorded `parse()` inputs. PHP's
      globals are a `Globals` value; `putenv()` is an overlay, since
      setting the process environment is `unsafe`. `.env.local.php` is
      read as dump-env's literal array, not run. Upstream loops forever on
      a value ending in `$(`; the port fails instead (crate README,
      "Differences"). Then the two commands behind the `console` feature:
      DebugCommandTest and DotenvDumpCommandTest (19 of 19) and 266
      recorded command runs; in `parity/done.txt`.

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
      is M1-D (scheduling adjustment below). Stage 3 (November 22):
      `Definition/Dumper/YamlReferenceDumper.php` (behind the `yaml`
      feature, on symfony-yaml's `Inline::dump()`; stack row above),
      `Definition/Dumper/XmlReferenceDumper.php` and
      `Definition/ArrayShapeGenerator.php`: 25 more tests (192 of 334),
      plus 1,020 outputs recorded on 34 generated trees
      (`config_dumpers.php`), which also found that a node implementing
      only NodeInterface has no `getParent()` for XmlReferenceDumper (now
      a `CustomNode` that does not extend BaseNode). Stage 4 (November
      22, gate X3's first consumer): `Util/XmlUtils.php` and its two
      exceptions on `php-dom`: XmlUtilsTest 7 of 7 (199 of 334), plus
      3,045 `phpize()` values, 71 conversions and 120 `parse()` /
      `loadFile()` outcomes recorded (`config_xml_utils.php`; stage log
      [docs/milestones/m1-c-xml-x3.md](docs/milestones/m1-c-xml-x3.md)).
      Stage 5 (November 23, M1-C): the loader exceptions,
      `FileLocator.php`, `Loader/*` (Loader, LoaderResolver,
      DelegatingLoader, FileLoader, GlobFileLoader, ParamConfigurator)
      and the filesystem resources (FileResource, FileExistenceResource,
      DirectoryResource, GlobResource), with Normalization-, Merge- and
      FinalizationTest for the Definition code already ported: 90 more
      tests, 1 `n/a` (GlobResourceTest::testPhar, no phar reader), 289 of
      334. Recorded (`config_loaders.php`): 94 `parse_url()` schemes, 432
      LoaderLoadException messages, 432 `locate()` calls, 1,680
      GlobResource cases and 456 `import()` calls, all matching; they
      found PHP's `parse_url()` authority rules, glibc's `.*` matching `.`
      and `..`, and `realpath()` refusing `file://`. symfony-finder's
      `fs` gains a files-and-directories `glob()` (`glob_dirs()`
      unchanged). Stage log
      [docs/milestones/m1-c-config-loaders.md](docs/milestones/m1-c-config-loaders.md).
      Stage 6 (November 23, M1-C): `ConfigCache.php`,
      `ResourceCheckerConfigCache.php`, `ConfigCacheFactory.php`,
      `ResourceCheckerConfigCacheFactory.php`, their interfaces and
      `Resource/SelfCheckingResourceChecker.php` /
      `SkippingResourceChecker.php`: 18 more tests, 1 `n/a`
      (ConfigCacheFactoryTest: a non-callable cannot be passed), 307 of
      334. Meta files are PHP `serialize()` text (php-value), with
      upstream's `.meta.json` beside them. The atomic write is the new
      `symfony-filesystem` crate (`dumpFile()` subset, 27 of 157 tests;
      stack row above). Recorded (`config_cache.php`): 27 writes, 1,170
      `isFresh()` calls and 39 filesystem calls; all match but 9 listed
      (1 `r:` reference, 8 GlobResource restores). Stage log
      [docs/milestones/m1-c-config-cache.md](docs/milestones/m1-c-config-cache.md).
      Config stays partial (25 rows `pending`): the reflection-based
      resources (23 tests; decision 17) and `Definition/Loader`,
      `Definition/Configurator` and `Definition/Configuration.php` (2
      tests; DefinitionFileLoader includes PHP files).
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

- [x] M1-B (`symfony-console`, the subset the Yaml and Dotenv commands
      need). Done: the formatter, styles, Output, Input and completion
      input (PR #6); then Command, AsCommand, HelperSet, FormatterHelper,
      StringInput, NullOutput, Application's registry (`find()`,
      namespaces, alternatives; `run()` is M7) and CommandTester /
      CommandCompletionTester: CommandTest 40 of 43 (3 `n/a`: closure
      `$this` binding), ApplicationTest 45 of 147, HelperSetTest,
      FormatterHelperTest, StringInputTest, NullOutputTest, plus 17,805
      recorded calls (`scripts/record.sh console`, `builtins.json`).
      Recording found that natural order does not skip whitespace right
      after a run of digits; symfony-finder had the same bug and is fixed
      (its corpus extended; the old answers unchanged). Then OutputWrapper,
      TableStyle, TableCell, TableCellStyle, TableSeparator, Table,
      TrimmedBufferOutput, SymfonyStyle (no questions, progress bars or
      trees: M7) and GithubActionReporter: TableTest 32 of 36 (4 pending:
      section output), SymfonyStyleTest 6 of 22 (2 `n/a`; `testOutputs`
      pending for command_7's questions, the other 27 fixtures pass),
      OutputWrapperTest 1 of 4 (3 `n/a`: malformed UTF-8),
      GithubActionReporterTest, TableStyleTest, TableCellStyleTest; plus
      1,500 generated tables and 2,500 `wrap()` calls recorded from PHP.
      Then Yaml's LintCommand and Dotenv's two commands with their 30
      tests: the stage's exit evidence. Stage log:
      [docs/milestones/m1-b-console.md](docs/milestones/m1-b-console.md).
      The rest of Console stays M7 work.

- [ ] M1-C, XML gate X1 (`php-dom`, decision 9): the parser and the
      read-only DOM the consumers need (inventory and recording in
      [docs/milestones/m1-c-xml-x1.md](docs/milestones/m1-c-xml-x1.md)).
      `loadXML()` with and without `validateOnParse`: 457 of 472 recorded
      documents give PHP's answer in both modes (load result, every
      libxml error with its level, code, message, line and column, and
      the tree), on 1,346 recorded errors; the other 15 are refused (file,
      network, iconv) or listed (billion laughs' error count). Nothing is
      fetched. X1 passed for that corpus; open: decision 14, X2 (XSD) and
      X3 (`XmlUtils`, SimpleXML/XPath subset, the XLIFF, Qt and Validator
      loaders).
- [ ] M1-C, XML gate X2 (`php_dom::schema`): `schemaValidateSource()`
      for the XSD features of the schemas Symfony validates against at the
      pin (XLIFF 1.2 transitional, 2.0, 2.2 with `xml.xsd`; Validator's
      constraint-mapping; inventory, findings and refusals in
      [docs/milestones/m1-c-xml-x2.md](docs/milestones/m1-c-xml-x2.md)).
      1,321 of 1,325 recorded documents against 65 schemas give PHP's
      result and every libxml error (922 recorded), including libxml's
      "Expected is ( … )" lists pinned by 480 content-model probes; the
      other 4 are refused (`xsi:type`, `complexContent`, `all`, counted
      bounds). Imports come only from the caller's resolver; anything
      outside the feature set is refused with its line. X2 passed for that
      corpus; open: X3 (resolver mapping for XliffUtils and XmlValidator,
      `XmlUtils`, SimpleXML/XPath subset, the loaders); schema refusals
      surface as decision 14 (accepted) says for parse refusals.
- [ ] M1-C, XML gate X3 (consumers). Done: `XmlUtils` (symfony-config,
      stage 4 above): parse and schema refusals are XmlParsingExceptions
      with the refusal's text (decision 14); schema imports resolve as
      libxml resolves them for a schema read from memory (against the
      working directory, `file:` URLs, `%XX` decoded, nothing else read).
      Every recorded outcome matches except two imports libxml fails to
      load, which php-dom refuses
      ([docs/milestones/m1-c-xml-x3.md](docs/milestones/m1-c-xml-x3.md)).
      X3 passed for XmlUtils. Done (November 23): XliffFileLoader and
      XliffUtils (new crate symfony-translation, with MessageCatalogue):
      the `xml.xsd` resolver mapping, SimpleXML's view of the document
      written on php-dom (children without a prefix, direct text, attributes
      without a namespace), the stream-wrapper rules, mbstring's encodings;
      XliffFileLoaderTest 31 of 31 (with symfony/config's `FileResource`),
      XliffUtilsTest 2 of 4 (2 n/a), MessageCatalogueTest 18 of 18; 306 of
      308 recorded cases match, 2 listed (`FileResource` resolves
      `foo://bar` and `file://localhost/…` otherwise than PHP)
      ([docs/milestones/m1-c-xml-x3-xliff.md](docs/milestones/m1-c-xml-x3-xliff.md)).
      php-dom's `getElementsByTagName()` now compares the local name, as
      PHP 8.4 does. X3 passed for the XLIFF loader. Done (November 23):
      QtFileLoader, with a `DOMXPath::evaluate()` subset in php-dom
      (location paths on the child axis, name and `text()` tests, text,
      attribute and position predicates, unions); QtFileLoaderTest 5 of 5,
      133 of 134 recorded cases match, 1 listed (the same `FileResource`
      difference) and 1 `evaluate()` refused
      ([docs/milestones/m1-c-xml-x3-qt.md](docs/milestones/m1-c-xml-x3-qt.md)).
      X3 passed for QtFileLoader; open: Validator's XmlFileLoader (M5-A).

- [x] CssSelector (`symfony-css-selector`): every file; 80 of 85
      upstream tests, 5 `n/a` (TranslatorTest's DOM tests evaluate XPath
      with libxml; no XPath engine under decision 9) whose 191 rows'
      XPath strings are checked against PHP's; every data provider
      replayed from the oracle (`scripts/record.sh css-selector`) plus 233
      recorded cases; in `parity/done.txt`.
- [x] OptionsResolver (`symfony-options-resolver`): every file, all 234
      upstream tests (providers replayed from the oracle), plus 360
      recorded edge cases (`scripts/record.sh options-resolver`); in
      `parity/done.txt`. Closures receive the resolver as a shared handle;
      reflection on default closures is the caller's `DefaultValue::Lazy`.
      `is_callable()` covers closures only (decision 12).

- [x] Finder (`symfony-finder`, pilot November 20): every PHP file;
      139 of 143 upstream tests, 4 `n/a` with reasons; in
      `parity/done.txt`. PHP patterns run on `fancy-regex` through a
      PCRE translator checked on recorded `preg_match` answers; the
      date grammar is a subset (decision 13). No advisories listed for
      symfony/finder.
- [ ] String (`symfony-string`). Done: ByteString, CodePointString,
      UnicodeString, AbstractString (with TruncateMode), AbstractUnicodeString,
      the exceptions and `u()` / `b()` / `s()`: ByteStringTest 68 of 68,
      CodePointStringTest 73 of 78, UnicodeStringTest 75 of 81 (1 `n/a`),
      FunctionsTest 3 of 3, every provider replayed from the oracle, plus
      59,808 recorded positions and 4,208 calls of the PHP functions they
      use, and mbstring's and ICU's mappings of every code point
      ([m1-string.md](docs/milestones/m1-string.md)). Console keeps
      passing on the new API. Then LazyString (LazyStringTest 7 of 7, 100
      recorded cases) and the English, French and Spanish inflectors (9
      of 9, 10,629 recorded words through each): 235 of 266. Pending:
      `ascii()` and the `locale*()` methods (ICU transliterators, 10
      rows) and the Slugger (20, needs them). No advisories listed for
      symfony/string; `fromRandom()`'s randomness is decision 15.

## M2 progress

Stack rows (searched November 22):

| Component | Options compared | Choice | Reason |
|---|---|---|---|
| PHP's `filter_var()` and `BackedEnum::from()` (ParameterBag, InputBag) | No Rust crate reproduces PHP's filter extension; validator crates ([validator](https://docs.rs/validator), [garde](https://docs.rs/garde)) check typed struct fields with their own rules and messages; php2rust's `php-rt` has no filter or enum support | Written in `php-value` from the PHP manual and recorded answers, no dependency | The bags' behaviour is PHP's: string conversion of scalars, `FILTER_NULL_ON_FAILURE`, array walking, `default`, hex/octal as unsigned 64-bit, coercive enum arguments and their messages. 22,932 `filter_var()` calls and 2,450 bag calls recorded from PHP 8.4.27 pin them |
| MIME types and guessing (symfony/mime `MimeTypes`, for File and UploadedFile; searched November 24 with `cargo search`) | Extension tables: [mime_guess](https://docs.rs/mime_guess) 2.0 / new_mime_guess (their own extension lists and order); content guessing: [magic](https://docs.rs/magic) 0.17 and filemagic (libmagic FFI), [pure-magic](https://docs.rs/pure-magic) 0.5 and [libmagic-rs](https://docs.rs/libmagic-rs) 0.12 (pure Rust) and [file-format](https://docs.rs/file-format) (own signatures); no Rust port of symfony/mime | Port `MimeTypes.php` (its `MAP` and `REVERSE_MAP` generated from upstream by `scripts/gen-mime-types.py`) and `FileBinaryMimeTypeGuesser.php` in a new in-workspace crate `symfony-mime`, no external dependency | `getExtensions()` / `getMimeTypes()` answers are Symfony's own tables and order, which no extension crate reproduces. Content guessing follows libmagic; which one to use is decision 18 |
| `FileinfoMimeTypeGuesser` content guessing (PHP's `finfo`; compared November 24 on 12,843 upstream files and 124 samples against PHP 8.4.27) | [pure-magic](https://docs.rs/pure-magic) 0.5.2 with [magic-db](https://docs.rs/magic-db) 0.7.2 (safe Rust, `forbid(unsafe_code)`, GPL-3.0 OR BSD-2-Clause); pure-magic loading the `file` 5.45 Magdir (6 of 341 files fail to parse, fewer matches); [libmagic-rs](https://docs.rs/libmagic-rs) 0.12 (no MIME output); `magic` 0.17 / `magic-sys` (FFI to the system's libmagic, excluded by decision 18) | `pure-magic` =0.5.2 and `magic-db` =0.7.2 in `symfony-mime`, under the BSD-2-Clause option | Closest to PHP's answers (97.5% of upstream files, 86% of the binary samples), no C library, works on Windows. It adds about 60 crates to the lock (pest, regex, serde, postcard, jiff, flate2, tar, memmap2, stacker); `stacker`/`psm` compile a small assembly file with `cc`, so a C toolchain is needed to build. Versions are pinned with `=`, because a new rule set changes answers; the differences list (`crates/mime/tests/data/fileinfo_differences.tsv`) shows the effect of an update |
| Routing's Route, RouteCompiler and RouteCollection (searched November 24) | No Rust port of Symfony Routing on GitHub or crates.io; router crates ([matchit](https://docs.rs/matchit), [path-tree](https://docs.rs/path-tree), [route-recognizer](https://docs.rs/route-recognizer), axum's and actix's routers) have their own pattern languages: no inline `{name<requirement>?default}`, separators before optional trailing variables, host patterns, PCRE requirements or Symfony's compiled regex and tokens | Ported from Route.php, RouteCompiler.php and RouteCollection.php on php-value and php-pcre (workspace crates), no new dependency | The compiled regex, tokens, static prefix and `serialize()` bytes are what the matcher dumpers, generator and cached routes read; 2,000 routes and 300 collection scripts recorded from PHP 8.4.27 pin them |
| PHP's `parse_url()` (RequestContext::fromUri(), searched November 24) | [url](https://docs.rs/url) (WHATWG URL Standard: normalizes, lowercases, percent-encodes, other failures), [http::Uri](https://docs.rs/http) and [fluent-uri](https://docs.rs/fluent-uri) (strict RFC 3986: reject control bytes, spaces and `[` hosts PHP accepts), symfony-filesystem's scheme-only `parse_url_scheme()` | Written in `php-value::url` from the PHP manual and recorded answers, no dependency | PHP's components for malformed input (`host:port` without `//`, `false` for empty hosts and long ports, control bytes as `_`, `file:///`) are what `fromUri()` reads; 12,119 calls recorded from PHP 8.4.27 pin them |
| PHP's loose equality `==` (UrlGenerator's `array_udiff_assoc()` against defaults; searched November 24) | No Rust crate implements it: [ext-php-rs](https://docs.rs/ext-php-rs) and [phper](https://docs.rs/phper) bind the PHP engine through FFI (needs libphp and `unsafe`); symfony-console's `loose_equals_str()` and symfony-finder's `loose_eq()` cover a string or scalar operand only | Written in `php-value::compare` from the PHP manual's comparison table, no dependency | Every PHP type is compared (arrays by key, objects by class and properties, numeric strings, big integer strings as bytes, NAN); 8,649 pairs recorded from PHP 8.4.27 pin it. The two crate copies can move to it later |

- [ ] HttpFoundation (`symfony-http-foundation`), stage 1 (November 22;
      [docs/milestones/m2-a-http-foundation-bags.md](docs/milestones/m2-a-http-foundation-bags.md)):
      `ParameterBag.php`, `InputBag.php`, the bag exceptions and
      `HeaderUtils.php` without `makeDisposition()` and `parseQuery()`, on
      `php-value`, which gains `filter_var()` (5 filters), `BackedEnum::from()`,
      `Closure` and `\Stringable`. ParameterBagTest 39 of 41, InputBagTest 23
      of 25 (4 `n/a`: PHP references), HeaderUtilsTest 6 of 13; plus
      22,932 recorded `filter_var()` calls, 2,450 bag calls and 3,000
      `split()` calls (`scripts/record.sh php-value`, `http-foundation`).
      Recording found that `split()` mishandled whitespace separators and a
      one-separator group whose first value unquotes to `''` (fixed; the
      demo's Accept-Language path does not use either). Next: AcceptHeader
      and AcceptHeaderItem as an API, HeaderBag, ServerBag,
      ResponseHeaderBag, Cookie, FileBag (needs UploadedFile); IpUtils,
      `makeDisposition()` and `setTrustedProxies()` wait for decision 16.
- [ ] HttpFoundation stage 2 (November 23;
      [docs/milestones/m2-a-http-foundation-headers.md](docs/milestones/m2-a-http-foundation-headers.md)):
      `Cookie.php`, `HeaderBag.php`, `ResponseHeaderBag.php` (without
      `makeDisposition()`, decision 16) and `ServerBag.php`, on `php-value`,
      which gains `time()` (mockable, as ClockMock), `gmdate()`,
      `strtotime()` and `createFromFormat(DATE_RFC2822)` subsets that
      refuse what they do not read as PHP does, URL encoding, non-strict
      `base64_decode()` and `ksort()`. CookieTest 53 of 53, HeaderBagTest
      20 of 20, ResponseHeaderBagTest 19 of 19, ServerBagTest 14 of 14;
      plus 382 recorded date answers, 2,277 Cookie calls, 400 header-bag
      call scripts and 360 `$_SERVER` arrays, all matching. No advisory is
      listed for these files. Next: AcceptHeader and AcceptHeaderItem as an
      API, `parse_str()` and `HeaderUtils::parseQuery()`, FileBag (needs
      UploadedFile).
- [ ] Version move to v8.1.11 (decision 16, November 23;
      [docs/milestones/m2-a-pin-8.1.11.md](docs/milestones/m2-a-pin-8.1.11.md)):
      http-foundation, http-kernel and routing pinned at v8.1.11, mounted
      over the demo's vendor copy in the oracle (`oracle/compose.yaml`,
      checked by `fetch-upstream.sh --verify`). Their manifests gain 42
      tests (13, 5 and 24), all `pending`; every existing vector records
      unchanged at v8.1.11. Ported at v8.1.11: `IpUtils.php` (IpUtilsTest
      18 of 19, 1 `n/a`) on php-value's new IP built-ins and PHP 8
      string/number rules, and `HeaderUtils::makeDisposition()` (3 more
      HeaderUtilsTest tests, its `/D` anchor pinned); 37,802 IpUtils calls
      and 2,880 `makeDisposition()` calls recorded, all matching.
      GHSA-38cx-cq6f-5755 is now `covered`. Not ported, waiting for their
      classes: `Request::setTrustedProxies()` and `prepareBaseUrl()`'s
      v8.1.11 fixes (Request.php), UrlHelper, BinaryFileResponse,
      MethodRequestMatcher, SessionHandlerFactory, the compiled matcher's
      host-variable fix and RouterListener's sub-request context.
- [ ] Routing (`symfony-routing`), stage 1 (November 24;
      [docs/milestones/m2-a-routing-route.md](docs/milestones/m2-a-routing-route.md)):
      `Route.php`, `CompiledRoute.php`, `RouteCompiler.php`,
      `RouteCompilerInterface.php`, `RouteCollection.php`, `Alias.php` and
      the exceptions they throw, on `php-value` (one additive method,
      `PhpObject::is_stringable()`) and `php-pcre`, with resources behind
      an optional `config` feature. RouteTest 24 of 24, RouteCompilerTest
      10 of 10, CompiledRouteTest 2 of 2, RouteCollectionTest 24 of 24;
      plus 2,000 generated routes (construction, compile, serialize),
      1,000 requirements, 300 collection scripts and 131 malformed
      `unserialize()` payloads recorded, all matching. Recording found
      `findNextSeparator()`'s TypeError for a newline after a variable.
      The demo's typed matcher and generator are unchanged. Next:
      `RequestContext.php` and `Matcher/UrlMatcher.php` on these routes.
- [ ] Routing stage 2, shared by the matcher and generator stages
      (November 24;
      [docs/milestones/m2-a-routing-request-context.md](docs/milestones/m2-a-routing-request-context.md)):
      `RequestContext.php` (without `fromRequest()`, which needs
      HttpFoundation's Request) and every `Exception/*.php` class, on
      `php-value`, which gains `parse_url()`. RequestContextTest 16 of 18
      (1 `n/a`, `testFromRequest` pending); 12,119 `parse_url()` calls and
      3,000 `fromUri()` calls recorded, all matching. Next, in separate
      sessions: `Matcher/UrlMatcher.php` and `Generator/UrlGenerator.php`
      (with GHSA-72xp-p242-47p9's fix) on these classes.
- [ ] Routing stage 3, the generator (November 24;
      [docs/milestones/m2-a-routing-url-generator.md](docs/milestones/m2-a-routing-url-generator.md)):
      `Generator/UrlGenerator.php` with its two interfaces, on the ported
      Route, RouteCollection and RequestContext. `php-value` gains loose
      equality (`compare::loose_equals()`) and `http_build_query()` with
      RFC 3986 and floats. UrlGeneratorTest 91 of 91; 3,000 `generate()`
      and 2,000 `getRelativePath()` calls, 8,649 `==` pairs and 1,022
      `http_build_query()` calls recorded, all matching.
      GHSA-72xp-p242-47p9 is `covered`. The demo's typed generator is
      unchanged. Next: `CompiledUrlGenerator.php` and its dumper.
- [ ] HttpFoundation stage 3 (November 23;
      [docs/milestones/m2-a-http-foundation-stage3.md](docs/milestones/m2-a-http-foundation-stage3.md)):
      `AcceptHeader.php` and `AcceptHeaderItem.php` as an API
      (AcceptHeaderTest 6 of 6, AcceptHeaderItemTest 5 of 5; `filter()` on
      php-pcre) and `HeaderUtils::parseQuery()` (HeaderUtilsTest now 13 of
      13). PHP's input-variable rules move from `query.rs` to
      `php_value::vars`, shared with a new `parse_str()`; `query.rs` keeps
      its API and its 160 recorded vectors. Recorded and matching: 254
      `parse_str()` inputs, 3,000 `parseQuery()` calls, 1,500 Accept
      headers with 6,000 `has()`/`get()` queries, 2,000 `get()` calls among
      overlapping ranges, 21 `filter()` calls and 500 items.
      `accept_language.rs` now reads through AcceptHeader. No advisory is
      listed for these files. Next: FileBag with File and UploadedFile
      (MIME guessing needs symfony/mime), then Request.
- [ ] HttpFoundation stage 4 (November 24;
      [docs/milestones/m2-a-http-foundation-files.md](docs/milestones/m2-a-http-foundation-files.md)):
      `File/File.php`, `File/UploadedFile.php`, `FileBag.php` and the File
      exceptions, on a new `symfony-mime` crate (`MimeTypes.php` and
      `FileBinaryMimeTypeGuesser.php`; MimeTypesTest 13 of 13,
      FileBinaryMimeTypeGuesserTest 7 of 8, 1 `n/a`; the finfo guesser is
      decision 18). FileTest 9 of 9, UploadedFileTest 23 of 23,
      FileBagTest 9 of 9; plus 419 recorded answers (`parseFilesize()`,
      `getMaxFilesize()` under 126 ini pairs, client names and MIME types,
      error codes, 71 `$_FILES` arrays through FileBag), all matching.
      `is_uploaded_file()` / `move_uploaded_file()` read a list the upload
      code fills. No advisory is listed for these files. Next: Request.
- [ ] symfony/mime finfo guesser (November 24, decision 18 option (c);
      [docs/milestones/m2-a-mime-fileinfo.md](docs/milestones/m2-a-mime-fileinfo.md)):
      `FileinfoMimeTypeGuesser.php` on `pure-magic` with `magic-db`.
      FileinfoMimeTypeGuesserTest 8 of 8. PHP 8.4.27's answers recorded
      for 124 sample files and the 12,843 files of `upstream/symfony`
      (`scripts/record.sh mime-fileinfo`): 107 and 12,525 match, and the
      test checks each of the 335 differences. `MimeTypes::new()` now
      registers it, as upstream. No advisory is listed for this file.

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

### M5-A bounded stage: Translation catalogues (November 24)

Scope: audit the existing `MessageCatalogue.php` port and its 18 tests;
port the separate `Tests/Catalogue/MessageCatalogueTest.php` metadata regression,
and `Catalogue/OperationInterface.php`, `AbstractOperation.php`,
`MergeOperation.php`, `TargetOperation.php`. Pinned symfony/translation
v8.1.5 (`d9e1caba`); MergeOperationTest 12 and TargetOperationTest 13
(including the seven inherited tests for each). Keep lazy evaluation,
shared input/result identity, fallbacks, ordered PHP array keys, ICU
migration and both sorts of metadata. Record uncovered paths in the oracle.

This session changes only symfony-translation behaviour; no edits to
php-value, Routing or HttpFoundation, and no new dependency or lock change.
Translator, other loaders, formatters, extraction and commands remain
pending in M5-A. Demo checks/comparisons are paused by the owner; any
acceptance depending on them remains open. Stage evidence: 26 newly ported tests,
528 scenarios / 7,782 calls matching PHP (including four without intl).
Host formatting/Clippy, Translation tests, source/parity checks and the full
Linux workspace checks pass (restricted-capability container; unchanged
Windows OS-message test failure recorded). Details in
[docs/milestones/m5-a-translation-catalogue.md](docs/milestones/m5-a-translation-catalogue.md).
