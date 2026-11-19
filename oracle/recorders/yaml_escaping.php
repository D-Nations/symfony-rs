<?php
// Symfony Yaml's Escaper and Unescaper on fixed and random inputs, for
// crates/yaml/tests/data/escaping.json (scripts/record.sh yaml).
// Strings travel as base64; a thrown exception as "!" + its message.

require '/demo/vendor/autoload.php';

use Symfony\Component\Yaml\Escaper;
use Symfony\Component\Yaml\Unescaper;

$inputs = ['', 'plain', 'it\'s', 'a "quote"', 'back\\slash', '\\"', "tab\there", "nl\nx", "\x00\x01\x1f\x7f",
    "\u{85}\u{a0}\u{2028}\u{2029}", 'null', 'NULL', '~', 'True', 'off', 'Y', '.inf', '.NaN', '...', '-dash', '?q',
    '|p', '<a', '>b', '=c', '!d', '%e', '@f', '`g', "\u{3000}ideo", "a\u{1680}b", "x\u{180e}y", 'a:b', 'a: b',
    '{x}', '[y]', 'a,b', '&r', '*s', '#c', 'é', "\xff", "a\xc3", '0', '12', '1.5', 'key: "v"'];

$escapes = ['\\0', '\\a', '\\b', '\\t', "\\\t", '\\n', '\\v', '\\f', '\\r', '\\e', '\\ ', '\\"', '\\/', '\\\\',
    '\\N', '\\_', '\\L', '\\P', '\\x41', '\\xe9', '\\xZZ', '\\x4', '\\u00e9', '\\u20AC', '\\uD800', '\\U0001F600',
    '\\U00110000', '\\UFFFFFFFF', '\\q', '\\é', "a\\\nb", 'end\\', 'mix \\t\\x41\\u00e9 done'];

mt_srand(5);
$alphabet = ['a', ' ', "'", '"', '\\', ':', "\t", "\n", '#', 'é', "\u{2028}", 'x', 'u', '0', 'F', '-', '{'];
for ($i = 0; $i < 300; ++$i) {
    $s = '';
    for ($n = mt_rand(0, 12); $n > 0; --$n) {
        $s .= $alphabet[mt_rand(0, \count($alphabet) - 1)];
    }
    $inputs[] = $s;
    $escapes[] = $s;
}

$unescaper = new Unescaper();
$catch = static function (callable $f) {
    try {
        return base64_encode($f());
    } catch (\Throwable $e) {
        return '!' . $e->getMessage();
    }
};

$escaping = [];
foreach ($inputs as $s) {
    $escaping[] = [
        base64_encode($s),
        Escaper::requiresDoubleQuoting($s),
        base64_encode(Escaper::escapeWithDoubleQuotes($s)),
        Escaper::requiresSingleQuoting($s),
        base64_encode(Escaper::escapeWithSingleQuotes($s)),
    ];
}

$unescaping = [];
foreach ($escapes as $s) {
    $unescaping[] = [
        base64_encode($s),
        $catch(static fn () => $unescaper->unescapeDoubleQuotedString($s)),
        base64_encode($unescaper->unescapeSingleQuotedString($s)),
    ];
}

echo json_encode(['escaping' => $escaping, 'unescaping' => $unescaping], JSON_UNESCAPED_SLASHES | JSON_PRETTY_PRINT), "\n";
