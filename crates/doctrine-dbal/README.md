# Doctrine DBAL

Private workspace crate: a port of `doctrine/dbal` 4.5.0 (`c435cd7e`), as
installed by symfony/demo. MIT; Doctrine source and tests copyright the
Doctrine Project. Unofficial, not affiliated with or endorsed by Doctrine.

```rust
use std::collections::BTreeMap;

use doctrine_dbal::sql::Parser;
use doctrine_dbal::{ArrayParameterType, ExpandArrayParameters, WrapperParameterType};
use php_value::{Key, PhpArray, Value};

let mut params = PhpArray::new();
params.insert("ids", Value::Array(PhpArray::list([Value::Int(1), Value::Int(2)])));
let mut types = BTreeMap::new();
types.insert(Key::from("ids"), WrapperParameterType::Array(ArrayParameterType::Integer));

let mut visitor = ExpandArrayParameters::new(&params, &types);
Parser::new(false)
    .parse(b"SELECT * FROM t WHERE id IN (:ids)", &mut visitor)
    .unwrap();
assert_eq!(visitor.get_sql().as_bytes(), b"SELECT * FROM t WHERE id IN (?, ?)");
assert_eq!(visitor.get_parameters(), &[Value::Int(1), Value::Int(2)]);
```

| Contract | Source | Evidence |
|---|---|---|
| Statement parameters found by `SQL\Parser`: named (`:name`), positional (`?`, not `??`), skipped inside ANSI or MySQL string literals, backtick and bracket identifiers (not `ARRAY[`), `::` casts and comments | `SQL/Parser.php`, `SQL/Parser/Visitor.php` | ParserTest (4); oracle vectors (`tests/parser_vectors.rs`) |
| List parameters expanded (`IN (?)` to `IN (?, ?, ?)`, empty list to `NULL`), named parameters made positional, types moved to their new positions; `MissingNamedParameter` / `MissingPositionalParameter` messages | `ExpandArrayParameters.php`, `ArrayParameters/Exception/*.php` | ExpandArrayParametersTest (3); oracle vectors |
| `ParameterType` and `ArrayParameterType` cases and `toElementParameterType()` | `ParameterType.php`, `ArrayParameterType.php` | ExpandArrayParametersTest |
| `Type` trait and the process-wide registry: `getType()`, `hasType()`, `addType()` (class or instance), `overrideType()`, `lookupName()`, `getTypesMap()`, identity by instance; `TypeRegistry` and its `TypeNotFound`, `TypesAlreadyExists`, `TypeAlreadyRegistered`, `TypeNotRegistered`, `UnknownColumnType`, `TypeArgumentCountError` messages | `Types/Type.php`, `Types/TypeRegistry.php`, `Types/Types.php`, `Types/Exception/*.php` | TypeTest (4), TypeRegistryTest (13), TypeAlreadyRegisteredTest (1), TypeNotRegisteredTest (1) |
| The 31 built-in types' PHP and database conversions, binding types, SQL declarations (through the platform) and `ConversionException` messages: strings, GUID, integers, floats, decimal, `BcMath\Number`, boolean, binary, blob, JSON and JSON object, simple array, enum, date, time, date-time (mutable, immutable, tz, UTC, variable) and date interval | `Types/*Type.php`, `Types/ConversionException.php`, `Types/Exception/*.php` | 164 tests of the 27 type test files (`tests/types_*_test.rs`; the inherited `BaseDateTypeTestCase` tests repeated in each date class), ConversionExceptionTest (4), SerializationFailedTest (1) |
| The `AbstractPlatform` methods the types call (declaration snippets, boolean conversion, date formats) and the column exceptions they throw (`ColumnLengthRequired`, `ColumnPrecisionRequired`, `ColumnScaleRequired`, `ColumnValuesRequired`, `InvalidColumnDeclaration`) | `Platforms/AbstractPlatform.php` (partly), `Exception/*.php` | The type tests above, through a stub platform as upstream mocks it; no concrete platform is ported |
| `DateTime::createFromFormat()`, `new DateTime($text)` and `new DateInterval($duration)` as the date types use them (in php-value) | php-value | 12,036 + 89 + 1,602 recorded answers (`tests/date_vectors.rs`); the texts refused are counted there |

Vectors: `scripts/record.sh doctrine-dbal` runs `oracle/recorders/doctrine_dbal.php`
and writes `tests/data/php.json`: the upstream data providers, and the
tokens that the real parser gives for generated statements, and PHP's
answers for generated date formats, date texts and interval durations.

## Differences

PHP's parser is one PCRE pattern. PCRE stops at its limits where these
matchers, which do not backtrack, go on; PHP then throws
`SQL\Parser\Exception\RegularExpressionError`, which is not ported. Found
with the oracle (PHP 8.4.27, default `pcre.backtrack_limit` and JIT):

- An unterminated `/*` comment followed by about 20 or more bytes without
  `*`: "Backtrack limit exhausted" (the comment pattern backtracks
  exponentially). The Rust parser reads `/` as a single byte and goes on,
  as PHP does for shorter tails; the database then refuses the SQL.
- A MySQL-mode string literal of about 6,200 bytes or more, a bracket
  identifier of about 25,000 bytes, or an unterminated comment with many
  `*`: "JIT stack limit exhausted". The Rust parser accepts them.

`tests/parser_vectors.rs` checks that each statement PHP refuses is one of
these cases. Whether to copy these limits is a decision for the owner
(PLAN.md, decision 21).

- A list parameter given as an object is refused with the `TypeError`
  message that `count()` gives for a non-countable value. For a
  `\Countable` object PHP's message names `appendTypedParameter()` and a
  file path instead.

Types:

- No resources: a stream given to `BinaryType`, `BlobType`, `JsonType` or
  `JsonObjectType` cannot be written; `BlobType` refuses a string
  (`ConversionError::Unsupported`) where PHP opens a `php://temp` stream.
  The upstream tests that read a stream (BinaryTest, JsonTest,
  JsonObjectTest `testJsonResourceConvertsToPHPValue`, a `tmpfile()` row of
  ConversionExceptionTest) stay pending.
- No recursive values: `json_encode()`'s recursion error cannot happen, so
  JsonTest and JsonObjectTest `testSerializationFailure` stay pending.
- No IANA time zones: DateTest `testDateRestsSummerTimeAffection`
  (Europe/Berlin) stays pending; the UTC date-time tests that use
  America/New_York use the fixed offset -05:00, which gives the same
  answers for the January dates they convert.
- Date and interval texts outside what php-value reads (relative words,
  `R` recurrences, `/` periods, texts of only blanks) are refused
  (`ConversionError::Unsupported`) instead of answered.
- `TypeError` messages leave out PHP's `called in <file> on line <n>`
  suffix.
- `getEnumDeclarationSQL()` refuses a `length` that is not an int and values
  that are not UTF-8 (`Exception` class `Unsupported`), where PHP casts;
  `getDecimalTypeDeclarationSQL()` likewise refuses an array or object
  precision or scale.
- The `jsonb` deprecation (`Deprecation::trigger`) is not raised.
- `TypeAlreadyRegistered` and `TypeNotRegistered` name the instance by a
  hash of its address, not by PHP's `spl_object_hash()`.
- `BigIntType` with an out-of-range float throws the `AssertionError`
  PHP throws with assertions on (as under PHPUnit); with assertions off
  PHP returns the value.
- `BcMath\Number` is not in the oracle image; its parsing and comparison
  were checked on PHP 8.4.27 with the bcmath extension and are pinned in
  php-value's unit tests.
