<?php
// RequestContext::fromUri() and setBaseUrl() answers for
// crates/routing/tests/data/request_context.json (scripts/record.sh
// routing-request-context): generated URIs (schemes in both cases,
// authorities with ports, backslashes before and after ? and #, control
// bytes and spaces at either end), each with a few default hosts, schemes
// and ports; then generated base URLs. A context is [baseUrl, pathInfo,
// method, host, scheme, httpPort, httpsPort, queryString], typed.

error_reporting(E_ALL);

require '/demo/vendor/autoload.php';
require __DIR__ . '/lib/typed.php';

use Symfony\Component\Routing\RequestContext;

mt_srand(20261124);

function state(RequestContext $c): array
{
    return typed_value([$c->getBaseUrl(), $c->getPathInfo(), $c->getMethod(), $c->getHost(), $c->getScheme(), $c->getHttpPort(), $c->getHttpsPort(), $c->getQueryString()]);
}

function pick(array $a): mixed
{
    return $a[mt_rand(0, \count($a) - 1)];
}

$schemes = ['', 'http:', 'https:', 'HTTP:', 'Https:', 'ftp:', 'file:', ':'];
$hosts = ['', 'h', 'Example.COM', '[::1]', 'u:p@h', 'h\\x'];
$ports = ['', ':80', ':81', ':443', ':8443', ':0', ':65536', ':x'];
$paths = ['', '/', '/index.php', '/a/b/', '//x', '/a\\b', '/é', "/\x80"];
$tails = ['', '?q', '?a\\b', '#f', '#a\\b', '?q#f'];
$edges = ['', '', '', ' ', "\0", "\t", "\x1f", "\x7f", "\r", "\n"];

$uris = [];
for ($n = 0; $n < 3000; ++$n) {
    $uri = pick($schemes) . (mt_rand(0, 3) ? '//' : '') . pick($hosts) . pick($ports) . pick($paths) . pick($tails);
    if (!mt_rand(0, 3)) {
        $uri = pick($edges) . $uri . pick($edges);
    }
    if (!mt_rand(0, 9)) {
        $at = mt_rand(0, \strlen($uri));
        $uri = substr($uri, 0, $at) . pick(["\r", "\n", "\t", '\\']) . substr($uri, $at);
    }
    $host = pick(['localhost', 'Default.Host']);
    $scheme = pick(['http', 'https', 'HTTPS']);
    $httpPort = pick([80, 8080]);
    $httpsPort = pick([443, 8443]);
    $uris[] = [base64_encode($uri), base64_encode($host), base64_encode($scheme), $httpPort, $httpsPort, state(RequestContext::fromUri($uri, $host, $scheme, $httpPort, $httpsPort))];
}

$baseUrls = [];
foreach (['', '/', '//', '/a', '/a/', '/a//', 'a/b/', ' /', '/ ', "/\x80/"] as $baseUrl) {
    $baseUrls[] = [base64_encode($baseUrl), state((new RequestContext())->setBaseUrl($baseUrl))];
}

echo json_encode(['from_uri' => $uris, 'base_url' => $baseUrls], \JSON_THROW_ON_ERROR | \JSON_UNESCAPED_SLASHES), "\n";
