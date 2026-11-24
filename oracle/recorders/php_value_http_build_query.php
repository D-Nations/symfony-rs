<?php
// http_build_query($data, '', $separator, $encoding) answers for
// crates/php-value/tests/data/http_build_query.json (scripts/record.sh
// php-value): hand-picked arrays (scalars of every type, floats, nested
// and empty arrays, int and special-character keys), then generated ones,
// each with both encodings. An entry is [data, separator (base64),
// rfc3986, result (base64)], data typed as lib/typed.php does.

error_reporting(E_ALL);

require __DIR__ . '/lib/typed.php';

mt_srand(20261124);

$fixed = [
    [],
    ['a' => null],
    ['a' => true, 'b' => false, 'c' => 0, 'd' => -7, 'e' => ''],
    ['f' => 0.1, 'g' => 1.0, 'h' => -0.0, 'i' => 1e20, 'j' => 1.5e-7, 'k' => 123456789012345678.0, 'l' => 0.30000000000000004, 'm' => \NAN, 'n' => \INF, 'o' => -\INF, 'p' => 1e15, 'q' => 1e14, 'r' => 100000.0],
    ['a' => []],
    ['a' => [null]],
    ['a' => ['b' => ['c' => 'd']], 'e' => [1, 2]],
    [0 => 'x', 1 => 'y', 10 => 'z', -1 => 'w'],
    ['a b' => 'c d', 'é' => 'ü', '[x]' => '[y]', '~-._' => '~-._', '+&=?#/' => '+&=?#/'],
    ['a' => ['é' => 'x', 'a b' => 'y', 3 => 'z']],
    ['@:!;,*' => '@:!;,*', '%2F' => '%2F', '|' => '|'],
];

$scalars = [null, true, false, 0, 1, -1, 3.5, 0.1, 1e100, '', '0', 'a', 'a b', '/', '%', '+', '~', 'é', "\x00", "\xff"];
$keys = [0, 1, 7, 'a', 'b', 'a b', '%', '[', ']', 'é', '_fragment'];

function generated(int $depth): mixed
{
    global $scalars, $keys;
    if ($depth > 0 && !mt_rand(0, 3)) {
        $a = [];
        for ($n = mt_rand(0, 3); $n > 0; --$n) {
            $a[$keys[mt_rand(0, \count($keys) - 1)]] = generated($depth - 1);
        }

        return $a;
    }

    return $scalars[mt_rand(0, \count($scalars) - 1)];
}

$cases = $fixed;
for ($n = 0; $n < 500; ++$n) {
    $a = [];
    for ($m = mt_rand(0, 4); $m > 0; --$m) {
        $a[$keys[mt_rand(0, \count($keys) - 1)]] = generated(2);
    }
    $cases[] = $a;
}

$out = [];
foreach ($cases as $i => $data) {
    $separator = $i % 3 ? '&' : ($i % 2 ? ';' : '&amp;');
    foreach ([false, true] as $rfc3986) {
        $result = http_build_query($data, '', $separator, $rfc3986 ? \PHP_QUERY_RFC3986 : \PHP_QUERY_RFC1738);
        $out[] = [typed_value($data), base64_encode($separator), $rfc3986, base64_encode($result)];
    }
}

echo json_encode($out, \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
