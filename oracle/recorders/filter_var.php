<?php
// filter_var() behaviour for crates/php-value/tests/data/filter.json
// (scripts/record.sh php-value). Each case is [value, filter, options,
// result] in typed() form; options name their callback (see $callbacks)
// instead of carrying a closure.

error_reporting(E_ALL & ~E_DEPRECATED);
set_error_handler(static function (int $level, string $message): bool {
    throw new \ErrorException($message, 0, $level);
});

require __DIR__ . '/lib/typed.php';

final class Text implements \Stringable
{
    public function __construct(private string $text)
    {
    }

    public function __toString(): string
    {
        return $this->text;
    }
}

function encode_value(mixed $v): array
{
    if ($v instanceof Text) {
        return ['stringable', base64_encode((string) $v)];
    }

    return typed_value($v);
}

function encode_options(mixed $o): array
{
    if (\is_array($o)) {
        $pairs = [];
        foreach ($o as $k => $v) {
            $pairs[] = [typed_value($k), \is_array($v) ? encode_options($v) : (\is_string($v) && str_starts_with($v, 'callback:') ? ['callback', substr($v, 9)] : encode_value($v))];
        }

        return ['array', $pairs];
    }

    return typed_value($o);
}

$callbacks = [
    // what the callback receives
    'echo' => static fn ($v) => ['arg' => $v],
    'int' => static fn ($v) => (int) $v,
    'null' => static fn ($v) => null,
    'false' => static fn ($v) => false,
    'upper' => strtoupper(...),
];

function resolve(mixed $o, array $callbacks): mixed
{
    if (\is_array($o)) {
        foreach ($o as $k => $v) {
            $o[$k] = resolve($v, $callbacks);
        }

        return $o;
    }
    if (\is_string($o) && str_starts_with($o, 'callback:')) {
        return $callbacks[substr($o, 9)];
    }

    return $o;
}

$constants = [];
foreach (get_defined_constants(true)['filter'] as $name => $value) {
    $constants[] = [$name, $value];
}

$scalars = [
    null, true, false, 0, 1, -1, 123, 256, 255, -0x80, PHP_INT_MAX, PHP_INT_MIN, 0.0, -0.0, 1.0, 1.5, -2.5, 1e20, NAN, INF,
    '', ' ', '0', '1', '-1', '+1', '-0', '+0', '00', '01', '0123', '-0123', '12', ' 12', '12 ', ' 12 ', "12\n", "\t12", "\n12\r", "\v12", "\f12", "12\0",
    '12abc', 'abc', 'foo_BAR_012', 'foo_BAR_0+1-2', '1.0', '1e3', '1.5', '0x100', '0X1f', '0x', '0xg', '-0x10', '0x0', '0o17', '0O17', '017', '08', '0b11', '-017',
    '9223372036854775807', '9223372036854775808', '-9223372036854775808', '-9223372036854775809', '0x7fffffffffffffff', '0x8000000000000000', '0xffffffffffffffff', '0x10000000000000000', '0x0000000000000000001', '0xFFFFFFFFFFFFFFFF',
    '0777777777777777777777', '01000000000000000000000', '01777777777777777777777', '02000000000000000000000', '0o', '0o8', '00x1', '0x-1', '0x+1', '-0', '0o0', '00',
    'true', 'TRUE', 'True', 'yes', 'YES', 'on', 'On', 'off', 'no', 'false', 'FALSE', ' true ', "true\n", "\ttrue", 'y', 'n', 'bad', 'nul', 'null', "\vtrue\v", "\ftrue", "true\0", "\v0\v", "\t\n\r ", "\0",
    "caf\xc3\xa9", "\xff12", '١٢',
];
$objects = [new \stdClass(), new Text('12'), new Text('true'), new Text('bad'), new Text(' 0x1A ')];
$arrays = [[], ['12', 'bad'], ['valid' => '12', 'invalid' => 'bad'], ['values' => ['valid' => '12', 'invalid' => 'bad']], [['false']], ['false', 'true'], [new \stdClass()], ['values' => [new \stdClass()]], [['x' => [1, '2']]], [null, true, 1.5]];

$values = array_merge($scalars, $objects, $arrays);

$F = [
    'DEFAULT' => FILTER_DEFAULT, 'UNSAFE_RAW' => FILTER_UNSAFE_RAW, 'VALIDATE_INT' => FILTER_VALIDATE_INT,
    'VALIDATE_BOOL' => FILTER_VALIDATE_BOOL, 'SANITIZE_NUMBER_INT' => FILTER_SANITIZE_NUMBER_INT, 'CALLBACK' => FILTER_CALLBACK,
];

