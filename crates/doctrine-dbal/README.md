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

Vectors: `scripts/record.sh doctrine-dbal` runs `oracle/recorders/doctrine_dbal.php`
and writes `tests/data/php.json`: the upstream data providers, and the
tokens that the real parser gives for generated statements.

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
