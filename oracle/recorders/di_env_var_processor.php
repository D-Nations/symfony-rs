<?php
// EnvVarProcessor vectors (scripts/record.sh di-env-var-processor): the
// real Symfony\Component\DependencyInjection\EnvVarProcessor on the
// inputs its tests leave open, replayed by
// crates/dependency-injection/tests/env_var_processor_vectors.rs.
//
//   params  the container's parameters (a Container on a
//           FrozenParameterBag), as typed JSON (lib/typed.php)
//   cases   getEnv($prefix, $name, $getEnv): the value the $getEnv
//           closure returns for any name ("value"), the names it was
//           called with ("calls"), and the outcome: the value or
//           ["throws", class, base64(message)]
//   lookup  getEnv('string' or '', $name) reading $_ENV, $_SERVER and
//           getenv(): the superglobals set for the case ("env",
//           "server", "putenv") and the outcome
//
// Prefixes: every one of getProvidedTypes() except require (it runs PHP)
// and the seeded order of shuffle; const only with core constants.
//
// Output: crates/dependency-injection/tests/data/env_var_processor.json

ini_set('display_errors', 'stderr');
require __DIR__.'/lib/di_autoload.php';
require __DIR__.'/lib/typed.php';

use Symfony\Component\DependencyInjection\Container;
use Symfony\Component\DependencyInjection\EnvVarProcessor;
use Symfony\Component\DependencyInjection\ParameterBag\FrozenParameterBag;

function outcome(callable $f): array
{
    try {
        return typed_value($f());
    } catch (\Throwable $e) {
        return ['throws', $e::class, base64_encode($e->getMessage())];
    }
}

$params = [
    'bar' => 'BAR',
    'int' => 12,
    'float' => 1.5,
    'true' => true,
    'false' => false,
    'null' => null,
    'array' => [1, 2],
    'empty' => '',
    'pct' => '50%',
    'env(DI_TEST_PARAM)' => 'from param',
    'env(DI_TEST_NULL)' => null,
    'env(DI_TEST_ARRAY)' => ['a'],
];
$container = new Container(new FrozenParameterBag($params));

$numbers = [
    '1', '0', '-1', '+1', '1.1', '1e1', '1E+2', '-.5e-3', '.5', '5.', '1.e3', '+.5', ' 1.5 ', "\t2\n", "\v3", "\f3", "3\0",
    "\r4\r", '1e', 'e1', '.e3', '.', '1e+', '0x1A', '0b1', '017', '00012', 'inf', 'NAN', '1_000', '1,000', '--1', '', ' ',
    '1.2.3', '1 2', '1e400', '1e308', '1e309', '-1e309', '1e-400', '1e-320', '4.9e-324', '-0', '-00', '-0.0', '0.0', '0e0', '0e5',
    '-0e0', '-0e5', '0.0e5', '9223372036854775807', '9223372036854775808', '-9223372036854775808', '-9223372036854775809',
    '1e20', '-1e20', '9.3e18', '1e19', '18446744073709551616', '1.9e19', 'true', 'false', 'on', 'off', 'yes', 'no', 'null',
    'TRUE', ' true ', 'foo', '0.5', '-0.5', '0.00', '2.5', '-2.5',
    true, false, 0, 1, -7, 1.5, -0.0, 0.1, 1e100, 1e25, null, PHP_INT_MAX, PHP_INT_MIN,
];

$csvs = [
    ',', '1', 'a,b', ' "a" ,b', '"a"b,c', "a\nb,c", "\"a\nb\",c", "a,b\n", "a,b\r\n", "a,b\r", "a,b\n\n", "a,b\r\r\n",
    'a,"b', '"",', ' , ', '"a""b"', '"a" "b"', 'a"b"c', "a,b\nc,d", "\t\"a\"", "\n\"a\"", "\v\"a\"", "\f\"a\"", "\r\"a\"",
    "\0\"a\"", " \"a", "\"a\n", "\"a\"\n", "\"a\" \n", "a,\n", "\n", "\r\n", "\r", "\n\n", "\r\r", ',,', ',', '"', '""',
    '"a"""', '"a"" b', '"a,b","c', 'x,"y"z"w",v', "\xA0\"a\"", " \t \"a\"", 'a,b ', "\"a\r\n\"", "a\r\nb", "\"ab\"\r",
    "\"\n", "x,\"", "\"\"\"\n", "a\n,b", "\"a\n\",b\n", "\"a\" ,\n", '\\,\\\\', ',"""","foo""","\""",\,foo\\', "\"\xFF\",\xFE",
    'a;b', "a\tb", 1, 1.5, true, false, 0,
];

