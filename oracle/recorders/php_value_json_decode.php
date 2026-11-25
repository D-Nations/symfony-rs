<?php
// json_decode($json, true, 512, JSON_BIGINT_AS_STRING | JSON_THROW_ON_ERROR)
// answers, and the same without JSON_BIGINT_AS_STRING, for
// crates/php-value/tests/data/json_decode.json (scripts/record.sh
// php-value-decode), as Request::toArray() and getPayload() call it:
// hand-picked documents (numbers at the 64-bit edges, every escape,
// surrogates, UTF-8 edges, control bytes in and out of strings, repeated
// and numeric keys, depth at the limit), then generated documents and
// mutations of them (bytes inserted, removed or replaced). An entry is
// [input (base64), bigint_as_string, ["ok", typed value] or ["error",
// code, message]].

error_reporting(E_ALL);

require __DIR__ . '/lib/typed.php';

mt_srand(20261125);

$bs = '\\';
$fixed = [
    '', ' ', '1', '-0', '-0.0', '0.0', '1.0', '1e2', '1E2', '1e+2', '1e-2', '1E400', '-1E400', '1e-400', '4.9e-324', '0.1',
    '3.141592653589793238', '9223372036854775807', '9223372036854775808', '-9223372036854775808', '-9223372036854775809',
    '123456789012345678901234567890', '-123456789012345678901234567890', '12345678901234567890.5', '18446744073709551615',
    '0123', '01', '00', '-', '-01', '1.', '.5', '+1', '1e', '1e+', '1.e2', '- 1', ' 1 ', "\t1\n\r", "\f1", "1\f", "\x0b1",
    'true', 'false', 'null', 'TRUE', 'True', 'nul', 'truex', 'nullnull', '[true,false,null]',
    '"a"', '""', '"\u00e9"', '"\u00E9"', '"\u00eA"', '"\ud83d\ude00"', '"\ud800"', '"\udc00"', '"\ud800x"', '"\ud800\ud800"',
    '"\ud800\u0041"', '"a\udc00"', '"\u0000"', '"\u"', '"\u00"', '"\u00g0"', '"\x"', '"\U0041"', '"\/"', '"\\\\"',
    '"\"\\\\\/\b\f\n\r\t"', '"'.$bs."'".'"', "\"a\x01b\"", "\"a\x7fb\"", "\"a\tb\"", "\"a\nb\"", "\"a\x00b\"", "\"a\x1f\"",
    "\xff", "[\xff]", "\"\xc3", "\"\xc3\"", "\"\xc0\xaf\"", "\"\xed\xa0\x80\"", "\"\xf4\x8f\xbf\xbf\"", "\"\xf4\x90\x80\x80\"",
    "\"\xe0\xa0\x80\"", "\"\xe0\x80\x80\"", "\"\xc3\xa9\"", "\xc3\xa9", "[\xc3\xa9]", "1\xc3\xa9", "\x7f", "1\x7f", "\x01", "\x00",
    "1\x01", "\"a\xff", "\"\x01\xff\"", "\"\xff\x01\"", "[1 \x01]", "\"\\x\x01\"", "\"a\"\xff", "\xef\xbb\xbf1", "\"abc", "\"a\x01",
    '[]', '{}', '[1,2]', '[1,2,]', '[,1]', '[1 2]', '[1,,2]', '[1', '[1,2', '{"a":1}', '{"a":1,}', '{"a" 1}', '{"a":}', '{a:1}',
    '{1:2}', '{"a":1 "b":2}', '{"":1}', '{"1":1,"01":2,"-1":3,"-0":4,"9223372036854775808":5,"9223372036854775807":6,"1.5":7," 1":8}',
    '{"a":1,"a":2}', '{"a":1,"b":2,"a":3}', '{"\u0000a":1}', '{"0":"a","1":"b"}', '{"a":[1,{"b":null}]}', '[1]x', '1 2', '[1]]',
    '{"a":1}}', ']', '}', ':', ',', '"a":1', '[[[]]]', '[{"a":[{"b":[]}]}]', '{"a":{"b":{"c":"d"}}}',
    // a closer of the other kind where a closer may come, and elsewhere
    '{]', '[}', '{"a":1]', '[1}', '[1,}', '{"a":1,]', '{"a":]', '{"a"]', '{"a":1,"b"]', '[[1}]', '{"a":[}}',
    // a high surrogate escape followed by anything but a low one
    '"\ud83d\u12"', '"\ud83d\uzzzz"', '"\ud83dA"', '"\ud83d\\\\"', '"\ud83d"', '"\ud83d\n"', '"\ud83d\udc0"',
];
for ($n = 505; $n <= 515; ++$n) {
    $fixed[] = str_repeat('[', $n).str_repeat(']', $n);
    $fixed[] = str_repeat('[', $n).'1'.str_repeat(']', $n);
    $fixed[] = str_repeat('{"a":', $n).'1'.str_repeat('}', $n);
    $fixed[] = str_repeat('{"a":', $n).'{}'.str_repeat('}', $n);
    $fixed[] = str_repeat('[', $n)."\xff";
    $fixed[] = str_repeat('[', $n)."\x01";
    $fixed[] = str_repeat('[', $n).'x';
    $fixed[] = str_repeat('[', $n);
}

