<?php
// `$a == $b` answers for crates/php-value/tests/data/loose_equals.json
// (scripts/record.sh php-value): every ordered pair of a pool of values
// (null, bools, ints, floats, numeric and other strings, arrays, objects
// with and without __toString()). An entry is [a, b, a == b, warned],
// values typed as lib/typed.php does; a Text object (which implements
// __toString()) is ["stringable", base64].

error_reporting(E_ALL);

require __DIR__ . '/lib/typed.php';

class Text
{
    public function __construct(private string $text)
    {
    }

    public function __toString(): string
    {
        return $this->text;
    }
}

function encode(mixed $v): array
{
    if ($v instanceof Text) {
        return ['stringable', base64_encode((string) $v)];
    }
    if (\is_array($v)) {
        $pairs = [];
        foreach ($v as $k => $item) {
            $pairs[] = [typed_value($k), encode($item)];
        }

        return ['array', $pairs];
    }

    return typed_value($v);
}

$o1 = new stdClass();
$o1->a = 1;
$o2 = new stdClass();
$o2->a = '1';
$o3 = new stdClass();
$o3->a = 2;
$o4 = new stdClass();
$o4->a = 1;
$o4->b = 2;

$pool = [
    null, true, false,
    0, 1, -1, 2, 10, 42, \PHP_INT_MAX, \PHP_INT_MIN,
    0.0, -0.0, 1.0, 1.5, -1.5, 10.0, 0.1, 1e20, 9.2233720368547758E+18, \NAN, \INF, -\INF,
    '', ' ', '0', '1', '-1', '2', '10', '1.0', '1.5', '1e1', '1E1', '01', '+1', ' 1', '1 ', "1\n", "\t1", '1 1', '.5', '5.', '0.0', '-0', '00',
    '0x1A', '1abc', 'abc', 'ABC', 'null', 'true', 'false', 'Array', 'bar', 'INF', 'NAN',
    '9223372036854775807', '9223372036854775808', '9223372036854775809', '-9223372036854775808', '-9223372036854775809', '9.2233720368547758E+18', '1e19', '09223372036854775808', '1e1000', '-1e1000',
    [], [0], ['0'], [''], [null], [false], [1, 2], [2, 1], [1 => 2, 0 => 1], ['a' => 1], ['a' => '1'], ['a' => 1, 'b' => 2], ['b' => 2, 'a' => 1],
    [[1]], [['1']], ['foo', 'bar'], [1 => 'bar', 0 => 'foo'],
    new stdClass(), $o1, $o2, $o3, $o4, new Text('bar'), new Text('1'), new Text(''), new Text('1.0'),
];

$warnings = 0;
set_error_handler(static function () use (&$warnings) {
    ++$warnings;

    return true;
});

$out = [];
foreach ($pool as $a) {
    foreach ($pool as $b) {
        $warnings = 0;
        try {
            $equal = $a == $b;
        } catch (\Throwable $e) {
            $equal = $e::class;
        }
        $out[] = [encode($a), encode($b), $equal, $warnings > 0];
    }
}

echo json_encode($out, \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
