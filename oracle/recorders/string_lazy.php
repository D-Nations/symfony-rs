<?php
// symfony/string's LazyString where LazyStringTest leaves it open
// (scripts/record.sh string), for crates/string/tests/lazy_string_vectors.rs:
// what each kind of return value becomes, the TypeError texts with each
// kind of callable's name, retries after a throw, memoization, the
// fromCallable() argument check, resolve() and isStringable() on every
// kind of value, json_encode() and serialize(). No strict_types: upstream's
// file has none, so its string return type coerces scalars.
//
// Each case is [name, result]: a result is ["string", s], or
// ["throws", class, message] with this file's path replaced by FILE.

require '/demo/vendor/autoload.php';

use Symfony\Component\String\LazyString;

enum Suit
{
    case Hearts;
}

final class Named
{
    public function method(): array
    {
        return [];
    }

    public function text(): string
    {
        return 'text';
    }

    public function __invoke(): array
    {
        return [];
    }

    public static function make(): string
    {
        return 'made';
    }
}

final class Text implements \Stringable
{
    public function __construct(private string $s)
    {
    }

    public function __toString(): string
    {
        return $this->s;
    }
}

final class Invokable
{
    public int $calls = 0;

    public function __invoke(): string
    {
        return 'invoked '.++$this->calls;
    }

    public function other(string $a = 'x'): string
    {
        return 'other '.$a;
    }
}

function returns_array(): array
{
    return [1];
}

$cases = [];
$result = static function (callable $f): array {
    try {
        return ['string', (string) $f()];
    } catch (\Throwable $e) {
        return ['throws', $e::class, str_replace(__FILE__, 'FILE', $e->getMessage())];
    }
};
$case = static function (string $name, callable $f) use (&$cases, $result): void {
    $cases[] = [$name, $result($f)];
};

// What the callback's return value becomes.
$returns = [
    'int' => 42, 'negative int' => -7, 'int max' => PHP_INT_MAX, 'float' => 123.456, 'float 0.1+0.2' => 0.1 + 0.2,
    'float 1e100' => 1e100, 'float 1e15' => 1e15, 'float 1/3' => 1 / 3, 'float 1e14' => 1e14, 'float -1.5e-7' => -1.5e-7, 'float -0.0' => -0.0, 'float 1.0' => 1.0, 'float INF' => INF,
    'float NAN' => NAN, 'true' => true, 'false' => false, 'null' => null, 'empty string' => '', 'string' => 'abc',
    'array' => [], 'list' => [1, 2], 'stdClass' => new \stdClass(), 'Stringable' => new Text('stringable'),
    'enum' => Suit::Hearts, 'LazyString' => LazyString::fromCallable(static fn () => 'inner'),
];
foreach ($returns as $name => $value) {
    $case("returns $name", static fn () => LazyString::fromCallable(static fn () => $value));
}

// The callable's name in the TypeError.
$case('string callable returning array', static fn () => LazyString::fromCallable('returns_array'));
$case('static method string returning string', static fn () => LazyString::fromCallable('Named::make'));
$case('array callable returning array', static fn () => LazyString::fromCallable([new Named(), 'method']));
$case('static array callable', static fn () => LazyString::fromCallable([Named::class, 'make']));
$case('first-class method callable returning array', static fn () => LazyString::fromCallable((new Named())->method(...)));
$case('first-class function callable returning array', static fn () => LazyString::fromCallable(returns_array(...)));
$case('invokable object returning array', static fn () => LazyString::fromCallable(new Named()));
$case('lazy callable returning array', static fn () => LazyString::fromCallable([static fn () => new Named()]));
$case('lazy callable with method returning array', static fn () => LazyString::fromCallable([static fn () => new Named(), 'method']));
$case('lazy callable with method returning string', static fn () => LazyString::fromCallable([static fn () => new Named(), 'text']));
$case('lazy callable invokable', static fn () => LazyString::fromCallable([static fn () => new Invokable()]));
$case('lazy callable with arguments', static fn () => LazyString::fromCallable([static fn () => new Invokable(), 'other'], 'arg'));
$case('lazy callable with an undefined method', static fn () => LazyString::fromCallable([static fn () => new Invokable(), 'missing']));
$case('lazy callable factory returning a string', static fn () => LazyString::fromCallable([static fn () => 'strtoupper']));
$case('callable with arguments', static fn () => LazyString::fromCallable('sprintf', '%s-%s', 'a', 'b'));
$case('callable with arguments returning array', static fn () => LazyString::fromCallable('str_split', 'ab'));
$case('Closure::fromCallable of a function returning array', static fn () => LazyString::fromCallable(\Closure::fromCallable('returns_array')));
$case('exception from the callback', static fn () => LazyString::fromCallable(static function () {
    throw new \RuntimeException('boom');
}));
$case('TypeError from the callback', static fn () => LazyString::fromCallable(static function () {
    throw new \TypeError('own');
}));

