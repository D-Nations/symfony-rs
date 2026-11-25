<?php
// ParameterBag vectors (scripts/record.sh di-parameter-bag): the real
// Symfony\Component\DependencyInjection\ParameterBag\ParameterBag on
// edge cases its tests leave open, replayed by
// crates/dependency-injection/tests/parameter_bag_vectors.rs.
//
// The output holds the bags' parameters once ("bags"); each case names its
// bag by index, whether resolve() ran first, the call
// (resolveValue, resolveString, get or resolve) and its argument, and the
// outcome as typed JSON (lib/typed.php): the value (for resolve, all()
// afterwards) or ["throws", class, base64(message)] (messages may hold
// any bytes).
//
// Inputs: every whitespace kind inside and around placeholders (space,
// \t \n \v \f \r, NUL, 0x85, 0xA0), a trailing "\n" after a whole-value
// placeholder, runs of %, placeholder keys in nested arrays, ints, floats,
// bools, null and arrays inside strings, circular chains, missing names
// with alternatives and the nested-array hint, and non-UTF-8 bytes.
//
// Output: crates/dependency-injection/tests/data/parameter_bag.json

ini_set('display_errors', 'stderr');
require __DIR__.'/lib/di_autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\DependencyInjection\ParameterBag\ParameterBag;

function outcome(callable $f): array
{
    try {
        return typed_value($f());
    } catch (\Throwable $e) {
        return ['throws', $e::class, base64_encode($e->getMessage())];
    }
}

$base = [
    'foo' => 'bar',
    'int' => 12,
    'neg' => -3,
    'zero' => 0,
    'float' => 1.5,
    'negfloat' => -0.5,
    'bigfloat' => 1e25,
    'smallfloat' => 0.1 + 0.2,
    'intfloat' => 3.0,
    'nan' => NAN,
    'inf' => INF,
    'true' => true,
    'false' => false,
    'null' => null,
    'array' => ['a' => 1, 'b' => '%foo%'],
    'emptyarray' => [],
    'empty' => '',
    'pct' => '50%',
    'pcts' => '%%',
    'pctfoo' => '%%foo%%',
    'ref' => '%foo%',
    'ref2' => '%ref%',
    'refint' => '%int%',
    'refarray' => '%array%',
    'nested.arr' => ['x' => ['y' => 1]],
    'nested.str' => 'flat',
    'deep' => ['k' => '%foo%', '%foo%' => ['%int%' => 'x%ref2%']],
    'numstr' => '12',
    'floatstr' => '1.5',
    'numeric' => ' 12',
    'spaced key' => 'sp',
    'tab	key' => 'tab',
    "nbsp\xA0key" => 'nbsp',
    "nel\x85key" => 'nel',
    'utf' => 'é',
    'bin' => "\xFF\xFE",
    "bin\xFFname" => 'binvalue',
    'circ1' => '%circ2%',
    'circ2' => '%circ1%',
    'self' => '%self%',
    'selfstr' => 'a%selfstr%b',
    'loop1' => 'x%loop2%',
    'loop2' => 'y%loop3%',
    'loop3' => 'z%loop1%',
    'chainA' => 'x%chainB%',
    'chainB' => 'y%chainC%',
    'chainC' => 'z',
    'nl' => "%foo%\n",
    'arrayref' => ['%circ1%'],
    'missingref' => '%nope%',
    'Foo' => 'upper',
    'foo.bar' => 'dotted',
    'a' => 'short',
];

$spaces = [' ', "\t", "\n", "\v", "\f", "\r", "\0", "\x85", "\xA0"];

