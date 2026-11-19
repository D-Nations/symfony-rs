# Symfony Config

Private workspace crate: a port of `symfony/config` v8.1.10 (`4a0e7dd1`), as
installed by symfony/demo. MIT; Symfony source and tests copyright Fabien
Potencier. Unofficial, not affiliated with or endorsed by Symfony.

| Contract | Source | Evidence |
|---|---|---|
| Configuration nodes: VariableNode, ScalarNode, BooleanNode (nullable), NumericNode / IntegerNode / FloatNode (min, max), StringNode, EnumNode (values, unit and backed enums, casts from backing values), ArrayNode, PrototypedArrayNode; `normalize()` / `merge()` / `finalize()` with equivalent values, normalization and final validation closures, placeholders and unique prefixes, deprecations, attributes, info hints; every error message and exception class | `Definition/BaseNode.php`, `Definition/*Node.php`, `Definition/NodeInterface.php`, `Definition/PrototypeNodeInterface.php`, `Definition/Exception/*`, `Exception/LogicException.php` | BaseNodeTest, ArrayNodeTest, PrototypedArrayNodeTest, ScalarNodeTest, BooleanNodeTest, IntegerNodeTest, FloatNodeTest, StringNodeTest, EnumNodeTest (75 of 75); 1,938 recorded cases |
| ArrayNode details: dashed keys to underscores, XML singular-to-plural remapping, unrecognized options with "Did you mean" guesses (`levenshtein()` below 3, `sort()` order), extra keys ignored or kept, deep merge, `auto_enable`, new keys refused, required children, defaults | `Definition/ArrayNode.php` | ArrayNodeTest; recorded `unrecognized`, `pre_normalize` cases |
| PrototypedArrayNode details: key attributes (removed or kept, float keys through `var_export()`, duplicates), value-only elements and value prototypes, default children, minimum count, list and map merges | `Definition/PrototypedArrayNode.php` | PrototypedArrayNodeTest; recorded `keyed_normalize`, `prototyped_merge` cases |
| `Processor::process()`, `Processor::normalizeConfig()` | `Definition/Processor.php` | recorded `normalize_config` cases |

Vectors: `scripts/record.sh config` runs
`oracle/recorders/config_definition.php` on the pinned packages and writes
`tests/data/definition.json`; `tests/recorded.rs` replays it.

## Rust shape

- Every node class is a `Node`: a handle to one shared, mutable node, as a
  PHP object is. `Node::clone_node()` is PHP's `clone`. The classes are
  constructed with `VariableNode::new(...)`, `ArrayNode::new(...)` and so
  on; a node class written elsewhere implements `CustomNode`.
- A method the node's class does not have (`addChild()` on a ScalarNode)
  fails with PHP's `\Error`, "Call to undefined method", instead of
  changing anything.
- Exceptions are one `Error` carrying its `Class`; `Class::is_a()` answers
  `instanceof`, so `finalize()` can tell Definition exceptions, other
  `\Exception`s and `\Error`s apart as upstream does.
- Placeholders (`setPlaceholder()`, static in PHP) are thread-local: the
  same as PHP's one thread, and separate for parallel tests.
- Parents are strong references, as in PHP, so a tree is a reference cycle
  that is not freed; trees are built at build time.
- PHP's class table is `php_value::constants::Registry`: EnumNode looks up
  `$enumFqcn` there. Whether an enum is backed is read from its cases.
- `trigger_deprecation()` is `symfony-deprecation-contracts`: a deprecated
  child that is set reaches the handler installed on the thread.
- The PHP built-ins (`levenshtein()`, the loose comparison and `sort()`,
  `strtr()`, `json_encode()` of scalars, `var_export()` of enum cases,
  `BackedEnum::from()`'s casts) are written from PHP's documentation and
  checked against the recorded cases (`src/php.rs`).

## Differences

- `BackedEnum::from()` on an int-backed enum with a numeric string that has
  a fraction (`"1.5"`) truncates it, as PHP does, but does not raise PHP's
  `E_DEPRECATED` ("Implicit conversion from float-string").
- `Processor::normalizeConfig()` lists an object as one element. PHP calls
  `key()` on it (deprecated) and returns the object itself when its first
  property name is an int, which then fails the `array` return type.
- `sort()` is a stable sort with PHP 8's comparison; for keys whose
  comparisons are not transitive (mixes of numeric and non-numeric
  strings), PHP's own algorithm may order them differently. The recorded
  mixes agree.

## Not ported yet

`processConfiguration()` and `Definition/Configuration.php`,
`Definition/Builder/*` (TreeBuilder and the node definitions), the
reference dumpers, `ArrayShapeGenerator`, `Definition/Loader`,
`Definition/Configurator`, and the loaders, resources, `ConfigCache`,
`FileLocator` and `Util/XmlUtils` (with `php-dom`, PLAN.md decision 9):
259 upstream tests pending in `parity/symfony/config.tsv`.
