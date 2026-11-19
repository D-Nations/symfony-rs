<?php
// PHP value behaviour for crates/php-value/tests/data/php.json
// (scripts/record.sh php-value). Floats travel as 16 hex digits of their
// IEEE 754 bits; every other value as [type, value] from typed().

error_reporting(E_ALL & ~E_WARNING & ~E_DEPRECATED);

function bits(float $x): string
{
    return bin2hex(pack('E', $x));
}

function typed($v): array
{
    return match (true) {
        null === $v => ['null', null],
        \is_bool($v) => ['bool', $v],
        \is_int($v) => ['int', $v],
        \is_float($v) => ['float', bits($v)],
        \is_string($v) => ['string', base64_encode($v)],
        default => ['other', get_debug_type($v)],
    };
}

$strings = ['', '0', '1', '-1', '01', '+1', '1.0', ' 1', '1 ', "1\n", "\t1", '-0', '00', '0x1A', '0b11', '0o17',
    '9223372036854775807', '9223372036854775808', '-9223372036854775808', '-9223372036854775809',
    '1e3', '1E3', '1e', '.5', '5.', '-.5', '+.5', '1_000', 'abc', '12abc', ' 12abc', '1e1000', '-1e1000',
    'inf', 'INF', 'nan', '0.1', '1.5e-7', '123456789012345678901234567890', '  ', '.', '-', '+', 'e3',
    '0123', '-0123', '0.0', '-0.0', "1\0", '١', '1.2.3', '9223372036854775807.0', '0e0', '1e+3', '1e-3'];

$keys = [];
foreach ($strings as $s) {
    $a = [];
    $a[$s] = 1;
    $keys[] = [base64_encode($s), typed(array_key_first($a))];
}
foreach ([true, false, null] as $k) {
    $a = [];
    $a[$k] = 1;
    $keys[] = [typed($k), typed(array_key_first($a))];
}

$casts = [];
foreach ($strings as $s) {
    $casts[] = [base64_encode($s), (int) $s, bits((float) $s), is_numeric($s)];
}

$appends = [];
$scenario = static function (string $label, callable $build) use (&$appends) {
    try {
        $a = $build();
        $appends[] = [$label, array_map('typed', array_keys($a))];
    } catch (\Throwable $e) {
        $appends[] = [$label, get_class($e) . ': ' . $e->getMessage()];
    }
};
$scenario('empty', static function () { $a = []; $a[] = 'x'; return $a; });
$scenario('after 5', static function () { $a = [5 => 'a']; $a[] = 'x'; return $a; });
$scenario('after -5', static function () { $a = [-5 => 'a']; $a[] = 'x'; return $a; });
$scenario('after -5 and 2', static function () { $a = [-5 => 'a', 2 => 'b']; $a[] = 'x'; return $a; });
$scenario('after string key', static function () { $a = ['k' => 'a']; $a[] = 'x'; return $a; });
$scenario('after unset of the last', static function () { $a = ['a', 'b', 'c']; unset($a[2]); $a[] = 'x'; return $a; });
$scenario('after "7"', static function () { $a = ['7' => 'a']; $a[] = 'x'; return $a; });
$scenario('after PHP_INT_MAX', static function () { $a = [PHP_INT_MAX => 'a']; $a[] = 'x'; return $a; });
$scenario('overwrite keeps place', static function () { $a = ['a' => 1, 'b' => 2]; $a['a'] = 3; return $a; });

$floats = [0.0, -0.0, 1.0, -1.0, 0.1, 0.5, 1.5, 100.0, 1e14, 1e15, 1e16, 123456789012345.0, 1234567890123456.0,
    0.1 + 0.2, 1 / 3, 2 / 3, 1e-4, 1e-5, 0.0001234, 1.25e-10, 1e25, -1e25, 1.5e300, 5e-324, PHP_FLOAT_MAX,
    PHP_FLOAT_EPSILON, INF, -INF, NAN, 3.14159265358979, 12.0, 99999999999999.0, 999999999999999.0,
    0.30000000000000004, 1e100, 123.456, -0.000001, 7e-7];
mt_srand(11);
for ($i = 0; $i < 300; ++$i) {
    $floats[] = (mt_rand() / mt_getrandmax() - 0.5) * 10 ** mt_rand(-20, 25);
}
$float_strings = [];
foreach ($floats as $f) {
    $float_strings[] = [bits($f), (string) $f, var_export($f, true), json_encode($f)];
}

$radix = [];
foreach (['0', '1A', 'ff', 'FF', 'fffffffffffffff', '7fffffffffffffff', '8000000000000000', 'ffffffffffffffffff',
    'g1', '1g', '', 'xyz', '-1', '0x1A', '17', '777', '1777777777777777777777', '9', '18'] as $s) {
    $radix[] = [$s, typed(hexdec($s)), typed(octdec($s))];
}

echo json_encode([
    'keys' => $keys,
    'casts' => $casts,
    'appends' => $appends,
    'float_strings' => $float_strings,
    'radix' => $radix,
], JSON_UNESCAPED_SLASHES | JSON_PRETTY_PRINT), "\n";
