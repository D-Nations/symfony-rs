<?php
// symfony/expression-language vectors (scripts/record.sh
// expression-language) for crates/expression-language/tests/data/php.json,
// as typed JSON (lib/typed.php). What the upstream tests leave open:
//
//   values     the operands of the operator tables, by index. Objects of
//              the recorder's classes are ["object", "ElObj", props] (a
//              MethodObject with the methods below on the Rust side) and
//              ["stringable", base64] (class Text).
//   binary     per operator: [left index, right index, outcome index (in
//              `outcomes`), warnings]
//              (exception messages and expressions are base64; warnings
//              are indices into `warnings`, PHP's warning texts in base64)
//              of BinaryNode::evaluate() on two ConstantNodes.
//   unary      per operator: [index, outcome index, warnings] of UnaryNode.
//   range      [left, right, outcome] of the `..` operator.
//   matches    [regexp, subject, outcome] of the `matches` operator.
//   lexer      [expression, tokens or throws]; a token is [type, value,
//              cursor].
//   cases      expressions through ExpressionLanguage: [expression, names
//              (typed), values (typed, null when not evaluated), parse
//              (string)$nodes or throws, dump or throws, compile or
//              throws, evaluate or throws].
//   compiled   the expressions upstream's tests eval() after compile():
//              [expression, names, compiled source or throws, outcome of
//              eval() with the variables given].
//
// The recorder loads the pinned sources from upstream/ (scripts/
// fetch-upstream.sh), so it also runs in the plain PHP image:
//   docker run --rm -v "$PWD":/work:ro -w /work php:8.4.27-cli-bookworm \
//       php oracle/recorders/expression_language.php

namespace Psr\Cache {
    if (!interface_exists(CacheItemInterface::class)) {
        interface CacheItemInterface
        {
            public function getKey(): string;
            public function get(): mixed;
            public function isHit(): bool;
            public function set(mixed $value): static;
            public function expiresAt(?\DateTimeInterface $expiration): static;
            public function expiresAfter(int|\DateInterval|null $time): static;
        }
        interface CacheItemPoolInterface
        {
            public function getItem(string $key): CacheItemInterface;
            public function getItems(array $keys = []): iterable;
            public function hasItem(string $key): bool;
            public function clear(): bool;
            public function deleteItem(string $key): bool;
            public function deleteItems(array $keys): bool;
            public function save(CacheItemInterface $item): bool;
            public function saveDeferred(CacheItemInterface $item): bool;
            public function commit(): bool;
        }
    }
}

namespace Symfony\Contracts\Service {
    if (!interface_exists(ResetInterface::class)) {
        interface ResetInterface
        {
            public function reset();
        }
    }
}

namespace {
    if (is_file('/demo/vendor/autoload.php')) {
        require '/demo/vendor/autoload.php';
    }
    require __DIR__.'/lib/typed.php';

    $source = dirname(__DIR__, 2).'/upstream/symfony/expression-language';
    spl_autoload_register(static function (string $class) use ($source): void {
        $prefix = 'Symfony\\Component\\ExpressionLanguage\\';
        if (str_starts_with($class, $prefix) && is_file($file = $source.'/'.str_replace('\\', '/', substr($class, strlen($prefix))).'.php')) {
            require_once $file;
        }
    }, true, true);

    /** A pool that never hits: every parse() is fresh. */
    final class NoCacheItem implements Psr\Cache\CacheItemInterface
    {
        public function __construct(private string $key) {}
        public function getKey(): string { return $this->key; }
        public function get(): mixed { return null; }
        public function isHit(): bool { return false; }
        public function set(mixed $value): static { return $this; }
        public function expiresAt(?\DateTimeInterface $expiration): static { return $this; }
        public function expiresAfter(int|\DateInterval|null $time): static { return $this; }
    }
    final class NoCache implements Psr\Cache\CacheItemPoolInterface
    {
        public function getItem(string $key): Psr\Cache\CacheItemInterface { return new NoCacheItem($key); }
        public function getItems(array $keys = []): iterable { return []; }
        public function hasItem(string $key): bool { return false; }
        public function clear(): bool { return true; }
        public function deleteItem(string $key): bool { return true; }
        public function deleteItems(array $keys): bool { return true; }
        public function save(Psr\Cache\CacheItemInterface $item): bool { return true; }
        public function saveDeferred(Psr\Cache\CacheItemInterface $item): bool { return true; }
        public function commit(): bool { return true; }
    }

