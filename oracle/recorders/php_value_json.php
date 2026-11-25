<?php
// json_encode($value, $flags, $depth) and json_decode($json) (objects as
// stdClass) answers, for crates/php-value/tests/data/json_encode.json
// (scripts/record.sh php-value-json).
//
// "encode": hand-picked values (float layouts, every escape and flag,
// invalid UTF-8 pieces, errors that stop the encoding and errors that do
// not, depth at the limit, enum cases, JsonSerializable objects), then
// seeded random values under random flags and depths. An entry is [typed
// value, flags, depth, result]: ["json", base64 text, error code] for a
// string (null when JSON_THROW_ON_ERROR alone leaves json_last_error()
// as it was), ["false", code, message], or ["throws", class, code,
// message, previous [class, message] or null].
// A JsonSerializableValue is typed as an object of that class whose
// property "v" is what jsonSerialize() returns; a JsonSerializableThrows
// throws \Exception with its property "message".
//
// "decode": json_decode($json, false, 512, $bigint ? JSON_BIGINT_AS_STRING
// : 0) with JSON_THROW_ON_ERROR of hand-picked and generated documents and
// mutations of them, as [input (base64), bigint, ["ok", typed value] or
// ["error", code, message]].

error_reporting(E_ALL);

require __DIR__ . '/lib/typed.php';

enum UnitCase { case A; case B; }
enum IntCase: int { case One = 1; case Big = PHP_INT_MAX; }
enum StringCase: string { case X = 'x'; case Tag = '<a href="/">'; case Bad = "\xff"; }

final class JsonSerializableValue implements \JsonSerializable
{
    public function __construct(public mixed $v)
    {
    }

    public function jsonSerialize(): mixed
    {
        return $this->v;
    }
}

final class JsonSerializableThrows implements \JsonSerializable
{
    public function __construct(public string $message)
    {
    }

    public function jsonSerialize(): mixed
    {
        throw new \Exception($this->message);
    }
}

mt_srand(20261125);
$pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];

$flagBits = [JSON_HEX_TAG, JSON_HEX_AMP, JSON_HEX_APOS, JSON_HEX_QUOT, JSON_FORCE_OBJECT, JSON_NUMERIC_CHECK, JSON_UNESCAPED_SLASHES, JSON_PRETTY_PRINT, JSON_UNESCAPED_UNICODE, JSON_PARTIAL_OUTPUT_ON_ERROR, JSON_PRESERVE_ZERO_FRACTION, JSON_UNESCAPED_LINE_TERMINATORS, JSON_INVALID_UTF8_IGNORE, JSON_INVALID_UTF8_SUBSTITUTE, JSON_THROW_ON_ERROR, JSON_OBJECT_AS_ARRAY, JSON_BIGINT_AS_STRING];

$floats = [0.0, -0.0, 1.0, -1.0, 0.1, 0.5, 1.5, -2.5, 100.0, 1e15, 1e16, 1e17, 1e18, 1.5e17, 123456789012345678.0, 0.0001, 0.00001, 0.000123, 1e-7, 1e25, -1e25, 1.7976931348623157e308, 5e-324, 2.2250738585072014e-308, 1/3, 2/3, 9007199254740993.0, 3.14159, 1e300, INF, -INF, NAN, (float) PHP_INT_MAX, 0.30000000000000004];
$ints = [0, 1, -1, 42, PHP_INT_MAX, PHP_INT_MIN, 1000000];
$strings = ['', 'a', 'foo bar', '<>\'&"', '/', 'a/b', '\\', "\x08\x0c\n\r\t", "\x00", "\x01\x1f\x7f", 'é', "\u{2028}", "\u{2029}", "a\u{2028}b", '😀', "\u{FFFD}", "\u{10FFFF}", "\u{FEFF}", 'ಠ_ಠ',
    "\xff", "a\xffb", "\xc3", "\xc3(", "\xe2\x82", "\xe2\x82x", "\xe2x", "\xf0\x9f\x98", "\xf0\x9f\x98x", "\xed\xa0\x80", "\xed\xbf\xbf", "\xc0\xaf", "\xc1\xbf", "\xe0\x80\x80", "\xf0\x80\x80\x80", "\xf4\x90\x80\x80", "\xf5\x80\x80\x80", "\xf8\x88\x80\x80\x80", "\x80\x80", "\xfe\xff", "\xc2\xc2\xa9", "\xe0\xa0", "\xee\x80", "\xf0\x90\x80", "é\xff😀",
    '0', '1', '-1', '1.0', '1.5', '-0', '-0.0', '0.0', ' 1', '1 ', "\n1", "1\n", '+1', '.5', '1.', '1e2', '1E2', '1e500', '-1e500', '1e308', '1e309', '-1.5e-400', '0x1A', '0b1', '1_000', '012', '9223372036854775807', '9223372036854775808', '-9223372036854775809', ' ', 'abc', '1a', 'INF', 'NAN'];
