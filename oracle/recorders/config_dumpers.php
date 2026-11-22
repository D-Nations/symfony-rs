<?php
// Symfony Config's reference dumpers and ArrayShapeGenerator on generated
// trees (scripts/record.sh config). Each case is a tree spec and what
// YamlReferenceDumper::dumpNode(), dumpAtPath() (through a configuration
// whose builder returns the tree), XmlReferenceDumper::dumpNode() (with
// and without a namespace) and ArrayShapeGenerator::generate() give for
// it, each built afresh. crates/config/tests/recorded_dumpers.rs builds
// the same trees in Rust and replays the cases.
//
// A spec is {c: class, n: name, ...}: c is array, proto, scalar, bool,
// nbool (nullable), int, float, numeric, string, enum, variable or custom
// (a NodeInterface that does not extend BaseNode); ch children, p the
// prototype, key [attribute, remove], info, ex (example), def (default),
// nulldef, addif, req, dep [package, version, message], types (normalized
// types), extra [ignore, remove], deep, xml [[singular, plural]], values
// and fqcn (enum), min / max (numeric). Values are typed (lib/typed.php).

namespace Oracle\ConfigDumpers;

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Config\Definition\ArrayNode;
use Symfony\Component\Config\Definition\ArrayShapeGenerator;
use Symfony\Component\Config\Definition\BooleanNode;
use Symfony\Component\Config\Definition\Builder\TreeBuilder;
use Symfony\Component\Config\Definition\ConfigurationInterface;
use Symfony\Component\Config\Definition\Dumper\XmlReferenceDumper;
use Symfony\Component\Config\Definition\Dumper\YamlReferenceDumper;
use Symfony\Component\Config\Definition\EnumNode;
use Symfony\Component\Config\Definition\FloatNode;
use Symfony\Component\Config\Definition\IntegerNode;
use Symfony\Component\Config\Definition\NodeInterface;
use Symfony\Component\Config\Definition\NumericNode;
use Symfony\Component\Config\Definition\PrototypedArrayNode;
use Symfony\Component\Config\Definition\ScalarNode;
use Symfony\Component\Config\Definition\StringNode;
use Symfony\Component\Config\Definition\VariableNode;

enum UnitEnum { case Foo; case Bar; }
enum IntEnum: int { case One = 1; case Two = 2; }
enum StrEnum: string { case Foo = 'foo'; case Slash = 'a/b'; case Accent = 'é'; }

// "Array to string conversion" warnings are not part of the outcome.
set_error_handler(static fn () => true, \E_WARNING | \E_DEPRECATED | \E_USER_DEPRECATED);

class CustomNode implements NodeInterface
{
    public function getName(): string { return 'custom_node'; }
    public function getPath(): string { return 'custom'; }
    public function isRequired(): bool { return false; }
    public function hasDefaultValue(): bool { return true; }
    public function getDefaultValue(): mixed { return true; }
    public function normalize(mixed $value): mixed { return $value; }
    public function merge(mixed $leftSide, mixed $rightSide): mixed { return array_merge($leftSide, $rightSide); }
    public function finalize(mixed $value): mixed { return $value; }
}

class NodeTreeBuilder extends TreeBuilder
{
    public function __construct(private \Closure $build) {}

    public function buildTree(): NodeInterface
    {
        return ($this->build)();
    }
}

function decode(array $t): mixed
{
    return match ($t[0]) {
        'null' => null,
        'bool', 'int' => $t[1],
        'float' => unpack('E', hex2bin($t[1]))[1],
        'string' => base64_decode($t[1]),
        'array' => (static function () use ($t) {
            $a = [];
            foreach ($t[1] as [$k, $v]) {
                $a[decode($k)] = decode($v);
            }

            return $a;
        })(),
        'enum' => \constant($t[1].'::'.$t[2]),
    };
}

