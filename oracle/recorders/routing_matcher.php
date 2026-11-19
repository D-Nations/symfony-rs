<?php
// Records the installed Symfony Routing matcher, without booting/editing the app.
// Moved from symfony-demo-rs compare/routing_vectors.php; run by scripts/record.sh.
// Source: vendor/symfony/routing at 41b32d2c5d907abe2c3ffaa7a112dd7492353822.
require '/demo/vendor/autoload.php';
use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;
use Symfony\Component\Routing\RequestContext;
use Symfony\Component\Routing\Matcher\RedirectableUrlMatcher;
use Symfony\Component\Routing\Exception\MethodNotAllowedException;
use Symfony\Component\Routing\Exception\ResourceNotFoundException;

$routes = new RouteCollection();
foreach ([
    ['foo', '/foo', ['POST'], []],
    ['foo2', '/foo', ['PUT', 'DELETE'], []],
    ['item', '/item/{bar}', ['GET'], ['bar' => '[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*']],
    ['with', '/with/', ['GET'], []],
    ['colon', '/foo:bar/', ['GET'], []],
    ['head', '/head', ['HEAD'], []],
    ['static', '/fallback/foo/', ['GET'], []],
    ['fallback', '/fallback/{bar}', ['GET'], []],
    ['param', '/param/{bar}', [], []],
    ['root', '/', [], []],
] as [$name, $path, $methods, $requirements]) {
    $routes->add($name, new Route($path, [], $requirements, [], '', [], $methods));
}
$out = [];
foreach (['GET', 'HEAD', 'POST', 'PUT', 'DELETE', 'OPTIONS'] as $method) {
    $matcher = new class($routes, new RequestContext('', $method)) extends RedirectableUrlMatcher {
        public function redirect(string $path, string $route, ?string $scheme = null): array {
            return ['_redirect_path' => $path];
        }
    };
    foreach (['/', '/foo', '/foo/', '/item/rust-port', '/item/a--b', '/with', '/with/', '/with//', '/foo%3Abar', '/param/bar%2523', '/param/a%2Fb', '/fallback/foo', '/fallback/foo/', '/head/', '/missing'] as $path) {
        try {
            $match = $matcher->match($path);
            $expected = ['kind' => isset($match['_redirect_path']) ? 'redirect' : 'found', 'route' => $match['_route'], 'bar' => $match['bar'] ?? null];
            if (isset($match['_redirect_path'])) $expected['path'] = $match['_redirect_path'];
        } catch (MethodNotAllowedException $error) {
            $expected = ['kind' => 'method_not_allowed', 'allow' => $error->getAllowedMethods()];
        } catch (ResourceNotFoundException $error) {
            $expected = ['kind' => 'not_found'];
        }
        $out[] = ['method' => $method, 'path' => $path, 'expected' => $expected];
    }
}
echo json_encode($out, JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR), "\n";
