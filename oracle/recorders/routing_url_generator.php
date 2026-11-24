<?php
// UrlGenerator::generate() answers for what its tests leave open, for
// crates/routing/tests/data/url_generator.json (scripts/record.sh
// routing-url-generator): generated collections (a route with inline
// settings, defaults, requirements, host and schemes; sometimes localized
// variants with _canonical_route and aliases), contexts, strictness,
// default locales, and one generate() call with parameters of every PHP
// type (objects with public properties, with __toString(), with
// neither), each reference type. Then getRelativePath() on generated
// paths, and the GHSA-72xp-p242-47p9 check: the pre-fix pattern accepts
// the test's input, the pinned code throws.
//
// A collection entry is ["route", name, path, defaults, requirements,
// options, host, schemes] or ["alias", name, target]; strings are base64,
// values typed (lib/typed.php), a Text object (with __toString()) is
// ["stringable", base64]. A result is ["ok", base64 url, logged] or
// ["throws", class, base64 message]; logged lists the logger's
// [level, message, given] records.

error_reporting(E_ALL);
ini_set('memory_limit', '1G');

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Psr\Log\AbstractLogger;
use Symfony\Component\Routing\Generator\UrlGenerator;
use Symfony\Component\Routing\RequestContext;
use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;

mt_srand(20261124);

set_error_handler(static fn () => true);

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

class Plain
{
}

class Recorder extends AbstractLogger
{
    public array $records = [];

