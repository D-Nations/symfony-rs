<?php
// RequestContext::fromRequest() and matchRequest() answers for
// crates/routing/tests/data/from_request.json (scripts/record.sh
// routing-from-request), at the pinned symfony/routing and
// symfony/http-foundation v8.1.11.
//
// The requests are those of crates/http-foundation/tests/data/request.json
// (http_foundation_request.php): each case's configuration and input,
// replayed here, so routing sees the hosts, ports, proxies and script
// names that recorder generates. Then "edges": SERVER_PORT and QUERY_STRING
// values of every type, with and without HTTPS, for the int and ?string
// parameters fromRequest() passes them to.
//
// Each request answers, in this order and on the same Request object
// (its getters cache, and getHost() throws only once):
//   1. fromRequest() on a clone of one of CONTEXTS (picked by case index):
//      the context's state, or the exception;
//   2. UrlMatcher::matchRequest() on ROUTES with that context;
//   3. RedirectableUrlMatcher::matchRequest() (redirect() returns the path,
//      route and scheme it was given) with that context;
//   4. the matcher's context after the two calls (it must be unchanged).
// A result is ["ok", typed value] or ["throws", class, message (base64)].
// A TypeError's message loses PHP's ", called in <file> on line <n>",
// which names the PHP file and is not part of the port's message.
// A context is [baseUrl, pathInfo, method, host, scheme, httpPort,
// httpsPort, queryString], typed.

error_reporting(E_ALL);
ini_set('memory_limit', '2G');

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\Routing\Matcher\RedirectableUrlMatcher;
use Symfony\Component\Routing\Matcher\UrlMatcher;
use Symfony\Component\Routing\RequestContext;
use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;

set_error_handler(static fn () => true);

/** The contexts fromRequest() starts from: [baseUrl, method, host, scheme, httpPort, httpsPort, path, queryString]. */
const CONTEXTS = [
    ['', 'GET', 'localhost', 'http', 80, 443, '/', ''],
    ['', 'POST', 'example.com', 'http', 8080, 8443, '/', ''],
    ['/base', 'PUT', 'ctx.test', 'https', 81, 444, '/p', 'q=1'],
];

function routes(): RouteCollection
{
    $routes = new RouteCollection();
    $routes->add('home', new Route('/'));
    $routes->add('foo_get', new Route('/foo', [], [], [], '', [], ['GET']));
    $routes->add('foo_post', new Route('/foo', [], [], [], '', [], ['POST', 'PUT']));
    $routes->add('slash', new Route('/foo/bar/'));
    $routes->add('secure', new Route('/secure', [], [], [], '', ['https']));
    $routes->add('plain', new Route('/plain', [], [], [], '', ['http']));
    $routes->add('host', new Route('/h', [], [], [], '{sub}.example.com'));
    $routes->add('trusted', new Route('/t', [], [], [], 'trusted.test'));
    $routes->add('var', new Route('/a/{x}', ['x' => 'd'], ['x' => '[^/]+']));
    $routes->add('index', new Route('/index.php/{rest}', [], ['rest' => '.*']));

    return $routes;
}

final class Redirecting extends RedirectableUrlMatcher
{
    public function redirect(string $path, string $route, ?string $scheme = null): array
    {
        return ['_redirect' => $path, '_redirect_route' => $route, '_redirect_scheme' => $scheme];
    }
}

function message(\Throwable $e): string
{
    $m = $e->getMessage();
    if ($e instanceof \TypeError) {
        $m = preg_replace('/, called in .* on line \d+$/s', '', $m);
    }

    return $m;
}

function result(callable $f): array
{
    try {
        return ['ok', typed_value($f())];
    } catch (\Throwable $e) {
        return ['throws', $e::class, base64_encode(message($e))];
    }
}