function build(array $s, ?NodeInterface $parent = null): NodeInterface
{
    $n = $s['n'];
    $num = static fn ($k) => isset($s[$k]) ? decode($s[$k]) : null;
    $node = match ($s['c']) {
        'array' => new ArrayNode($n, $parent),
        'proto' => new PrototypedArrayNode($n, $parent),
        'scalar' => new ScalarNode($n, $parent),
        'bool' => new BooleanNode($n, $parent),
        'nbool' => new BooleanNode($n, $parent, nullable: true),
        'int' => new IntegerNode($n, $parent, $num('min'), $num('max')),
        'float' => new FloatNode($n, $parent, $num('min'), $num('max')),
        'numeric' => new NumericNode($n, $parent, $num('min'), $num('max')),
        'string' => new StringNode($n, $parent),
        'enum' => new EnumNode($n, $parent, array_map(decode(...), $s['values'] ?? []), enumFqcn: $s['fqcn'] ?? null),
        'variable' => new VariableNode($n, $parent),
        'custom' => new CustomNode(),
    };
    if ($node instanceof CustomNode) {
        return $node;
    }
    if (isset($s['info'])) {
        $node->setInfo($s['info']);
    }
    if (\array_key_exists('ex', $s)) {
        $node->setExample(decode($s['ex']));
    }
    if (!empty($s['req'])) {
        $node->setRequired(true);
    }
    if (isset($s['dep'])) {
        $node->setDeprecated(...$s['dep']);
    }
    if (isset($s['types'])) {
        $node->setNormalizedTypes($s['types']);
    }
    if ($node instanceof ArrayNode) {
        if (isset($s['addif'])) {
            $node->setAddIfNotSet($s['addif']);
        }
        if (isset($s['extra'])) {
            $node->setIgnoreExtraKeys(...$s['extra']);
        }
        if (isset($s['deep'])) {
            $node->setPerformDeepMerging($s['deep']);
        }
        if (isset($s['xml'])) {
            $node->setXmlRemappings($s['xml']);
        }
    }
    if ($node instanceof PrototypedArrayNode) {
        if (isset($s['key'])) {
            $node->setKeyAttribute(...$s['key']);
        }
        if (isset($s['p'])) {
            $node->setPrototype(build($s['p'], $node));
        }
        if (!empty($s['nulldef'])) {
            $node->setNullAsDefault();
        }
    }
    if (\array_key_exists('def', $s)) {
        $node->setDefaultValue(decode($s['def']));
    }
    foreach ($s['ch'] ?? [] as $child) {
        $node->addChild(build($child, $node));
    }

    return $node;
}

// ---- Value and spec helpers ------------------------------------------------

function t(mixed $v): array
{
    return \typed_value($v);
}

function arr(string $name, array $children = [], array $extra = []): array
{
    return ['c' => 'array', 'n' => $name, 'ch' => $children] + $extra;
}

function proto(string $name, array $prototype, array $extra = []): array
{
    return ['c' => 'proto', 'n' => $name, 'p' => $prototype] + $extra;
}

function leaf(string $class, string $name, array $extra = []): array
{
    return ['c' => $class, 'n' => $name] + $extra;
}

// ---- The trees ----------------------------------------------------------

$trees = [];

// Scalar defaults, through Inline::dump() and writeValue().
$defaults = [
    null, true, false, 0, 1, -7, \PHP_INT_MAX, 0.0, -0.0, 1.0, 1.5, 0.1, 1e20, 1.5e-7, \INF, -\INF, \NAN,
    '', '0', 'plain', 'with space', 'a: b', '@ref', '%param%', "multi\nline", 'yes', 'null', '~', '123', '1.5',
    "tab\there", 'é', "quote'd", 'dq"d', '- dash', '#hash', '%%%%not_defined%%%%',
    [], ['a', 'b'], ['x' => 1, 'y' => true], ['a', ['b', 'c'], []], ['k' => ['n' => [], 'm' => ['z']]], [5 => 'five', 'six'],
    ['@logger' => null, '%kernel%' => 1.0, 3 => 'int key'], [true, null, 1.5, 'x'],
    UnitEnum::Foo, IntEnum::Two, StrEnum::Slash,
];
$children = [];
foreach ($defaults as $i => $d) {
    $children[] = leaf('variable', "v$i", ['def' => t($d)]);
    if (!\is_array($d)) {
        $children[] = leaf('scalar', "s$i", ['def' => t($d)]);
    }
}
$trees['defaults'] = arr('root', $children);
// An object in an array default: implode() throws in the XML dumper.
$trees['defaults_enum_list'] = arr('root', [leaf('variable', 'v', ['def' => t([UnitEnum::Bar])])]);
$trees['defaults_backed_list'] = arr('root', [leaf('variable', 'v', ['def' => t(['k' => IntEnum::One])])]);