$urls = [
    'https://symfony.com', 'https://symfony.com/', 'https://symfony.com//', 'https://symfony.com/blog/',
    'mysql://user:pa%40ss@localhost:3306/db?charset=utf8#frag', 'redis://localhost', 'redis://:secret@localhost:6379/1',
    'postgres://u%20x:p%2Fy@host/path%20x', 'smtp://user@host', 'http://[::1]:80/x', 'file:///etc/passwd', 'file://host/x',
    'http://foo.com\\bar', '\\\\foo.com/bar', "a\rb", "a\nb", "a\tb", "\0foo", "foo\0", ' foo', 'foo ', ':', '',
    'localhost', '//host/path', 'http://', 'http:///x', 'http://host?a\\b', 'http://host#a\\b', 'http://h\\ost?x',
    "http://host/\x7F", "http://host/\x01", 'http://user:@host', 'http://:@host', 'http://host:0', 'http://host:99999',
    'http://host:abc', 'scheme:path', 'a://b', '0://b', "http://host/\xFF",
];

$queries = [
    'https://example.com', 'https://example.com?', 'https://example.com?foo=bar&baz=123', 'foo=bar&test=1', 'a[]=1&a[]=2',
    'a[b][c]=1', 'https://h?0', 'https://h?a=1#f', '?a=1', 'a=1#f', 'a b=c d', 'a+b=%20', 'x=1&x=2', '', 'http://h?a=%00b',
    '0://h?q=1', 'noscheme?q=1', ':', 'a.b=1&a b=2', 1, true,
];

$jsons = [
    '[1]', '{"key": "value"}', 'invalid_json', '1', '1.1', 'true', 'false', '"foo"', 'null', '', ' ', '[', '{"a":1,}',
    "[\"\xFF\"]", '[1e400]', '{"0":1,"1":2}', '{"":1}', '[[[[1]]]]', "[\"\\u0000\"]", "[\"a\x01\"]", '{"a":1}{', '[1] ',
    ' [1]', '{"a":{"b":[]}}', '12345678901234567890', '[12345678901234567890]', 1, true, false, 0,
];

$base64s = ['aGVsbG8=', '/+0=', '_-0=', 'aGVsbG8', 'a', 'aGVs bG8=', '====', '', '!!!', 'Zm9v=OmJhcg==', 1, true];

$resolves = [
    '%bar%', 'x%bar%y', '%%', '%%bar%%', '%int%', '%float%', '%true%', '%false%', '%null%', '%array%', '%missing%',
    '%baar%', '%env(FOO)%', '%env()%', '%env(%', '%pct%', '100%', '%', '%bar', "% bar%", "%bar\v%", "%bar\x85%",
    "%bar\xA0%", '%bar%%int%', '%%%bar%%%', 'no placeholder', '', '%empty%', 'a%env(X)%b%bar%', 1, true, 1.5,
];

$cases = [];
$add = static function (string $prefix, string $name, mixed $value) use (&$cases, $container): void {
    $calls = [];
    $processor = new EnvVarProcessor($container);
    $result = outcome(static function () use ($processor, $prefix, $name, $value, &$calls) {
        return $processor->getEnv($prefix, $name, static function (string $n) use ($value, &$calls) {
            $calls[] = $n;

            return $value;
        });
    });
    $cases[] = [
        'prefix' => $prefix,
        'name' => $name,
        'value' => typed_value($value),
        'calls' => $calls,
        'result' => $result,
    ];
};

foreach ($numbers as $n) {
    foreach (['bool', 'not', 'int', 'float', 'string', ''] as $prefix) {
        $add($prefix, 'foo:bar', $n);
    }
}
foreach ($csvs as $c) {
    $add('csv', 'foo', $c);
}
foreach ($urls as $u) {
    $add('url', 'foo', $u);
}
foreach ($queries as $q) {
    $add('query_string', 'foo', $q);
}
foreach ($jsons as $j) {
    $add('json', 'foo', $j);
}
foreach ($base64s as $b) {
    $add('base64', 'foo', $b);
}
foreach ($resolves as $r) {
    $add('resolve', 'foo', $r);
}
foreach ([" hello\n", "\0\t\v x \r", "\f x \f", "\xA0x\xA0", 'x', '', 1.5, true] as $t) {
    $add('trim', 'foo', $t);
}
foreach (['foo: Data123!@-_ + bar', "\xFF~.", '', 'a/b?c=d&e', 1.5, false] as $u) {
    $add('urlencode', 'foo', $u);
}
foreach (['PHP_INT_MAX', 'PHP_EOL', 'NULL', 'null', 'TRUE', 'True', 'M_PI', 'UNDEFINED', '', 'PHP_INT_SIZE', 1, true] as $c) {
    $add('const', 'foo', $c);
}
foreach ([['a' => 1], [], ['index' => null], ['index' => 0], ['index' => ['x']], [0 => 'zero'], ['1' => 'one'], 'string', null, 1, ["\xFF" => 1, 'k' => 2]] as $k) {
    $add('key', 'index:foo', $k);
    $add('key', '0:foo', $k);
    $add('key', '1:foo', $k);
}
$add('key', 'index', ['index' => 1]);
$add('key', ':foo', ['' => 'empty key']);
foreach (['x', '', null, '0', 0, false, [], ['a']] as $d) {
    foreach ([':foo', 'bar:foo', 'int:foo', 'null:foo', 'missing:foo', 'foo', 'array:foo', 'bar:foo:baz'] as $name) {
        $add('default', $name, $d);
    }
    $add('defined', 'foo', $d);
}
foreach ([['foo', 'bar'], [], 'string', null, 1, ['a' => 1]] as $s) {
    $add('shuffle', 'foo', $s);
}
foreach (['/missing-file', '', null, ['x'], 1, "/dev/null\0x"] as $f) {
    $add('file', 'foo', $f);
    $add('require', 'foo', $f);
}
foreach (['enum', 'unknown', 'base64', 'json', 'csv', 'url', 'query_string', 'resolve', 'trim', 'urlencode', 'const', 'string', 'bool', 'int', 'float', 'not', 'shuffle', 'key', 'default', 'defined'] as $prefix) {
    $add($prefix, 'foo', null);
    $add($prefix, 'foo', ['array']);
}
$add('', '', 'x');
$add('enum', 'foo', 'x');
$add('enum', 'Bogus:foo', 'x');
$add('enum', 'Bogus:foo', 1.5);