$keys = ['a', 'b', 'é', '', '0', '1', '01', '-1', '<k>', "\xff", "a\xffb", '/', "\x00", 'a"b', 'ä/ö', "\u{2028}", 5, -3, 0, 2];

function scalar(): mixed
{
    global $floats, $ints, $strings, $pick;
    return match (mt_rand(0, 9)) {
        0 => null,
        1 => 0 === mt_rand(0, 1),
        2, 3 => $pick($ints),
        4, 5 => $pick($floats),
        6 => $pick([UnitCase::A, IntCase::One, IntCase::Big, StringCase::X, StringCase::Tag, StringCase::Bad, StringCase::X, IntCase::One]),
        default => $pick($strings),
    };
}

function value(int $depth): mixed
{
    global $keys, $pick;
    if ($depth <= 0 || mt_rand(0, 2) === 0) {
        return scalar();
    }
    $items = [];
    $kind = mt_rand(0, 9);
    $n = mt_rand(0, 4);
    for ($i = 0; $i < $n; ++$i) {
        if ($kind < 4) {
            $items[] = value($depth - 1);
        } else {
            $items[$pick($keys)] = value($depth - 1);
        }
    }
    return match (true) {
        7 === $kind => (object) $items,
        8 === $kind => new \ArrayObject($items),
        9 === $kind && 0 === mt_rand(0, 3) => new JsonSerializableThrows($pick(['This error is expected', 'boom'])),
        9 === $kind => new JsonSerializableValue(0 === mt_rand(0, 1) ? $items : value($depth - 1)),
        default => $items,
    };
}

function nested(int $n, mixed $inner, bool $object = false): mixed
{
    for ($i = 0; $i < $n; ++$i) {
        $inner = $object ? (object) ['a' => $inner] : [$inner];
    }
    return $inner;
}

