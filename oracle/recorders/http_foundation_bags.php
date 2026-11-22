<?php
// ParameterBag and InputBag answers for what their tests leave open, for
// crates/http-foundation/tests/data/bags.json (scripts/record.sh
// http-foundation). Each case is [bag class, method, stored value,
// arguments, outcome, notices]; the stored value sits under the key 'k'
// and the call asks for 'k' (or 'missing' when the case says so).
// Exception and notice messages are base64.

error_reporting(E_ALL);

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\HttpFoundation\InputBag;
use Symfony\Component\HttpFoundation\ParameterBag;

enum IntBacked: int
{
    case One = 1;
    case Minus = -5;
    case Big = PHP_INT_MAX;
}

enum StringBacked: string
{
    case A = 'a';
    case One = '1';
    case Empty = '';
}

enum Unit
{
    case Only;
}

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

$notices = [];
set_error_handler(static function (int $level, string $message) use (&$notices): bool {
    $notices[] = [$level, base64_encode($message)];

    return true;
});

$values = [
    null, true, false, 0, 1, -1, 2, -5, 12, PHP_INT_MAX, 0.0, -0.0, 1.0, 1.5, -5.0, 2.5, 1e20, NAN, INF,
    '', ' ', '0', '1', '-1', '+1', ' 1', '1 ', '1.0', '1.5', '1e0', '01', '0x1', '1abc', 'abc', 'a', 'A', 'true', 'off',
    'foo_BAR_012', 'foo_BAR_0+1-2', "caf\xc3\xa9 \xff\x80 \xe2\x85\xa0 \xc2\xb2 ABC xyz 0189 \t\n", '9223372036854775807', '9223372036854775808', '-5',
    new \stdClass(), new Text('1'), new Text('abc'), IntBacked::One, StringBacked::A, Unit::Only,
    [], ['1'], ['a' => ['b' => '2']],
];

$cases = [];
$call = static function (string $class, string $method, mixed $stored, array $args, bool $missing = false) use (&$cases, &$notices): void {
    $notices = [];
    try {
        $bag = new $class();
        $bag->set('k', $stored);
    } catch (\Throwable $e) {
        $cases[] = [$class, 'set', encode_value($stored), [], ['throws', $e::class, base64_encode($e->getMessage())], []];

        return;
    }
    try {
        $result = ['value', encode_value($bag->$method($missing ? 'missing' : 'k', ...$args))];
    } catch (\Throwable $e) {
        $result = ['throws', $e::class, base64_encode($e->getMessage())];
    }
    $cases[] = [$class, $method, encode_value($stored), array_map('encode_value', $args), $result, $notices, $missing];
};

foreach ([ParameterBag::class, InputBag::class] as $class) {
    foreach ($values as $value) {
        foreach (['get', 'getString', 'getAlpha', 'getAlnum', 'getDigits', 'getInt', 'getBoolean'] as $method) {
            $call($class, $method, $value, []);
        }
        $call($class, 'all', $value, []);
        $call($class, 'getEnum', $value, [IntBacked::class]);
        $call($class, 'getEnum', $value, [StringBacked::class]);
        $call($class, 'getEnum', $value, [IntBacked::class, IntBacked::Minus]);
        $call($class, 'filter', $value, []);
        $call($class, 'filter', $value, [null, FILTER_VALIDATE_INT]);
        $call($class, 'filter', $value, [null, FILTER_VALIDATE_INT, FILTER_NULL_ON_FAILURE]);
        $call($class, 'filter', $value, [null, FILTER_VALIDATE_BOOL, FILTER_REQUIRE_ARRAY]);
        $call($class, 'filter', $value, [null, FILTER_VALIDATE_INT, FILTER_FORCE_ARRAY]);
        $call($class, 'filter', $value, [null, FILTER_VALIDATE_INT, ['flags' => FILTER_REQUIRE_SCALAR]]);
        $call($class, 'filter', $value, [null, FILTER_DEFAULT, ['options' => ['default' => 'D']]]);
    }
    // defaults, for a missing key
    foreach ($values as $default) {
        $call($class, 'get', 'stored', [$default], true);
        $call($class, 'filter', 'stored', [$default], true);
        $call($class, 'filter', 'stored', [$default, FILTER_VALIDATE_INT], true);
        if (\is_string($default)) {
            foreach (['getString', 'getAlpha', 'getAlnum', 'getDigits'] as $method) {
                $call($class, $method, 'stored', [$default], true);
            }
        }
        if (\is_int($default)) {
            $call($class, 'getInt', 'stored', [$default], true);
        }
        if (\is_bool($default)) {
            $call($class, 'getBoolean', 'stored', [$default], true);
        }
    }
    $call($class, 'getEnum', 'stored', [IntBacked::class], true);
    $call($class, 'getEnum', 'stored', [IntBacked::class, IntBacked::One], true);
    $call($class, 'all', 'stored', [], true);
}

echo json_encode($cases, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
