<?php
// OptionsResolver behaviour its upstream tests leave open, as typed JSON
// (scripts/record.sh options-resolver): each case's resolved array or
// thrown exception. crates/options-resolver/tests/edges.rs replays the
// same cases by name.
//
//   php options_resolver_edges.php

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\OptionsResolver\Exception\NoSuchOptionException;
use Symfony\Component\OptionsResolver\Options;
use Symfony\Component\OptionsResolver\OptionsResolver;

$cases = [];
$case = static function (string $name, callable $f) use (&$cases): void {
    $deprecations = [];
    set_error_handler(static function (int $type, string $message) use (&$deprecations) {
        $deprecations[] = $message;

        return true;
    });
    try {
        $outcome = typed_call($f);
    } finally {
        restore_error_handler();
    }
    $cases[] = ['name' => $name, 'outcome' => $outcome, 'deprecations' => $deprecations];
};

// formatOptions() shifts the resolver's own $parentsOptions: after a
// caught error, later messages of the same nested resolver lose a level.
$case('parents_shift_after_caught_error', static function () {
    $r = new OptionsResolver();
    $r->setOptions('foo', static function (OptionsResolver $n) {
        $n->setDefined('x');
        $n->setDefault('a', static function (Options $o) {
            try {
                return $o['x'];
            } catch (NoSuchOptionException $e) {
                return $e->getMessage();
            }
        });
        $n->setDefault('b', 1);
        $n->setAllowedTypes('b', 'string');
    });

    return $r->resolve();
});
$case('parents_shift_caught_message', static function () {
    $r = new OptionsResolver();
    $r->setOptions('foo', static function (OptionsResolver $n) {
        $n->setDefined('x');
        $n->setDefault('a', static function (Options $o) {
            try {
                return $o['x'];
            } catch (NoSuchOptionException $e) {
                return $e->getMessage();
            }
        });
    });

    return $r->resolve();
});
$case('two_levels_prefix', static function () {
    $r = new OptionsResolver();
    $r->setOptions('a', static function (OptionsResolver $n) {
        $n->setOptions('b', static function (OptionsResolver $m) {
            $m->setRequired(['y', 'x']);
        });
    });

    return $r->resolve();
});
$case('prototype_in_prototype_prefix', static function () {
    $r = new OptionsResolver();
    $r->setOptions('a', static function (OptionsResolver $n) {
        $n->setPrototype(true);
        $n->setOptions('b', static function (OptionsResolver $m) {
            $m->setDefined('c');
            $m->setAllowedTypes('c', 'int');
        });
    });

    return $r->resolve(['a' => ['k' => ['b' => ['c' => 'no']]]]);
});
$case('undefined_sorted', static function () {
    $r = new OptionsResolver();
    $r->setDefined(['z', '3', 'B', 'a']);

    return $r->resolve(['b' => 1, 10 => 1, 'a1' => 1, 2 => 1, '1.5' => 1, 'x1' => 1, '1e1' => 1, ' 3' => 1, -1 => 1]);
});
$case('missing_sorted', static function () {
    $r = new OptionsResolver();
    $r->setRequired(['b', '10', 'a', '2', '1.5', ' 3']);

    return $r->resolve();
});
$case('int_option_names', static function () {
    $r = new OptionsResolver();
    $r->setDefaults(['foo', 'bar']);

    return $r->resolve(['1' => 'baz']);
});
$case('allowed_values_merge_string_keys', static function () {
    $r = new OptionsResolver();
    $r->setDefault('o', 'none');
    $r->setAllowedValues('o', ['a' => 'x', 'y']);
    $r->addAllowedValues('o', ['a' => 'z', 'w']);

    return $r->resolve();
});
$case('format_values', static function () {
    $r = new OptionsResolver();
    $r->setDefault('o', 0.1 + 0.2);
    $r->setAllowedValues('o', [1.0, -0.0, 1e100, 1.5e-7, \INF, -\INF, \NAN, 7, true, null, [], new \stdClass()]);

    return $r->resolve();
});
foreach (['numeric', 'scalar', 'iterable', 'countable', 'object', 'callable', 'resource', 'STDCLASS', '\DateTimeInterface', 'ArrayObject', 'Countable', 'Traversable', 'mixed', ' int ', 'string|', 'int|(string', '(int|string', 'boolean', 'integer', 'long', 'double', 'real', 'Int'] as $type) {
    $values = [
        'int' => 1, 'float' => 1.5, 'numeric_string' => ' 1', 'trailing_space' => '1 ', 'exponent' => '1e3',
        'hex' => '0x1A', 'word' => 'abc', 'empty' => '', 'true' => true, 'null' => null, 'array' => [1],
        'stdclass' => new \stdClass(), 'arrayobject' => new \ArrayObject([]), 'datetime' => new \DateTimeImmutable('@0'),
        'closure' => static fn () => null,
    ];
    foreach ($values as $label => $value) {
        $case("type $type $label", static function () use ($type, $value) {
            $r = new OptionsResolver();
            $r->setDefault('o', $value);
            $r->setAllowedTypes('o', $type);

            $resolved = $r->resolve();

            return \is_object($resolved['o']) ? 'ok' : $resolved;
        });
    }
}
$case('deprecation_name_twice', static function () {
    $r = new OptionsResolver();
    $r->setDefined('o');
    $r->setDeprecated('o', '', '', '%name% and %name%%name');

    return $r->resolve(['o' => 1]);
});
$case('deprecation_undefined', static function () {
    $r = new OptionsResolver();
    $r->setDefined(['b', 'a']);

    return $r->setDeprecated('c', 'p', '1');
});
$case('nested_closure_returns_array_prototype_non_array', static function () {
    $r = new OptionsResolver();
    $r->setOptions('a', static function (OptionsResolver $n) {
        $n->setPrototype(true);
    });

    return $r->resolve(['a' => ['x' => [], 'y' => 3]]);
});
$case('ignore_undefined_with_required', static function () {
    $r = new OptionsResolver();
    $r->setIgnoreUndefined();
    $r->setRequired('r');

    return $r->resolve(['x' => 1]);
});

$lines = [];
foreach ($cases as $c) {
    $lines[] = '  ' . json_encode($c, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR);
}
echo "[\n", implode(",\n", $lines), "\n]\n";