// The string lookup: $_ENV, $_SERVER, getenv(), then env(NAME).
$lookups = [];
$lookup = static function (string $prefix, string $name, array $env, array $server, array $putenv) use (&$lookups, $container): void {
    $savedEnv = $_ENV;
    $savedServer = $_SERVER;
    foreach ($env as $k => $v) {
        $_ENV[$k] = $v;
    }
    foreach ($server as $k => $v) {
        $_SERVER[$k] = $v;
    }
    foreach ($putenv as $p) {
        putenv($p);
    }
    $processor = new EnvVarProcessor($container);
    $result = outcome(static fn () => $processor->getEnv($prefix, $name, static fn () => throw new \LogicException('getEnv called')));
    foreach ($putenv as $p) {
        putenv(explode('=', $p, 2)[0]);
    }
    $_ENV = $savedEnv;
    $_SERVER = $savedServer;
    $lookups[] = [
        'prefix' => $prefix,
        'name' => $name,
        'env' => typed_value($env),
        'server' => typed_value($server),
        'putenv' => $putenv,
        'result' => $result,
    ];
};
foreach (['string', ''] as $prefix) {
    $lookup($prefix, 'DI_A', ['DI_A' => 'x'], [], []);
    $lookup($prefix, 'DI_A', ['DI_A' => ''], ['DI_A' => 's'], ['DI_A=p']);
    $lookup($prefix, 'DI_A', [], ['DI_A' => 's'], ['DI_A=p']);
    $lookup($prefix, 'DI_A', ['DI_A' => null], ['DI_A' => 's'], []);
    $lookup($prefix, 'DI_A', ['DI_A' => null], ['DI_A' => null], ['DI_A=p']);
    $lookup($prefix, 'DI_A', [], ['DI_A' => ''], ['DI_A=p']);
    $lookup($prefix, 'HTTP_DI_A', [], ['HTTP_DI_A' => 'h'], []);
    $lookup($prefix, 'HTTP_DI_A', [], ['HTTP_DI_A' => 'h'], ['HTTP_DI_A=p']);
    $lookup($prefix, 'HTTP_DI_A', ['HTTP_DI_A' => 'e'], ['HTTP_DI_A' => 'h'], []);
    $lookup($prefix, 'DI_A', [], [], ['DI_A=p']);
    $lookup($prefix, 'DI_A', [], [], ['DI_A=']);
    $lookup($prefix, 'DI_A', [], [], []);
    $lookup($prefix, 'DI_A', ['DI_A' => false], [], ['DI_A=p']);
    $lookup($prefix, 'DI_A', ['DI_A' => true], [], []);
    $lookup($prefix, 'DI_A', ['DI_A' => 0], [], []);
    $lookup($prefix, 'DI_A', ['DI_A' => 1.5], [], []);
    $lookup($prefix, 'DI_A', ['DI_A' => []], [], []);
    $lookup($prefix, 'DI_A', ['DI_A' => ['x']], [], []);
    $lookup($prefix, 'DI_TEST_PARAM', [], [], []);
    $lookup($prefix, 'DI_TEST_PARAM', ['DI_TEST_PARAM' => ''], [], []);
    $lookup($prefix, 'DI_TEST_PARAM', [], [], ['DI_TEST_PARAM=']);
    $lookup($prefix, 'DI_TEST_NULL', [], [], []);
    $lookup($prefix, 'DI_TEST_ARRAY', [], [], []);
    $lookup($prefix, '123', ['123' => 'numeric'], [], []);
    $lookup($prefix, 'DI_A:B', ['DI_A:B' => 'colon'], [], []);
    $lookup($prefix, '', ['' => 'empty'], [], []);
}

echo json_encode([
    'params' => typed_value($params),
    'cases' => $cases,
    'lookups' => $lookups,
], \JSON_UNESCAPED_SLASHES | \JSON_THROW_ON_ERROR), "\n";