    public function log($level, string|\Stringable $message, array $context = []): void
    {
        $given = $context['given'] ?? null;
        $this->records[] = [$level, base64_encode((string) $message), base64_encode(\is_scalar($given) || null === $given || $given instanceof \Stringable ? (string) $given : get_debug_type($given))];
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
    if ($v instanceof \stdClass) {
        $pairs = [];
        foreach (get_object_vars($v) as $k => $item) {
            $pairs[] = [typed_value($k), encode($item)];
        }

        return ['object', 'stdClass', $pairs];
    }

    return typed_value($v);
}

function pick(array $a): mixed
{
    return $a[mt_rand(0, \count($a) - 1)];
}

function some(array $pool, int $max): array
{
    $out = [];
    for ($n = mt_rand(0, $max); $n > 0; --$n) {
        $out[pick(array_keys($pool))] = null;
    }

    return array_keys($out);
}

$paths = ['/', '', '/testing', '/{foo}', '/{foo}/{bar}', '/a/{foo}.{_format}', '/{foo}{bar}', '/{x}/../{y}', '/{!foo}', '/{foo<\d+>?5}',
    '/blog/{page<\d+>?1}', '/{_locale}/x', '/{slug}/{page}', '/dir/./{foo}', '//{foo}', '/{foo}/{bar}/{baz}', '/é/{foo}', '/{foo}/b(ar/{bar}',
    "/@:[]()*'\"+,;=!|~%/{foo}", '/{foo}.{!_format}', '/get{foo}', '/{foo}/', '/x/{foo?}', '/{bar}-{foo}', '/{foo}/{_fragment}'];
$requirements = ['\d+', '.+', 'en|fr|vi|de', '[a-z]+', '\pL+', '.+(?=/x)', '(?<!^).+', 'a#b', '(?=a)a', '.*', '[^/]+', 'x|', '(?:b)ar|baz', '.+(?=/b\(ar/)'];
$scalars = [null, '', '0', 'x', 'bar', 0, 1, 5, 1.5, true, false, '/evil.com', 'a/b', '../..', '.', '..', 'é', "\x80", ' ', '?#&=+', 'en', 'fr',
    'en_GB', '123', 42, -1, 1.0, 0.1, 'vi', 'abc', '1', 'a#b', '%2F', 'html'];
$names = ['q', '_fragment', '_query', '_locale', 'foo', 'bar', 'baz', '123', 'a b', 'page', 'slug', 'x', 'y', '_format', 'domain', 'sub'];
$hosts = ['', '', '', '{sub}.example.com', 'example.com', '{foo}.{domain}', 'EXAMPLE.com', 'localhost'];
$schemes = [[], [], [], ['http'], ['https'], ['Ftp', 'https'], ['file'], ['https', 'http']];

function value(int $depth): mixed
{
    global $scalars, $names;
    $r = mt_rand(0, 19);
    if ($depth > 0 && 0 === $r) {
        $a = [];
        for ($n = mt_rand(0, 3); $n > 0; --$n) {
            $a[pick($names)] = value($depth - 1);
        }

        return $a;
    }
    if ($depth > 0 && 1 === $r) {
        $o = new \stdClass();
        for ($n = mt_rand(0, 2); $n > 0; --$n) {
            $o->{pick(['a', 'b', 'c'])} = value($depth - 1);
        }

        return $o;
    }
    if (2 === $r) {
        return new Text(pick(['t', '', 'a/b', '5']));
    }
    if (3 === $r && $depth < 2) {
        return new Plain();
    }
    if (4 === $r && $depth > 0) {
        return [pick($scalars), pick($scalars)];
    }

    return pick($scalars);
}

function route_entry(string $name, string $path): array
{
    global $requirements, $hosts, $schemes, $names;
    $defaults = [];
    foreach (some(array_flip($names), 3) as $n) {
        $defaults[$n] = value(1);
    }
    $reqs = [];
    foreach (some(array_flip($names), 2) as $n) {
        $reqs[$n] = pick($requirements);
    }

    return ['route', $name, $path, $defaults, $reqs, mt_rand(0, 4) ? [] : ['utf8' => true], pick($hosts), pick($schemes)];
}

function build(array $entries): RouteCollection
{
    $routes = new RouteCollection();
    foreach ($entries as $e) {
        if ('route' === $e[0]) {
            $routes->add($e[1], new Route($e[2], $e[3], $e[4], $e[5], $e[6], $e[7]));
        } else {
            $routes->addAlias($e[1], $e[2]);
        }
    }

    return $routes;
}

function encode_entry(array $e): array
{
    if ('alias' === $e[0]) {
        return ['alias', base64_encode($e[1]), base64_encode($e[2])];
    }

    return ['route', base64_encode($e[1]), base64_encode($e[2]), encode($e[3]), encode($e[4]), encode($e[5]), base64_encode($e[6]), array_map('base64_encode', $e[7])];
}

$cases = [];
while (\count($cases) < 3000) {
    $entries = [route_entry('test', pick($paths))];
    if (!mt_rand(0, 2)) {
        foreach (some(['en' => 1, 'fr' => 1, 'en_GB' => 1, 'de_AT_x' => 1], 3) as $locale) {
            $e = route_entry("test.$locale", pick($paths));
            $e[3]['_locale'] = $locale;
            if (mt_rand(0, 3)) {
                $e[3]['_canonical_route'] = pick(['test', 'test', 'test', 'other']);
            }
            $e[4]['_locale'] = $locale;
            $entries[] = $e;
        }
        if (!mt_rand(0, 2)) {
            $entries[] = ['alias', pick(['test.de', 'test.fr', 'alias']), pick(['test', 'test.en', 'missing'])];
        }
        if (!mt_rand(0, 5)) {
            array_shift($entries);
        }
    }

    try {
        build($entries);
    } catch (\Throwable) {
        continue;
    }

    $context = [pick(['', '/app.php', '/a b']), pick(['', 'localhost', 'example.com', 'x.example.com']), pick(['', 'http', 'https', 'ftp', 'http', 'http']),
        pick([80, 80, 8080]), pick([443, 443, 8443]), pick(['/', '/a/b/', '/a/b/c', '', '/x/y']), []];
    if (!mt_rand(0, 3)) {
        foreach (some(array_flip($names), 2) as $n) {
            $context[6][$n] = value(0);
        }
    }
    $strict = pick([true, true, true, false, null]);
    $defaultLocale = pick([null, null, 'en', 'fr', 'en_GB', '']);
    $parameters = [];
    // most route variables are given, mostly with plain values
    foreach ($entries as $e) {
        if ('route' === $e[0]) {
            preg_match_all('/\{!?(\w+)/', $e[2].' '.$e[6], $m);
            foreach ($m[1] as $variable) {
                if (mt_rand(0, 9) < 8) {
                    $parameters[$variable] = mt_rand(0, 3) ? pick(['x', 'bar', '5', 'en', 'fr', 'a/b', 'abc', '123', 'vi', 'html', 'example.org']) : value(0);
                }
            }
        }
    }
    foreach (some(array_flip($names), 3) as $n) {
        $parameters[$n] = '_query' === $n && mt_rand(0, 3) ? [pick($names) => value(1), pick($names) => value(1)] : value(2);
    }
    $name = pick(['test', 'test', 'test', 'test', 'test', 'test', 'test.en', 'alias', 'missing']);
    $referenceType = mt_rand(0, 3);

    $logger = new Recorder();
    $requestContext = new RequestContext($context[0], 'GET', $context[1], $context[2], $context[3], $context[4], $context[5]);
    $requestContext->setParameters($context[6]);
    $generator = new UrlGenerator(build($entries), $requestContext, $logger, $defaultLocale);
    $generator->setStrictRequirements($strict);
    try {
        $result = ['ok', base64_encode($generator->generate($name, $parameters, $referenceType)), $logger->records];
    } catch (\Throwable $e) {
        $result = ['throws', $e::class, base64_encode($e->getMessage())];
    }

    $cases[] = [
        array_map('encode_entry', $entries),
        [base64_encode($context[0]), base64_encode($context[1]), base64_encode($context[2]), $context[3], $context[4], base64_encode($context[5]), encode($context[6])],
        $strict,
        null === $defaultLocale ? null : base64_encode($defaultLocale),
        base64_encode($name),
        encode($parameters),
        $referenceType,
        $result,
    ];
}

$segments = ['', 'a', 'b', 'c', '.', '..', 'x:y', ':z', 'é', '%20'];
$relative = [];
for ($n = 0; $n < 2000; ++$n) {
    $p = [];
    foreach ([0, 1] as $i) {
        $s = mt_rand(0, 3) ? '/' : '';
        for ($m = mt_rand(0, 4); $m > 0; --$m) {
            $s .= pick($segments) . (mt_rand(0, 3) ? '/' : '');
        }
        $p[$i] = $s;
    }
    if (!mt_rand(0, 9)) {
        $p[1] = $p[0];
    }
    $relative[] = [base64_encode($p[0]), base64_encode($p[1]), base64_encode(UrlGenerator::getRelativePath($p[0], $p[1]))];
}

// GHSA-72xp-p242-47p9: the pre-fix check was preg_match('#^'.$requirement.'$#i', $value).
$advisory = [
    'pre_fix_matches' => 1 === preg_match('#^en|fr|vi|de$#i', '/evil.com'),
    'pinned' => (function () {
        $routes = new RouteCollection();
        $routes->add('test', new Route('/{_locale}/blog', [], ['_locale' => 'en|fr|vi|de']));
        try {
            return (new UrlGenerator($routes, new RequestContext('/app.php')))->generate('test', ['_locale' => '/evil.com']);
        } catch (\Throwable $e) {
            return $e::class;
        }
    })(),
];

echo json_encode(['generate' => $cases, 'relative' => $relative, 'advisory' => $advisory], \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
