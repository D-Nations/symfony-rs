<?php
// HeaderBag and ResponseHeaderBag answers for what their tests leave open,
// for crates/http-foundation/tests/data/header_bags.json (scripts/record.sh
// http-foundation): 400 seeded scripts of set/remove/replace/directive/
// cookie calls, each followed by the bag's all(), keys(), string form,
// allPreserveCase() (responses) and Cache-Control readers. The clock is
// fixed as ClockMock fixes it. Texts are base64.

namespace Symfony\Component\HttpFoundation {
    const RECORDER_NOW = 1700000000;

    function time(): int
    {
        return RECORDER_NOW;
    }

    function gmdate(string $format, ?int $timestamp = null): string
    {
        return \gmdate($format, $timestamp ?? RECORDER_NOW);
    }

    function strtotime(string $datetime, ?int $baseTimestamp = null): int|false
    {
        return \strtotime($datetime, $baseTimestamp ?? RECORDER_NOW);
    }
}

namespace {
    error_reporting(E_ALL);
    set_error_handler(static fn () => true);

    require '/demo/vendor/autoload.php';

    use Symfony\Component\HttpFoundation\HeaderBag;
    use Symfony\Component\HttpFoundation\ResponseHeaderBag;

    mt_srand(20261123);
    $pick = static fn (array $a) => $a[mt_rand(0, \count($a) - 1)];
    $names = ['foo', 'Foo', 'FOO_BAR', 'foo-bar', 'Cache-Control', 'cache_control', 'ETag', 'Last-Modified', 'Expires', 'Date', 'Set-Cookie', 'set_cookie', '123', '0', '-1', '1.5', 'X-A', 'x-b', 'a b', 'Content-Type', ''];
    $values = [null, '', '0', 'bar', 'public', 'private', 'no-cache', 'max-age=10', 'public, max-age=100', 's-maxage=5', 'no-store, must-revalidate', 'max-age="0"', 'a=1, b, c="x y"', 'Thu, 23 Mar 2017 09:15:12 GMT', 'x'];
    $cookies = ['foo=bar', 'a=1; path=/x', 'b=2; domain=d.com; secure', '0=zero', 'c=3; Max-Age=60', 'd', 'e=; expires=Fri, 20 May 2011 15:25:52 GMT'];
    $directives = ['public', 'private', 'max-age', 's-maxage', 'no-cache', 'must-revalidate', '0', '10', 'Z'];
    $b = static fn (?string $s) => null === $s ? null : base64_encode($s);
    $pairs = static function (array $a) use ($b) {
        $out = [];
        foreach ($a as $k => $v) {
            $out[] = [\is_int($k) ? $k : $b($k), array_map($b, $v)];
        }

        return $out;
    };

    $scripts = [];
    for ($n = 0; $n < 400; ++$n) {
        $response = 0 === $n % 2;
        $ops = [];
        $initial = [];
        for ($i = mt_rand(0, 3); $i > 0; --$i) {
            $initial[] = [$b($pick($names)), 0 === mt_rand(0, 4) ? [$b($pick($values)), $b($pick($values))] : $b($pick($values))];
        }
        for ($i = mt_rand(1, 8); $i > 0; --$i) {
            $op = $pick($response ? ['set', 'set', 'set', 'remove', 'add', 'del', 'replace', 'cookie', 'cookie', 'clear', 'rmcookie'] : ['set', 'set', 'set', 'remove', 'add', 'del', 'replace']);
            $ops[] = match ($op) {
                'set' => ['set', $b($pick($names)), 0 === mt_rand(0, 4) ? [$b($pick($values)), $b($pick($values))] : $b($pick($values)), 0 !== mt_rand(0, 2)],
                'remove' => ['remove', $b($pick($names))],
                'add' => ['add', $pick($directives), 0 === mt_rand(0, 1) ? true : $b($pick(['10', 'x y', '', '0']))],
                'del' => ['del', $pick($directives)],
                'replace' => ['replace', [[$b($pick($names)), $b($pick($values))]]],
                'cookie' => ['cookie', $b($pick($cookies))],
                'clear' => ['clear', $pick(['foo', 'a', '0'])],
                'rmcookie' => ['rmcookie', $pick(['foo', 'a', '0', 'b']), $pick(['/', '/x', null]), $pick([null, 'd.com'])],
            };
        }

        try {
            $toKeys = static function (array $list) use (&$toKeys) {
                $out = [];
                foreach ($list as [$k, $v]) {
                    $out[base64_decode($k)] = \is_array($v) ? array_map(static fn ($x) => null === $x ? null : base64_decode($x), $v) : (null === $v ? null : base64_decode($v));
                }

                return $out;
            };
            $bag = $response ? new ResponseHeaderBag($toKeys($initial)) : new HeaderBag($toKeys($initial));
            foreach ($ops as $op) {
                match ($op[0]) {
                    'set' => $bag->set(base64_decode($op[1]), \is_array($op[2]) ? array_map(static fn ($x) => null === $x ? null : base64_decode($x), $op[2]) : (null === $op[2] ? null : base64_decode($op[2])), $op[3]),
                    'remove' => $bag->remove(base64_decode($op[1])),
                    'add' => $bag->addCacheControlDirective($op[1], true === $op[2] ? true : base64_decode($op[2])),
                    'del' => $bag->removeCacheControlDirective($op[1]),
                    'replace' => $bag->replace($toKeys($op[1])),
                    'cookie' => $bag->setCookie(Symfony\Component\HttpFoundation\Cookie::fromString(base64_decode($op[1]))),
                    'clear' => $bag->clearCookie($op[1]),
                    'rmcookie' => $bag->removeCookie($op[1], $op[2], $op[3]),
                };
            }
            $directiveValues = [];
            foreach ($directives as $d) {
                $v = $bag->getCacheControlDirective($d);
                $directiveValues[] = [$bag->hasCacheControlDirective($d), \is_string($v) ? $b($v) : $v];
            }
            $result = [
                'all' => $pairs($bag->all()),
                'keys' => array_map(static fn ($k) => \is_int($k) ? $k : $b($k), $bag->keys()),
                'string' => $b((string) $bag),
                'count' => \count($bag),
                'directives' => $directiveValues,
                'preserve' => $response ? $pairs($bag->allPreserveCase()) : null,
                'without_cookies' => $response ? $pairs($bag->allPreserveCaseWithoutCookies()) : null,
            ];
        } catch (\Throwable $e) {
            $result = ['throws' => [$e::class, base64_encode($e->getMessage())]];
        }
        $scripts[] = ['response' => $response, 'initial' => $initial, 'ops' => $ops, 'result' => $result];
    }

    echo json_encode(['now' => Symfony\Component\HttpFoundation\RECORDER_NOW, 'directives' => $directives, 'scripts' => $scripts], JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR), "\n";
}
