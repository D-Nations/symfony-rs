<?php
// Symfony Config's Definition nodes on generated inputs, and the PHP
// built-ins the port writes from documentation (scripts/record.sh config).
// Each case is [op, args, outcome]: args and outcome are typed values
// (lib/typed.php), the outcome being the result or ["throws", class,
// message]. crates/config/tests/recorded.rs replays them.

namespace Oracle\Config;

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Config\Definition\ArrayNode;
use Symfony\Component\Config\Definition\BooleanNode;
use Symfony\Component\Config\Definition\EnumNode;
use Symfony\Component\Config\Definition\FloatNode;
use Symfony\Component\Config\Definition\IntegerNode;
use Symfony\Component\Config\Definition\NumericNode;
use Symfony\Component\Config\Definition\Processor;
use Symfony\Component\Config\Definition\PrototypedArrayNode;
use Symfony\Component\Config\Definition\ScalarNode;
use Symfony\Component\Config\Definition\StringNode;
use Symfony\Component\Config\Definition\VariableNode;

enum UnitEnum { case Foo; case Bar; }
enum IntEnum: int { case One = 1; case Two = 2; case Neg = -3; }
enum StrEnum: string { case Foo = 'foo'; case One = '1'; case Empty = ''; case Slash = 'a/b'; case Accent = 'é'; }

// Deprecations (from BackedEnum::from() on a float-string) are not part of
// the outcome; the port does not reproduce them.
set_error_handler(static fn () => true, \E_DEPRECATED | \E_USER_DEPRECATED);

$cases = [];
function record(string $op, array $args, callable $f): void
{
    global $cases;
    $cases[] = [$op, array_map('\typed_value', $args), \typed_call($f)];
}

$scalars = [
    null, true, false, 0, 1, -1, 5, \PHP_INT_MAX, 0.0, -0.0, 0.5, 1.5, -2.5, 1e20, 1.0, 0.1, 1e-7,
    '', '0', '1', '-1', '1.5', ' 1', '1 ', '1abc', 'abc', 'foo', '1e3', '01', '0x1A', ' ',
];
$values = array_merge($scalars, [[], ['a' => 1], [1, 2], new \stdClass(), UnitEnum::Foo, IntEnum::One, StrEnum::Foo]);

function scalar_node(string $kind): \Symfony\Component\Config\Definition\BaseNode
{
    return match ($kind) {
        'variable' => new VariableNode('test'),
        'scalar' => new ScalarNode('test'),
        'boolean' => new BooleanNode('test'),
        'boolean_nullable' => new BooleanNode('test', null, '.', true),
        'integer' => new IntegerNode('test'),
        'float' => new FloatNode('test'),
        'string' => new StringNode('test'),
        'numeric' => new NumericNode('test'),
    };
}

// Type checks, and the empty-value check with its json_encode().
foreach (['variable', 'scalar', 'boolean', 'boolean_nullable', 'integer', 'float', 'string', 'numeric'] as $kind) {
    foreach ($values as $v) {
        record('normalize', [$kind, $v], static fn () => scalar_node($kind)->normalize($v));
        record('finalize_not_empty', [$kind, $v], static function () use ($kind, $v) {
            $node = scalar_node($kind);
            $node->setAllowEmptyValue(false);
            $node->setInfo('the info');

            return $node->finalize($v);
        });
    }
}

// Minimum and maximum, with PHP's loose comparison and %s of the value.
foreach (['numeric', 'integer', 'float'] as $kind) {
    foreach ([[1, null], [null, 1], [0.5, 1.5], [-1, 5], [1e20, null]] as [$min, $max]) {
        foreach ($scalars as $v) {
            record('numeric', [$kind, $min, $max, $v], static function () use ($kind, $min, $max, $v) {
                $node = match ($kind) {
                    'numeric' => new NumericNode('n', null, $min, $max),
                    'integer' => new IntegerNode('n', null, $min, $max),
                    'float' => new FloatNode('n', null, $min, $max),
                };

                return $node->finalize($v);
            });
        }
    }
}

// Enums: lists of values, and enum classes with their casts.
$enumSpecs = [
    ['values', ['foo', 'bar', 1, 1.5, 1.0, true, null, '', 'a/b', 'é', UnitEnum::Bar]],
    ['values', [0, '0', false, -0.0]],
    ['fqcn', UnitEnum::class],
    ['fqcn', IntEnum::class],
    ['fqcn', StrEnum::class],
];
$enumInputs = array_merge($scalars, [
    UnitEnum::Foo, IntEnum::One, IntEnum::Neg, StrEnum::Foo, StrEnum::Empty,
    '2', ' 2 ', '-3', '+1', '2.0e0', '2.9', '9223372036854775808', '-9223372036854775809', "1\n", "\x0b1", 'a/b', 'é',
]);
foreach ($enumSpecs as [$by, $spec]) {
    $make = static fn () => 'values' === $by ? new EnumNode('e', null, $spec) : new EnumNode('e', null, enumFqcn: $spec);
    record('enum_permissible', [$by, $spec, true], static fn () => $make()->getPermissibleValues(', ', true));
    record('enum_permissible', [$by, $spec, false], static fn () => $make()->getPermissibleValues(' | ', false));
    foreach ($enumInputs as $v) {
        record('enum_finalize', [$by, $spec, $v], static fn () => $make()->finalize($v));
    }
}