// The fromCallable() argument check.
foreach ([
    'empty array' => [], 'string list' => ['foo'], 'ints' => [1, 2], 'object and method' => [new \stdClass(), 'x'],
    'null' => [null], 'nested array' => [[1], 'x'], 'three ints' => [1, 2, 3], 'Closure and int' => [static fn () => new Invokable(), 5],
    'bool and float' => [true, 1.5],
] as $name => $callback) {
    $case("fromCallable $name", static fn () => LazyString::fromCallable($callback));
}

// Memoization, retries, and a second cast after a TypeError.
$count = 0;
$memo = LazyString::fromCallable(static function () use (&$count) {
    return 'call '.++$count;
});
$case('memoized first', static fn () => $memo);
$case('memoized second', static fn () => $memo);
$tries = 0;
$retry = LazyString::fromCallable(static function () use (&$tries) {
    if (1 === ++$tries) {
        throw new \RuntimeException('first try');
    }

    return 'try '.$tries;
});
$case('retry first', static fn () => $retry);
$case('retry second', static fn () => $retry);
$case('retry third', static fn () => $retry);
$calls = 0;
$bad = LazyString::fromCallable(static function () use (&$calls) {
    ++$calls;

    return [];
});
$case('type error first', static fn () => $bad);
$case('type error second', static fn () => $bad);
$cases[] = ['type error callback calls', ['string', (string) $calls]];
$invokable = new Invokable();
$lazy = LazyString::fromCallable([static fn () => $invokable]);
$case('lazy factory memoized first', static fn () => $lazy);
$case('lazy factory memoized second', static fn () => $lazy);
$factoryCalls = 0;
$throwing = LazyString::fromCallable([static function () use (&$factoryCalls) {
    ++$factoryCalls;

    return new class {
        private int $n = 0;

        public function __invoke(): string
        {
            if (1 === ++$this->n) {
                throw new \RuntimeException('invoke failed');
            }

            return 'invoke '.$this->n;
        }
    };
}]);
$case('lazy factory, invoke throws first', static fn () => $throwing);
$case('lazy factory, invoke throws second', static fn () => $throwing);
$cases[] = ['lazy factory calls', ['string', (string) $factoryCalls]];

// fromStringable() and resolve().
foreach ([
    'string' => 'abc', 'int' => 123, 'float' => 123.456, 'true' => true, 'false' => false, 'float 1e100' => 1e100,
    'float 0.1+0.2' => 0.1 + 0.2, 'float 1/3' => 1 / 3, 'float 1e14' => 1e14, 'float 0.00001' => 0.00001, 'float -0.0' => -0.0,
    'Stringable' => new Text('text'), 'LazyString' => LazyString::fromCallable(static fn () => 'lazy'),
    'null' => null, 'array' => [], 'stdClass' => new \stdClass(), 'enum' => Suit::Hearts,
] as $name => $value) {
    $case("fromStringable $name", static fn () => LazyString::fromStringable($value));
    $case("resolve $name", static fn () => LazyString::resolve($value));
}

$out = ['cases' => $cases, 'isStringable' => [], 'json' => [], 'serialize' => []];
foreach ([
    'string' => 'a', 'empty string' => '', 'int' => 0, 'float' => 0.0, 'true' => true, 'false' => false, 'null' => null,
    'array' => [], 'stdClass' => new \stdClass(), 'Stringable' => new Text('x'), 'LazyString' => LazyString::fromStringable('x'),
    'enum' => Suit::Hearts, 'Closure' => static fn () => 'x', 'resource' => \STDIN,
] as $name => $value) {
    $out['isStringable'][] = [$name, LazyString::isStringable($value)];
}
foreach (['abc' => 'abc', 'unicode' => 'é/"', 'empty' => ''] as $name => $value) {
    $out['json'][] = [$name, json_encode(LazyString::fromCallable(static fn () => $value)), json_encode([LazyString::fromStringable($value)])];
    $s = serialize(LazyString::fromCallable(static fn () => $value));
    $u = unserialize($s);
    $out['serialize'][] = [$name, $s, (string) $u, $u instanceof LazyString];
}

echo json_encode($out, JSON_PRETTY_PRINT | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE | JSON_THROW_ON_ERROR), "\n";
