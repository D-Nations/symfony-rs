<?php
// Route, RouteCompiler, CompiledRoute, RouteCollection and Alias answers
// for what their tests leave open, for crates/routing/tests/data/route.json
// (scripts/record.sh routing-route):
//   routes       generated paths and hosts with inline settings, defaults,
//                requirements and options through new Route(), then
//                compile() and serialize()
//   requirements generated requirements through setRequirement()
//   collections  generated scripts of RouteCollection calls (add with
//                priorities, aliases, deprecations, remove, prefixes,
//                addCollection), with all(), the aliases and get() after
//                each step
//   unserialize  Route and CompiledRoute payloads with a value of another
//                type in each property
// Values are typed (lib/typed.php); a thrown error is ["throws", class,
// base64 message], since messages quote patterns that need not be UTF-8.

error_reporting(E_ALL);
ini_set('memory_limit', '1G');

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Routing\CompiledRoute;
use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;

mt_srand(20261124);

// Warnings and deprecations PHP emits along the way (undefined keys while
// unserializing, trigger_deprecation()) are collected, not printed.
$notices = [];
set_error_handler(function (int $level, string $message) use (&$notices) {
    $notices[] = [$level, $message];

    return true;
});

function call(callable $f): array
{
    try {
        return typed_value($f());
    } catch (\Throwable $e) {
        return ['throws', $e::class, base64_encode($e->getMessage())];
    }
}

function pick(array $a): mixed
{
    return $a[mt_rand(0, \count($a) - 1)];
}

function route_state(Route $r): array
{
    return [
        'path' => $r->getPath(),
        'host' => $r->getHost(),
        'defaults' => $r->getDefaults(),
        'requirements' => $r->getRequirements(),
        'options' => $r->getOptions(),
        'schemes' => $r->getSchemes(),
        'methods' => $r->getMethods(),
        'condition' => $r->getCondition(),
    ];
}

function compiled_state(CompiledRoute $c): array
{
    return [
        $c->getStaticPrefix(),
        $c->getRegex(),
        $c->getTokens(),
        $c->getPathVariables(),
        $c->getHostRegex(),
        $c->getHostTokens(),
        $c->getHostVariables(),
        $c->getVariables(),
    ];
}

$names = ['foo', 'bar', 'id', '_format', '_locale', 'x', 'y', 'é', "\xE9", 'a_b', '_fragment', '9a', str_repeat('n', 33), str_repeat('m', 32)];
$texts = ['/', '/', '/', 'foo', '.', '-', ',', '~', '|', '§', 'é', "\xE9", ' ', "\n", '0', '{', '}', '<', '>', '?', ':', '!', 'static', '{s', 's}'];
$requirements = ['\d+', '^\d+$', '\A\d+\z', '[a-z]{2}', '(a|b)', '(?:a|b)', '((x))', '.*', '.+', '.', 'é', "\xE9", '\pL', '\p{Lu}', '\X', '\\\\X', 'en|fr', 'en', '0', '\z', '^', '$', '', 'a\zb\z', '[^/]+', '(?P<n>x)', '(*F)', '(?(x)y)'];
$defaultValues = [null, '', 'default', '0', 'en', 'html', 1, true, ['a' => 'b']];

function variable(): string
{
    global $names, $requirements;
    $name = pick($names);
    $important = mt_rand(0, 4) ? '' : '!';
    $mapping = mt_rand(0, 6) ? '' : (':' . pick(['arg', 'arg.prop', 'é', '']));
    $requirement = mt_rand(0, 3) ? '' : ('<' . pick($requirements) . '>');
    $default = mt_rand(0, 3) ? '' : ('?' . pick(['', 'value', 'a<b>', '\\', '<>', '0']));

    return '{' . $important . $name . $mapping . $requirement . $default . '}';
}

function pattern(int $max): string
{
    global $texts;
    $pattern = '';
    $count = mt_rand(0, $max);
    for ($i = 0; $i < $count; ++$i) {
        $pattern .= mt_rand(0, 1) ? pick($texts) : variable();
    }

    return $pattern;
}

