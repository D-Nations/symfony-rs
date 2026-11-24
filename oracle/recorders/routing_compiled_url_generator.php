<?php
// CompiledUrlGeneratorDumper and CompiledUrlGenerator answers for
// crates/routing/tests/data/compiled_url_generator.json (scripts/record.sh
// routing-compiled-url-generator): generated collections of routes
// (inline settings, defaults, requirements, hosts, schemes, localized
// variants) and aliases (chains, cycles, missing targets, deprecations,
// localized names, name prefixes through addCollection()), each dumped
// and read back with `require`, then generate() calls through
// CompiledUrlGenerator.
//
// A collection entry is ["route", name, path, defaults, requirements,
// options, host, schemes], ["alias", name, target, deprecation] (null or
// [package, version, message]) or ["prefix", prefix] (the collection so far
// is added to a new one under that name prefix); strings are base64,
// values typed (lib/typed.php). A dump is ["ok", typed array that
// `require` returns, deprecations] or ["throws", class, base64 message]; a
// call is [context, default locale, name, parameters, reference type,
// result, deprecations], the result ["ok", base64 url] or ["throws",
// class, base64 message], then the same call's result and deprecations
// through UrlGenerator on the collection. Deprecations are base64 messages.

error_reporting(E_ALL);
ini_set('memory_limit', '1G');

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Routing\Generator\CompiledUrlGenerator;
use Symfony\Component\Routing\Generator\Dumper\CompiledUrlGeneratorDumper;
use Symfony\Component\Routing\Generator\UrlGenerator;
use Symfony\Component\Routing\RequestContext;
use Symfony\Component\Routing\Route;
use Symfony\Component\Routing\RouteCollection;

mt_srand(20261124);

$deprecations = [];
set_error_handler(static function (int $level, string $message) use (&$deprecations) {
    if (\E_USER_DEPRECATED === $level) {
        $deprecations[] = base64_encode($message);
    }

    return true;
});

function pick(array $a): mixed
{
    return $a[mt_rand(0, \count($a) - 1)];
}

function some(array $pool, int $max): array
{
    $out = [];
    for ($n = mt_rand(0, $max); $n > 0; --$n) {
        $out[pick($pool)] = null;
    }

    return array_keys($out);
}

$paths = ['/', '/testing', '/{foo}', '/{foo}/{bar}', '/a/{foo}.{_format}', '/{!foo}', '/blog/{page<\d+>?1}', '/{_locale}/x', '/{foo}/', '/x/{foo?}', '/é/{foo}'];
$requirements = ['\d+', '.+', 'en|fr', '[a-z]+', '[^/]+'];
$scalars = [null, '', '0', 'x', 'bar', 0, 1, 1.5, true, false, 'en', 'fr', 'html', [], ['a', 'b'], ['k' => [1, null]]];
$vars = ['foo', 'bar', 'page', '_format', '_locale', 'q'];
// '10', '1e1' and '010' are equal under ==, as array_search() compares
$names = ['a', 'b', 'c', 'test', 'test.en', 'test.fr', 'test.en_GB', 'x.en', '123', "q'uote", '10', '1e1', '010'];
$hosts = ['', '', '', '{sub}.example.com', 'example.com'];
$schemes = [[], [], ['https'], ['ftp', 'https']];

function route_entry(string $name): array
{
    global $paths, $requirements, $scalars, $vars, $hosts, $schemes;
    $defaults = [];
    foreach (some($vars, 2) as $n) {
        $defaults[$n] = pick($scalars);
    }
    if (preg_match('/\.(\w+)$/', $name, $m) && mt_rand(0, 3)) {
        $defaults['_locale'] = mt_rand(0, 5) ? $m[1] : pick(['en', 'fr', 7]);
        if (mt_rand(0, 2)) {
            $defaults['_canonical_route'] = substr($name, 0, strrpos($name, '.'));
        }
    }
    $reqs = [];
    foreach (some($vars, 1) as $n) {
        $reqs[$n] = pick($requirements);
    }

    $path = pick($paths);

    return ['route', $name, $path, $defaults, $reqs, str_contains($path, 'é') && mt_rand(0, 5) ? ['utf8' => true] : [], pick($hosts), pick($schemes)];
}

function build(array $entries): RouteCollection
{
    $routes = new RouteCollection();
    foreach ($entries as $e) {
        if ('route' === $e[0]) {
            $routes->add($e[1], new Route($e[2], $e[3], $e[4], $e[5], $e[6], $e[7]));
        } elseif ('alias' === $e[0]) {
            $alias = $routes->addAlias($e[1], $e[2]);
            if (null !== $e[3]) {
                $alias->setDeprecated(...$e[3]);
            }
        } else {
            $outer = new RouteCollection();
            $routes->addNamePrefix($e[1]);
            $outer->addCollection($routes);
            $routes = $outer;
        }
    }

    return $routes;
}