$cases = [];
$fixedFlags = [0, JSON_PRESERVE_ZERO_FRACTION, 15, JSON_PARTIAL_OUTPUT_ON_ERROR, JSON_THROW_ON_ERROR, JSON_PARTIAL_OUTPUT_ON_ERROR | JSON_THROW_ON_ERROR, JSON_INVALID_UTF8_SUBSTITUTE, JSON_INVALID_UTF8_IGNORE, JSON_INVALID_UTF8_IGNORE | JSON_INVALID_UTF8_SUBSTITUTE, JSON_INVALID_UTF8_SUBSTITUTE | JSON_UNESCAPED_UNICODE, JSON_UNESCAPED_UNICODE, JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_LINE_TERMINATORS, JSON_UNESCAPED_LINE_TERMINATORS, JSON_UNESCAPED_SLASHES, JSON_NUMERIC_CHECK, JSON_NUMERIC_CHECK | JSON_PRESERVE_ZERO_FRACTION, JSON_PRETTY_PRINT, JSON_FORCE_OBJECT, JSON_FORCE_OBJECT | JSON_PRETTY_PRINT];
foreach (array_merge($floats, $ints, $strings, [null, true, false, UnitCase::A, IntCase::Big, StringCase::Tag, StringCase::Bad]) as $v) {
    foreach ($fixedFlags as $flags) {
        $cases[] = [$v, $flags, 512];
    }
}
$structures = [
    [], [1, 2], [3 => 1], [1 => 'a', 0 => 'b'], ['a' => 1], (object) [], (object) ['1' => 2], new \ArrayObject([1, 2]), new \ArrayObject(['a' => 1]), new \ArrayObject(),
    ['x' => [], 'y' => (object) [], 'z' => [1, ['a' => 2]]], [[], [[]], (object) []], ['a' => (object) ['b' => []]],
    [1, NAN, "\xff"], ["\xff", INF], [INF, UnitCase::A], [UnitCase::A, INF], [UnitCase::A, "\xff"], ["\xff", UnitCase::A], [INF, "\xff", INF],
    [1 => [2 => INF]], ['a' => UnitCase::A], ["\xff" => 1], ["\xff" => INF], ["a\xffb" => 1, 'b' => "\xff"],
    [nested(3, 1), "\xff"], [nested(3, 1), INF], ["\xff", nested(3, 1)], [INF, nested(3, 1)], [UnitCase::A, nested(3, 1)], [nested(3, 1), UnitCase::A],
    new JsonSerializableValue([1, 2]), new JsonSerializableValue(new JsonSerializableValue('x')), [new JsonSerializableValue(INF)], new JsonSerializableThrows('This error is expected'),
    [1, new JsonSerializableThrows('boom')], [INF, new JsonSerializableThrows('boom')], ["\xff", new JsonSerializableThrows('boom')], new JsonSerializableValue(new JsonSerializableThrows('boom')), new JsonSerializableValue(null),
    new JsonSerializableValue(UnitCase::A), new JsonSerializableValue("\xff"), new JsonSerializableValue(nested(2, 1)), [new JsonSerializableValue([1])], new JsonSerializableValue(new \ArrayObject([1])),
    ['a"<' => 1, 5 => 2, -3 => 3], ['é' => 1], ['/' => 1], [0 => 'a', 2 => 'b'], [-1 => 1], ['0' => 'a', '1' => 'b'],
    (object) ['a' => 1, 'b' => (object) []], [[1, [2, [3, []]]], (object) ['k' => [(object) []]]],
];
foreach ($structures as $v) {
    foreach (array_merge($fixedFlags, [JSON_PRETTY_PRINT | JSON_PARTIAL_OUTPUT_ON_ERROR, 15 | JSON_THROW_ON_ERROR]) as $flags) {
        foreach ([512, 0, 1, 2, 3] as $depth) {
            $cases[] = [$v, $flags, $depth];
        }
    }
}
foreach ([JSON_PARTIAL_OUTPUT_ON_ERROR, 0, JSON_THROW_ON_ERROR] as $flags) {
    for ($n = 0; $n <= 6; ++$n) {
        foreach ([[], 1, (object) [], "\xff", INF, new JsonSerializableValue([1])] as $inner) {
            foreach ([false, true] as $object) {
                foreach ([0, 1, 2, 3, 4, 5, 6] as $depth) {
                    $cases[] = [nested($n, $inner, $object), $flags, $depth];
                }
            }
        }
    }
    foreach ([510, 511, 512, 513] as $n) {
        $cases[] = [nested($n, 1), $flags, 512];
        $cases[] = [nested($n, [], true), $flags, 512];
    }
}
for ($n = 0; $n < 6000; ++$n) {
    $flags = 0;
    foreach ($flagBits as $bit) {
        if (0 === mt_rand(0, 4)) {
            $flags |= $bit;
        }
    }
    if (0 === mt_rand(0, 3)) {
        $flags = 15 | ($flags & (JSON_THROW_ON_ERROR | JSON_PRESERVE_ZERO_FRACTION | JSON_PRETTY_PRINT));
    }
    $cases[] = [value(mt_rand(0, 4)), $flags, 0 === mt_rand(0, 5) ? mt_rand(0, 4) : 512];
}

$encode = [];
foreach ($cases as [$v, $flags, $depth]) {
    try {
        $json = json_encode($v, $flags, $depth);
        // JSON_THROW_ON_ERROR alone leaves json_last_error() as it was
        $error = JSON_THROW_ON_ERROR === ($flags & (JSON_THROW_ON_ERROR | JSON_PARTIAL_OUTPUT_ON_ERROR)) ? null : json_last_error();
        $result = false === $json ? ['false', $error, json_last_error_msg()] : ['json', base64_encode($json), $error];
    } catch (\Throwable $e) {
        $previous = $e->getPrevious();
        $result = ['throws', $e::class, $e->getCode(), $e->getMessage(), null === $previous ? null : [$previous::class, $previous->getMessage()]];
    }
    $encode[] = [typed_value($v), $flags, $depth, $result];
}