$scalars = ['0', '-1', '7', '1.5', '-2.5e3', '1e999', '9223372036854775808', 'true', 'false', 'null', '""', '"a"', '"\u00e9"', '"\ud83d\ude00"', '"x\ny"', '"é"'];
$keys = ['"a"', '"b"', '"0"', '"1"', '"01"', '"-1"', '""', '"é"', '"a b"', '"\u0000"'];

function generated(int $depth): string
{
    global $scalars, $keys;
    if ($depth > 0 && mt_rand(0, 2) === 0) {
        $items = [];
        $object = mt_rand(0, 1) === 1;
        for ($n = mt_rand(0, 4); $n > 0; --$n) {
            $items[] = ($object ? $keys[mt_rand(0, \count($keys) - 1)].':' : '').generated($depth - 1);
        }
        $ws = [' ', '', "\n", "\t", "\r"];
        $sep = $ws[mt_rand(0, 4)].','.$ws[mt_rand(0, 4)];

        return ($object ? '{' : '[').implode($sep, $items).($object ? '}' : ']');
    }

    return $scalars[mt_rand(0, \count($scalars) - 1)];
}

$noise = ['', ' ', ',', ':', '[', ']', '{', '}', '"', '\\', "\x00", "\x01", "\x7f", "\xff", "\xc3", '-', '0', 'e', '.', 'u', 'n'];
$docs = $fixed;
for ($n = 0; $n < 3000; ++$n) {
    $doc = generated(mt_rand(0, 5));
    $docs[] = $doc;
    if ($doc !== '') {
        $at = mt_rand(0, \strlen($doc) - 1);
        $docs[] = match (mt_rand(0, 2)) {
            0 => substr($doc, 0, $at).$noise[mt_rand(0, \count($noise) - 1)].substr($doc, $at),
            1 => substr($doc, 0, $at).substr($doc, $at + 1),
            default => substr($doc, 0, $at).$noise[mt_rand(0, \count($noise) - 1)].substr($doc, $at + 1),
        };
    }
}

$cases = [];
foreach ($docs as $doc) {
    foreach ([true, false] as $bigint) {
        try {
            $value = json_decode($doc, true, 512, ($bigint ? JSON_BIGINT_AS_STRING : 0) | JSON_THROW_ON_ERROR);
            $result = ['ok', typed_value($value)];
        } catch (\JsonException $e) {
            $result = ['error', $e->getCode(), $e->getMessage()];
        }
        $cases[] = [base64_encode($doc), $bigint, $result];
    }
}

// typed values of 511 levels nest deeper than json_encode()'s default 512
echo json_encode($cases, JSON_THROW_ON_ERROR, 1_000_000), "\n";