// One scalar root per default: the XML attribute path differs at the root.
foreach ([1.0, [1, [2]], UnitEnum::Foo, StrEnum::Accent, "a\nb", ['x' => []]] as $i => $d) {
    $trees["root_default_$i"] = arr('root', [leaf('variable', 'value', ['def' => t($d)])]);
}

// Examples: strings, lists, maps, nested and falsy.
$examples = ['example', '0', '', 'a: b', ['foo'], ['foo', 'bar'], ['k' => 'v', 'l' => 'w'], [['nested']], ['a', 1, true, null], []];
$children = [];
foreach ($examples as $i => $e) {
    $children[] = leaf('scalar', "e$i", ['ex' => t($e)]);
    $children[] = leaf('variable', "ve$i", ['ex' => t($e), 'def' => t([])]);
    $children[] = arr("ae$i", [], ['ex' => t($e)]);
}
$trees['examples'] = arr('root', $children);
$trees['example_enum'] = arr('root', [leaf('scalar', 'e', ['ex' => t([UnitEnum::Foo])])]);

// Info, required, deprecations and enums, at depth 0 and below.
$infos = ['one line', "two\nlines", "three\nlines\nhere", '0', '', "trailing\n", "a\r\nb"];
foreach ($infos as $i => $info) {
    $trees["info_root_$i"] = leaf('scalar', 'top', ['info' => $info]);
    $trees["info_array_root_$i"] = arr('top', [leaf('scalar', 'c', ['info' => $info, 'req' => true])], ['info' => $info]);
}
$trees['comments'] = arr('root', [
    leaf('scalar', 'req', ['req' => true]),
    leaf('scalar', 'dep', ['dep' => ['vendor/pkg', '2.0', 'The "%node%" at "%path%" is gone.']]),
    leaf('scalar', 'dep_no_package', ['dep' => ['', '', 'Gone: %path%']]),
    leaf('scalar', 'dep_zero', ['dep' => ['0', '0', 'Zero %node%']]),
    leaf('scalar', 'all', ['req' => true, 'info' => "info\nmore", 'ex' => t('ex'), 'dep' => ['v/p', '1', 'msg']]),
    leaf('enum', 'en', ['values' => [t('a'), t(1), t(1.5), t(true), t(null), t(UnitEnum::Bar)]]),
    leaf('enum', 'en_default', ['values' => [t('a'), t('b')], 'def' => t('b')]),
    leaf('enum', 'en_unit', ['fqcn' => UnitEnum::class, 'def' => t(UnitEnum::Foo)]),
    leaf('enum', 'en_int', ['fqcn' => IntEnum::class]),
    leaf('enum', 'en_str', ['fqcn' => StrEnum::class, 'info' => 'backed']),
    leaf('enum', 'en_slash', ['values' => [t('*/5'), t('a"b'), t("é/")], 'req' => true]),
    arr('nested', [leaf('scalar', 'deep', ['info' => "x\ny", 'dep' => ['p', 'v', '%path%']])], ['info' => 'nested info', 'req' => true]),
]);

// Key widths around %-21s and %-20s.
$trees['widths'] = arr('root', [
    leaf('scalar', str_repeat('a', 19)), leaf('scalar', str_repeat('b', 20)), leaf('scalar', str_repeat('c', 21)),
    leaf('scalar', str_repeat('d', 19), ['def' => t('x')]), leaf('scalar', str_repeat('e', 20), ['def' => t(1)]),
    leaf('variable', 'map', ['def' => t([str_repeat('k', 18) => 1, str_repeat('l', 19) => 2, str_repeat('m', 25) => 3])]),
]);

