<?php
// Symfony Config's Definition\Builder on generated inputs
// (scripts/record.sh config). Each case is [op, args, outcome], as in
// config_definition.php. Trees are built from a list of steps applied to
// `new ArrayNodeDefinition('root')`; crates/config/tests/recorded_builder.rs
// applies the same steps in Rust and replays the cases.

namespace Oracle\Config;

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Config\Definition\Builder\ArrayNodeDefinition;
use Symfony\Component\Config\Definition\Builder\FloatNodeDefinition;
use Symfony\Component\Config\Definition\Builder\IntegerNodeDefinition;
use Symfony\Component\Config\Definition\Builder\NodeBuilder;
use Symfony\Component\Config\Definition\Builder\ScalarNodeDefinition;
use Symfony\Component\Config\Definition\Processor;

enum UnitEnum { case Foo; case Bar; }
enum IntEnum: int { case One = 1; case Two = 2; case Neg = -3; }
enum StrEnum: string { case Foo = 'foo'; case One = '1'; case Empty = ''; case Slash = 'a/b'; case Accent = 'é'; }

// Deprecations (a float's implicit conversion to int) are not part of
// the outcome; the port does not reproduce them.
set_error_handler(static fn () => true, \E_DEPRECATED | \E_USER_DEPRECATED);

$cases = [];
function record(string $op, array $args, callable $f): void
{
    global $cases;
    $outcome = \typed_call($f);
    // A message that is not UTF-8 (sprintf()'s %c) goes as base64.
    if ('throws' === $outcome[0] && !mb_check_encoding($outcome[2], 'UTF-8')) {
        $outcome = ['throws_base64', $outcome[1], base64_encode($outcome[2])];
    }
    $cases[] = [$op, array_map('\typed_value', $args), $outcome];
}

function apply(array $steps): ArrayNodeDefinition
{
    $d = new ArrayNodeDefinition('root');
    foreach ($steps as $s) {
        match ($s[0]) {
            'can_be_enabled' => $d->canBeEnabled(),
            'can_be_disabled' => $d->canBeDisabled(),
            'if' => (function () use ($d, $s) {
                $e = $d->beforeNormalization();
                match ($s[1]) {
                    'string' => $e->ifString(),
                    'null' => $e->ifNull(),
                    'array' => $e->ifArray(),
                    'true' => $e->ifTrue(),
                    'false' => $e->ifFalse(),
                    'always' => $e->always(),
                    'empty' => $e->ifEmpty(),
                };
                $e->then(static fn ($v) => ['wrapped' => $v])->end();
            })(),
            'accept' => $d->acceptAndWrap($s[1], $s[2]),
            'true_like' => $d->treatTrueLike($s[1]),
            'false_like' => $d->treatFalseLike($s[1]),
            'null_like' => $d->treatNullLike($s[1]),
            'scalar_child' => $d->children()->scalarNode($s[1])->defaultValue($s[2]),
            'list_child' => $d->children()->arrayNode($s[1])->scalarPrototype(),
            'prototype' => $d->prototype($s[1]),
            'proto_array' => $d->arrayPrototype()->children()->scalarNode('id')->end()->scalarNode('v')->end(),
            'key' => $d->useAttributeAsKey($s[1], $s[2]),
            'default_children' => $d->addDefaultChildrenIfNoneSet($s[1]),
            'default' => $d->defaultValue($s[1]),
            'required' => $d->isRequired(),
            'cannot_be_empty' => $d->cannotBeEmpty(),
            'at_least_one' => $d->requiresAtLeastOneElement(),
            'defaults_if_not_set' => $d->addDefaultsIfNotSet(),
            'xml' => $d->fixXmlConfig($s[1], $s[2]),
            'no_deep_merging' => $d->performNoDeepMerging(),
            'ignore_extra' => $d->ignoreExtraKeys($s[1]),
            'unset' => $d->canBeUnset($s[1]),
            'deny_overwrite' => $d->cannotBeOverwritten(),
            'no_new_keys' => $d->disallowNewKeysInSubsequentConfigs(),
            'normalize_keys' => $d->normalizeKeys($s[1]),
            'deprecated' => $d->setDeprecated('vendor/pkg', '1.2', $s[1]),
        };
    }

    return $d;
}