    /** Public properties and three methods; the Rust tests build the same. */
    final class ElObj
    {
        public function __construct(public mixed $a = 1, public mixed $b = null, public mixed $c = 'x') {}
        public function name(): string { return 'el'; }
        public function nothing(): mixed { return null; }
        public function add(mixed $x, mixed $y): mixed { return $x + $y; }
        public function self(): self { return $this; }
    }

    final class Text implements \Stringable
    {
        public function __construct(private string $text) {}
        public function __toString(): string { return $this->text; }
    }

    enum ElEnum
    {
        case One;
        case Two;
    }

    enum ElBacked: string
    {
        case Bar = 'bar';
    }

    define('EL_CONST', 42);

    use Symfony\Component\ExpressionLanguage\ExpressionLanguage;
    use Symfony\Component\ExpressionLanguage\Lexer;
    use Symfony\Component\ExpressionLanguage\Node\BinaryNode;
    use Symfony\Component\ExpressionLanguage\Node\ConstantNode;
    use Symfony\Component\ExpressionLanguage\Node\UnaryNode;

    function enc(mixed $v): array
    {
        if ($v instanceof Text) {
            return ['stringable', base64_encode((string) $v)];
        }
        if (is_array($v)) {
            $pairs = [];
            foreach ($v as $k => $item) {
                $pairs[] = [typed_value($k), enc($item)];
            }

            return ['array', $pairs];
        }

        return typed_value($v);
    }

    $warnings = [];
    $warningTexts = [];
    set_error_handler(static function (int $no, string $message) use (&$warnings): bool {
        $warnings[] = $message;

        return true;
    });

    /** [outcome, warnings] of $f. */
    function run(callable $f): array
    {
        global $warnings;
        $warnings = [];
        try {
            $outcome = enc($f());
        } catch (\Throwable $e) {
            $outcome = ['throws', $e::class, base64_encode($e->getMessage())];
        }

        global $warningTexts;
        $indices = [];
        foreach ($warnings as $text) {
            $indices[] = $warningTexts[$text] ??= count($warningTexts);
        }

        return [$outcome, $indices];
    }

    /** The index of an outcome in the `outcomes` table. */
    function outcome_index(array $outcome): int
    {
        global $outcomes;
        $key = json_encode($outcome, JSON_THROW_ON_ERROR);

        return $outcomes[$key] ??= count($outcomes);
    }
    $outcomes = [];

    $values = [
        null, true, false, 0, 1, -1, 2, 7, -7, PHP_INT_MAX, PHP_INT_MIN,
        1.5, -2.5, 0.0, -0.0, 1e20, INF, NAN,
        '', '0', '1', '2', '10', 'abc', 'a', 'b', '1.5', ' 1', '1 ', '5 apples', '1e3', '0x1A', '00', '-0',
        [], [1], [1, 2], ['a' => 1], ['b' => 1], [1, 'x'], [2], ['0' => 1],
        new ElObj(), new ElObj(2), new \stdClass(), new Text('abc'), new Text('1'), ElEnum::One,
    ];
    $binaryOperators = ['or', '||', 'xor', 'and', '&&', '|', '^', '&', '==', '===', '!=', '!==', '<', '>', '>=', '<=',
        'not in', 'in', 'contains', 'starts with', 'ends with', '<<', '>>', '+', '-', '~', '*', '/', '%', '**'];
    $unaryOperators = ['not', '!', '-', '+', '~'];