// Prototypes of every class, keyed and not, with and without defaults.
$protoClasses = ['scalar', 'bool', 'nbool', 'int', 'float', 'numeric', 'string', 'variable'];
$children = [];
foreach ($protoClasses as $c) {
    $children[] = proto("p_$c", leaf($c, ''));
    $children[] = proto("pk_$c", leaf($c, '', ['info' => "$c info"]), ['key' => ['id', true]]);
}
$children[] = proto('p_enum', leaf('enum', '', ['values' => [t('x'), t('y')]]));
$children[] = proto('p_enum_fqcn', leaf('enum', '', ['fqcn' => StrEnum::class]));
$children[] = proto('p_enum_unit', leaf('enum', '', ['fqcn' => UnitEnum::class]));
foreach (['str' => 'dflt', 'false' => false, 'true' => true, 'null' => null, 'int' => 3, 'float' => 2.5, 'arr' => ['a', 'b'], 'empty' => []] as $k => $d) {
    $children[] = proto("pd_$k", leaf('variable', '', ['def' => t($d)]));
}
$trees['prototypes'] = arr('root', $children, ['xml' => [['p_scalar_one', 'p_scalar'], ['proto_item', 'p_bool']]]);
$trees['prototype_unit_default'] = arr('root', [proto('p', leaf('variable', '', ['def' => t(UnitEnum::Foo)]))]);

// Array prototypes, nested prototypes, key attributes "0" and "".
$entry = arr('', [leaf('scalar', 'title', ['req' => true]), leaf('scalar', 'path', ['def' => t('/')])]);
$trees['nested'] = arr('root', [
    proto('list', $entry),
    proto('keyed', $entry, ['key' => ['name', true]]),
    proto('keyed_kept', $entry, ['key' => ['name', false]]),
    proto('zero_key', $entry, ['key' => ['0', true]]),
    proto('zero_key_scalar', leaf('scalar', ''), ['key' => ['0', true]]),
    proto('empty_key', $entry, ['key' => ['', true]]),
    proto('two', proto('', $entry, ['key' => ['locale', true]]), ['key' => ['page', true]]),
    proto('two_unkeyed', proto('', $entry), ['key' => ['page', true]]),
    proto('two_inner_unkeyed', proto('', $entry, ['key' => ['locale', true]])),
    proto('three', proto('', proto('', leaf('scalar', '', ['info' => 'leaf']), ['key' => ['c', true]]), ['key' => ['b', true]]), ['key' => ['a', true]]),
    proto('list_of_lists', proto('', leaf('scalar', ''))),
    proto('null_default', leaf('scalar', ''), ['nulldef' => true]),
    proto('null_default_keyed', $entry, ['nulldef' => true, 'key' => ['k', true]]),
    proto('with_default', leaf('scalar', ''), ['def' => t(['x', 'y'])]),
    proto('with_map_default', leaf('variable', ''), ['def' => t(['a' => 1, 'b' => ['c' => null]])]),
    proto('info_proto', arr('', [leaf('scalar', 'x')], ['info' => 'element info'])),
    arr('empty_array'),
    arr('empty_addif', [], ['addif' => true]),
    arr('children_addif', [leaf('scalar', 'c', ['def' => t(1)])], ['addif' => true]),
], ['xml' => [['entry', 'list'], ['key', 'keyed'], ['zero', 'zero_key']]]);