function encode_entry(array $e): array
{
    return match ($e[0]) {
        'route' => ['route', base64_encode($e[1]), base64_encode($e[2]), typed_value($e[3]), typed_value($e[4]), typed_value($e[5]), base64_encode($e[6]), array_map('base64_encode', $e[7])],
        'alias' => ['alias', base64_encode($e[1]), base64_encode($e[2]), null === $e[3] ? null : array_map('base64_encode', $e[3])],
        default => ['prefix', base64_encode($e[1])],
    };
}

function outcome(callable $f): array
{
    try {
        return ['ok', $f()];
    } catch (\Throwable $e) {
        return ['throws', $e::class, base64_encode($e->getMessage())];
    }
}

$cases = [];
$file = sys_get_temp_dir() . '/compiled_url_generator.php';
while (\count($cases) < 1500) {
    $entries = [];
    $defined = [];
    for ($n = mt_rand(1, 6); $n > 0; --$n) {
        $r = mt_rand(0, 9);
        if ($r < 5 || !$defined) {
            $entries[] = route_entry($defined[] = pick($names));
        } elseif ($r < 9) {
            // mostly a name defined so far, so that most dumps succeed
            $target = mt_rand(0, 6) ? pick($defined) : pick($names);
            $deprecation = mt_rand(0, 3) ? null : ['foo/bar', pick(['1.0', '']), pick(['', 'foo %alias_id%.', 'The "%alias_id%" alias.'])];
            $entries[] = ['alias', $defined[] = pick($names), $target, $deprecation];
        } else {
            $entries[] = ['prefix', pick(['sub_', 'p.'])];
        }
    }
    try {
        build($entries);
    } catch (\Throwable) {
        continue;
    }

    $deprecations = [];
    $dump = outcome(static function () use ($entries, $file) {
        file_put_contents($file, (new CompiledUrlGeneratorDumper(build($entries)))->dump());

        return require $file;
    });
    $dumpDeprecations = $deprecations;
    $calls = [];
    if ('ok' === $dump[0]) {
        $compiled = $dump[1];
        $dump[1] = typed_value($compiled);
        for ($n = 0; $n < 3; ++$n) {
            $context = [pick(['', '/app.php']), pick(['localhost', 'example.com']), pick(['http', 'https']), mt_rand(0, 2) ? [] : ['_locale' => pick(['en', 'fr', 'en_GB', 'de'])]];
            $defaultLocale = pick([null, null, 'en', 'fr_CA', 'en_GB']);
            $parameters = [];
            foreach (some($vars, 4) as $v) {
                $parameters[$v] = pick(['x', 'bar', '5', 'en', 'fr', 'html', 'en_GB', 3]);
            }
            // mostly a compiled name, or one with its locale suffix removed
            $known = array_map('strval', array_keys($compiled));
            $name = match (mt_rand(0, 4)) {
                0 => pick(array_merge($names, ['test', 'x', 'sub_a', 'sub_test', 'p.b'])),
                1 => preg_replace('/\.\w+$/', '', pick($known)),
                default => pick($known),
            };
            $referenceType = mt_rand(0, 3);
            $requestContext = new RequestContext($context[0], 'GET', $context[1], $context[2]);
            $requestContext->setParameters($context[3]);
            $generator = new CompiledUrlGenerator($compiled, $requestContext, null, $defaultLocale);
            $deprecations = [];
            $result = outcome(static fn () => base64_encode($generator->generate($name, $parameters, $referenceType)));
            $compiledDeprecations = $deprecations;
            // the same call through UrlGenerator on the collection (RouteCollection::get())
            $deprecations = [];
            $plain = new UrlGenerator(build($entries), $requestContext, null, $defaultLocale);
            $plainResult = outcome(static fn () => base64_encode($plain->generate($name, $parameters, $referenceType)));
            $calls[] = [
                [base64_encode($context[0]), base64_encode($context[1]), base64_encode($context[2]), typed_value($context[3])],
                null === $defaultLocale ? null : base64_encode($defaultLocale),
                base64_encode($name),
                typed_value($parameters),
                $referenceType,
                $result,
                $compiledDeprecations,
                $plainResult,
                $deprecations,
            ];
        }
    }
    $dump[] = $dumpDeprecations;

    $cases[] = [array_map('encode_entry', $entries), $dump, $calls];
}
@unlink($file);

echo json_encode($cases, \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
