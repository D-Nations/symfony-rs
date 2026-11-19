# Symfony Yaml (in progress)

Private workspace crate: a port of `symfony/yaml` v8.1.9 (`6107ee26`), as
installed by symfony/demo. MIT; Symfony source and tests copyright Fabien
Potencier. Unofficial, not affiliated with or endorsed by Symfony.
Values are `php-value` values (PLAN.md decision 3).

| Contract | Source | Evidence |
|---|---|---|
| `Yaml::PARSE_*` / `DUMP_*` flag values | `Yaml.php` | — |
| `ParseException` message: final dot moved after ` in "<file>"` (json_encode, unescaped slashes and Unicode), ` at line N` (N ≥ 0), ` (near "<snippet>")` (truthy snippet) | `Exception/ParseException.php` | ParseExceptionTest (2); unit tests |
| Double-quoted escapes (`\0 \a \b \t \n \v \f \r \e \  \" \/ \\ \N \_ \L \P \xHH \uHHHH \UHHHHHHHH`), PCRE alternative order (`\xZZ` is `\x` + `ZZ`), `utf8chr()` bytes, `Found unknown escape character "%s".`; single-quoted `''` | `Unescaper.php` | 333 recorded cases |
| When strings need double or single quoting (keywords, `\s`/`\p{Zs}` under `/u`), and how they are escaped (`str_replace` order) | `Escaper.php` | 347 recorded cases |
| `TaggedValue` | `Tag/TaggedValue.php` | InlineTest tag tests |
| Inline YAML: flow sequences and mappings, quoted and plain scalars, ints/floats/octal/hex/`.inf`/`.nan`, booleans and null, anchors and aliases, merge keys, `!!str` `!!float` `!!binary`, custom tags, `!php/const` `!php/enum` `!php/object`, `PARSE_OBJECT_FOR_MAP`, nesting and alias limits, and `dump()` with the `DUMP_*` flags | `Inline.php`, `ParserState.php` | InlineTest (83 of 85; 1 `n/a`, 1 waiting for Parser), on upstream's provider data |
| Timestamps: what PHP's date parser accepts among strings matching the timestamp regex, rollover of out-of-range fields, int/float timestamps and `PARSE_DATETIME` objects | `Inline.php` + PHP behaviour | 1,221 recorded inputs |

Vectors: `scripts/record.sh yaml` runs `oracle/recorders/yaml_*.php` with
the pinned Symfony Yaml and writes `tests/data/`, including the upstream
tests' data-provider rows (`yaml_test_providers.php`).

## Rust shape

- PHP's static `Inline` state is an `Inline` value. Constants, enums and
  the classes `unserialize()` may create come from an `Environment`
  (PHP's global state); `Environment::new()` has PHP's core constants and
  the `Yaml::*` flags.
- Input and strings are bytes (`PhpString`), as in PHP. Where upstream
  applies a `/u` pattern, input that is not UTF-8 fails with PCRE's
  message.
- `!php/object` unserializes only allowed classes, with PHP's
  `allowed_classes` semantics (PLAN.md decision 3).

## Not ported yet

`Parser.php`, `Dumper.php`, the `Yaml::parse*` / `dump` entry points and
`Command/LintCommand.php` (waits for Console); ParserTest, DumperTest,
YamlTest, YamlTestSuiteTest, LintCommandTest and one InlineTest method
that uses `Yaml::parse()` (256 tests).