$strings = [
    '', '%', '%%', '%%%', '%%%%', '%%%%%', '%%foo%%', '%%foo%', '%foo%%', '%%%foo%%%',
    '%foo%', '%foo%%foo%', 'a%foo%b', '%foo%a%foo%', '100%', '100%%', '%foo', 'foo%',
    "%foo%\n", "%foo%\n\n", "%foo%\r\n", "%foo%\r", "\n%foo%", ' %foo%', '%foo% ', "%foo%\t",
    "%int%\n", "%array%\n", "%null%\n", "%foo%\nx",
    '%int%', 'x%int%', '%neg%', 'x%neg%', '%zero%', 'x%zero%',
    '%float%', 'x%float%', 'x%negfloat%', 'x%bigfloat%', 'x%smallfloat%', 'x%intfloat%', 'x%nan%', 'x%inf%',
    '%true%', 'x%true%', '%false%', 'x%false%', '%null%', 'x%null%',
    '%array%', 'x%array%', '%emptyarray%', 'x%emptyarray%',
    '%empty%', 'x%empty%y', '%pct%', 'x%pct%', '%pcts%', 'x%pcts%', '%pctfoo%', 'x%pctfoo%',
    '%ref%', 'x%ref%', '%ref2%', 'x%ref2%', '%refint%', 'x%refint%', '%refarray%', 'x%refarray%',
    '%deep%', 'x%deep%', '%numstr%', 'x%numstr%', '%floatstr%', 'x%floatstr%', '%numeric%', 'x%numeric%',
    '%spaced key%', 'x%spaced key%', "%tab\tkey%", "%nbsp\xA0key%", "x%nbsp\xA0key%", "%nel\x85key%", "x%nel\x85key%",
    '%utf%', 'x%utf%', '%bin%', 'x%bin%', "\xFF%foo%", "%foo%\xFF", "%bin\xFFname%", "x%bin\xFFname%", "%\xFF%", "x%\xFF%",
    '%circ1%', 'x%circ1%', '%self%', 'x%self%', '%selfstr%', 'x%selfstr%', '%loop1%', 'x%loop1%',
    '%chainA%', 'x%chainA%', '%nl%', 'x%nl%', '%arrayref%', '%missingref%', 'x%missingref%',
    '%missing%', 'x%missing%', '%fooo%', 'x%fooo%', '%fo%', '%Foo%', '%FOO%', '%a%', '%b%',
    '%nested.arr.x%', '%nested.arr.x.y%', 'x%nested.arr.x%', '%nested.str.x%', '%nested%', '%nested.missing%',
    '%foo.bar.baz%', '%array.a%', '%0%', '%00%', '%12%', '%-1%', '%env(FOO)%', 'x%env(FOO)%y',
    '%foo%%int%', '%foo%%%int%', '%%%foo%', '%foo%%%', 'a%%b%%c', '%a%%b%', '%%a%%',
];
foreach ($spaces as $s) {
    $strings[] = "%{$s}foo%";
    $strings[] = "%foo{$s}%";
    $strings[] = "%fo{$s}o%";
    $strings[] = "{$s}%foo%";
    $strings[] = "%foo%{$s}";
    $strings[] = "x%foo{$s}%y";
    $strings[] = "%{$s}%";
    $strings[] = "%%{$s}%%";
}

$arrays = [
    [],
    ['%foo%'],
    ['%foo%' => '%int%'],
    ['%int%' => 'v'],
    ['%float%' => 'v'],
    ['%negfloat%' => 'v'],
    ['%true%' => 'v'],
    ['%false%' => 'v'],
    ['%null%' => 'v'],
    ['%array%' => 'v'],
    ['%numstr%' => 'v'],
    ['%floatstr%' => 'v'],
    ['%numeric%' => 'v'],
    ['%%' => 'v'],
    ['%%foo%%' => '%%foo%%'],
    ['x%foo%' => ['%foo%' => ['%foo%' => '%ref2%']]],
    ['12' => '%foo%', 3 => '%int%', '%foo%' => 'a', 'bar' => 'b'],
    ['%foo%' => 1, 'bar' => 2],
    ['bar' => 1, '%foo%' => 2],
    ['%missing%' => 1],
    ['k' => '%missing%'],
    ['k' => ['%circ1%']],
    ['%circ1%' => 1],
    ["%foo%\n" => 1],
    ["%foo\v%" => 1],
    [1, '%int%', 2.5, true, null, ['%true%']],
    ['%bin%' => "%bin\xFFname%"],
    ['%empty%' => 'v'],
    ['%zero%' => 'v', '0' => 'w'],
];

$scalars = [0, 1, -1, 1.5, -0.0, true, false, null];

$names = [
    'foo', 'Foo', 'FOO', 'fooo', 'fo', 'f', 'missing', '', '0', 'a', 'b', 'nested', 'nested.arr', 'nested.arr.x',
    'nested.arr.x.y', 'nested.str.x', 'nested.missing', 'nested.', '.nested', 'foo.bar', 'foo.bar.baz', 'foo.baz',
    'array.a', 'array.a.b', 'x.y.z', '...', 'refx', 'circ', 'loop', 'deep.k', 'spaced key', 'spaced', 'tab', 'é',
    'utf', 'ut', "bin\xFFname", "bin\xFF", 'chain', 'chainD', 'null', 'emptyarray',
];