$routes = [];
for ($n = 0; $n < 2000; ++$n) {
    $path = pattern(6);
    $host = mt_rand(0, 3) ? '' : pattern(3);
    $defaults = [];
    for ($i = mt_rand(0, 2); $i > 0; --$i) {
        $defaults[pick($names)] = pick($defaultValues);
    }
    if (!mt_rand(0, 5)) {
        $defaults['_locale'] = pick(['en', 'fr', 'e.n']);
        $defaults['_canonical_route'] = 'r';
    }
    $reqs = [];
    for ($i = mt_rand(0, 2); $i > 0; --$i) {
        $reqs[pick($names)] = pick($requirements);
    }
    if (!mt_rand(0, 5)) {
        $reqs['_locale'] = pick(['en', 'fr', 'e\.n', 'en|fr']);
    }
    $options = mt_rand(0, 4) ? [] : ['utf8' => pick([true, false, 1, '0'])];
    $schemes = pick([[], 'Https', ['HTTP', 'https']]);
    $methods = pick([[], 'get', ['Post', 'PUT']]);

    $route = null;
    $constructed = call(function () use (&$route, $path, $defaults, $reqs, $options, $host, $schemes, $methods) {
        $route = new Route($path, $defaults, $reqs, $options, $host, $schemes, $methods);

        return route_state($route);
    });
    $compiled = null;
    $after = null;
    $serialized = null;
    if ($route) {
        $compiled = call(fn () => compiled_state($route->compile()));
        $after = typed_value(route_state($route));
        $serialized = base64_encode(serialize($route));
    }
    $routes[] = [
        typed_value([$path, $defaults, $reqs, $options, $host, (array) $schemes, (array) $methods]),
        $constructed,
        $compiled,
        $after,
        $serialized,
    ];
}

$requirementAnswers = [];
$pieces = ['^', '$', '\A', '\z', '\d', '+', 'a', '\\'];
for ($n = 0; $n < 1000; ++$n) {
    $r = '';
    for ($i = mt_rand(0, 5); $i > 0; --$i) {
        $r .= pick($pieces);
    }
    $requirementAnswers[] = [base64_encode($r), call(fn () => (new Route('/'))->setRequirement('k', $r)->getRequirement('k'))];
}

// A collection script is a list of steps; each step is recorded with its
// answer and the collection's state after it.
function collection_state(RouteCollection $c, array $probe): array
{
    global $notices;
    $all = [];
    foreach ($c->all() as $name => $route) {
        $all[] = [$name, $route->getPath(), $c->getPriority($name), $route->getDefault('_canonical_route')];
    }
    $aliases = [];
    foreach ($c->getAliases() as $name => $alias) {
        $aliases[] = [$name, $alias->getId(), $alias->isDeprecated()];
    }
    $gets = [];
    foreach ($probe as $name) {
        $notices = [];
        $gets[] = [call(fn () => $c->get($name)?->getPath()), array_map(fn ($n) => $n[1], $notices)];
    }

    return typed_value([$all, $aliases, $gets]);
}

$routeNames = ['a', 'b', 'c', 'p_a', '0', '1', 'é'];
function step(): array
{
    global $routeNames;
    return match (mt_rand(0, 9)) {
        0, 1, 2 => ['add', pick($routeNames), pick(['/x', '/{y}', '/z/{w}']), pick([0, 0, 1, -1, 2]), mt_rand(0, 3) ? null : pick($routeNames)],
        3, 4 => ['alias', pick($routeNames), pick($routeNames)],
        5 => ['deprecate', pick($routeNames), pick(['', 'The "%alias_id%" is gone.', 'no placeholder', "bad\n%alias_id%", 'x */ %alias_id%', '0']), pick(['', 'vendor/pkg', '0']), pick(['', '1.2', '0'])],
        6 => ['remove', mt_rand(0, 1) ? [pick($routeNames)] : [pick($routeNames), pick($routeNames)]],
        7 => ['name_prefix', pick(['p_', '', '0'])],
        8 => ['prefix', pick(['/pre', ' / ', '0', '/{z}/', ''])],
        9 => ['collection', array_map(fn () => step_simple(), range(1, mt_rand(0, 4)))],
    };
}
function step_simple(): array
{
    global $routeNames;
    return match (mt_rand(0, 2)) {
        0, 1 => ['add', pick($routeNames), pick(['/x', '/{y}']), pick([0, 1, -1]), null],
        2 => ['alias', pick($routeNames), pick($routeNames)],
    };
}