$v = ['prototype', 'variable'];
$trees = [
    [],
    [['can_be_enabled'], ['scalar_child', 'foo', 'bar']],
    [['can_be_disabled'], ['scalar_child', 'foo', 'bar']],
    [['can_be_enabled'], ['scalar_child', 'foo', 'bar'], ['unset', true]],
    [$v],
    [['prototype', 'scalar'], ['at_least_one']],
    [['prototype', 'scalar'], ['cannot_be_empty']],
    [['prototype', 'scalar'], ['key', 'id', true]],
    [['proto_array'], ['key', 'id', true]],
    [['proto_array'], ['key', 'id', false]],
    [['proto_array'], ['default_children', null]],
    [['proto_array'], ['default_children', 3]],
    [['proto_array'], ['default_children', 0]],
    [['proto_array'], ['default_children', -1]],
    [['proto_array'], ['default_children', true]],
    [['proto_array'], ['default_children', 1.0]],
    [['proto_array'], ['default_children', 2.5]],
    [['proto_array'], ['default_children', -2.5]],
    [['proto_array'], ['default_children', 1e20]],
    [['proto_array'], ['default_children', \NAN]],
    [['proto_array'], ['default_children', 2.5], ['key', 'id', true]],
    [['proto_array'], ['default_children', 'x']],
    [['proto_array'], ['default_children', 'x'], ['key', 'id', true]],
    [['proto_array'], ['default_children', ['a', 'b']], ['key', 'id', true]],
    [['proto_array'], ['default_children', null], ['key', 'id', true]],
    [['proto_array'], ['default_children', 2], ['key', 'id', true]],
    [['proto_array'], ['default_children', 0], ['key', 'id', true]],
    [['prototype', 'scalar'], ['default', ['a', 'b']]],
    [['prototype', 'scalar'], ['default', null]],
    [['prototype', 'scalar'], ['default', 'x']],
    [['prototype', 'scalar'], ['default', []], ['default_children', null]],
    [['prototype', 'scalar'], ['defaults_if_not_set']],
    [['prototype', 'boolean'], ['key', 'k', true]],
    [['prototype', 'integer']],
    [['scalar_child', 'a', 'x'], ['key', 'k', true]],
    [['scalar_child', 'a', 'x'], ['default', []]],
    [['scalar_child', 'a', 'x'], ['defaults_if_not_set']],
    [['scalar_child', 'a', 'x'], ['required']],
    [['scalar_child', 'a', 'x'], ['cannot_be_empty']],
    [['scalar_child', 'a', 'x'], ['at_least_one']],
    [['scalar_child', 'a', 'x'], ['default_children', 2]],
    [['xml', 'item', null], ['list_child', 'items']],
    [['xml', 'child', 'children'], ['list_child', 'children']],
    [['scalar_child', 'foo_bar', 'x'], ['normalize_keys', false]],
    [['scalar_child', 'foo_bar', 'x']],
    [['scalar_child', 'a', 'x'], ['ignore_extra', true]],
    [['scalar_child', 'a', 'x'], ['ignore_extra', false]],
    [$v, ['no_deep_merging']],
    [['list_child', 'l'], ['scalar_child', 'a', 'x']],
    [['scalar_child', 'a', 'x'], ['deny_overwrite']],
    [['prototype', 'scalar'], ['key', 'k', true], ['no_new_keys']],
    [['accept', ['string'], null], $v],
    [['accept', ['string', 'int'], 'name'], ['scalar_child', 'name', 'd']],
    [['accept', ['bool', 'null'], null], $v],
    [['accept', ['backed-enum'], 'v'], $v],
    [['accept', ['int'], '5'], $v],
    [['accept', ['string', 'array'], null], $v],
    [['accept', ['any'], null]],
    [['accept', [], null], $v],
    [['if', 'string'], $v],
    [['if', 'null'], ['if', 'array'], $v],
    [['if', 'true'], $v],
    [['if', 'false'], $v],
    [['if', 'always'], $v],
    [['if', 'empty'], $v],
    [['true_like', ['x' => 1]], $v],
    [['false_like', ['x' => 1]], $v],
    [['true_like', []], $v],
    [['null_like', null], $v],
    [['accept', ['int'], null], ['if', 'string'], $v],
    [['can_be_enabled'], ['if', 'string'], ['scalar_child', 'foo', 'bar']],
    [['scalar_child', 'a', 'x'], ['deprecated', 'Gone: %node% at %path%.']],
];
$configs = [
    [], [null], [true], [false], [[]], ['str'], [5], [['a' => 'y']], [['foo' => 'baz']],
    [['enabled' => false]], [['enabled' => true], ['foo' => 'z']], [false, ['foo' => 'z']],
    [[['id' => 'k1', 'v' => 1], ['id' => 'k2', 'v' => 2]]], [[['id' => 'k1'], ['id' => 'k1', 'v' => 3]]],
    [['x', 'y'], ['z']], [['item' => 'one']], [['item' => ['one', 'two']]], [['children' => ['c']]], [['child' => 'c']],
    [['foo-bar' => 1]], [['foo_bar' => 1]], [['a' => '1', 'extra' => 2]], [['l' => ['a', 'b']], ['l' => ['c']]],
    [['a' => 'x'], ['a' => 'y']], [['k1' => 'a'], ['k2' => 'b']], [['name' => 'n']], [IntEnum::One], [StrEnum::Foo],
    [UnitEnum::Foo], [[1, 2, 3]], [['k' => true]], [[]], [['wrapped' => 'w']],
];
foreach ($trees as $steps) {
    record('types', [$steps], static fn () => apply($steps)->getNode()->getNormalizedTypes());
    record('default', [$steps], static fn () => apply($steps)->getNode()->getDefaultValue());
    foreach ($configs as $config) {
        record('process', [$steps, $config], static fn () => (new Processor())->process(apply($steps)->getNode(), $config));
    }
}