    $out = ['values' => array_map('enc', $values), 'binary' => [], 'unary' => []];
    foreach ($binaryOperators as $op) {
        foreach ($values as $i => $left) {
            foreach ($values as $j => $right) {
                if (in_array($op, ['<<', '>>'], true) && is_int($right) && $right > 64) {
                    continue;
                }
                [$outcome, $w] = run(static fn () => (new BinaryNode($op, new ConstantNode($left), new ConstantNode($right)))->evaluate([], []));
                $out['binary'][$op][] = [$i, $j, outcome_index($outcome), $w];
            }
        }
    }
    foreach ($unaryOperators as $op) {
        foreach ($values as $i => $value) {
            [$outcome, $w] = run(static fn () => (new UnaryNode($op, new ConstantNode($value)))->evaluate([], []));
            $out['unary'][$op][] = [$i, outcome_index($outcome), $w];
        }
    }

    $bounds = [0, 1, 3, -2, 1.5, 2.5, -0.5, 'a', 'e', 'A', 'z', '1', '3', '10', '12', '', 'ab', '1.5', '2e0', null, true, false, [], 'x1'];
    $out['range'] = [];
    foreach ($bounds as $left) {
        foreach ($bounds as $right) {
            [$outcome, $w] = run(static fn () => (new BinaryNode('..', new ConstantNode($left), new ConstantNode($right)))->evaluate([], []));
            $out['range'][] = [enc($left), enc($right), $outcome];
        }
    }

    $regexps = ['/^[a-z]+$/', '/abc/i', '#a.c#s', '/^a$/m', '/\d+/', '/(a)(b)?/', '/[/', '/a', 'abc', '', '/a/Z', '/a/e', "/\xff/u",
        '/é/u', '/^.$/u', '/^.$/', '{a}', '(a)', '[a]', '<a>', '/a/x', '/ a b /x', '/a{2}/', '/\bfoo\b/', 5, null, 1.5, true, [], new Text('/x/')];
    $subjects = ['abc', 'ABC', '', 'a', "a\nb", '123', 'é', "\xff", null, 12, 1.5, true, [], new Text('xyz'), new ElObj()];
    $out['matches'] = [];
    foreach ($regexps as $regexp) {
        foreach ($subjects as $subject) {
            [$outcome, $w] = run(static fn () => (new BinaryNode('matches', new ConstantNode($subject), new ConstantNode($regexp)))->evaluate([], []));
            $out['matches'][] = [enc($regexp), enc($subject), $outcome];
        }
    }

    $lexer = new Lexer();
    $expressions = [
        '1_000', '1.', '1.e3', '.5e-3', '1e', '1e+', '1E-2_0', '0x10', '1..2', '1...2', '99999999999999999999', '9223372036854775807', '9223372036854775808',
        '"\x41\101\n\q\\\\"', "'it\\'s'", '"a\"b"', "'\\x'", '"\\0\\00\\000\\0000"', '"\\777"', "'\\xg'", '"unterminated', "'also",
        "a\tin\nb", 'a.not', '(not a)', 'notx', 'a in(b)', 'a in', 'not', 'a not in b', 'a not  in b', 'x matches y', 'xmatches y',
        'a starts with b', 'a starts  with b', 'a ends with(b)', 'a contains b', 'a ?? b', 'a?.b', 'a ? b : c', 'a?:b', '?', '??', '?.',
        'a === b !== c', 'a<=b>=c<d>e', 'a<<b>>c', 'a**b*c', '!a', '!!a', 'a||b&&c', 'a|b^c&d', 'a~b', '~a', '-a', '+a', 'a%b/c',
        "\xe9t\xe9", "\x7fa", 'a_b1', '_', '$a', '@', '#', '`', '\\', '"/* not */"', '/* a */ b /* c', '/**/', '/* */ /* */1', 'a/*b*/c',
        '[1, 2]', '{a: 1}', '(', ')', '[', ']', '{', '}', '(]', '[)', '{]', '((a)', 'a)', '', ' ', "\r\n\t\v\f",
    ];
    $out['lexer'] = [];
    foreach ($expressions as $expression) {
        try {
            $tokens = [];
            $stream = $lexer->tokenize($expression);
            $reflection = new \ReflectionProperty($stream, 'tokens');
            foreach ($reflection->getValue($stream) as $token) {
                $tokens[] = [$token->type, typed_value($token->value), $token->cursor];
            }
            $out['lexer'][] = [base64_encode($expression), ['tokens', $tokens]];
        } catch (\Throwable $e) {
            $out['lexer'][] = [base64_encode($expression), ['throws', $e::class, base64_encode($e->getMessage())]];
        }
    }

