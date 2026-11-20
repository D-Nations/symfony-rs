# Symfony CssSelector

Private workspace crate: a port of `symfony/css-selector` v8.1.6
(`08e29051`), as installed by symfony/demo. MIT; Symfony source and tests
copyright Fabien Potencier; the component is itself a port of Python's
cssselect (copyright Ian Bicking). No PHP source code is used. Unofficial,
not affiliated with or endorsed by Symfony or PHP.

```rust
use symfony_css_selector::CssSelectorConverter;

let converter = CssSelectorConverter::new(true); // false for XML documents
assert_eq!(
    converter.to_xpath(b"div > .item").unwrap(),
    b"descendant-or-self::div/*[@class and contains(concat(' ', normalize-space(@class), ' '), ' item ')]"
);
assert_eq!(
    converter.to_xpath(b"h1:").unwrap_err().message(),
    b"Expected identifier, but <eof at 3> found."
);
```

Selectors and XPath are byte strings, as in PHP: the patterns work on
bytes (no `/u`), so non-UTF-8 input is translated as PHP translates it.

| Contract | Source | Evidence |
|---|---|---|
| `CssSelectorConverter::toXPath()` with and without HTML support, any prefix; the LRU cache (`$maxCachedItems`, oldest evicted, hits moved last) | `CssSelectorConverter.php` | CssSelectorConverterTest (5); 233 recorded cases × 3 (HTML, XML, no prefix) |
| Tokenizer: whitespace, identifiers, hashes, strings, numbers, comments, delimiters; unicode escapes (mod 0x200000, dropped from U+10000, surrogates encoded), simple and newline escapes; token positions | `Parser/Tokenizer/*`, `Parser/Handler/*`, `Parser/Reader.php`, `Parser/Token.php`, `Parser/TokenStream.php` | the six handler tests, ReaderTest, TokenStreamTest; recorded cases |
| Parser: selector lists, combinators, attributes and operators, `:not()`, `:is()`, `:where()`, `:has()` (nesting limits of 16), functions, pseudo-classes and pseudo-elements, the `:scope` position rule, `parseSeries()`; `SyntaxErrorException` messages | `Parser/Parser.php`, `Exception/*` | ParserTest (11 tests on upstream's provider rows); recorded cases |
| Shortcut parsers for the `e`, `ns|e`, `.c`, `#id` forms and the empty selector | `Parser/Shortcut/*` | the four shortcut tests |
| Node tree: `__toString()` forms and specificity, with PHP truthiness (`"0"` counts as empty) | `Node/*` | the twelve node tests (25) |
| Translator and extensions: node, combination, function (`nth-*`, `contains`, `lang`), pseudo-class, attribute matching, relation, HTML (`:checked`, `:link`, `:disabled`, `:enabled`, ...); `ExpressionErrorException` messages; series arithmetic leaving the `int` range printed as PHP's floats | `XPath/*` | TranslatorTest (10 of 15 run here; see below); the XPath of all 191 DOM rows; recorded cases |

Vectors: `scripts/record.sh css-selector` runs
`oracle/recorders/css_selector.php` on PHP 8.4.27 and writes
`tests/data/php.json`: every upstream data provider (objects with their
private properties, so the tests rebuild the same nodes and tokens), the
XPath PHP generated and what it selected for TranslatorTest's DOM rows,
and the extra cases `tests/vectors.rs` checks.

## Not ported

- TranslatorTest's five DOM tests (testXmlLang, testHtmlIds,
  testHtmlShakespear, testOnlyOfTypeFindsSingleChildrenOfGivenType,
  testDisabledAndEnabledInsideFieldsets) evaluate the XPath with libxml.
  There is no XPath engine here (PLAN.md decision 9), so they are `n/a`;
  `dom_rows_translate_as_php_did` checks that every one of their 191 rows
  translates to exactly the XPath PHP evaluated.

## Differences

- A `TypeError` (a null attribute name from `[*|=x]`, or a null value
  passed to the `=`, `|=` or `!=` translators through the public API) has
  PHP's message without its `, called in <file> on line <n>` suffix.
- `ATTRIBUTE_VALUE_IN_LOWER_CASE` on a valueless attribute gives `""` as
  PHP does, without PHP's deprecation notice.

## Rust shape

- `NodeInterface` is closed (the node classes are `@internal`): `Node` is
  an enum, `__toString()` is `to_php_string()`.
- Extensions return their translators as named closures; `NodeExtension`'s
  flags are shared with its closures, so `HtmlExtension::new` setting them
  after registration works as upstream's `$this` closures do.
- Errors are one `Exception` type whose `Kind` is the PHP class.
- The caches and `$maxCachedItems` are per thread (PHP keeps static
  properties per request); `cache_keys()`, `clear_cache()` and the parser's
  depth accessors stand in for upstream's reflection.
