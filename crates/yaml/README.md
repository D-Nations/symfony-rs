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
| `TaggedValue` | `Tag/TaggedValue.php` | — |

Vectors: `scripts/record.sh yaml` runs `oracle/recorders/yaml_*.php` with
the pinned Symfony Yaml and writes `tests/data/`.

## Not ported yet

`Inline.php`, `Parser.php`, `ParserState.php`, `Dumper.php`, the
`Yaml::parse*` / `dump` entry points and `Command/LintCommand.php`;
InlineTest, ParserTest, DumperTest, YamlTest, YamlTestSuiteTest and
LintCommandTest (340 tests).