// thenInvalid(): sprintf() of the message with json_encode() of the value.
$messages = [
    'Invalid value %s', '%s', '%5s|', '%-5s|', '%05s', "%'*8s", '%-\'#8s|', '%.3s', '%.0s|', '%d', '%5d', '%05d', '%+d',
    '%-05d|', "%'x5d", '%+05d', '%u', '%x', '%X', '%o', '%b', '%c', '%1$s %1$s', '%2$s', '%s %s', '%', 'abc %',
    '%y', '%%s %s', '%0$s', 'no placeholder', '% 5s|', '%10.2s|', '%e', '%f', '%5$s %y', '%y %5$s', "%'",
    '%-05s|', '%-05u|', '%-05x|', "%-'x5d|", '%-+05d|', '%E', '%F', '%+f', '%.2f', '%10.3f|', '%-10.2e|', '%05.1f',
    '%.0f', '%.0e', '%.60f', '%+.1e', '%-05.1f|', '%+05d', '%5.1s|',
];
$values = [
    'value', 'é', 'a/b', '"q"', "tab\t", 123, -5, 0, 1.5, 1.0, 0.1, -0.0, 1e20, 3.0e-5, true, false, null, [], [1, 2],
    ['a' => 1, 'b' => [true, null]], [1 => 'x'], new \stdClass(), (object) ['a' => 1, 'b' => 'c'], UnitEnum::Foo, IntEnum::Neg,
    StrEnum::Slash, "\xff", \INF, \NAN, '12abc', ' 42', '9999999999999999999999', '-7.9',
];
foreach ($messages as $message) {
    foreach ($values as $value) {
        record('then_invalid', [$message, $value], static function () use ($message, $value) {
            $d = new ArrayNodeDefinition('root');
            $d->beforeNormalization()->always()->thenInvalid($message)->end();

            return $d->getNode()->normalize($value);
        });
    }
}

