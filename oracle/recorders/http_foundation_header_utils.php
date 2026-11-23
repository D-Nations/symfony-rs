<?php
// HeaderUtils answers for what its tests leave open, for
// crates/http-foundation/tests/data/header_utils.json (scripts/record.sh
// http-foundation): split() over generated headers and separator lists,
// quote() and unquote() over generated strings, makeDisposition() over
// generated names. Strings are base64; a thrown error is ["throw", class,
// message].
//
// With argv[1] = a path to another HeaderUtils.php (an earlier release),
// only the makeDisposition() section is recorded with that file.

error_reporting(E_ALL);

if (isset($argv[1])) {
    require $argv[1];
} else {
    require '/demo/vendor/autoload.php';
}
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

$names = ['', 'foo', 'foo.html', 'foo bar.html', 'a"b', 'a\\b', 'a/b', 'a%20b', '%', "foo\n", "foo\r\n", "\n", "foo\nbar", "foo\t", "\x7f", "\x1f", '~', ' ', 'föö.html', "\xff", "foo\0", 'tok;en', 'a=b', 'é'];
$disposition = [];
foreach (['attachment', 'inline', 'invalid', 'ATTACHMENT', ''] as $type) {
    foreach ($names as $filename) {
        foreach ($names as $fallback) {
            try {
                $answer = base64_encode(HeaderUtils::makeDisposition($type, $filename, $fallback));
            } catch (\Throwable $e) {
                $answer = ['throw', $e::class, $e->getMessage()];
            }
            $disposition[] = [base64_encode($type), base64_encode($filename), base64_encode($fallback), $answer];
        }
    }
}

if (isset($argv[1])) {
    echo json_encode(['disposition' => $disposition], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
    exit;
}

// parseQuery() over generated queries (drawn after the sections above, so
// their cases do not change): [query, ignoreBrackets, separator, result]
$queryPieces = ['a', 'b', '1', '0', '.', ' ', '+', '[', ']', '[]', '[x]', '[1]', '[ ]', '=', '&', ';', '%20', '%2E', '%5B', '%5D',
    '%00', "\0", '%', '%zz', '%41', '_', '-', 'é', "\xff", '9223372036854775807', '[a.b]', '=v', '==', 'a=1', '&&'];
$query = [];
for ($n = 0; $n < 3000; ++$n) {
    $q = '';
    $length = mt_rand(0, 8);
    for ($i = 0; $i < $length; ++$i) {
        $q .= $queryPieces[mt_rand(0, \count($queryPieces) - 1)];
    }
    $ignoreBrackets = 1 === mt_rand(0, 3);
    $separator = 1 === mt_rand(0, 3) ? ';' : '&';
    $query[] = [base64_encode($q), $ignoreBrackets, $separator, typed_value(HeaderUtils::parseQuery($q, $ignoreBrackets, $separator))];
}

echo json_encode(['split' => $split, 'strings' => $strings, 'disposition' => $disposition, 'query' => $query], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
