# PSR-3 logger interface

Private workspace crate: a port of `psr/log` 3.0.2 (`f16e1d58`), as
installed by symfony/demo. MIT; psr/log copyright PHP Framework
Interoperability Group. Unofficial.

| Contract | Source | Evidence |
|---|---|---|
| `LoggerInterface::log(level, message, context)` and the eight level methods, which delegate to `log` | `LoggerInterface.php`, `LoggerTrait.php`, `AbstractLogger.php` | Unit tests |
| Levels and their string forms (`emergency` … `debug`) | `LogLevel.php` | Unit tests |
| `NullLogger` discards records | `NullLogger.php` | Unit test |

The package has no PHPUnit tests upstream. Levels are a closed enum, so
PSR-3's `InvalidArgumentException` for an unknown level cannot occur.
Context is an ordered list of named values: strings, (under `exception`)
an error, or any other PHP value as a php-value `Value` (PSR-3's
`mixed`, such as RouterListener's route parameters). `LoggerAwareInterface`/`LoggerAwareTrait` are not ported: Rust
code takes the logger in its constructor.