    $obj = new ElObj(['k' => 'v', 'n' => null], null, new ElObj(5));
    $vars = [
        'a' => 1, 'b' => 2, 'c' => 3, 's' => 'str', 'n' => null, 't' => true, 'f' => false, 'fl' => 1.5,
        'arr' => ['x' => 1, 'y' => ['z' => 2], 0 => 'zero', 'nul' => null], 'list' => [1, 2, 3], 'empty' => [],
        'obj' => $obj, 'std' => (object) ['p' => 1, 'q' => null], 'ao' => new \ArrayObject(['k' => 'v', 0 => 'z']),
        'text' => new Text('hello'), 'e' => ElEnum::One, 'zero' => 0, 'num' => '12', 're' => '/^s/',
    ];
    $cases = [
        // precedence and associativity
        '1 + 2 * 3', '(1 + 2) * 3', '2 ** 3 ** 2', '-2 ** 2', '(-2) ** 2', '2 ** -1', '10 - 2 - 3', '100 / 10 / 5', '7 % 3 * 2',
        '1 + 2 ~ 3 + 4', '"a" ~ 1 + 2', '1 < 2 == true', 'not 1 == 2', '!1 == 2', 'not true and false', 'not (true and false)',
        'true or false and false', 'true xor true or true', 'true || false && false', '1 | 2 ^ 3 & 4', '1 << 2 + 1', '1 .. 3 + 1',
        '-1 .. 2', '1 in [1, 2] == true', 'a + b * c', 'a ~ b ~ c', '-a ** 2', '- -a', '+-a', '~-a', 'not not a', '!!a',
        // ternaries and ??
        'a ? b : c', 'f ? b : c', 'a ?: c', 'f ?: c', 'n ?: "d"', 'a ? b', 'f ? b', 'a ? b ? c : 1 : 2', 'a ? f ? 1 : 2 : 3',
        'n ?? a', 'a ?? b', 'undefined ?? "d"', 'undefined ?? undefined2 ?? 3', 'n ?? n ?? "x"', 'a ?? b ? c : 1', 'f ?? 1',
        'arr["x"] ?? "d"', 'arr["missing"] ?? "d"', 'arr["nul"] ?? "d"', 'arr.x ?? "d"', 'arr["y"]["z"] ?? "d"', 'arr["y"]["q"] ?? "d"',
        'obj.a["k"] ?? "d"', 'obj.a["n"] ?? "d"', 'obj.b ?? "d"', 'obj.missing ?? "d"', 'obj.b.c ?? "d"', 'obj.c.a ?? "d"',
        'std.p ?? "d"', 'std.q ?? "d"', 'std.r ?? "d"', 'ao["k"] ?? "d"', 'ao["x"] ?? "d"', 's[0] ?? "d"', 's[5] ?? "d"', 's["0"] ?? "d"',
        's[-1] ?? "d"', 's["x"] ?? "d"', 'a[0] ?? "d"', 'n[0] ?? "d"', 'n.p ?? "d"', 'obj.nothing() ?? "d"', 'obj.nothing().x ?? "d"',
        'std["p"] ?? "d"', 'text.x ?? "d"', 'list[(a)] ?? "d"', 'arr[obj.b] ?? "d"', 'arr[n] ?? "d"',
        // null-safe
        'n?.a', 'n?.a()', 'n?.a.b', 'n?.a["b"]', 'n?.[0]', 'n?.[0].b', 'obj?.a', 'obj?.name()', 'obj.b?.c', 'obj.b?.c()', 'obj.nothing()?.x',
        'arr?.["x"]', 'list?.[1]', 'ao?.["k"]', 's?.[0]', 'a?.[0]', 'obj.b?.c.d', 'obj.b?.["x"].y', 'obj.nothing()?.x.y',
        // attribute and method access
        'obj.a', 'obj.c.a', 'obj.name()', 'obj.NAME()', 'obj.add(1, 2)', 'obj.add("1", 2.5)', 'obj.add(a, b)', 'obj.self().name()',
        'obj.missing', 'obj.missing()', 'std.p', 'std.missing', 'std.p()', 'e.name', 'e.value', 'e.name()', 'text.x', 'a.b', 'a.b()',
        's.length', 'n.a', 'n.a()', 'arr.x', 'arr.x()', 'list.0', 'obj.not', 'obj.matches', 'obj.in', 'obj.not in list',
        'arr["x"]', 'arr["y"]["z"]', 'arr[0]', 'arr["0"]', 'arr[0.5]', 'arr[true]', 'arr[n]', 'arr["missing"]', 'list[1]', 'list["1"]',
        'list[-1]', 'list[[]]', 'arr[obj]', 's[0]', 'a[0]', 'n[0]', 'obj[0]', 'ao["k"]', 'ao[0]', 'ao["missing"]', 'std["p"]',
        // literals
        '[1, "a", true, null]', '{a: 1, "b": 2, 3: 4, (a + 1): 5}', '{1: "a", 1: "b"}', '{"1": "a"}', '{(fl): 1}', '{(n): 1}',
        '{(arr): 1}', '{(t): 1}', '[]', '{}', '[[1], {a: [2]}]', '{a: 1,}', '[1,]', '"\\u00e9"', '"a\\tb"', "'\\\\n'", '0.1 + 0.2',
        '1e3', '1_000.5', '.5', '1.', '99999999999999999999', 'TRUE', 'False', 'NULL', 'nULL',
        // operators on variables
        'num + 1', 'num ~ 1', 'num == 12', 'num === 12', 'num < 9', '"abc" < "abd"', '"10" < "9"', '"10" < "9a"', 'list == [1, 2, 3]',
        'list === [1, 2, 3]', '[2, 1] == [1 => 1, 0 => 2]', 'list < [1, 2, 4]', 'a in list', '"1" in list', 'a not in list',
        's contains "t"', 's starts with "s"', 's ends with "r"', 's matches re', 's matches "/^S/i"', 'num matches "/^\\\\d+$/"',
        '"abc" matches "/b/" ~ "x"', 'text ~ "!"', 'text == "hello"', 'text contains "ell"', 'obj ~ "x"', 'arr ~ "x"', 'a / zero',
        'a % zero', 'a / "0"', 'a / "abc"', 'a / ""', 'a / "0x"', 'a / n', 'a / 0.0', 'fl % 1', '7 / 2', '6 / 3', '-7 % 3', '7 % -3',
        '2 ** 63', '2 ** 62', '(-2) ** 63', '(-2) ** 64', '0 ** 0', '0 ** -1', '9223372036854775807 + 1', '-9223372036854775807 - 2',
        '9223372036854775807 * 2', '-(-9223372036854775807 - 1)', '1 << 64', '1 << 63', '-1 >> 64', '1 << -1', '"a" | "b"', '"ab" & "c"',
        '~"a"', '~1.5', '~n', '-s', '+num', '-"1.5"', '-n', '-t', 'not s', '!arr', '!empty',
        // functions
        'min(1, 2, 3)', 'max(1, 2, 3)', 'min(list)', 'max([])', 'min()', 'min(1)', 'max("a", 0)', 'min("a", 0)', 'max("10", "9")',
        'max([1, 2], [1, 3])', 'min(1, "1")', 'max(1, "1")', 'min(n, 0)', 'max(t, 2)', 'max(fl, 1)', 'min(-0.0, 0)',
        'constant("EL_CONST")', 'constant("\\\\EL_CONST")', 'constant("PHP_INT_MAX")', 'constant("UNDEFINED_C")', 'constant("ElEnum::One")',
        'constant("Missing::X")', 'constant("ElEnum::Three")', 'constant()', 'constant(1)', 'constant(arr)', 'enum("ElEnum::Two")',
        'enum("ElBacked::Bar").value', 'enum("ElBacked::Bar").name', 'enum("EL_CONST")', 'enum("Missing::X")', 'enum("ElEnum::Three")',
        'enum("ElEnum::One") == e', 'enum("ElEnum::One") === e', 'enum("ElEnum::One") == enum("ElEnum::Two")', 'enum()', 'nope()',
        'Min(1, 2)', 'min (1, 2)', 'min(1, 2,)',
        // names and errors
        'a +', 'a b', '(a', 'a)', 'a[1', 'a.', 'a.1', 'a."b"', 'a..b', 'a.[0]', 'a?.[]', '{a b}', '{a: 1 b: 2}', '{+: 1}', '[1 2]',
        'foo', 'fo', 'ab', 'foo(', 'min(1 2)', 'a ? b :', 'a ??', '1 = 2', '1 => 2', '!', '@', 'a ! b', 'a not b', 'a not in', '',
        '   ', '/* only a comment */', 'a // b', 'a ** ** b', 'a === === b', 'a in [', '"a" "b"', '1 2', 'true false', 'a.b.c.d.e',
    ];
    $el = new ExpressionLanguage(new NoCache());
    $names = array_keys($vars);
    $out['cases'] = [];
    foreach ($cases as $expression) {
        $row = [base64_encode($expression)];
        try {
            $parsed = $el->parse($expression, $names);
            $row[] = ['string', base64_encode((string) $parsed->getNodes())];
            try {
                $row[] = ['string', base64_encode($parsed->getNodes()->dump())];
            } catch (\Throwable $e) {
                $row[] = ['throws', $e::class, base64_encode($e->getMessage())];
            }
        } catch (\Throwable $e) {
            $row[] = ['throws', $e::class, base64_encode($e->getMessage())];
            $row[] = ['throws', $e::class, base64_encode($e->getMessage())];
        }
        $row[] = run(static fn () => $el->compile($expression, $names))[0];
        $row[] = run(static fn () => $el->evaluate($expression, $vars))[0];
        $out['cases'][] = $row;
    }
    $out['vars'] = [];
    foreach ($vars as $k => $v) {
        $out['vars'][] = [$k, enc($v)];
    }

