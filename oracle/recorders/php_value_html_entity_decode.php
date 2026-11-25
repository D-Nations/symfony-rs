<?php
// html_entity_decode($string) answers (PHP's default flags) for
// crates/php-value/tests/data/html_entity_decode.json (scripts/record.sh
// php-value-decode): every named entity of the translation table, with and
// without its ";" and in other cases; every numeric reference from 0 to
// 0x10FFFF in steps that cover each boundary, decimal and hex, with
// leading zeros and overflowing digits; then generated strings of
// fragments. An entry is [input (base64), result (base64)].

error_reporting(E_ALL);

mt_srand(20261125);

$cases = [];
$add = static function (string $s) use (&$cases): void {
    $cases[] = [base64_encode($s), base64_encode(html_entity_decode($s))];
};

foreach (get_html_translation_table(HTML_ENTITIES, ENT_QUOTES | ENT_HTML401) as $entity) {
    $name = substr($entity, 1, -1);
    $add($entity);
    $add('&'.$name);
    $add('&'.strtoupper($name).';');
    $add('&'.ucfirst(strtolower($name)).';');
    $add('x&'.$name.';y&'.$name.'z;');
}
foreach (['apos', 'AMP', 'nbsp ', 'euro', 'hellip', 'ndash', 'Amp', 'ampx', 'am', 'quot', 'lt', 'gt', '', '#', '#x', '#X', 'x', '#-1', '#+1', '# 1', '#1 ', '#0x41', '#x-41', '#1.5', '#1e2'] as $name) {
    $add('&'.$name.';');
}

$points = [];
foreach ([0, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x1F, 0x20, 0x26, 0x27, 0x3B, 0x7E, 0x7F, 0x80, 0x9F, 0xA0, 0xFF, 0x100, 0x7FF, 0x800, 0xD7FF, 0xD800, 0xDBFF, 0xDC00, 0xDFFF, 0xE000, 0xFDD0, 0xFFFD, 0xFFFE, 0xFFFF, 0x10000, 0x1FFFE, 0x10FFFE, 0x10FFFF, 0x110000, 0x110001, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF, 0x100000000, 0x100000041] as $p) {
    $points[] = $p;
}
for ($p = 0; $p < 0x300; ++$p) {
    $points[] = $p;
}
for ($n = 0; $n < 2000; ++$n) {
    $points[] = mt_rand(0, 0x10FFFF);
}
foreach ($points as $p) {
    $add('&#'.$p.';');
    $add(sprintf('&#x%x;', $p));
    $add(sprintf('&#X%X;', $p));
    $add('&#'.$p);
}
foreach (['&#0000000000065;', '&#00000000000000000000000000000065;', '&#x00000000000000000041;', '&#99999999999999999999;', '&#18446744073709551681;', '&#4294967361;', '&#x10000000000000041;', '&#9223372036854775807;'] as $s) {
    $add($s);
}

$fragments = ['&', ';', '#', 'x', 'X', 'a', 'amp', 'lt', 'eacute', 'Eacute', '&amp;', '&#', '&#x', '65', '41', 'e9', 'zz', ' ', "\xff", "\xc3", "\xc3\xa9", "\0", "\n", '&&', '&;', '0', '%26', '+'];
for ($n = 0; $n < 3000; ++$n) {
    $s = '';
    for ($m = mt_rand(1, 10); $m > 0; --$m) {
        $s .= $fragments[mt_rand(0, \count($fragments) - 1)];
    }
    $add($s);
}
$add(str_repeat('&', 100000).';');
$add(str_repeat('&#', 50000).'65;');

echo json_encode($cases, JSON_THROW_ON_ERROR), "\n";
