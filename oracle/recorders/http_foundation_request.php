<?php
// Request answers for what RequestTest leaves open, for
// crates/http-foundation/tests/data/request.json (scripts/record.sh
// http-foundation-request), at the pinned v8.1.11: generated $_SERVER
// arrays (hosts, ports, HTTPS, request URIs, script names, IIS variables,
// forwarded headers, Accept headers, method overrides) through new
// Request(), and generated URIs through Request::create(), each under a
// generated configuration (trusted proxies and header set, trusted hosts,
// method override settings, $_SERVER['REMOTE_ADDR']). Each request then
// answers the same calls in the same order (CALLS below), so caches and
// the "throw once" flags are exercised as an application would.
//
// A case is [config, kind, input, results]: config is [proxies, header
// set, hosts, parameter override, allowed overrides, global REMOTE_ADDR];
// kind "server" has input [server (typed), request (typed)], kind
// "create" has [uri (base64), method, parameters (typed), server
// (typed)]. A result is ["ok", typed value] or ["throws", class, message
// (base64)]; a create() that throws has results ["throws", ...] only.
// Then format, normalizeQueryString() and body cases, and getPort() on
// SERVER_PORT values of every type.

error_reporting(E_ALL);
ini_set('memory_limit', '2G');

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\HttpFoundation\Request;

mt_srand(20261125);

set_error_handler(static fn () => true);

function pick(array $items): mixed
{
    return $items[mt_rand(0, \count($items) - 1)];
}

function chance(int $percent): bool
{
    return mt_rand(1, 100) <= $percent;
}

function result(callable $f): array
{
    try {
        return ['ok', typed_value($f())];
    } catch (\Throwable $e) {
        return ['throws', $e::class, base64_encode($e->getMessage())];
    }
}

const ADDRS = ['127.0.0.1', '10.0.0.1', '10.1.2.3', '192.168.0.5', '88.88.88.88', '::1', '2001:db8::1', 'fd00::1', '::ffff:10.0.0.5', '1.1.1.1', 'unknown', ''];
const HOSTS = ['example.com', 'EXAMPLE.com', 'example.com:8080', 'example.com:443', 'sub.example.com', 'localhost', 'localhost:80', '[::1]', '[::1]:8443', '[::1', '127.0.0.1', '127.0.0.1:81', '1.2.3', '1.2.3.4.', 'a..b', '.a', 'a.', 'evil.com', 'my_host', "h\xe9", '0', '', 'example.com:abc', ' example.com ', 'example.com:0', 'trusted.test', 'x.trusted.test', 'host:8080', 'ex ample.com'];
const PATHS = ['/', '', '/foo', '/foo/', '/foo/bar', '//evil.com/x', '/\\evil', '/a%20b/c', '/index.php', '/index.php/foo', '/app/index.php/x?y=1', '/sub/app.php/a/b', '/web/app.php', '/web/', '/web/x', '/x?a=b&c=d', '/x?a[]=1&a[]=2', '/x?b=2&a=1', '/x#frag', '/x?q#f', '?a=b', 'x', 'x?a=b', 'http://example.com/a/b?c=d', 'https://example.com:8443/p', 'http://example.com', '/%41%42c', '/api%2Ffoo/bar', '/foo%20bar/app.php/home', '/ROUTING/x', '/routing/index.php/y', '//evil/index.php/x', '/\evil/index.php', '/\evil/index.php/admin'];
const SCRIPTS = [
    [],
    ['SCRIPT_NAME' => '/index.php', 'SCRIPT_FILENAME' => '/var/www/index.php', 'PHP_SELF' => '/index.php'],
    ['SCRIPT_NAME' => '/app/index.php', 'SCRIPT_FILENAME' => '/var/www/app/index.php', 'PHP_SELF' => '/app/index.php/x'],
    ['SCRIPT_NAME' => '/web/app.php', 'SCRIPT_FILENAME' => '/srv/web/app.php'],
    ['SCRIPT_FILENAME' => '/srv/sub/app.php', 'PHP_SELF' => '/sub/app.php/a'],
    ['SCRIPT_NAME' => '/foo bar/app.php', 'SCRIPT_FILENAME' => '/home/foo bar/app.php', 'PHP_SELF' => '/foo bar/app.php'],
    ['SCRIPT_FILENAME' => '/var/www/routing/index.php', 'SCRIPT_NAME' => '/routing/index.php', 'PHP_SELF' => '/routing/index.php/y', 'IIS_WasUrlRewritten' => '1'],
    ['ORIG_SCRIPT_NAME' => '/index.php', 'SCRIPT_FILENAME' => '/var/www/index.php'],
    ['SCRIPT_NAME' => '', 'SCRIPT_FILENAME' => ''],
    ['SCRIPT_FILENAME' => '/var/www/api/index.php', 'PHP_SELF' => '/other/x.php'],
    // script names a client chose, starting with a host
    ['SCRIPT_NAME' => '//evil/index.php', 'SCRIPT_FILENAME' => '/var/www/index.php', 'PHP_SELF' => '//evil/index.php'],
    ['SCRIPT_NAME' => '/\\evil/index.php', 'SCRIPT_FILENAME' => '/var/www/index.php', 'PHP_SELF' => '/\\evil/index.php'],
];
const FORWARDED_FOR = ['1.2.3.4', '1.2.3.4, 10.0.0.1', '10.0.0.1, 1.2.3.4', 'unknown, 5.6.7.8', '[2001:db8::2]:80, 1.2.3.4:123', '::ffff:1.2.3.4', '', ' , ', '1.2.3.4,,5.6.7.8'];
const FORWARDED = ['for=1.2.3.4', 'for=1.2.3.4;proto=https;host=fwd.example.com', 'for="[2001:db8::2]:80", for=10.0.0.1', 'host="a.example.com:8443";proto=http', 'for=_hidden', 'proto=https', 'host=example.com', 'for', 'for=1.2.3.4, for=5.6.7.8;host=b.example.com:81', 'for="1.2.3.4:5"'];
const ACCEPT = ['text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8', 'application/json', 'application/ld+json;q=0.5, text/csv', '*/*', '', 'text/plain;format=flowed, text/*;q=0.1', 'application/vnd.api+json, application/problem+json;q=0.9', 'image/webp'];
const LANGUAGES = ['en-us,en;q=0.5', 'fr-FR, fr;q=0.9, en;q=0.8', 'zh-hans-tw, zh-hant-tw', 'i-cherokee', 'pt-BR-u-ca-gregory', 'de', '', '*', 'en, en, fr', 'x-klingon;q=0.1, de-at'];
const METHODS = ['GET', 'POST', 'post', 'PUT', 'HEAD', 'PURGE', 'QUERY', ''];

