# Symfony Config

Private workspace crate: a port of `symfony/config` v8.1.10 (`4a0e7dd1`), as
installed by symfony/demo. MIT; Symfony source and tests copyright Fabien
Potencier. Unofficial, not affiliated with or endorsed by Symfony.

| Contract | Source | Evidence |
|---|---|---|
| Configuration nodes: VariableNode, ScalarNode, BooleanNode (nullable), NumericNode / IntegerNode / FloatNode (min, max), StringNode, EnumNode (values, unit and backed enums, casts from backing values), ArrayNode, PrototypedArrayNode; `normalize()` / `merge()` / `finalize()` with equivalent values, normalization and final validation closures, placeholders and unique prefixes, deprecations, attributes, info hints; every error message and exception class | `Definition/BaseNode.php`, `Definition/*Node.php`, `Definition/NodeInterface.php`, `Definition/PrototypeNodeInterface.php`, `Definition/Exception/*`, `Exception/LogicException.php` | BaseNodeTest, ArrayNodeTest, PrototypedArrayNodeTest, ScalarNodeTest, BooleanNodeTest, IntegerNodeTest, FloatNodeTest, StringNodeTest, EnumNodeTest (75 of 75); 1,938 recorded cases |
| ArrayNode details: dashed keys to underscores, XML singular-to-plural remapping, unrecognized options with "Did you mean" guesses (`levenshtein()` below 3, `sort()` order), extra keys ignored or kept, deep merge, `auto_enable`, new keys refused, required children, defaults | `Definition/ArrayNode.php` | ArrayNodeTest; recorded `unrecognized`, `pre_normalize` cases |
| PrototypedArrayNode details: key attributes (removed or kept, float keys through `var_export()`, duplicates), value-only elements and value prototypes, default children, minimum count, list and map merges | `Definition/PrototypedArrayNode.php` | PrototypedArrayNodeTest; recorded `keyed_normalize`, `prototyped_merge` cases |
| `Processor::process()`, `Processor::normalizeConfig()`, `Processor::processConfiguration()`; ConfigurationInterface | `Definition/Processor.php`, `Definition/ConfigurationInterface.php` | recorded `normalize_config` cases |
| The tree builders: TreeBuilder (root type, custom NodeBuilder, built once, path separator), NodeBuilder (type mapping, `setNodeClass()`, subclasses overriding `getNodeClass()`, builders copied to child definitions, return-type checks), every node definition (Variable, Scalar, String, Boolean, Integer / Float with `min()` / `max()`, Enum with values or an enum class, Array with prototypes) and their `getNode()`; info, examples, `docUrl()`, defaults, required, deprecation, equivalent values, `cannotBeEmpty()`, `cannotBeOverwritten()`; every InvalidDefinitionException of concrete and prototype nodes | `Definition/Builder/*.php` (19 files) | ArrayNodeDefinitionTest, BooleanNodeDefinitionTest, EnumNodeDefinitionTest, ExprBuilderTest, NodeBuilderTest, NodeDefinitionTest, NumericNodeDefinitionTest, TreeBuilderTest (92 of 92); 4,624 recorded cases |
| ArrayNodeDefinition details: `canBeEnabled()` / `canBeDisabled()` (and their merges), `acceptAndWrap()`, the normalized types it declares, default children (counts, names, PHP's coercion of the argument), key attributes, `fixXmlConfig()`, `find()` with any path separator; ExprBuilder's `if*()` / `then*()` rules, with `thenInvalid()`'s `sprintf()` and `json_encode()` | `Definition/Builder/ArrayNodeDefinition.php`, `Definition/Builder/ExprBuilder.php` | recorded `types`, `default`, `process`, `then_invalid`, `find` cases |
| Reference dumpers: YamlReferenceDumper (`dump()`, `dumpAtPath()` and its "Unable to find node" UnexpectedValueException, `dumpNode()`; defaults, examples and keys through symfony-yaml's `Inline::dump()`, behind the `yaml` feature) and XmlReferenceDumper (`dump()` / `dumpNode()` with or without a namespace; XML remappings, prototypes of every class and their placeholder values, attribute comments, `writeValue()`); a NodeInterface that does not extend BaseNode | `Definition/Dumper/YamlReferenceDumper.php`, `Definition/Dumper/XmlReferenceDumper.php` | YamlReferenceDumperTest, XmlReferenceDumperTest (7 of 7); recorded `yaml`, `yaml_at`, `xml`, `xml_ns` outputs |
| `ArrayShapeGenerator::generate()`: PHPDoc shapes of every node class, quoted keys (`addslashes()`), optional keys and deep merging, normalized types (`array_unique()`, `sort()`), unsealed shapes, inline comments with deprecations, info and `json_encode()` defaults, `*/` escaped | `Definition/ArrayShapeGenerator.php` | ArrayShapeGeneratorTest (18 of 18); recorded `shape` outputs |
| XmlUtils: `parse()` (`validateOnParse`, libxml's errors as `[LEVEL code] message (in file - line l, column c)`, document types refused, schema files with their imports resolved as libxml resolves them for a schema read from memory, validation callables: their result tested for truth, an `\Exception` as the previous exception, an `\Error` propagated), `loadFile()` (not a file, not readable, empty after `trim()`, InvalidXmlException rewrapped with the file name; `load_file_named()` for a `file://` URL, named as given), `convertDomElementToArray()` (with and without the prefix check), `phpize()`; XmlParsingException and InvalidXmlException; `libxml_use_internal_errors()` set, restored and passed to callables as upstream does | `Util/XmlUtils.php`, `Util/Exception/XmlParsingException.php`, `Util/Exception/InvalidXmlException.php` | XmlUtilsTest (7 of 7); recorded `phpize` (3,045 values), `convert` (71 documents), `parse` and `load` (120 outcomes) |
| Loader exceptions: LoaderLoadException (resource as `json_encode()` or "resource of type", the previous exception's message with one period removed, source resource, bundle `@` hints, type hint, code), FileLoaderImportCircularReferenceException, FileLocatorFileNotFoundException with `getPaths()` | `Exception/LoaderLoadException.php`, `Exception/FileLoaderImportCircularReferenceException.php`, `Exception/FileLocatorFileNotFoundException.php` | LoaderLoadExceptionTest (9 of 9); recorded `loader_load` (432 messages), `circular` |
| FileLocator: `locate()` (absolute names, `$currentPath` first, `array_unique()`, first or every match, messages and tried paths), `isAbsolutePath()` with `parse_url()`'s scheme rules | `FileLocator.php`, `FileLocatorInterface.php` | FileLocatorTest (5 of 5); recorded `scheme` (94 names), `locate` (432 calls) |
| Loaders: Loader (resolver, `resolve()`, `import()`), LoaderResolver, DelegatingLoader, FileLoader (`import()` with glob patterns, exclusions, `ignoreErrors`, `glob` type, `_excluded`; `glob()` and the resource it sets; `doImport()`'s locator delegation, loading set, circular-reference detection and exception wrapping), GlobFileLoader, ParamConfigurator; DirectoryAwareLoaderInterface | `Loader/*.php` (9 files) | LoaderTest, LoaderResolverTest, DelegatingLoaderTest, FileLoaderTest (24 of 24); recorded `import` (456 calls) |
| Filesystem resources: FileResource, FileExistenceResource, DirectoryResource (`isFresh()` over the tree with a file pattern; `__toString()` as `hash('xxh128', serialize(...))`), GlobResource (`glob()` with and without `GLOB_BRACE`, `expandGlob()`, `/**/` patterns, excluded prefixes, `forExclusion`, the Finder fallback, `__toString()`, hash, `__serialize()` / `__unserialize()`); ResourceInterface, SelfCheckingResourceInterface | `Resource/FileResource.php`, `Resource/FileExistenceResource.php`, `Resource/DirectoryResource.php`, `Resource/GlobResource.php`, `Resource/ResourceInterface.php`, `Resource/SelfCheckingResourceInterface.php` | FileResourceTest, FileExistenceResourceTest, DirectoryResourceTest, GlobResourceTest (40 of 41; testPhar `n/a`); recorded `glob` (1,680 cases), `xxh128` |
| Definition behaviour end to end | (the nodes and builders above) | NormalizationTest, MergeTest, FinalizationTest (12 of 12) |
| Caches: ResourceCheckerConfigCache (`getPath()`, `isFresh()` with the first supporting checker deciding, the meta file read with every registered class allowed, an unknown class, a syntax error or trailing data making it stale, a scalar meta value fresh, a non-resource element a TypeError; `write()` through symfony-filesystem's atomic `dumpFile()`, the `serialize()` meta file and its `.json` copy, modes `0666 & ~umask()`), ConfigCache (debug and production, skipped resource types), ConfigCacheFactory, ResourceCheckerConfigCacheFactory, SelfCheckingResourceChecker (answers shared by every instance), SkippingResourceChecker | `ConfigCache.php`, `ConfigCacheInterface.php`, `ResourceCheckerConfigCache.php`, `ResourceCheckerInterface.php`, `ConfigCacheFactory.php`, `ConfigCacheFactoryInterface.php`, `ResourceCheckerConfigCacheFactory.php`, `Resource/SelfCheckingResourceChecker.php`, `Resource/SkippingResourceChecker.php` | ConfigCacheTest, ResourceCheckerConfigCacheTest (18 of 18; ConfigCacheFactoryTest's one test `n/a`); recorded `write` (27 cases), `fresh` (1,170 `isFresh()` calls) |

Vectors: `scripts/record.sh config` runs
`oracle/recorders/config_definition.php`, `config_builder.php` and
`config_dumpers.php` on the pinned packages and writes
`tests/data/definition.json`, `tests/data/builder.json` and
`tests/data/dumpers.json`; `tests/recorded.rs`,
`tests/recorded_builder.rs` and `tests/recorded_dumpers.rs` replay them.
The dumper vectors are 1,020 outputs on 34 generated trees.
`scripts/record.sh config-xml-utils` runs `config_xml_utils.php` and
writes `tests/data/xml_utils.json`, which `tests/recorded_xml_utils.rs`
replays (stage log
[m1-c-xml-x3.md](../../docs/milestones/m1-c-xml-x3.md)).
`scripts/record.sh config-loaders` runs `config_loaders.php` and writes
`tests/data/loaders.json`, which `tests/recorded_loaders.rs` replays on a
copy of `tests/fixtures` (stage log
[m1-c-config-loaders.md](../../docs/milestones/m1-c-config-loaders.md)).
`scripts/record.sh config-cache` runs `config_cache.php` and writes
`tests/data/cache.json`, which `tests/recorded_cache.rs` replays (Linux
and other Unix only; stage log
[m1-c-config-cache.md](../../docs/milestones/m1-c-config-cache.md)).

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
- A PHP subclass of a node class that overrides nothing (`class BarNode
  extends ArrayNode {}`) is the node with `Node::with_class()`;
  `is_instance_of()` answers `instanceof` for every node.
- A definition is a `Definition<C, P>`: a handle to one shared, mutable
  definition (as a PHP object is) of class `C` (`class::Array`,
  `class::Scalar`, ...; `NodeDefinition` is any class), whose `end()`
  gives `P`, as upstream's `@template TParent` describes. Methods return
  a handle to the same definition, so a chain and a variable kept aside
  see the same changes. Where upstream declares a return type
  (`arrayNode(): ArrayNodeDefinition`, `variablePrototype()`, ...), a
  definition of another class (from `setNodeClass()`) fails with PHP's
  TypeError. Fallible PHP methods return `Result`.
- Subclasses: a definition class is a `NodeClass` (a built-in, a subclass
  that overrides nothing, or a `CustomDefinition` with its own
  `createNode()` and overrides of `setParent()` / `setPathSeparator()`);
  a NodeBuilder subclass is a `NodeBuilderClass` overriding
  `getNodeClass()`.
- PHP's optional closure arguments are two methods: `always()` /
  `always_then(f)`, `if_true()` / `if_true_with(f)`, `if_false()` /
  `if_false_with(f)`. An if closure returns any `Truthy` (a bool, a
  `Value`, an int), tested as PHP tests truth.
- ReflectionProperty reads of a definition are `property($name)`;
  `getChildNodeDefinitions()` gives the children.
- `docUrl()` reads Composer's `InstalledVersions::getVersion()` from
  `composer::set_installed_versions()`, which the application calls at
  start-up (process-wide, as Composer's data is). An unknown package is
  Composer's OutOfBoundsException.
- A [`CustomNode`] whose `extends_base_node()` is false stands for a
  class that implements NodeInterface without extending BaseNode:
  `is_base_node()` (`instanceof BaseNode`) is false, `getNode()` does not
  set its attributes, the dumpers skip BaseNode's methods on it, and
  calling one that NodeInterface lacks fails as PHP does
  (XmlReferenceDumper's `getParent()`, ArrayShapeGenerator's `BaseNode`
  parameters).
- The dumpers return the reference as bytes. `dump_node_at_path()` is
  `dumpAtPath()` on a tree already built, the steps after `buildTree()`.
  XmlReferenceDumper renames a prototype that is itself a
  PrototypedArrayNode, as upstream's `setName()` does.
- XmlUtils works on `php_dom::Document`. `$schemaOrCallable` is
  `Option<Validation>`: `Validation::Schema(path)` or
  `Validation::Callable(f)`, where `f(&dom, internal_errors, &mut
  errors)` returns a `Value` and pushes the libxml errors of what it
  validates (libxml's buffer in PHP). A PHP string naming a function is
  a callable there; here the caller picks the variant.
  `XmlUtils::convert_dom_element_to_array()` returns `Result`, for PHP's
  TypeError on a child that is not an element, text or comment.
- The PHP built-ins (`levenshtein()`, the loose comparison and `sort()`,
  `strtr()`, `json_encode()`, `sprintf()`, `var_export()` of enum cases,
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

- A definition made by a NodeBuilder that has no parent has a null parent
  in PHP, so its `end()` gives null; here `end()` gives the builder (the
  type is fixed when the method returns). Its `getNode()` is the same.
- A definition still parented by its builder fails `getNode()` with the
  TypeError of the node's constructor, without PHP's ", called in <file>
  on line <n>".
- A second `getNode()` of a definition with `beforeNormalization()` rules
  (after `TreeBuilder::setPathSeparator()`): PHP warns "Undefined
  property: Closure::$allowedTypes" and adds a null to the node's
  normalized types. The port does not warn and leaves the null out.
- `TreeBuilder<C>` checks the root's class when it is created; PHP fails
  later, at the first method the root does not have.
- Where the dumpers print an array as "Array" (`(string)`, `implode()`,
  concatenation), PHP also raises its "Array to string conversion"
  warning; the port does not. ArrayShapeGenerator's TypeError for a node
  that is not a BaseNode has no ", called in <file> on line <n>".
- `sprintf()` (in `thenInvalid()`) refuses `g`, `G`, `h`, `H` and `*`
  widths with an `\Error` naming them, and does not raise PHP's notice
  when a float precision above 53 is capped. A float `$children` with a
  fraction is truncated without PHP's `E_DEPRECATED`.

- XmlUtils reports a document or schema `php-dom` refuses (it would
  read a file or the network, or uses an encoding or schema construct
  php-dom does not implement) as an XmlParsingException carrying the
  refusal's text (PLAN.md decision 14). PHP parses it, or for an import
  it cannot load, warns and skips the import. Recorded: an import of a
  file outside the working directory, and a location with a raw space.
- XmlUtils's TypeError for a processing instruction or entity reference
  has no ", called in <file> on line <n>". Filesystem's IOException (an
  unreadable file, a race after `is_readable()`) gives PHP's text for
  "Permission denied" and "No such file or directory", Rust's for other
  I/O errors.
- Schema errors name the document as the working directory plus `/`, as
  PHP does on Linux (recorded); PHP's form on Windows is not recorded.

- Loaders are `Arc<dyn LoaderInterface>`; a loader and the resolver it
  is given keep each other alive, as in PHP, until the process ends.
  FileLoader's static `$loading` set is per thread. `class_exists()` in
  `FileLoader::import()` is what `FileLoader::set_class_exists()` answers
  (no class by default; PHP asks its autoloader). `glob()` collects the
  paths before `import()` loads them, where PHP's generator interleaves
  the two.
- GlobResource refuses `phar://` prefixes (no PHP archive reader).
  `GLOB_BRACE` is taken as defined, as on the oracle's glibc. The
  iterator's values are paths, not `SplFileInfo` objects.
- A directory GlobResource or DirectoryResource cannot open is SPL's
  UnexpectedValueException with "RecursiveDirectoryIterator::__construct(path):
  Failed to open directory: reason"; that text is not recorded.
- Resources restored by `from_serialized()` check property types with
  messages modelled on PHP's typed-property errors (not recorded).
- The loaders' TypeErrors for a non-string resource given to a locator
  name the interface method, not the implementing class.
- Cache metadata is a slice of `Arc<dyn ResourceInterface>` (a list, as
  upstream's tests pass it). The classes a meta file may name are a
  `ResourceClasses` registry (Config's four filesystem resources by
  default; `register()` adds one), standing for PHP's autoloader.
  ResourceCheckerInterface implementations name their class for the
  TypeError a non-resource element gives; its ", called in <file> on line
  <n>" suffix is not reproduced. SelfCheckingResourceChecker's shared
  answers are process-wide (`clear_cache()` empties them, as upstream's
  tests do through reflection). OPcache has no counterpart.
- Listed differences (tests/recorded_cache.rs): an object met twice in a
  resource's `__serialize()` array is written again where PHP writes
  `r:<n>;` (php-value's `serialize()` has no references; 1 recorded
  write); GlobResource's restore refuses any object as a Stringable,
  where PHP refuses only a `\Stringable` and then fails the typed
  property with a TypeError (8 recorded `isFresh()` calls).
- `FileLocator::isAbsolutePath()`'s `parse_url()` scheme is
  symfony-filesystem's (`php::parse_url_scheme`), shared with
  `Path::isAbsolute()`, which tells null from false.

## Not ported yet

- `ClassExistenceResource`, `ReflectionClassResource` and
  `ComposerResource` (23 tests): they ask PHP's autoloader and reflection
  at run time (PLAN.md decision 17).
- `Definition/Configuration.php`, `Definition/Loader` and
  `Definition/Configurator` (2 tests): DefinitionFileLoader includes PHP
  files.

25 upstream tests pending in `parity/symfony/config.tsv`.
