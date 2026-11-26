# Symfony ExpressionLanguage

Private workspace crate: a port of `symfony/expression-language` v8.1.9
(`9c2729bc`), as installed by symfony/demo. MIT; Symfony source and tests
copyright Fabien Potencier. No PHP source code is used: the PHP operators
and built-ins in `src/php.rs` are written from PHP's documented and
recorded behaviour, on `php-value` and `php-pcre`. Unofficial, not
affiliated with or endorsed by Symfony or PHP.

```rust
use php_value::{PhpArray, Value};
use symfony_expression_language::ExpressionLanguage;

let language = ExpressionLanguage::new();
let mut values = PhpArray::new();
values.insert("price", Value::Int(40));
let label = language.evaluate("price * 2 > 50 ? 'high' : 'low'", &values).unwrap();
assert_eq!(label, Value::from("high"));

let names = PhpArray::list([Value::from("price")]);
assert_eq!(language.compile("price * 2", &names).unwrap(), "($price * 2)");
```

| Contract | Source | Evidence |
|---|---|---|
| Tokens (names, numbers with `_` and exponents, quoted strings with `stripcslashes()`, operators, punctuation, comments), bracket checks, upstream's operator regex | `Lexer.php`, `Token.php`, `TokenStream.php`, `Resources/bin/generate_operator_regex.php` | LexerTest (7); 87 recorded tokenizations |
| Precedence and associativity, unary and binary operators, ternaries, `?:`, `??`, `?.`, arrays and hashes, attribute and method access, names and functions with the `IGNORE_UNKNOWN_*` flags, "Did you mean" proposals, nesting depth limit, `lint()` | `Parser.php`, `SyntaxError.php` | ParserTest (11); 322 recorded expressions (parse tree) |
| Node trees: `__toString()`, `dump()`, `compile()` to PHP source byte for byte (upstream's `matches` closure included), `evaluate()` | `Node/*.php`, `Compiler.php` | The 11 node test classes; 322 expressions (dump, compiled source, result); 55 compiled sources that upstream's tests `eval()` |
| PHP 8.4 semantics of every operator: arithmetic and its `TypeError`s, `**`, `%`, bitwise operators on ints and strings, comparisons (`zend_compare`: numeric strings, objects, enum cases, NAN), `==` / `===`, `in`, `contains`, `starts with`, `ends with`, `~`, `..` (`range()`), `matches` (`preg_match()`) | `Node/BinaryNode.php`, `Node/UnaryNode.php` | BinaryNodeTest; 69,024 binary and 240 unary operations on 48 operands, 576 ranges, 450 `matches` calls recorded from PHP 8.4.27 |
| Functions: `register()`, `addFunction()`, providers, `ExpressionFunction::fromPhp()` with its messages, built-in `constant()`, `min()`, `max()` and `enum()`; no registration after the parser exists | `ExpressionFunction.php`, `ExpressionFunctionProviderInterface.php`, `ExpressionLanguage.php` | ExpressionFunctionTest (2), ExpressionLanguageTest (36) |
| The parse cache: the PSR-6 key (`asort()` of the names, `rawurlencode()`, flags), one parse per key, parsed expressions passed through | `ExpressionLanguage.php`, `Expression.php`, `ParsedExpression.php` | testCachedParse, testCachingWithDifferentNamesOrder |

Vectors: `scripts/record.sh expression-language` runs
`oracle/recorders/expression_language.php` on PHP 8.4.27 and writes
`tests/data/php.json`; `tests/php.rs` replays it. The tests that `eval()`
compiled PHP compare our source with the source PHP ran, and PHP's
result with the upstream assertion.

Parity: 101 of 106 upstream tests ported. The 5 `testSerialization`
tests are `n/a`: they test PHP's `serialize()` of objects.

## Differences

- PHP's warnings ("A non-numeric value", "Undefined array key",
  "Attempt to read property") are not reported. The result is the value
  PHP goes on with.
- `range()` builds at most `php::MAX_RANGE` (2^20) elements. Above that
  it fails with `Class::Unsupported`. PHP builds up to its memory limit.
- `matches` compiles patterns with php-pcre (fancy-regex). A pattern
  that PHP rejects is rejected with the same `SyntaxError`, but the
  reason after "is not valid: " is php-pcre's text (39 of the 450
  recorded calls). A PCRE feature that php-pcre does not translate
  fails with `Class::Unsupported`.
- `TypeError` and `ArgumentCountError` messages stop where PHP adds
  ", called in FILE on line N" or "in FILE on line N".
- Objects: methods can be called on `MethodObject`s (and closures'
  `__invoke`). Array access on objects is `ArrayObject`'s. Other
  objects have properties only.
- `constant()` and `enum()` read a `php_value::constants::Registry`.
  `ExpressionLanguage::new()` has PHP's core constants. `PHP_VERSION` is
  not one of them; `with_constants()` takes a registry with the
  application's constants and enums. A class constant of a class that is
  not a registered enum fails with `Class "X" not found`.
- `SerializedParsedExpression` is not ported: it needs PHP's
  `unserialize()` of the node objects.

## Rust shape

- The node classes are one closed `Node` enum. `GetAttrNode` keeps its
  two mutable flags in atomics, so `evaluate()` takes `&self`.
- Exceptions are one `Error` with a `Class` (`SyntaxError`,
  `RuntimeException`, `TypeError`, ...) and the message bytes;
  `Class::is_a()` follows PHP's class hierarchy.
- The PSR-6 pool is the `CacheItemPool` trait. The default `ArrayCache`
  returns copies, as `ArrayAdapter` stores serialized values.
- PHP functions that `fromPhp()` can find are a `PhpFunctions` table;
  `PhpFunctions::define()` adds a user function.
- `compile()` returns `PhpString` (bytes), as PHP strings are.
- The parser and the node walks recurse, as upstream's do, up to the
  256 nesting levels that the parser accepts. A debug build can need
  more than 1 MiB of stack for an expression near that limit (Windows
  gives 1 MiB to the main thread). Parse such expressions on a thread
  with a larger stack (`std::thread::Builder::stack_size`).
  `testDeeplyNestedExpressionIsRejected` runs on a 16 MiB thread.
