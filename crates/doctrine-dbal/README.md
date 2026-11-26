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
| `QueryBuilder` SQL for SELECT (DISTINCT, FROM with aliases, inner/left/right/full joins in upstream's nesting order, WHERE, GROUP BY, HAVING, ORDER BY, LIMIT/OFFSET, FOR UPDATE [SKIP LOCKED], WITH), UNION [ALL], INSERT, UPDATE and DELETE; `where`/`andWhere`/`orWhere` and the HAVING equivalents' nesting; resets; the SQL cache, kept by `setValue()`, `setParameter()` and a failed `addUnion()` as in PHP | `Query/QueryBuilder.php`, `Query/*.php`, `SQL/Builder/{Select,DefaultSelect,Union,DefaultUnion,With}SQLBuilder.php` | QueryBuilderTest (73 of 84); oracle call sequences on two platforms (`tests/query_builder_vectors.rs`) |
| Parameters: `setParameter(s)`, `getParameter(Type)(s)` with PHP's key normalization and order, `createNamedParameter()` (`:dcValueN`, custom placeholder) and `createPositionalParameter()` sharing one counter; `clone` gives object parameters a new identity | `Query/QueryBuilder.php` | QueryBuilderTest; oracle call sequences |
| Messages of `QueryException`, `NonUniqueAlias`, `UnknownAlias`, `NotSupported` (`FOR UPDATE`, `SKIP LOCKED`) and the negative-offset `InvalidArgumentException` | `Query/QueryException.php`, `Query/Exception/*.php`, `Platforms/Exception/NotSupported.php`, `Exception/InvalidArgumentException.php` | QueryBuilderTest; oracle call sequences |
| `ExpressionBuilder` comparisons, `IS [NOT] NULL`, `[NOT] LIKE ... ESCAPE`, `[NOT] IN (...)`, `and()`/`or()`; `literal()` delegates to the connection's `quote()` | `Query/Expression/ExpressionBuilder.php` | ExpressionBuilderTest (19) |
| `CompositeExpression` `and()`/`or()`, immutable `with()`, `count()`, string form (one part bare, else parenthesized and joined) | `Query/Expression/CompositeExpression.php` | CompositeExpressionTest (3) |
| `AbstractPlatform` defaults: `createSelectSQLBuilder()` (`FOR UPDATE`, `SKIP LOCKED`), `createUnionSQLBuilder()`, `createWithSQLBuilder()`, `modifyLimitQuery()`/`doModifyLimitQuery()`, `getUnionSelectPartSQL()` (`(sub)`), `getUnionAllSQL()`, `getUnionDistinctSQL()` | `Platforms/AbstractPlatform.php` | oracle call sequences ("default" platform: the methods as PostgreSQLPlatform keeps them) |
| `Type` trait and the process-wide registry: `getType()`, `hasType()`, `addType()` (class or instance), `overrideType()`, `lookupName()`, `getTypesMap()`, identity by instance; `TypeRegistry` and its `TypeNotFound`, `TypesAlreadyExists`, `TypeAlreadyRegistered`, `TypeNotRegistered`, `UnknownColumnType`, `TypeArgumentCountError` messages | `Types/Type.php`, `Types/TypeRegistry.php`, `Types/Types.php`, `Types/Exception/*.php` | TypeTest (4), TypeRegistryTest (13), TypeAlreadyRegisteredTest (1), TypeNotRegisteredTest (1) |
| The 31 built-in types' PHP and database conversions, binding types, SQL declarations (through the platform) and `ConversionException` messages: strings, GUID, integers, floats, decimal, `BcMath\Number`, boolean, binary, blob, JSON and JSON object, simple array, enum, date, time, date-time (mutable, immutable, tz, UTC, variable) and date interval | `Types/*Type.php`, `Types/ConversionException.php`, `Types/Exception/*.php` | 164 tests of the 27 type test files (`tests/types_*_test.rs`; the inherited `BaseDateTypeTestCase` tests repeated in each date class), ConversionExceptionTest (4), SerializationFailedTest (1) |
| The `AbstractPlatform` methods the types call (declaration snippets, boolean conversion, date formats) and the column exceptions they throw (`ColumnLengthRequired`, `ColumnPrecisionRequired`, `ColumnScaleRequired`, `ColumnValuesRequired`, `InvalidColumnDeclaration`) | `Platforms/AbstractPlatform.php` (partly), `Exception/*.php` | The type tests above, through a stub platform as upstream mocks it; no concrete platform is ported |
| `DateTime::createFromFormat()`, `new DateTime($text)` and `new DateInterval($duration)` as the date types use them (in php-value) | php-value | 12,036 + 89 + 1,602 recorded answers (`tests/date_vectors.rs`); the texts refused are counted there |
| `Connection`: lazy connect, `executeQuery()`, `executeStatement()`, `prepare()`, the `fetch*()` and `iterate*()` helpers, `insert()`/`update()`/`delete()`, list parameter expansion, type conversion of bound values, `quote()`/`quoteIdentifier()`, `lastInsertId()`, auto-commit, nested transactions with savepoints, rollback-only, `transactional()` with upstream's rollback rules, isolation levels, `close()`, and the conversion of driver exceptions (with the query) | `Connection.php`, `ConnectionException.php`, `TransactionIsolationLevel.php` | ConnectionTest (29 of 33, over a recording test driver); functional ConnectionTest (17 of 21), TransactionTest (1 of 6), WriteTest (19 of 20), NamedParametersTest, DataAccessTest (36), BooleanBindingTest, AutoIncrementColumnTest on `pdo_sqlite` |
| `Result` and `Statement`: `fetchNumeric/Associative/One()`, `fetchAll*()`, `fetchAllKeyValue()` (`NoKeyValue`), `fetchAllAssociativeIndexed()`, `fetchFirstColumn()`, the `iterate*()` forms, `rowCount()`, `columnCount()`, `getColumnName()`, `bindValue()` with wrapper types, `executeQuery()`/`executeStatement()` | `Result.php`, `Statement.php`, `Driver/FetchUtils.php` | Functional ResultTest, StatementTest (11 of 12), Connection/FetchTest (16), Connection/FetchEmptyTest (6) |
| Exceptions: the `SQLite` exception converter (`UniqueConstraintViolation`, `NotNullConstraintViolation`, `TableNotFound`, `TableExists`, `InvalidFieldName`, `NonUniqueFieldName`, `SyntaxError`, `ReadOnly`, `LockWaitTimeout`, `ConnectionException`, ...), `DriverException`'s message (`An exception occurred while executing a query: ...`), the `Exception/*` messages | `Driver/API/SQLite/ExceptionConverter.php`, `Exception/*.php` | Functional ExceptionTest (9 of 12); the recorded `pdo_sqlite` errors |
| The `pdo_sqlite` driver on sqlx: value storage classes, PDO's binding conversions, `SQLSTATE` messages, `rowCount()`, `lastInsertId()` (`NoIdentityValue`), `quote()`, `getServerVersion()`, `path`/`memory` parameters | `Driver/PDO/SQLite/Driver.php`, `Driver/PDO/*.php`, `Driver/AbstractSQLiteDriver.php`, `Driver/*.php` interfaces | `tests/pdo_sqlite_vectors.rs` (recorded `pdo_sqlite` answers) |
| `DriverManager::getConnection()` (driver map, `driverClass`, `wrapperClass` checks, URL parameters through `DsnParser`), `getAvailableDrivers()`; `DsnParser::parse()`; the `Configuration` parts the connection reads (auto-commit, type comments) | `DriverManager.php`, `Tools/DsnParser.php`, `Configuration.php` | DriverManagerTest (7 of 8), DsnParserTest (2), ConfigurationTest (2) |
| `QueryBuilder` execution: `executeQuery()`, `executeStatement()` and the `fetch*()` methods over a `Connection` | `Query/QueryBuilder.php` | QueryBuilderTest execution tests (9; `tests/query_builder_execution_test.rs`) |

Vectors: `scripts/record.sh doctrine-dbal` runs `oracle/recorders/doctrine_dbal.php`
and writes `tests/data/php.json`: the upstream data providers, the
tokens that the real parser gives for generated statements, and
`QueryBuilder` call sequences with each call's result, the final SQL or
exception, and the parameters, and PHP's answers for generated date
formats, date texts and interval durations. Some date rows depend on the
clock (`now`, an empty text), so a new recording changes them.

`ConnectionStandIn` is the part of `Connection.php` the query builder
uses (`getDatabasePlatform()`, `quote()`, `createExpressionBuilder()`,
`createQueryBuilder()`); `Connection` implements it, and a test can too.

Connection vectors: `scripts/record.sh doctrine-dbal-connection` runs
`oracle/recorders/doctrine_dbal_connection.php` and writes
`tests/data/connection.json`: the providers of DsnParserTest,
DriverManagerTest, NamedParametersTest and DataAccessTest; the tables of
the functional tests as `SQLitePlatform` creates them; the SQL that
`SQLitePlatform` builds for those tests; and `pdo_sqlite`'s answers for
bindings, errors, `lastInsertId()` and connect parameters
(`tests/pdo_sqlite_vectors.rs`). `src/platforms/sqlite_platform_stand_in.rs`
holds the place of `SQLitePlatform.php` until the platform slice is
merged.

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

### Query

- Not ported yet, as they need `Cache\QueryCacheProfile`:
  `enableResultCache()` and `disableResultCache()`; their test stays
  pending. QueryBuilderTest `testExecuteStatement` stays pending too: it
  passes named parameters to `?` placeholders, which only a mocked
  `Connection::executeStatement()` accepts (a real one throws
  `MissingPositionalParameter`), and `Connection` cannot be mocked here.
- `(string) $qb` is `get_sql()`, which returns the exceptions that PHP's
  `__toString()` throws; there is no `Display`.
- A `QueryBuilder` given to `with()`, `union()` or `addUnion()` is moved
  into the query (and shared by its clones, as PHP shares the object). PHP
  keeps a reference, so changing the sub-builder afterwards changes the
  outer query; in Rust it cannot be changed once given.
- `clone` gives `stdClass`, `ArrayObject` and recorded-class (generic)
  parameters a new identity, as `__clone()` does. Other objects
  (closures, enum cases, custom `PhpObject`s) are shared: they have no
  general copy here, and PHP refuses to clone an enum case (`Error`),
  which `Clone` cannot report.
- An unknown join alias that is a canonical decimal integer (`'9'`)
  becomes an int array key in PHP, and `UnknownAlias::new()` refuses it
  with a `TypeError` naming the file and line. The port throws the
  `UnknownAlias` that PHP means; `tests/query_builder_vectors.rs` checks
  that this is the only difference in the recorded sequences.
- SQL fragments are Rust strings (UTF-8); PHP accepts any bytes.
  Parameter values and keys keep PHP's types (`Value`, `Key`).
- PHP's variadic arguments are a first argument and a list (`where($p,
  [])`, `addSelect('a', &[])`); PHP's default arguments are explicit:
  `DEFAULT_PARAMETER_TYPE` (`ParameterType::STRING`),
  `ConflictResolutionMode::default()` (`ORDINARY`), `UnionType::default()`
  (`DISTINCT`), `distinct(true)`.
- `modifyLimitQuery()` is final in PHP; a Rust trait method cannot be
  final, so platforms must override `do_modify_limit_query()` only.
- PHP's `From` class is `FromTable`, so that it does not shadow Rust's
  `From` trait.

### Types

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

### Connection

- The API is async (PLAN.md, decision 22): the methods that may reach the
  database are `async fn`; a driver result is read at execute time, so
  `Result`'s `fetch*()` and `iterate*()` are synchronous. One connection
  serves one task at a time, as one PDO handle does.
- `pdo_sqlite` reads only the first statement given to `prepare()` or
  `query()`; sqlx runs them all. The recorded vectors skip these inputs.
- A string parameter that is not UTF-8 is refused (sqlx binds text as
  Rust strings); PDO binds the bytes as TEXT. Bind it as `BINARY` or
  `LARGE_OBJECT`.
- `driverOptions` (PDO attributes) are ignored; `persistent` is not
  supported (upstream skips its test on `SQLite`).
- A column name that is not UTF-8 is read lossily (sqlx gives column
  names as Rust strings).
- `getColumnName()` past the last column: PDO answers `100 another row
  available` when nothing was fetched yet and rows remain; that rule is
  kept, but after every row was fetched the port throws
  `InvalidColumnIndex` where PDO's message can depend on the statement's
  state.
- A `PDOException` is not a separate level of the `previous` chain: the
  driver's exception is the `PDO\Exception` itself.
- `wrapperClass` must be `Doctrine\DBAL\Connection`: Rust has no
  `Connection` subclasses, so any other class is refused with
  `InvalidWrapperClass` (DriverManagerTest `testCustomWrapper` and
  ConnectionTest `testItFailsDuringCommitBeforeTouchingDb` stay pending).
  A custom `driverClass` is registered with `DriverManager::get_connection_with()`
  (a `DriverClass`), and a DSN's driver class must be declared to
  `DsnParser`.
- The drivers of the driver map other than `pdo_sqlite` build a
  connection, but `connect()` refuses with `Unsupported` until they are
  ported.
- Parameter types are a `BTreeMap` (integer keys first), so
  `is_string(key($types))` is true only when every key is a string; PHP
  looks at the first key in insertion order. Mixed keys do not occur in
  DBAL's own calls.
- An `ArrayParameterType` that reaches `getBindingInfo()` (a value that
  was not expanded) throws the `TypeError` of PHP's parameter check, with
  its message but without the `called in <file> on line <n>` suffix.
- `ConnectionStandIn` is synchronous: through it, `quote()` needs a
  connection that is already open (otherwise `Unsupported`), and the
  platform is known without the database only for a driver that does not
  read the server version (the `SQLite` driver does not).
- Not ported: the result cache (`executeCacheQuery()`,
  `QueryCacheProfile`), the schema manager (`createSchemaManager()`, its
  ConnectionTest tests pending), `getNativeConnection()`, middlewares,
  the deprecation notices. A `BlobType` stream cannot be read
  (StatementTest `testFetchLongBlob` pending).
- Pending because upstream skips or needs other platforms on `SQLite`:
  functional ConnectionTest's three savepoints-not-supported tests and
  `testPersistentConnection`, TransactionTest's five connection-loss
  tests, WriteTest `testLastInsertIdNotSupported`, ExceptionTest's
  invalid user, password and host tests.