$cases = [];
$bagsSeen = [];
$add = static function (array $parameters, bool $resolved, string $op, mixed $arg) use (&$cases, &$bagsSeen): void {
    $typed = typed_value($parameters);
    $key = serialize($typed);
    $bagsSeen[$key] ??= [\count($bagsSeen), $typed];
    $bag = new ParameterBag($parameters);
    $result = outcome(static function () use ($bag, $resolved, $op, $arg) {
        if ($resolved) {
            $bag->resolve();
        }

        return match ($op) {
            'resolveValue' => $bag->resolveValue($arg),
            'resolveString' => $bag->resolveString($arg),
            'get' => $bag->get($arg),
            'resolve' => (function () use ($bag) { $bag->resolve(); return $bag->all(); })(),
        };
    });
    $cases[] = [
        'bag' => $bagsSeen[$key][0],
        'resolved' => $resolved,
        'op' => $op,
        'arg' => typed_value($arg),
        'result' => $result,
    ];
};

foreach ($strings as $string) {
    $add($base, false, 'resolveValue', $string);
    if ('' !== $string) {
        $add($base, false, 'resolveString', $string);
    }
}
foreach ($arrays as $array) {
    $add($base, false, 'resolveValue', $array);
}
foreach ($scalars as $scalar) {
    $add($base, false, 'resolveValue', $scalar);
}
foreach ($names as $name) {
    $add($base, false, 'get', $name);
}

// A resolved bag: values come back escaped.
$clean = array_filter(
    $base,
    static fn ($k) => !\in_array($k, ['circ1', 'circ2', 'self', 'selfstr', 'loop1', 'loop2', 'loop3', 'arrayref', 'missingref', 'nan'], true),
    \ARRAY_FILTER_USE_KEY,
);
foreach (['%foo%', 'x%foo%', '%pct%', 'x%pct%', '%pcts%', 'x%pcts%', '%pctfoo%', 'x%pctfoo%', '%array%', '%deep%', '%%', 'a%%b', '%ref2%', 'x%ref2%', '%nl%', 'x%int%', 'x%float%', '%missing%', "%foo%\n"] as $string) {
    $add($clean, true, 'resolveValue', $string);
}
foreach (['%foo%' => '%pct%', 'k' => ['%pcts%']] as $key => $value) {
    $add($clean, true, 'resolveValue', [$key => $value]);
}

// resolve() on whole bags.
$bags = [
    $clean,
    ['a' => '%b%', 'b' => '%c%', 'c' => 'end'],
    ['a' => '%%', 'b' => 'x%%y', 'c' => '%a%', 'd' => 'x%a%'],
    ['a' => '%b%', 'b' => '%a%'],
    ['a' => 'x%b%', 'b' => 'y%a%'],
    ['a' => '%missing%'],
    ['a' => ['k' => '%missing%']],
    ['a' => ['%missing%' => 1]],
    ['a' => 'x%b%', 'b' => ['array']],
    ['a' => 'x%b%', 'b' => null],
    ['a' => 'x%b%', 'b' => true],
    ['a' => ['%b%' => '%b%'], 'b' => 'key'],
    ['a' => ['%b%' => 1], 'b' => null],
    ['a' => "%b%\n", 'b' => [1]],
    ['a' => "%b\x85%", "b\x85" => 'nel'],
    ['a' => "%b\xA0%", "b\xA0" => 'nbsp'],
    ['a' => "%b\v%", "b\v" => 'vt'],
    ['a' => '%b%', 'b' => 1.5, 'c' => 'x%b%'],
    ['a' => 'x%b%', 'b' => 1e100],
    ['a' => 'x%b%', 'b' => -0.0],
    ['a' => 'x%b%', 'b' => 0.1],
    ['a' => 'x%b%', 'b' => PHP_INT_MAX],
    ['nested.a' => ['b' => 1], 'c' => '%nested.a.b%'],
    ['foo' => 1, 'c' => '%fooo%'],
];
foreach ($bags as $bag) {
    $add($bag, false, 'resolve', null);
}

echo json_encode([
    'bags' => array_column(array_values($bagsSeen), 1),
    'cases' => $cases,
], \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR), "\n";