function apply(RouteCollection $c, array $step): mixed
{
    switch ($step[0]) {
        case 'add':
            $route = new Route($step[2]);
            if (null !== $step[4]) {
                $route->setDefault('_canonical_route', $step[4]);
            }
            $c->add($step[1], $route, $step[3]);

            return null;
        case 'alias':
            $c->addAlias($step[1], $step[2]);

            return null;
        case 'deprecate':
            $alias = $c->getAlias($step[1]);
            if (!$alias) {
                return 'no alias';
            }
            $alias->setDeprecated($step[3], $step[4], $step[2]);

            return $alias->getDeprecation('NAME');
        case 'remove':
            $c->remove(1 === \count($step[1]) ? $step[1][0] : $step[1]);

            return null;
        case 'name_prefix':
            $c->addNamePrefix($step[1]);

            return null;
        case 'prefix':
            $c->addPrefix($step[1], ['d' => 'v'], []);

            return null;
        case 'collection':
            $sub = new RouteCollection();
            foreach ($step[1] as $s) {
                apply($sub, $s);
            }
            $c->addCollection($sub);

            return null;
    }

    throw new \LogicException('unknown step');
}

$collections = [];
for ($n = 0; $n < 300; ++$n) {
    $c = new RouteCollection();
    $steps = [];
    for ($i = mt_rand(1, 8); $i > 0; --$i) {
        $step = step();
        $answer = call(fn () => apply($c, $step));
        $steps[] = [typed_value($step), $answer, collection_state($c, $routeNames)];
    }
    $collections[] = $steps;
}

// unserialize(): one property of another type, or missing.
function payload(string $class, array $data): string
{
    $payload = \sprintf('O:%d:"%s":%d:{', \strlen($class), $class, \count($data));
    foreach ($data as $key => $value) {
        $payload .= serialize($key) . serialize($value);
    }

    return $payload . '}';
}

$others = ['int' => 5, 'float' => 1.5, 'true' => true, 'false' => false, 'null' => null, 'array' => ['x'], 'object' => new \stdClass(), 'missing' => null];
$routeData = ['path' => '/p', 'host' => 'h', 'defaults' => ['d' => 1], 'requirements' => ['r' => 'x'], 'options' => ['o' => 2], 'schemes' => ['https'], 'methods' => ['GET'], 'condition' => 'c'];
$compiledData = ['vars' => ['v'], 'path_prefix' => '/p', 'path_regex' => '{^/p$}sD', 'path_tokens' => [['text', '/p']], 'path_vars' => [], 'host_regex' => null, 'host_tokens' => [], 'host_vars' => []];

$unserialize = [];
foreach (['route' => [Route::class, $routeData], 'compiled' => [CompiledRoute::class, $compiledData]] as $kind => [$class, $base]) {
    foreach (array_keys($base) as $property) {
        foreach ($others as $type => $value) {
            $data = $base;
            if ('missing' === $type) {
                unset($data[$property]);
            } else {
                $data[$property] = $value;
            }
            $payload = payload($class, $data);
            $result = call(function () use ($payload, $kind) {
                $object = unserialize($payload);

                return 'route' === $kind ? route_state($object) : compiled_state($object);
            });
            $unserialize[] = [$kind, base64_encode($payload), $result];
        }
    }
    // compiled: a non-CompiledRoute value
    if ('route' === $kind) {
        foreach (['array' => ['x'], 'object' => new \stdClass(), 'int' => 1] as $type => $value) {
            $data = $base + ['compiled' => $value];
            $payload = payload($class, $data);
            $unserialize[] = [$kind, base64_encode($payload), call(fn () => route_state(unserialize($payload)))];
        }
    }
}

echo json_encode([
    'routes' => $routes,
    'requirements' => $requirementAnswers,
    'collections' => $collections,
    'unserialize' => $unserialize,
], \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