    // The expressions upstream's tests eval() after compile(), with the
    // variables they use.
    $enum = 'enum("ElEnum::One")';
    $compiled = [
        ['enum("PHP_VERSION")', [], []],
        ['enum("ElEnum::One")', [], []],
        ['enum("ElBacked::Bar")', [], []],
        [$enum.' == foo', ['foo'], ['foo' => null]],
        ['foo == '.$enum, ['foo'], ['foo' => null]],
        [$enum.'.name ~ v', ['v'], ['v' => 'bar']],
        ['['.$enum.', v]', ['v'], ['v' => 'bar']],
        ['false and foo', ['foo' => 'foo'], ['foo' => 'foo']],
        ['false && foo', ['foo' => 'foo'], ['foo' => 'foo']],
        ['true || foo', ['foo' => 'foo'], ['foo' => 'foo']],
        ['true or foo', ['foo' => 'foo'], ['foo' => 'foo']],
        ['regexp matches "x"', ['regexp'], ['regexp' => 'this is not a regexp']],
    ];
    $nullSafe = new ElObj();
    foreach (['foo?.bar', 'foo?.bar()', 'foo.bar?.baz', 'foo.bar?.baz()', 'foo["bar"]?.baz', 'foo["bar"]?.baz()', 'foo.nothing()?.baz',
        'foo.nothing()?.baz()', 'foo?.[0]', 'foo?.[0].bar', 'foo?.bar.baz', 'foo?.bar["baz"]', 'foo?.bar["baz"]["qux"]',
        'foo?.bar["baz"]["qux"].quux', 'foo?.bar["baz"]["qux"].quux()', 'foo?.bar().baz', 'foo?.bar()["baz"]', 'foo?.bar()["baz"]["qux"]',
        'foo?.bar()["baz"]["qux"].quux', 'foo?.bar()["baz"]["qux"].quux()'] as $expression) {
        $foo = match ($expression) {
            'foo.bar?.baz', 'foo.bar?.baz()' => (object) ['bar' => null],
            'foo["bar"]?.baz', 'foo["bar"]?.baz()' => ['bar' => null],
            'foo.nothing()?.baz', 'foo.nothing()?.baz()' => $nullSafe,
            default => null,
        };
        $compiled[] = [$expression, ['foo' => 'foo'], ['foo' => $foo]];
    }
    foreach ([
        ['foo?.bar.baz', (object) ['bar' => null]],
        ['foo?.bar["baz"]', (object) ['bar' => null]],
        ['foo?.bar["baz"].qux.quux', (object) ['bar' => ['baz' => null]]],
        ['foo?.[0].bar', [null]],
    ] as [$expression, $foo]) {
        $compiled[] = [$expression, ['foo' => 'foo'], ['foo' => $foo]];
    }
    foreach ([
        ['bar ?? "default"', null], ['foo.bar ?? "default"', null], ['foo.bar.baz ?? "default"', (object) ['bar' => null]],
        ['foo.bar ?? foo.baz ?? "default"', null], ['foo[0] ?? "default"', []], ['foo["bar"] ?? "default"', ['bar' => null]],
        ['foo["baz"] ?? "default"', ['bar' => null]], ['foo["bar"]["baz"] ?? "default"', ['bar' => null]],
        ['foo["bar"].baz ?? "default"', ['bar' => null]], ['foo.nothing().baz ?? "default"', $nullSafe],
        ['foo.bar.baz.bam ?? "default"', (object) ['bar' => null]], ['foo?.bar?.baz?.qux ?? "default"', (object) ['bar' => null]],
        ['foo[123][456][789] ?? "default"', [123 => []]], ['foo.bar ?? "default"', 'foo'], ['foo.bar ?? "default"', 123],
        ['foo.bar.baz ?? "default"', (object) ['bar' => 'foo']], ['foo["bar"] ?? "default"', 'foo'], ['foo[0] ?? "default"', 123],
        ['foo[0]["bar"] ?? "default"', ['foo']],
    ] as [$expression, $foo]) {
        $compiled[] = [$expression, ['foo' => 'foo'], ['foo' => $foo]];
    }
    $out['compiled'] = [];
    foreach ($compiled as [$expression, $names, $variables]) {
        $source = run(static fn () => $el->compile($expression, $names))[0];
        $outcome = ['throws', 'none', 'not compiled'];
        $evalWarnings = [];
        if ('string' === $source[0]) {
            [$outcome, $evalWarnings] = run(static function () use ($el, $expression, $names, $variables) {
                extract($variables);

                return eval('return '.$el->compile($expression, $names).';');
            });
        }
        $vars2 = [];
        foreach ($variables as $k => $v) {
            $vars2[] = [$k, enc($v)];
        }
        $out['compiled'][] = [base64_encode($expression), enc($names), $vars2, $source, $outcome, $evalWarnings];
    }

    restore_error_handler();
    $out['warnings'] = array_map('base64_encode', array_keys($warningTexts));
    $out['outcomes'] = array_map(static fn (string $json) => json_decode($json, true, 512, JSON_THROW_ON_ERROR), array_keys($outcomes));
    echo json_encode($out, JSON_UNESCAPED_SLASHES | JSON_THROW_ON_ERROR), "\n";
}