// Shapes: quoting, normalized types, sealed or not, deep merging.
$trees['shape'] = arr('root', [
    leaf('string', 'int'), leaf('string', 'INT'), leaf('string', 'Float'), leaf('string', 'bool'),
    leaf('string', 'null'), leaf('string', 'scalar'), leaf('string', 'mixed'), leaf('string', '@at'),
    leaf('string', "q'uote"), leaf('string', 'd"q'), leaf('string', 'back\\slash\'s'), leaf('string', "nul\0byte'"),
    leaf('string', 'a*/b'), leaf('string', 'a b'),
    leaf('numeric', 'num'), leaf('int', 'int_range', ['min' => t(1), 'max' => t(5)]), leaf('float', 'f', ['def' => t(1.0)]),
    leaf('nbool', 'nb'), leaf('nbool', 'nb_null', ['def' => t(null)]), leaf('bool', 'b_null', ['def' => t(null)]),
    leaf('scalar', 'ws', ['info' => "a\tb\x0bc\x0cd\re\n f\xc2\xa0g\xc2\x85h  ", 'def' => t('é/ü "x"')]),
    leaf('scalar', 'dflt_float', ['def' => t(0.1)]), leaf('scalar', 'dflt_big', ['def' => t(1e100)]),
    leaf('variable', 'dflt_unit', ['def' => t(UnitEnum::Foo)]), leaf('variable', 'dflt_backed', ['def' => t(StrEnum::Slash)]),
    leaf('variable', 'dflt_list', ['def' => t([1, 'a/b', ['x' => 2.0]])]), leaf('variable', 'dflt_inf', ['def' => t(\INF)]),
    leaf('scalar', 'dep', ['dep' => ['p', '1', "The \"%node%\"\nat %path%"], 'info' => 'i']),
    arr('typed', [leaf('scalar', 'x')], ['types' => ['string', 'array', 'int', 'backed-enum', 'string', 'bool', 'null']]),
    arr('typed_any', [leaf('scalar', 'x')], ['types' => ['any', 'array']]),
    arr('typed_enum_only', [leaf('scalar', 'x')], ['types' => ['backed-enum']]),
    arr('typed_numeric', [leaf('scalar', 'x')], ['types' => ['10', '9', 'a', 'B']]),
    arr('extra', [leaf('scalar', 'x')], ['extra' => [true, false]]),
    arr('no_deep', [leaf('scalar', 'r', ['req' => true]), leaf('scalar', 'o')], ['deep' => false]),
    arr('req_deep', [leaf('scalar', 'r', ['req' => true])]),
    proto('proto_null', leaf('string', ''), ['nulldef' => true, 'types' => ['string']]),
    proto('proto_typed', $entry, ['types' => ['bool', 'array', 'string'], 'key' => ['k', true]]),
    proto('proto_entries', $entry),
    proto('proto_custom_default', arr('', [leaf('scalar', 'a')]), ['def' => t([['a' => 'x/y']])]),
    arr('empty_null', [], ['addif' => false]),
]);
$trees['shape_custom_child'] = arr('root', [leaf('custom', 'ignored')]);
$trees['custom_root'] = leaf('custom', 'ignored');
$trees['scalar_root'] = leaf('enum', 'top', ['values' => [t('a'), t(null)]]);

// ---- Recording ----------------------------------------------------------

$paths = ['', 'x', 'nested', 'nested.deep', 'nested.missing', 'req.sub', 'keyed', 'keyed.name', 'keyed.name.title',
    'two.page', 'two.page.locale', 'two.page.locale.path', 'two_unkeyed.page', 'list', 'list.title', 'zero_key.0',
    'zero_key_scalar', 'three.a.b.c', 'empty_key', 'value', 'c', 'p_scalar.x', 'pk_scalar.id', 'v1', '.v1'];

$cases = [];
foreach ($trees as $name => $spec) {
    $tree = static fn () => build($spec);
    $configuration = new class(new NodeTreeBuilder($tree)) implements ConfigurationInterface {
        public function __construct(private TreeBuilder $builder) {}

        public function getConfigTreeBuilder(): TreeBuilder
        {
            return $this->builder;
        }
    };
    $atPaths = [];
    foreach ($paths as $path) {
        $atPaths[] = [$path, \typed_call(static fn () => (new YamlReferenceDumper())->dumpAtPath($configuration, $path))];
    }
    $cases[] = [
        'name' => $name,
        'tree' => $spec,
        'yaml' => \typed_call(static fn () => (new YamlReferenceDumper())->dumpNode($tree())),
        'yaml_at' => $atPaths,
        'xml' => \typed_call(static fn () => (new XmlReferenceDumper())->dumpNode($tree())),
        'xml_ns' => \typed_call(static fn () => (new XmlReferenceDumper())->dumpNode($tree(), 'urn:ns')),
        'xml_ns_zero' => \typed_call(static fn () => (new XmlReferenceDumper())->dumpNode($tree(), '0')),
        'shape' => \typed_call(static fn () => ArrayShapeGenerator::generate($tree())),
    ];
}

echo json_encode($cases, \JSON_PRETTY_PRINT | \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR), "\n";