// find() with path separators.
foreach (['.', '/', '.|', '::'] as $separator) {
    foreach (['a', 'a.b', 'a/b', 'a.|b', 'a::b', 'x.y', 'x.y.z', 'x.y/z', 'x.y.|z', 'x.y::z', '0', '1', 'a.c', 'a/c', '', 'b', 'a.b.c', 'a/b/c', 'q::r'] as $path) {
        record('find', [$separator, $path], static function () use ($separator, $path) {
            $root = new ArrayNodeDefinition('root');
            $root->setPathSeparator($separator)
                ->children()
                    ->arrayNode('a')->children()->booleanNode('b')->end()->end()->end()
                    ->arrayNode('x.y')->children()->scalarNode('z')->end()->end()->end()
                    ->scalarNode('0')->end()
                    ->arrayNode('q')->end()
                ->end();
            $found = $root->find($path);

            return [$found::class, (new \ReflectionProperty($found, 'name'))->getValue($found)];
        });
    }
}

// min() and max().
foreach (['integer', 'float'] as $kind) {
    foreach ([[3, 5], [5, 3], [3, 3], [1.5, 1], [1, 1.5], [0.1, 0.3], [1e20, 5], [-0.0, 0.0], [0.3, 0.1], [2, 2.0]] as [$a, $b]) {
        foreach (['min_max', 'max_min'] as $order) {
            record('bounds', [$kind, $order, $a, $b], static function () use ($kind, $order, $a, $b) {
                $d = 'integer' === $kind ? new IntegerNodeDefinition('n') : new FloatNodeDefinition('n');
                'min_max' === $order ? $d->min($a)->max($b) : $d->max($a)->min($b);

                return $d->getNode()->finalize('integer' === $kind ? 4 : 4.0);
            });
        }
    }
}

// docUrl(), with Composer's versions.
foreach (['symfony/config', 'symfony/yaml', 'phpunit/phpunit'] as $package) {
    record('version', [$package], static fn () => \Composer\InstalledVersions::getVersion($package));
}
foreach (['https://x/{package}/{version:major}.{version:minor}', '{version:major}{version:major}', 'none', '{package}{PACKAGE}'] as $uri) {
    foreach (['symfony/config', 'symfony/yaml', '', '0', null, 'nope/nope'] as $package) {
        record('doc_url', [$uri, $package], static function () use ($uri, $package) {
            $d = new ArrayNodeDefinition('r');
            $d->docUrl($uri, $package);

            return (new \ReflectionProperty($d, 'attributes'))->getValue($d);
        });
    }
}

// A definition still parented by its builder, and return types.
foreach (['array', 'variable', 'scalar', 'string', 'boolean', 'integer', 'float', 'enum'] as $type) {
    record('parent_builder', [$type], static function () use ($type) {
        $d = (new ArrayNodeDefinition('r'))->children()->node('x', $type);
        if ('enum' === $type) {
            $d->values(['a']);
        }

        return $d->getNode()::class;
    });
}
foreach (['arrayNode', 'scalarNode', 'booleanNode', 'integerNode', 'floatNode', 'enumNode', 'variableNode', 'stringNode'] as $method) {
    foreach (['array', 'scalar', 'boolean', 'integer', 'float', 'enum', 'variable', 'string'] as $type) {
        record('return_type', [$method, $type], static fn () => (new NodeBuilder())->setNodeClass($type, 'array' === $type ? ScalarNodeDefinition::class : ArrayNodeDefinition::class)->{$method}('a')::class);
    }
}
foreach (['variablePrototype', 'scalarPrototype', 'stringPrototype', 'booleanPrototype', 'integerPrototype', 'floatPrototype', 'arrayPrototype', 'enumPrototype'] as $method) {
    record('prototype_return_type', [$method], static function () use ($method) {
        $d = new ArrayNodeDefinition('r');
        foreach (['array', 'scalar', 'boolean', 'integer', 'float', 'enum', 'variable', 'string'] as $type) {
            $d->children()->setNodeClass($type, 'array' === $type ? ScalarNodeDefinition::class : ArrayNodeDefinition::class);
        }

        return $d->{$method}()::class;
    });
}

echo json_encode($cases, \JSON_THROW_ON_ERROR), "\n";
