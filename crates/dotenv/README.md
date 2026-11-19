# Symfony Dotenv

Private workspace crate: a port of `symfony/dotenv` v8.1.6 (`4ea87b35`), as
installed by symfony/demo. MIT; Symfony source and tests copyright Fabien
Potencier. Unofficial, not affiliated with or endorsed by Symfony.

| Contract | Source | Evidence |
|---|---|---|
| .env syntax: names (`export`), unquoted, single- and double-quoted and concatenated values, escapes, comments, line breaks; `$VAR`, `${VAR}`, `${VAR:-default}`, `${VAR:=default}` and their escaping; `$(...)` commands; every error message with its context lines | `Dotenv.php`, `Exception/*` | DotenvTest on upstream's provider rows (`testParse`: 93, `testParseWithFormatError`: 19); 231 generated inputs recorded from PHP |
| `load()`, `overload()`, `loadEnv()` (`.env`, `.env.dist`, `.env.local`, `.env.$env`, `.env.$env.local`, test envs), `bootEnv()` (`.env.local.php`, the debug var), `populate()`, `SYMFONY_DOTENV_VARS` / `SYMFONY_DOTENV_PATH`, deferred resolution across files, self-references, external vars from `$_ENV`, `$_SERVER` or only `getenv()`, `usePutenv()` | `Dotenv.php` | DotenvTest (38 of 38) |

Vectors: `scripts/record.sh dotenv` records upstream's provider rows
(`oracle/recorders/test_providers.php`) and `parse()` on generated inputs
(`oracle/recorders/dotenv_parse.php`).

## Rust shape

- PHP's globals are a `Globals` value: `$_ENV`, `$_SERVER` (php-value
  arrays), what `getenv()` reads and `putenv()` writes, and the runner of
  `$(...)` commands. `Globals::new()` is empty; `Globals::from_process()`
  starts from the process environment as PHP's CLI does.
- `putenv()` writes an overlay that `getenv()` reads first: changing the
  real process environment is `unsafe` in Rust, which the crates forbid.
  Commands get the variables passed explicitly, as Symfony Process passes
  them.
- `$(...)` runs `echo $(...)` with `/bin/sh -c`, with the environment
  Symfony Process gives a command on the CLI (the file's own variables,
  then `$_ENV`, then `getenv()` limited to the names in `$_SERVER`). On
  Windows it fails with upstream's `LogicException`. Process's `"${:NAME}"`
  placeholders and its 60-second timeout are not reproduced; Dotenv will
  use the Process port when it exists.
- Upstream's PCRE patterns are matchers written to PCRE's semantics
  (possessive quantifiers, the lookbehind, the recursive pattern for
  balanced parentheses).

## Differences

- `.env.local.php` is not run: `bootEnv()` reads the literal array
  `composer dump-env` writes (`array(...)` or `[...]` of scalars, single-
  or double-quoted strings without interpolation, comments). Any other PHP
  in it fails with a `LogicException` naming the file.
- A value that ends right after `$(` (`FOO=$(` at the end of the data)
  makes upstream read past the end of the data and loop until PHP's time
  limit; the port fails with "Missing closing parenthesis.", as for any
  unclosed `$(`.
- testGetVariablesValueFromEnvFirst's command reads the variable with
  `printenv` instead of `php -r`: PHP is not installed on hosts here.

## Not ported yet

`Command/DebugCommand.php` and `Command/DotenvDumpCommand.php` with their
tests (19): they wait for the Console port.
