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
| Inline YAML: flow sequences and mappings, quoted and plain scalars, ints/floats/octal/hex/`.inf`/`.nan`, booleans and null, anchors and aliases, merge keys, `!!str` `!!float` `!!binary`, custom tags, `!php/const` `!php/enum` `!php/object`, `PARSE_OBJECT_FOR_MAP`, nesting and alias limits, and `dump()` with the `DUMP_*` flags | `Inline.php`, `ParserState.php` | InlineTest (84 of 85; 1 `n/a`), on upstream's provider data |
| Block YAML: mappings and sequences (compact and unindented), block scalars with chomping and indentation indicators, plain and quoted multi-line scalars, multi-line flow collections, anchors and aliases (circular references), merge keys, tags on blocks, `%YAML` headers, document markers and the root node's properties, line numbers and snippets in errors, `parseFile()` | `Parser.php` | ParserTest (171 of 172, 1 `n/a`): every call each upstream test makes, replayed |
| Timestamps: what PHP's date parser accepts among strings matching the timestamp regex, rollover of out-of-range fields, int/float timestamps and `PARSE_DATETIME` objects | `Inline.php` + PHP behaviour | 1,221 recorded inputs |

Vectors: `scripts/record.sh yaml` runs `oracle/recorders/yaml_*.php` with
the pinned Symfony Yaml and writes `tests/data/`, including the upstream
tests' data-provider rows (`yaml_test_providers.php`) and, for ParserTest,
every call each upstream test makes to the parser with what PHP returned
(`yaml_test_calls.php`: it runs the upstream test methods on the oracle
with a recording subclass of the real Parser, and fails unless every
upstream test passes there). `tests/parser_test.rs` replays those calls
under the upstream test names and checks the exception each test expects.
`tests/Fixtures/` is upstream's Tests/Fixtures (MIT), without the PHP
enums.

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
- The nested parsers PHP creates for blocks share the anchors, the
  `ParserState` and Inline's static state with their parent; here one
  borrowed `Shared` holds them for every block of a parse.
- Upstream's patterns stay `regex` patterns where the crate matches the
  same; the ones whose possessive quantifiers or lookarounds change the
  match (the mapping-key line, document markers, `%YAML` header, root
  anchor) are matchers written to PCRE's semantics.

## Differences

- Where PHP throws a `TypeError` instead of a `ParseException` (a scalar
  merged under `<<: &anchor`, an object as a merge item), the port returns
  a `ParseException` with the `TypeError`'s message, without its
  "called in <file> on line N" suffix.

## Not ported yet

`Dumper.php`, the `Yaml::parse*` / `dump` entry points and
`Command/LintCommand.php` (waits for Console); DumperTest, YamlTest,
YamlTestSuiteTest and LintCommandTest (83 tests).
