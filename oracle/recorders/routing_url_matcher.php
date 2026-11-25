<?php
// UrlMatcher::match() and RedirectableUrlMatcher::match() answers for what
// their tests leave open, for crates/routing/tests/data/url_matcher.json
// (scripts/record.sh routing-url-matcher): generated collections (static
// segments, variables with inline requirements, defaults and mappings,
// important and adjacent variables, trailing slashes, UTF-8 routes, hosts
// with variables, schemes, methods, priorities, _canonical_route) and
// requests derived from their paths (variables filled, encoded bytes,
// trailing slashes added or removed, other paths), in contexts of several
// methods, schemes and hosts. Each request is matched by both matchers;
// the redirecting one's redirect() returns ['_redirect' => [$path,
// $route, $scheme]]. Then edge collections: two routes whose paths differ
// only by a trailing slash, with schemes and methods, in both orders.
//
// A collection entry is [name, path, defaults, requirements, options,
// host, schemes, methods, priority]; strings are base64, values typed
// (lib/typed.php). A result is ["ok", typed attributes] or ["throws",
// class, base64 message, base64 allowed methods].

error_reporting(E_ALL);
ini_set('memory_limit', '1G');

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Routing\Exception\MethodNotAllowedException;
use Symfony\Component\Routing\Matcher\RedirectableUrlMatcher;
use Symfony\Component\Routing\Matcher\UrlMatcher;
use Symfony\Component\Routing\RequestContext;
use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;

mt_srand(20261125);

set_error_handler(static fn () => true);

final class Redirecting extends RedirectableUrlMatcher
{
    public function redirect(string $path, string $route, ?string $scheme = null): array
    {
        return ['_redirect' => [$path, $route, $scheme]];
    }
}

function pick(array $items): mixed
{
    return $items[mt_rand(0, \count($items) - 1)];
}

function chance(int $percent): bool
{
    return mt_rand(1, 100) <= $percent;
}

const WORDS = ['foo', 'bar', 'a', 'blog', 'x.y', 'admin', 'api', 'p'];
const UTF8_WORDS = ['é', 'bär', 'è'];
const REQUIREMENTS = ['\d+', '[a-z]+', '.+', '.*', 'foo|bar', '(a|b)', '[^/]+', '\w+', 'en|fr', '[^.]+', '/(?!/)'];
const VALUES = ['1', '123', 'abc', 'foo', 'bar', 'x.y', 'a/b', '%2F', 'é', '%C3%A9', '', 'foo%23', 'EN', 'en', '0', 'a-b', '%FF', ' ', '%0A', 'foo/'];

/** A random path pattern and the variable names it uses. */
function path_pattern(bool $utf8): array
{
    if (chance(8)) {
        return ['/', []];
    }
    $segments = [];
    $vars = [];
    $n = mt_rand(1, 4);
    for ($i = 0; $i < $n; ++$i) {
        $name = 'v'.\count($vars);
        switch (mt_rand(0, 9)) {
            case 0:
            case 1:
            case 2:
                $segments[] = $utf8 && chance(30) ? pick(UTF8_WORDS) : pick(WORDS);
                break;
            case 3:
                $segments[] = '{'.$name.'<'.pick(REQUIREMENTS).'>}';
                $vars[] = $name;
                break;
            case 4:
                $segments[] = '{'.$name.'?'.pick(['x', '', 'd1', 'null']).'}';
                $vars[] = $name;
                break;
            case 5:
                $segments[] = '{!'.$name.'}';
                $vars[] = $name;
                break;
            case 6:
                $segments[] = '{'.$name.':entity'.(chance(50) ? '.slug' : '').'}';
                $vars[] = $name;
                break;
            case 7:
                $b = 'v'.(\count($vars) + 1);
                $segments[] = '{'.$name.'}'.pick(['.', '-', '']).'{'.$b.'}';
                $vars[] = $name;
                $vars[] = $b;
                break;
            default:
                $segments[] = pick(WORDS).'{'.$name.'}';
                $vars[] = $name;
        }
    }
    if (chance(15)) {
        $segments[] = '{_format}';
        $vars[] = '_format';
    }

    return ['/'.implode('/', $segments).(chance(25) ? '/' : ''), $vars];
}

/** A random route entry. */
function route_entry(int $index): array
{
    $utf8 = chance(15);
    [$path, $vars] = path_pattern($utf8);
    $defaults = [];
    $requirements = [];
    foreach ($vars as $var) {
        if (chance(30)) {
            $defaults[$var] = pick(['x', 'aaa', '', '0', null, 'default']);
        }
        if (chance(25)) {
            $requirements[$var] = pick(REQUIREMENTS);
        }
    }
    if (chance(5)) {
        $defaults['_canonical_route'] = pick(['canonical', null, 'r0']);
    }
    if (chance(10)) {
        $defaults['_controller'] = 'App\\Controller::'.pick(WORDS);
    }
    $host = '';
    if (chance(20)) {
        $host = pick(['example.com', '{sub}.example.com', '{h}', 'app.{domain}', 'EN.example.com']);
        if (str_contains($host, '{domain}') && chance(50)) {
            $requirements['domain'] = pick(['.+', '[a-z]+', '(?<name>.+)']);
        }
        if (str_contains($host, '{sub}') && chance(50)) {
            $requirements['sub'] = pick(['en|fr', 'EN|FR']);
        }
    }
    $schemes = chance(20) ? pick([['https'], ['http'], ['https', 'http'], ['FTP', 'HTTPS']]) : [];
    $methods = chance(35) ? pick([['GET'], ['POST'], ['PUT', 'DELETE'], ['get', 'head'], ['HEAD'], ['GET', 'OPTIONS'], ['post']]) : [];
    $options = $utf8 ? ['utf8' => true] : (chance(5) ? ['utf8' => false] : []);

    return ['r'.$index.(chance(5) ? '$é|' : ''), $path, $defaults, $requirements, $options, $host, $schemes, $methods, chance(15) ? mt_rand(-1, 2) : 0];
}

