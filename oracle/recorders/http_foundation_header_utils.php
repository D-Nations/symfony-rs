<?php
// HeaderUtils answers for what its tests leave open, for
// crates/http-foundation/tests/data/header_utils.json (scripts/record.sh
// http-foundation): split() over generated headers and separator lists,
// quote() and unquote() over generated strings. Strings are base64.

error_reporting(E_ALL);

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\HttpFoundation\HeaderUtils;

mt_srand(20261122);

$pieces = ['a', 'b c', 'tok', '"q"', '"x\\"y"', '"open', '\\', '\\"', ' ', '  ', "\t", "\n", "\f", "\v", "\0", ',', ';', '=', ', ', ' ;', ' = ', 'é', "\xff", '""', '"a,b"'];
$separatorSets = [',', ';', '=', ',;', ',=', ';=', ',;=', ' ', "\t", ', ', '; ', ' =', ",\t", '=;,', ';,'];

$split = [];
for ($n = 0; $n < 3000; ++$n) {
    $header = '';
    $length = mt_rand(0, 9);
    for ($i = 0; $i < $length; ++$i) {
        $header .= $pieces[mt_rand(0, \count($pieces) - 1)];
    }
    $separators = $separatorSets[mt_rand(0, \count($separatorSets) - 1)];
    $split[] = [base64_encode($header), base64_encode($separators), typed_value(HeaderUtils::split($header, $separators))];
}

$strings = [];
foreach ($pieces as $a) {
    foreach ($pieces as $b) {
        $s = $a . $b;
        $strings[] = [base64_encode($s), base64_encode(HeaderUtils::quote($s)), base64_encode(HeaderUtils::unquote($s))];
    }
}
foreach (['', "foo\n", "foo\n\n", "\nfoo", "a\\\nb", "a\\", 'a-b_c.d~e', "foo\r\n"] as $s) {
    $strings[] = [base64_encode($s), base64_encode(HeaderUtils::quote($s)), base64_encode(HeaderUtils::unquote($s))];
}

echo json_encode(['split' => $split, 'strings' => $strings], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