function generated_server(): array
{
    $server = [];
    if (chance(85)) {
        $server['REMOTE_ADDR'] = pick(ADDRS);
    }
    if (chance(80)) {
        $server['HTTP_HOST'] = pick(HOSTS);
    }
    if (chance(50)) {
        $server['SERVER_NAME'] = pick(['servername', 'example.org', 'EXAMPLE.org', '', 'bad host']);
    }
    if (chance(20)) {
        $server['SERVER_ADDR'] = pick(['10.9.9.9', '::1']);
    }
    if (chance(70)) {
        $server['SERVER_PORT'] = pick(['80', '443', '8080', 80, 443, 8443, '', 'x']);
    }
    if (chance(40)) {
        $server['HTTPS'] = pick(['on', 'off', 'OFF', '1', '', true, false, 1, 0]);
    }
    $r = mt_rand(0, 9);
    if ($r < 7) {
        $server['REQUEST_URI'] = pick(PATHS);
    } elseif ($r === 7) {
        $server['ORIG_PATH_INFO'] = pick(['/foo', '/a/b', '']);
    }
    if (chance(20)) {
        $server['UNENCODED_URL'] = pick(['/ROUTING/x', '/foo/bar', '']);
    }
    foreach (pick(SCRIPTS) as $k => $v) {
        $server[$k] = $v;
    }
    if (chance(40)) {
        $server['QUERY_STRING'] = pick(['a=1', 'b=2&a=1', 'x[]=1&x[]=2', '', 'foo', 'a=b%20c+d', '=x&y']);
    }
    if (chance(70)) {
        $server['REQUEST_METHOD'] = pick(METHODS);
    }
    if (chance(15)) {
        $server['HTTP_X_HTTP_METHOD_OVERRIDE'] = pick(['PUT', 'delete', 'GET', 'patch', 'b@d', '0', '']);
    }
    if (chance(35)) {
        $server['HTTP_X_FORWARDED_FOR'] = pick(FORWARDED_FOR);
    }
    if (chance(25)) {
        $server['HTTP_X_FORWARDED_HOST'] = pick(['fwd.example.com', 'fwd.example.com:8443, other', '[::1]:444', 'bad host', 'trusted.test']);
    }
    if (chance(25)) {
        $server['HTTP_X_FORWARDED_PROTO'] = pick(['https', 'http', 'HTTPS', 'ssl', 'on', '1', 'https, http', 'off']);
    }
    if (chance(20)) {
        $server['HTTP_X_FORWARDED_PORT'] = pick(['443', '8443', '', '80, 81', 'abc']);
    }
    if (chance(15)) {
        $server['HTTP_X_FORWARDED_PREFIX'] = pick(['/prefix', '/prefix/', '', '/a/b/']);
    }
    if (chance(25)) {
        $server['HTTP_FORWARDED'] = pick(FORWARDED);
    }
    if (chance(50)) {
        $server['HTTP_ACCEPT'] = pick(ACCEPT);
    }
    if (chance(50)) {
        $server['HTTP_ACCEPT_LANGUAGE'] = pick(LANGUAGES);
    }
    if (chance(20)) {
        $server['HTTP_ACCEPT_CHARSET'] = pick(['utf-8, iso-8859-1;q=0.5', '*', '']);
    }
    if (chance(20)) {
        $server['HTTP_ACCEPT_ENCODING'] = pick(['gzip, deflate, br', 'identity;q=0', '']);
    }
    if (chance(20)) {
        $server['HTTP_VIA'] = pick(['1.1 proxy', 'HTTP/1.0 a', '2.0', 'x', '']);
    }
    if (chance(40)) {
        $server['SERVER_PROTOCOL'] = pick(['HTTP/1.1', 'HTTP/2.0', 'HTTP/1.0']);
    }
    if (chance(15)) {
        $server['HTTP_IF_NONE_MATCH'] = pick(['"a", "b"', ' "a" ,, W/"b" ', '*', '']);
    }
    if (chance(15)) {
        $server['HTTP_PREFER'] = pick(['safe', 'return=minimal, safe', 'unsafe']);
    }
    if (chance(15)) {
        $server['HTTP_PRAGMA'] = pick(['no-cache', 'x']);
    }
    if (chance(15)) {
        $server['HTTP_CACHE_CONTROL'] = pick(['no-cache', 'max-age=0']);
    }
    if (chance(20)) {
        $server['CONTENT_TYPE'] = pick(['application/json', 'application/x-www-form-urlencoded', 'text/html; charset=utf-8', 'application/vnd.foo+json', '']);
    }
    if (chance(15)) {
        $server['HTTP_X_REQUESTED_WITH'] = pick(['XMLHttpRequest', 'xmlhttprequest']);
    }
    if (chance(15)) {
        $server['PHP_AUTH_USER'] = pick(['user', '0', '']);
        if (chance(50)) {
            $server['PHP_AUTH_PW'] = pick(['pw', '0', '']);
        }
    }

    return $server;
}