/** A request path derived from a route path, or another one. */
function request_path(array $entries): string
{
    if (chance(10)) {
        return pick(['/', '', '0', '//', '/nothing/here', '/foo%0a', '/%2F', '/a//b']);
    }
    $path = pick($entries)[1];
    $path = preg_replace_callback('#\{!?([\w\x80-\xFF]+)(?::[\w.]+)?(?:<[^>]*>)?(?:\?[^}]*)?\}#', static fn () => pick(VALUES), $path);
    if (chance(20)) {
        $path = str_ends_with($path, '/') ? rtrim($path, '/') : $path.'/';
    }
    if (chance(5)) {
        $path = strtoupper($path);
    }
    if (chance(10)) {
        $path = rawurlencode($path);
        $path = str_replace('%2F', '/', $path);
    }

    return $path;
}

function outcome(callable $f): array
{
    try {
        return ['ok', typed_value($f())];
    } catch (\Throwable $e) {
        $allowed = $e instanceof MethodNotAllowedException ? array_map('base64_encode', array_values($e->getAllowedMethods())) : [];

        return ['throws', $e::class, base64_encode($e->getMessage()), $allowed];
    }
}

$cases = [];
for ($c = 0; $c < 2500; ++$c) {
    $entries = [];
    $collection = new RouteCollection();
    $count = mt_rand(1, 6);
    for ($i = 0; $i < $count; ++$i) {
        $entry = route_entry($i);
        try {
            $route = new Route($entry[1], $entry[2], $entry[3], $entry[4], $entry[5], $entry[6], $entry[7]);
        } catch (\Throwable) {
            continue;
        }
        $collection->add($entry[0], $route, $entry[8]);
        $entries[] = $entry;
    }
    if (!$entries) {
        continue;
    }
    $requests = [];
    for ($r = 0; $r < 4; ++$r) {
        $path = request_path($entries);
        $method = pick(['GET', 'GET', 'HEAD', 'POST', 'PUT', 'OPTIONS']);
        $scheme = pick(['http', 'http', 'https']);
        $host = pick(['localhost', 'example.com', 'en.example.com', 'foo', 'app.example.com', 'app']);
        $baseUrl = chance(10) ? '/sub/front.php' : '';
        $context = new RequestContext($baseUrl, $method, $host, $scheme);
        $requests[] = [
            base64_encode($path), $method, $host, $scheme, base64_encode($baseUrl),
            outcome(static fn () => (new UrlMatcher(clone_collection($collection), clone $context))->match($path)),
            outcome(static fn () => (new Redirecting(clone_collection($collection), clone $context))->match($path)),
        ];
    }
    $cases[] = [
        array_map(static fn (array $e) => [
            base64_encode($e[0]), base64_encode($e[1]), typed_value($e[2]), typed_value($e[3]), typed_value($e[4]),
            base64_encode($e[5]), array_map('base64_encode', $e[6]), array_map('base64_encode', $e[7]), $e[8],
        ], $entries),
        $requests,
    ];
}

// Edges: two routes whose paths differ only by a trailing slash, with and
// without schemes and methods, in both orders, for each request variant.
foreach (['/foo', '/foo/{bar}', '/{a}'] as $base) {
    $variants = [$base, $base.'/'];
    foreach ([[], ['https']] as $schemesX) {
        foreach ([[], ['https'], ['POST'], ['GET']] as $y) {
            foreach ([[0, 1], [1, 0]] as $order) {
                $defs = [
                    ['x', $variants[$order[0]], $schemesX, []],
                    ['y', $variants[$order[1]], \in_array('https', $y, true) ? $y : [], \in_array('https', $y, true) ? [] : $y],
                ];
                $entries = [];
                $collection = new RouteCollection();
                foreach ($defs as [$name, $path, $schemes, $methods]) {
                    $collection->add($name, new Route($path, [], [], [], '', $schemes, $methods));
                    $entries[] = [$name, $path, [], [], [], '', $schemes, $methods, 0];
                }
                $requests = [];
                foreach (['/foo', '/foo/', '/foo/baz', '/foo/baz/', '/x', '/x/'] as $path) {
                    foreach ([['GET', 'http'], ['HEAD', 'https'], ['POST', 'http'], ['GET', 'https']] as [$method, $scheme]) {
                        $context = new RequestContext('', $method, 'localhost', $scheme);
                        $requests[] = [
                            base64_encode($path), $method, 'localhost', $scheme, base64_encode(''),
                            outcome(static fn () => (new UrlMatcher(clone_collection($collection), clone $context))->match($path)),
                            outcome(static fn () => (new Redirecting(clone_collection($collection), clone $context))->match($path)),
                        ];
                    }
                }
                $cases[] = [
                    array_map(static fn (array $e) => [
                        base64_encode($e[0]), base64_encode($e[1]), typed_value($e[2]), typed_value($e[3]), typed_value($e[4]),
                        base64_encode($e[5]), array_map('base64_encode', $e[6]), array_map('base64_encode', $e[7]), $e[8],
                    ], $entries),
                    $requests,
                ];
            }
        }
    }
}

/** A copy whose routes are not compiled yet, as a fresh matcher sees them. */
function clone_collection(RouteCollection $collection): RouteCollection
{
    $copy = new RouteCollection();
    foreach ($collection->all() as $name => $route) {
        $copy->add($name, clone $route, $collection->getPriority($name) ?? 0);
    }

    return $copy;
}

echo json_encode($cases, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