$flagSets = [
    0, FILTER_NULL_ON_FAILURE, FILTER_REQUIRE_SCALAR, FILTER_REQUIRE_ARRAY, FILTER_FORCE_ARRAY,
    FILTER_REQUIRE_SCALAR | FILTER_NULL_ON_FAILURE, FILTER_REQUIRE_ARRAY | FILTER_NULL_ON_FAILURE,
    FILTER_FORCE_ARRAY | FILTER_NULL_ON_FAILURE, FILTER_REQUIRE_ARRAY | FILTER_FORCE_ARRAY,
    FILTER_REQUIRE_SCALAR | FILTER_FORCE_ARRAY,
];
$intFlags = [FILTER_FLAG_ALLOW_HEX, FILTER_FLAG_ALLOW_OCTAL, FILTER_FLAG_ALLOW_HEX | FILTER_FLAG_ALLOW_OCTAL, FILTER_FLAG_ALLOW_HEX | FILTER_NULL_ON_FAILURE];

$optionSets = [];
foreach ($F as $name => $filter) {
    $sets = [];
    foreach ($flagSets as $flags) {
        $sets[] = $flags;
        $sets[] = ['flags' => $flags];
    }
    $sets[] = [];
    $sets[] = ['options' => ['default' => 'D']];
    $sets[] = ['flags' => FILTER_NULL_ON_FAILURE, 'options' => ['default' => 'D']];
    $sets[] = ['flags' => FILTER_REQUIRE_ARRAY, 'options' => ['default' => ['optional' => null]]];
    $sets[] = ['flags' => FILTER_FORCE_ARRAY, 'options' => ['default' => ['optional' => null]]];
    if ('VALIDATE_INT' === $name) {
        foreach ($intFlags as $flags) {
            $sets[] = $flags;
            $sets[] = ['flags' => $flags, 'options' => ['min_range' => 1, 'max_range' => 0xFF]];
        }
        $sets[] = ['options' => ['min_range' => 1, 'max_range' => 0xFF]];
        $sets[] = ['options' => ['min_range' => 12]];
        $sets[] = ['options' => ['max_range' => 12]];
        $sets[] = ['options' => ['min_range' => 13, 'max_range' => 12]];
        $sets[] = ['options' => ['min_range' => '5', 'max_range' => 12.5]];
        $sets[] = ['options' => ['min_range' => 1, 'max_range' => 0xFF, 'default' => 7]];
        $sets[] = ['options' => ['default' => null]];
        $sets[] = ['options' => ['default' => false]];
        $sets[] = ['flags' => FILTER_FLAG_ALLOW_OCTAL | FILTER_FLAG_ALLOW_HEX, 'options' => ['min_range' => -5, 'max_range' => 5]];
    }
    if ('CALLBACK' === $name) {
        $sets = [];
        foreach (['echo', 'int', 'null', 'false', 'upper'] as $cb) {
            foreach ([0, FILTER_NULL_ON_FAILURE, FILTER_REQUIRE_ARRAY, FILTER_FORCE_ARRAY, FILTER_REQUIRE_SCALAR, FILTER_REQUIRE_ARRAY | FILTER_NULL_ON_FAILURE] as $flags) {
                $sets[] = ['flags' => $flags, 'options' => 'callback:' . $cb];
            }
        }
        $sets[] = ['options' => 'callback:echo'];
        foreach (['false', 'null', 'int'] as $cb) {
            $sets[] = ['options' => 'callback:' . $cb, 'default' => 'D'];
            $sets[] = ['flags' => FILTER_REQUIRE_ARRAY, 'options' => 'callback:' . $cb];
        }
    }
    if ('SANITIZE_NUMBER_INT' === $name || 'DEFAULT' === $name || 'UNSAFE_RAW' === $name) {
        $sets[] = FILTER_FLAG_STRIP_LOW;
    }
    $optionSets[$name] = $sets;
}

$cases = [];
foreach ($optionSets as $name => $sets) {
    foreach ($sets as $options) {
        foreach ($values as $value) {
            try {
                $result = ['value', encode_value(filter_var($value, $F[$name], resolve($options, $callbacks)))];
            } catch (\Throwable $e) {
                $result = ['throws', $e::class, $e->getMessage()];
            }
            $cases[] = [encode_value($value), $F[$name], encode_options($options), $result];
        }
    }
}

echo json_encode(['constants' => $constants, 'cases' => $cases], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