function generated_config(): array
{
    $proxies = pick([[], ['10.0.0.1'], ['10.0.0.0/8', '::1'], ['REMOTE_ADDR'], ['private_ranges'], ['1.1.1.1', '127.0.0.1'], ['PRIVATE_SUBNETS', '88.88.88.88'], ['0.0.0.0/0']]);
    $set = pick([-1, Request::HEADER_X_FORWARDED_FOR, Request::HEADER_FORWARDED, Request::HEADER_X_FORWARDED_TRAEFIK, Request::HEADER_X_FORWARDED_AWS_ELB, Request::HEADER_X_FORWARDED_PROTO | Request::HEADER_X_FORWARDED_PORT, mt_rand(0, 63)]);
    $hosts = pick([[], [], ['^example\.com$'], ['^(.+\.)?example\.com$', 'localhost'], ['^[a-z]+\.test$', '^trusted\.test$'], ['^EXAMPLE\.COM$', '^localhost$'], ['example'], ['^127\.0\.0\.1$', '^\[::1\]$']]);
    $override = chance(30);
    $allowed = pick([null, null, [], ['PUT'], ['PUT', 'PATCH', 'DELETE']]);
    $remote = pick([null, '10.0.0.1', '88.88.88.88']);

    return [$proxies, $set, $hosts, $override, $allowed, $remote];
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

const RELATIVE = ['/foo/bar', '/foo/', '/', '/a:b/c', 'rel', '/foo/bar/baz/qux', ''];
const LOCALES = [null, ['en'], ['fr_FR', 'en_US'], ['zh_Hans', 'de'], ['pt_BR', 'pt']];

/** The calls, in order, each case answers. */
function calls(Request $r, int $i): array
{
    $relative = RELATIVE[$i % \count(RELATIVE)];
    $locales = LOCALES[$i % \count(LOCALES)];

    return [
        result(static fn () => $r->getMethod()),
        result(static fn () => $r->getRealMethod()),
        result(static fn () => $r->getRequestUri()),
        result(static fn () => $r->getPathInfo()),
        result(static fn () => $r->getBaseUrl()),
        result(static fn () => $r->getBasePath()),
        result(static fn () => $r->getScriptName()),
        result(static fn () => $r->getQueryString()),
        result(static fn () => $r->isFromTrustedProxy()),
        result(static fn () => $r->getScheme()),
        result(static fn () => $r->isSecure()),
        result(static fn () => $r->getPort()),
        result(static fn () => $r->getHost()),
        result(static fn () => $r->getHttpHost()),
        result(static fn () => $r->getHost()),
        result(static fn () => $r->getSchemeAndHttpHost()),
        result(static fn () => $r->getUri()),
        result(static fn () => $r->getUriForPath('/x')),
        result(static fn () => $r->getRelativeUriForPath($relative)),
        result(static fn () => $r->getClientIps()),
        result(static fn () => $r->getClientIp()),
        result(static fn () => $r->getProtocolVersion()),
        result(static fn () => $r->getUser()),
        result(static fn () => $r->getPassword()),
        result(static fn () => $r->getUserInfo()),
        result(static fn () => $r->getLanguages()),
        result(static fn () => $r->getPreferredLanguage($locales)),
        result(static fn () => $r->getCharsets()),
        result(static fn () => $r->getEncodings()),
        result(static fn () => $r->getAcceptableContentTypes()),
        result(static fn () => $r->getPreferredFormat()),
        result(static fn () => $r->getRequestFormat()),
        result(static fn () => $r->getContentTypeFormat()),
        result(static fn () => $r->getETags()),
        result(static fn () => $r->isNoCache()),
        result(static fn () => $r->isXmlHttpRequest()),
        result(static fn () => $r->preferSafeContent()),
        result(static fn () => $r->isMethodSafe()),
        result(static fn () => $r->isMethodIdempotent()),
        result(static fn () => $r->isMethodCacheable()),
        result(static fn () => $r->isMethod('post')),
        result(static fn () => (string) $r),
    ];
}

$cases = [];
for ($i = 0; $i < 2500; ++$i) {
    $config = generated_config();
    apply_config($config);
    $server = generated_server();
    $post = chance(20) ? ['_method' => pick(['PUT', 'delete', 'GET', 'bad method', ['x']])] : [];
    $query = chance(10) ? ['_method' => pick(['PATCH', 'get'])] : [];
    $request = new Request($query, $post, [], [], [], $server);
    $cases[] = [$config, 'server', [typed_value($server), typed_value($post), typed_value($query)], calls($request, $i)];
}

$uris = ['http://example.com', 'https://example.com:8443/a?b=c', 'http://user:pw@example.com/x', 'http://user@example.com', '/path?x=1&y=2', '/?', '?a=b', 'example.com:90/foo', 'http://[::1]:90/x', 'http://[::1', 'http://[foo]', 'http://a..b/', 'http://exa mple.com/', 'HTTPS://example.com', 'ftp://example.com/f', 'mailto:x@y', ':x', 'a:b/c', '//host/x', '///x', 'http://example.com/a#f', 'http://example.com/?a=&amp;b=1', '/x?a=%26amp;', '/x?a[b]=1&a[c]=2', '/\\x', 'http://x.com\\y', "/a\tb", ' /x', '/x ', '', '/', 'http://example.com:0/', 'http://example.com:65536/', 'http://-x.com/', 'http://x_y.com/', 'http://1.2.3.4/', 'http://1.2.3/', 'http://256.1.1.1/', 'http://[1:2:3:4:5:6:7:8]/', '/%zz', '/caf%C3%A9', "/\xc3\xa9", 'http://example.com?q=a+b&r=%20', '/x?&&a=1&&', '/?foo.bar=1&foo bar=2'];
for ($i = 0; $i < 1500; ++$i) {
    $config = generated_config();
    apply_config($config);
    $uri = pick($uris);
    if (chance(30)) {
        $uri .= pick(['', '/more', '?z=9', '#x']);
    }
    $method = pick(['GET', 'POST', 'put', 'PATCH', 'DELETE', 'QUERY', 'head', 'OPTIONS']);
    $parameters = chance(40) ? pick([['a' => '1'], ['z' => ['1', '2']], ['_method' => 'PUT'], ['b' => null, 'c' => true, 'd' => 1.5]]) : [];
    $server = [];
    foreach (generated_server() as $k => $v) {
        if (chance(40)) {
            $server[$k] = $v;
        }
    }
    $entry = [$config, 'create', [base64_encode($uri), $method, typed_value($parameters), typed_value($server)]];
    try {
        $request = Request::create($uri, $method, $parameters, [], [], $server);
        $entry[] = calls($request, $i);
    } catch (\Throwable $e) {
        $entry[] = ['throws', $e::class, base64_encode($e->getMessage())];
    }
    $cases[] = $entry;
}

apply_config([[], -1, [], false, null, null]);
$request = new Request();
$mimes = ['application/json', 'application/json; charset=utf-8', 'application/foo+json', 'application/foo+xml;x=y', 'text/csv', 'application/x-yaml', 'text/x-markdown', 'application/vnd.ms-excel', 'application/a+b+json', 'application/+', '', null, 'text/', 'text', 'application/xhtml+xml', 'application/soap+xml', 'APPLICATION/JSON', ' application/json ;q=1', 'multipart/form-data; boundary=x', 'application/x-www-form-urlencoded', 'application/vnd.wap.wbxml', 'application/x-json+cbor', '0', 'x/0'];
$formats = [];
foreach ($mimes as $mime) {
    $formats[] = [typed_value($mime), result(static fn () => $request->getFormat($mime)), result(static fn () => $request->getFormat($mime, true))];
}
$queries = ['', 'a', 'a=1&b=2', 'b=1&a=2', 'a[]=1&a[]=2', 'a[x]=1&a[y]=2', 'a=1&a=2', '%20a=%20', 'a+b=c+d', 'a=%2B', '&&a&&', '=x', 'a.b=1', 'a b=1', 'a[=1', 'utf=%E2%9C%93', 'é=è', 'a=1;b=2', 'z=1&y=2&x=3', '1=a&0=b', 'a%5B%5D=1'];
$normalized = [];
foreach ($queries as $q) {
    $normalized[] = [base64_encode($q), result(static fn () => Request::normalizeQueryString($q))];
}
$bodies = ['', '[]', '{}', '{"a":1}', '[1,2]', '"x"', '1', 'null', 'true', 'nope', '{"a":', '{"9223372036854775808":9223372036854775808}', "\xff"];
$payloads = [];
foreach ($bodies as $body) {
    $payloads[] = [
        base64_encode($body),
        result(static fn () => (new Request([], [], [], [], [], [], $body))->toArray()),
        result(static fn () => (new Request([], [], [], [], [], [], $body))->getPayload()->all()),
        result(static function () use ($body) {
            try {
                (new Request([], [], [], [], [], [], $body))->toArray();

                return null;
            } catch (\Throwable $e) {
                return [$e->getCode(), $e->getPrevious() ? $e->getPrevious()::class : null, $e->getPrevious()?->getMessage()];
            }
        }),
    ];
}

// getPort() returns SERVER_PORT under its int|string|null return type.
$serverPorts = [8.5, -0.5, 1e19, -1e19, 9.2233720368547758E+18, -9.2233720368547758E+18, NAN, INF, -INF, 1e15 + 0.5, -0.0, 1e30, 123.0, true, false, null, [], ['80'], '8080', '', 'x', 0, -1];
$ports = [];
foreach ($serverPorts as $port) {
    $ports[] = [typed_value($port), result(static fn () => (new Request([], [], [], [], [], ['SERVER_PORT' => $port]))->getPort())];
}

echo json_encode(['cases' => $cases, 'formats' => $formats, 'normalized' => $normalized, 'payloads' => $payloads, 'ports' => $ports], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