// Unrecognized options: sort() of the children's names, levenshtein()
// guesses and the messages.
$childSets = [
    ['alpha1', 'alpha2', 'beta'],
    ['b', 10, '9', 'a', 2, '10a'],
    ['foo_bar', 'foo-baz', 'x'],
    ['only'],
    [],
    ['abc', 'abd', 'abe', 'xbc'],
];
$extraSets = [['alpha3'], ['beta2', 'alpha'], ['zz'], ['fo'], ['9a', 'b1'], [3, 'xx'], ['ab', 'abx'], ['onl', 'only2']];
foreach ($childSets as $children) {
    foreach ($extraSets as $extra) {
        record('unrecognized', [$children, $extra], static function () use ($children, $extra) {
            $node = new ArrayNode('root');
            foreach ($children as $name) {
                $node->addChild(new ScalarNode($name));
            }

            return $node->normalize(array_fill_keys($extra, 'v'));
        });
    }
}

// Dashes in keys.
$preNormalize = new \ReflectionMethod(ArrayNode::class, 'preNormalize');
foreach ([
    ['foo-bar' => 1], [-1 => 1], ['a-b-c' => 1, 'a_b_c' => 2], ['a-b_c' => 1], ['--' => 1],
    ['a-b' => 1, 'a_b' => null], ['x' => 1, 'a-b' => 2, 'y' => 3], ['-5' => 1, '_5' => 2], ['a-' => 1],
] as $input) {
    record('pre_normalize', [$input], static fn () => $preNormalize->invoke(new ArrayNode('r'), $input));
}

// XML singulars to lists.
foreach ([
    [['a' => 'x'], 'a', null], [['as' => ['x', 'y']], 'a', null], [['a' => ['k' => 'v']], 'a', null],
    [['a' => [3 => 'x']], 'a', null], [['a' => []], 'a', null], [['a' => null], 'a', null],
    [['as' => null, 'a' => 'x'], 'a', null], [['children' => ['c']], 'child', 'children'],
    [['child' => 'c'], 'child', 'children'], [[], 'a', null], [['as' => 'x'], 'a', null],
    [['as' => true], 'a', null], [['a' => 5], 'a', null], [['a' => false], 'a', null],
] as [$config, $key, $plural]) {
    record('normalize_config', [$config, $key, $plural], static fn () => Processor::normalizeConfig($config, $key, $plural));
}

// Prototyped arrays keyed by attribute.
function keyed(bool $remove): PrototypedArrayNode
{
    $node = new PrototypedArrayNode('root');
    $node->setKeyAttribute('id', $remove);
    $prototype = new ArrayNode(null, $node);
    foreach (['id', 'foo', 'value'] as $name) {
        $prototype->addChild(new ScalarNode($name));
    }
    $node->setPrototype($prototype);

    return $node;
}
foreach ([
    [['id' => 1.5, 'foo' => 'x']], [['id' => 2.0, 'foo' => 'x']], [['id' => 'a', 'foo' => 1], ['id' => 'a', 'foo' => 2]],
    [['id' => true, 'foo' => 1]], [['id' => 7, 'foo' => 1], ['id' => '7', 'foo' => 2]], [['foo' => 1]],
    ['k' => ['foo' => 1]], [['id' => 'a', 'value' => 'v1'], ['id' => 'b', 'value' => 'v2']],
    [['id' => null, 'foo' => 1]], [5 => ['foo' => 1]], [['id' => 'x', 'value' => 'v', 'foo' => 'f']],
    ['scalar'], [['id' => 'a']],
] as $input) {
    foreach ([true, false] as $remove) {
        record('keyed_normalize', [$remove, $input], static fn () => keyed($remove)->normalize($input));
    }
}

// Merging prototyped arrays: lists append, maps merge per key.
foreach ([
    [['a', 'b'], ['c']], [['x' => 1], ['y' => 2, 'x' => 3]], [[], ['a']], [['a'], ['k' => 'v']],
    [[1 => 'a'], [1 => 'b']], [[0 => 'a', 2 => 'b'], [0 => 'c', 1 => 'd']],
] as [$left, $right]) {
    foreach ([true, false] as $allowNewKeys) {
        record('prototyped_merge', [$allowNewKeys, $left, $right], static function () use ($allowNewKeys, $left, $right) {
            $node = new PrototypedArrayNode('p');
            $node->setPrototype(new VariableNode('v'));
            $node->setAllowNewKeys($allowNewKeys);

            return $node->merge($left, $right);
        });
    }
}

// The built-ins themselves.
foreach ([['', ''], ['', 'abc'], ['kitten', 'sitting'], ['alpha3', 'alpha1'], ['é', 'e'], ['abc', 'abc'], ['flaw', 'lawn']] as [$a, $b]) {
    record('levenshtein', [$a, $b], static fn () => levenshtein($a, $b));
}
foreach ($scalars as $a) {
    foreach ([null, true, false, 0, 1, 5, 0.5, 1.5, 1e20, '', '1', '1.5', 'abc', '5', ' 1', '1e3'] as $b) {
        record('compare', [$a, $b], static fn () => $a <=> $b);
    }
}
foreach ([['b', 10, '9', 'a', 2, '10a'], ['1e3', 1000, '999', 'x'], [3, '3', 'a', 'A', '_', -1]] as $keys) {
    record('sort', [$keys], static function () use ($keys) {
        sort($keys);

        return $keys;
    });
}

echo json_encode($cases, \JSON_THROW_ON_ERROR), "\n";
