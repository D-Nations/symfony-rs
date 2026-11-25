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

Vectors: `scripts/record.sh doctrine-dbal` runs `oracle/recorders/doctrine_dbal.php`
and writes `tests/data/php.json`: the upstream data providers, the
tokens that the real parser gives for generated statements, and
`QueryBuilder` call sequences with each call's result, the final SQL or
exception, and the parameters.

`Connection.php` is not ported. `ConnectionStandIn` is the part of it the
query builder uses (`getDatabasePlatform()`, `quote()`,
`createExpressionBuilder()`, `createQueryBuilder()`); an application or a
test implements it.

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

- Not ported yet, as they need `Connection`, `Result` and
  `Cache\QueryCacheProfile`: `QueryBuilder::executeQuery()`,
  `executeStatement()`, `fetchAssociative()`, `fetchNumeric()`,
  `fetchOne()`, `fetchAllNumeric()`, `fetchAllAssociative()`,
  `fetchAllKeyValue()`, `fetchAllAssociativeIndexed()`,
  `fetchFirstColumn()`, `enableResultCache()` and `disableResultCache()`.
  Their 11 tests stay pending in the parity manifest.
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