// json_decode($json) documents
$bs = '\\';
$docs = [
    '', '1', '-0', '1.0', '1E400', '9223372036854775808', 'true', 'null', '"a"', '"é"', '"\ud800"', "\xff", '[]', '{}', '[{}]', '{"a":{}}', '[1,2]',
    '{"a":1}', '{"":1}', '{"1":1,"01":2,"-1":3,"1.5":4," 1":5}', '{"a":1,"a":2}', '{"a":1,"b":2,"a":3}', '{"0":"a","1":"b"}', '{"1":{"2":3}}',
    '{"\u0000a":1}', '{"a\u0000":1}', '{"\u0000":1}', '[{"\u0000":1}]', '{"b":{"\u0000":1},"c":"'."\xff".'"}', '{"a":{"\u0000":1}', '[{"\u0000":1}, ]',
    '{"\u0000":1'."\xff".'}', '{"\u0000":1'."\x01".'}', '{"\u0000":1 x}', '{"\u0000":1]', '{"\u0000":1,}', '{"\u0000":1,"b":2}', '{"\u0000":1}x', '{"\u0000":1}]',
    '{"\u0000":[1'."\xff".']}', '{"\u0000":{"a":1]}', '{"\u0000":', '{"\u0000":1', '{"\u0000" 1}', '{"\u0000":}', '{"\u0000":[}', '{"\u0000":"\ud800"}',
    '{"a":1]', '[1}', '{"a":[1}}', '{"a"]', '{"a":1,"b"]', '[1,{"a":1,}]', '{"a":{"b":{"c":"d"}}}', '[[[]]]', '[{"a":[{"b":[]}]}]',
    '{"a":1}}', '{"a":1} ', "{\"a\":1}\n", '{"a\"b":1}', '{"\/":1}', '{"'.$bs.'u00e9":1}',
];
for ($n = 509; $n <= 513; ++$n) {
    $docs[] = str_repeat('{"a":', $n).'1'.str_repeat('}', $n);
    $docs[] = str_repeat('{"a":', $n).'{}'.str_repeat('}', $n);
    $docs[] = str_repeat('[', $n).'{}'.str_repeat(']', $n);
}
$scalars = ['0', '-1', '7', '1.5', '-2.5e3', '1e999', '9223372036854775808', 'true', 'false', 'null', '""', '"a"', '"é"', '"x\ny"'];
$docKeys = ['"a"', '"b"', '"0"', '"1"', '"01"', '"-1"', '""', '"é"', '"\u0000"', '"\u0000a"', '"a\u0000"'];

function generated(int $depth): string
{
    global $scalars, $docKeys;
    if ($depth > 0 && mt_rand(0, 2) === 0) {
        $items = [];
        $object = mt_rand(0, 2) !== 0;
        for ($n = mt_rand(0, 4); $n > 0; --$n) {
            $items[] = ($object ? $docKeys[mt_rand(0, \count($docKeys) - 1)].':' : '').generated($depth - 1);
        }

        return ($object ? '{' : '[').implode(',', $items).($object ? '}' : ']');
    }

    return $scalars[mt_rand(0, \count($scalars) - 1)];
}

$noise = ['', ' ', ',', ':', '[', ']', '{', '}', '"', '\\', "\x00", "\x01", "\xff", "\xc3", '-', '0', 'e', 'u', 'n'];
for ($n = 0; $n < 2000; ++$n) {
    $doc = generated(mt_rand(1, 5));
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

$decode = [];
foreach ($docs as $doc) {
    foreach ([true, false] as $bigint) {
        try {
            $value = json_decode($doc, false, 512, ($bigint ? JSON_BIGINT_AS_STRING : 0) | JSON_THROW_ON_ERROR);
            $result = ['ok', typed_value($value)];
        } catch (\JsonException $e) {
            $result = ['error', $e->getCode(), $e->getMessage()];
        }
        $decode[] = [base64_encode($doc), $bigint, $result];
    }
}

echo json_encode(['encode' => $encode, 'decode' => $decode], JSON_THROW_ON_ERROR, 1_000_000), "\n";
