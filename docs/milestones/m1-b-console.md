# M1-B: minimum Console, and the Yaml and Dotenv commands

The stage in PLAN.md's delivery table: the Console infrastructure that
Yaml's `lint:yaml` and Dotenv's `debug:dotenv` and `dotenv:dump` run on,
then those three commands with their upstream tests. Its exit evidence
was "run the 11 Yaml and 19 Dotenv command tests"; they pass, and
symfony/yaml and symfony/dotenv are now in `parity/done.txt`. The rest of
Console stays M7 work.

Four pull requests, November 20-21: #6 (formatter, styles, Output,
Input, completion input), #7 (Command, Application's registry, the
testers), #8 (OutputWrapper, Table, SymfonyStyle, GithubActionReporter)
and the commands' PR.

## Prior art

Compared before the work started (PLAN.md stack rows "Console (M1-B
subset)", "String's Unicode layer" and "Console's OutputWrapper and Table
cell patterns"): no Rust port of Symfony Console exists; clap,
clap_complete, comfy-table and tabled have their own grammar, completion
protocol and table layout; symfony-demo-rs's `src/console` is a partial
port arranged around the demo's commands. Console is ported on
`php-value`, with unicode-rs's normalization and segmentation underneath
(checked against ICU) and fancy-regex for OutputWrapper's pattern
(decision 5's engine, already Finder's). `dotenv:dump` reads
composer.json with serde_json, an optional dependency of the new
`console` feature.

## Shape

- A PHP subclass of Command is a `CommandHandler`: the methods PHP code
  overrides, its class name and its `#[AsCommand]` attribute as a value.
  A handler receives the `Command` as `$this`.
- The commands sit behind a `console` feature of symfony-yaml and
  symfony-dotenv, so the parsers do not pull in Console.
- `lint:yaml` parses with an `Environment` of constants (`!php/const`),
  which a converter states at build time. The Dotenv commands share the
  application's `$_SERVER` and `$_ENV` as `Globals`.

## Evidence

| Package | Upstream tests | Recorded from PHP 8.4.27 |
|---|---|---|
| symfony/console (subset) | 320 of 1,152 ported, 11 `n/a`, the rest pending (M7) | 17,805 built-in and `find()` calls, 6,137 `strip_tags()` calls, 1,500 tables, 2,500 `wrap()` calls; SymfonyStyleTest's 27 output fixtures without questions |
| symfony/yaml | 340 of 342 (2 `n/a`), LintCommandTest 11 of 11 | 150 `lint:yaml` runs (every format, verbosity, `--parse-tags`, several files, `--exclude`, a directory, errors) |
| symfony/dotenv | 57 of 57, DebugCommandTest 15 and DotenvDumpCommandTest 4 | 266 command runs: `debug:dotenv` on five projects, three environments, filters and globals; `dotenv:dump` with the file it writes |

Found along the way and fixed:

- `stripcslashes()` keeps a final lone backslash (the manual does not
  say).
- Natural order (`strnatcmp()`) does not skip whitespace right after a
  run of digits. symfony-finder, already done, had the same bug; its
  corpus gained the cases, its old answers unchanged.

## Advisories

The upstream watch lists four symfony/yaml advisories, all fixed before
the pin; each now has a `covered` row in `parity/advisories.tsv` naming
the ported fix tests. Checked against the oracle with the releases
before each fix (installed in a throwaway container, `oracle/out/`):
v8.1.5 fails GHSA-2v3c's tests, v8.0.11 fails GHSA-4qpc's and
GHSA-c2p3's, and the pin matches the Rust results. GHSA-9frc's own tests
pass on v8.0.11 too, so `crates/yaml/tests/advisories.rs` pins inputs
that v8.0.11 fails (PCRE's backtrack limit) with the pin's answers. No
advisory is listed for symfony/console or symfony/dotenv.

## Left for later

- Console (M7): `Application::run()` and what it drives, the help, list
  and completion commands' work (they are registered by definition),
  questions, progress bars, trees, ConsoleOutput and its sections,
  invokable commands' parameter attributes, signals.
- `lint:yaml`'s deprecation handler: symfony/yaml 8 triggers no
  deprecation while parsing, so the port installs none.
- A file name or argument that is not UTF-8 is refused (decision 11).
- symfony/console has a newer patch release (v8.1.11) touching ported
  files; moving the pin is a separate step.

## Checks

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
`cargo test`, `scripts/sources.sh --check` and `scripts/parity-check.sh`
pass. symfony-demo-rs uses none of these crates, so its comparison is
unaffected.