function state(RequestContext $c): array
{
    return [$c->getBaseUrl(), $c->getPathInfo(), $c->getMethod(), $c->getHost(), $c->getScheme(), $c->getHttpPort(), $c->getHttpsPort(), $c->getQueryString()];
}

function context(int $i): RequestContext
{
    [$baseUrl, $method, $host, $scheme, $httpPort, $httpsPort, $path, $queryString] = CONTEXTS[$i % \count(CONTEXTS)];

    return new RequestContext($baseUrl, $method, $host, $scheme, $httpPort, $httpsPort, $path, $queryString);
}

function apply_config(array $config): void
{
    [$proxies, $set, $hosts, $override, $allowed, $remote] = $config;
    if (null === $remote) {
        unset($_SERVER['REMOTE_ADDR']);
    } else {
        $_SERVER['REMOTE_ADDR'] = $remote;
    }
    Request::setTrustedProxies($proxies, $set);
    Request::setTrustedHosts($hosts);
    (new \ReflectionProperty(Request::class, 'httpMethodParameterOverride'))->setValue(null, $override);
    Request::setAllowedHttpMethodOverride($allowed);
}

/** The typed JSON of http_foundation_request.php, back to PHP values. */
function untyped(array $t): mixed
{
    return match ($t[0]) {
        'null' => null,
        'bool', 'int' => $t[1],
        'float' => unpack('E', hex2bin($t[1]))[1],
        'string' => base64_decode($t[1], true),
        'array' => (static function () use ($t) {
            $a = [];
            foreach ($t[1] as [$k, $v]) {
                $a[untyped($k)] = untyped($v);
            }

            return $a;
        })(),
    };
}

function answers(Request $request, int $i, RouteCollection $routes): array
{
    $from = result(static fn () => state((clone context($i))->fromRequest($request)));
    $url = new UrlMatcher($routes, context($i));
    $redirecting = new Redirecting($routes, context($i));

    return [
        $from,
        result(static fn () => $url->matchRequest($request)),
        result(static fn () => $redirecting->matchRequest($request)),
        typed_value([state($url->getContext()), state($redirecting->getContext())]),
    ];
}

$routes = routes();
$data = json_decode(file_get_contents('/work/crates/http-foundation/tests/data/request.json'), true, 512, \JSON_THROW_ON_ERROR);

$cases = [];
foreach ($data['cases'] as $i => [$config, $kind, $input]) {
    apply_config($config);
    if ('server' === $kind) {
        $request = new Request(untyped($input[2]), untyped($input[1]), [], [], [], untyped($input[0]));
    } else {
        try {
            $request = Request::create(base64_decode($input[0], true), $input[1], untyped($input[2]), [], [], untyped($input[3]));
        } catch (\Throwable) {
            continue; // request.json pins the exception
        }
    }
    $cases[] = [$i, answers($request, $i, $routes)];
}

apply_config([[], -1, [], false, null, null]);
$ports = ['8080', ' 8080', '8080 ', '80x', '8.5', 8.5, -0.5, 1e30, '1e3', '-1', '0x1A', 'abc', '', true, false, null, 0, 65536, []];
$queries = ['a=1', '', 5, 1.5, true, false, null, ['x']];
$edges = [];
$i = 0;
foreach ($ports as $port) {
    foreach ([null, 'on'] as $https) {
        foreach ($queries as $query) {
            $server = ['REQUEST_URI' => '/foo', 'SERVER_NAME' => 'edge.test', 'SERVER_PORT' => $port, 'QUERY_STRING' => $query];
            if (null === $port) {
                unset($server['SERVER_PORT']);
            }
            if (null === $query && 0 === $i % 2) {
                unset($server['QUERY_STRING']);
            }
            if (null !== $https) {
                $server['HTTPS'] = $https;
            }
            $edges[] = [typed_value($server), answers(new Request([], [], [], [], [], $server), $i, $routes)];
            ++$i;
        }
    }
}

echo json_encode(['cases' => $cases, 'edges' => $edges], \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
