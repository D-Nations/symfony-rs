# DI container compiler trial

A small prototype that tests the M3 design (PLAN.md, "M3:
DependencyInjection", and [converter contract 0.2](../../docs/converter-contract.md),
section 4) before the real port starts. **This is not the M3 port.** It
ports no Symfony class, passes no gate, adds no parity manifest rows, and
is not in `parity/sources.tsv`. Results and findings:
[docs/milestones/m3-di-trial.md](../../docs/milestones/m3-di-trial.md).

| Crate | What it is |
|---|---|
| [`crates/di-emitter`](../../crates/di-emitter/README.md) (`symfony-di-emitter`) | The trial's compiler, now a workspace crate (November 26): reads the compiled container (a Rust `ContainerBuilder`, or the file Symfony's `XmlDumper` writes), checks it, splits it into application and request scope, and emits Rust (quote + prettyplease). Used from `demo/build.rs`. |
| `runtime/` (`di-trial-runtime`) | What the emitted code calls: service cells, the re-entrant "configure once" guard, and placeholder instances that record arguments, factories, calls and properties. |
| `demo/` (`di-trial-demo`) | symfony/demo's container (`data/container.xml`, recorded from PHP), compiled in `build.rs`; tests that resolve every service. |

## Input

`data/container.xml` is the demo's `ContainerBuilder` after all compiler
passes (APP_ENV=prod, APP_DEBUG=0), as `PhpDumper` receives it, written by
`XmlDumper` (`oracle/recorders/di_container.php`). The recorder replaces
tagged iterators and locators with their resolved services, because
`XmlDumper` writes only the tag. So this trial tests the back half of
the M3 compiler (graph to Rust); the front half (loaders, extensions and
compiler passes in Rust) is not in it. Since November 26 the emitter also
reads a `ContainerBuilder` compiled in Rust and gives the same graph as
for its `XmlDumper` file (`crates/di-emitter/tests/equivalence.rs`); the
demo keeps the PHP recording until the bundles are ported.

## Placeholders

Every PHP class is a generated newtype over `di_trial_runtime::Instance`,
so HTTP types (Request, RequestStack, sessions) and all other classes are
stand-ins. The accessors are typed (`fn router(&self) ->
Arc<classes::symfony::bundle::framework_bundle::routing::Router>`); the
arguments inside a placeholder are not.

## Supported input

Services, aliases, parameters (scalars and collections), inline services,
`service`/`service_closure`/`iterator`/`collection`/`constant`/`string`
arguments, `on-invalid` (exception, null, ignore, ignore_uninitialized),
factories (class, service, inline service, function, `constructor`),
method calls (with `returns-clone`), properties, configurators, `shared`,
`lazy`, `synthetic`, `%param%`, `%%` and `%env()%` in strings. Everything
else is refused at build time with its line
(`crates/di-emitter/tests/refusals.rs`).

## Run

```sh
cargo test -p symfony-di-emitter -p di-trial-demo
scripts/record.sh di-trial    # re-record data/container.xml and tests/data/php_classes.json
```

The build writes `target/debug/build/di-trial-demo-*/out/container.rs`
(the generated container) and `report.txt` (scopes, cycles, env
expressions).
